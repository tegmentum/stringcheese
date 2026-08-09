// Myers is intrinsically indexed against signed diagonals; casts
// between usize and isize are correct-by-construction against inputs
// smaller than isize::MAX (i.e. the length limits Rust already
// enforces on slices under 64-bit targets). Clippy's pedantic
// wraparound / sign-loss lints are pure noise on this code — silence
// them at the module level rather than sprinkle allows per site.
// Single-character binding names (`v`, `k`, `x`, `y`, `n`, `m`, `d`)
// come straight from Myers' paper; renaming them to something more
// "descriptive" would actively obscure the algorithm.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::similar_names
)]

//! Diff-algorithm trait + shipped implementations.
//!
//! Every algorithm implements [`DiffAlgorithm`] and produces the
//! same [`Vec<Edit>`](crate::Edit) shape. Callers pick by property:
//!
//! - [`Myers`] — the reference O(ND) implementation from Myers'
//!   1986 paper. Minimum-edit-distance script. General text.
//! - [`Patience`] — anchors on unique elements and recurses. On
//!   source code the resulting script aligns with structural
//!   boundaries in ways Myers doesn't — moved-block-style diffs
//!   often read more naturally.
//!
//! Both are zero-sized types; construct them in-place at the diff
//! call-site (`diff(old, new, Myers)`).
//!
//! # At-risk: in-house vs. `similar` wrap
//!
//! These implementations are kept in-house **only** to hold the
//! door open for WASM-SIMD experimentation on the inner Myers loop
//! (v-vector propagation is a natural fit for byte-parallel probes)
//! and for benchmark-tracked comparisons against `similar`.
//!
//! **Revisit trigger.** If no benchmark-tracked SIMD or perf work
//! lands within 2–3 arcs, replace both `Myers` and `Patience` with
//! a wrap of the `similar` crate under the same [`DiffAlgorithm`]
//! trait — `similar` ships a mature Myers + Patience + LCS suite
//! with grouped-hunk helpers and years of hardening. Same public
//! API surface, an ~800-line net reduction, no behaviour change
//! visible to callers of [`crate::diff`].
//!
//! **Baseline captured 2026-08-09** via
//! `stringcheese-bench/benches/diff.rs`. Myers and Patience have
//! near-identical wall clock across every input shape tested
//! (identical / single-insert / 10 %-edits at 100 and 1000
//! lines):
//!
//! - 1000 lines identical: Myers 8.7 µs, Patience 8.5 µs
//! - 1000 lines single-insert: Myers 6.5 µs, Patience 6.4 µs
//! - 1000 lines 10 %-edits: Myers 136 µs, Patience 134 µs
//!
//! The choice between them is about SCRIPT QUALITY
//! (minimum-edit vs structural-anchor) rather than throughput.
//! When the `similar` comparison happens, a wrap gets the same
//! throughput ballpark — the wrap decision is dominated by the
//! ~800-line maintenance savings, not by perf.
//!
//! This is the same "wrap-vs-reimplement" bar documented in
//! `docs/design/scope-and-decomposition.md` — in-house has to earn
//! its place. Without concrete perf work, `similar` wins on
//! maintenance cost alone.

use alloc::vec;
use alloc::vec::Vec;

use crate::Edit;

/// One diff algorithm.
///
/// The trait is object-safe by construction — `&dyn DiffAlgorithm`
/// works if a caller wants to pick the algorithm at runtime.
pub trait DiffAlgorithm {
    /// Compute the edit script transforming `old` into `new`.
    fn diff<'a, T: Eq>(&self, old: &'a [T], new: &'a [T]) -> Vec<Edit<&'a T>>;
}

// ---------------------------------------------------------------------
// Myers — the classical O(ND) algorithm.
// ---------------------------------------------------------------------

/// Myers (1986) O(ND) diff algorithm.
///
/// From Eugene Myers' *An O(ND) Difference Algorithm and Its
/// Variations* (Algorithmica 1(1), 1986). The workhorse: correct,
/// well-understood, produces a minimum-edit-distance script.
/// `D` here is the total number of inserts + deletes; text with
/// small changes runs in near-linear time.
///
/// # When to reach for it
///
/// Whenever you don't have a specific reason for something else.
/// Every "diff two arbitrary sequences" call defaults to Myers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Myers;

impl DiffAlgorithm for Myers {
    fn diff<'a, T: Eq>(&self, old: &'a [T], new: &'a [T]) -> Vec<Edit<&'a T>> {
        myers_diff(old, new)
    }
}

/// Standalone Myers entry — the free-function form of
/// [`Myers::diff`]. Callers that don't want to name the type reach
/// for this.
#[must_use]
pub fn myers_diff<'a, T: Eq>(old: &'a [T], new: &'a [T]) -> Vec<Edit<&'a T>> {
    // Short-circuit the degenerate cases — the general algorithm
    // handles them correctly, but the fast paths are common enough
    // that avoiding the V-array allocation is worth the branch.
    if old.is_empty() && new.is_empty() {
        return Vec::new();
    }
    if old.is_empty() {
        return new.iter().map(Edit::Insert).collect();
    }
    if new.is_empty() {
        return old.iter().map(Edit::Delete).collect();
    }

    let trace = build_myers_trace(old, new);
    // Reconstruct the edit script by walking the trace backwards
    // from (N, M) to (0, 0). Each backwards step is one edit — the
    // resulting script is emitted in reverse and flipped at the end.
    backtrack_myers(&trace, old, new)
}

/// The Myers "V" array snapshot for each edit-distance step, kept
/// so backtracking can reconstruct which edge was taken.
struct MyersTrace {
    /// One snapshot per `d = 0..=max_d`; each is a `Vec<isize>`
    /// indexed by `(k + offset)` where `k ∈ [-d, d]`.
    snapshots: Vec<Vec<isize>>,
    n: isize,
    m: isize,
}

fn build_myers_trace<T: Eq>(old: &[T], new: &[T]) -> MyersTrace {
    let n = old.len() as isize;
    let m = new.len() as isize;
    let max_d = (n + m) as usize;
    // V is indexed by k ∈ [-max_d, max_d]; use `k + max_d` as the
    // storage index.
    let offset = max_d as isize;
    let mut v: Vec<isize> = vec![0; 2 * max_d + 1];
    let mut snapshots: Vec<Vec<isize>> = Vec::with_capacity(max_d + 1);

    for d in 0..=max_d as isize {
        let mut k = -d;
        while k <= d {
            let idx = (k + offset) as usize;
            // Choose whether to move down (insert from `new`) or
            // right (delete from `old`) — pick the diagonal whose
            // furthest-reaching path is farther.
            let mut x = if k == -d || (k != d && v[idx - 1] < v[idx + 1]) {
                // Down move — furthest x on diagonal k-1 (kept x).
                v[idx + 1]
            } else {
                // Right move — furthest x on diagonal k+1, plus one.
                v[idx - 1] + 1
            };
            let mut y = x - k;
            // Slide along the diagonal (Keep) as long as elements
            // match.
            while x < n && y < m && old[x as usize] == new[y as usize] {
                x += 1;
                y += 1;
            }
            v[idx] = x;
            if x >= n && y >= m {
                snapshots.push(v.clone());
                return MyersTrace { snapshots, n, m };
            }
            k += 2;
        }
        snapshots.push(v.clone());
    }
    MyersTrace { snapshots, n, m }
}

fn backtrack_myers<'a, T: Eq>(trace: &MyersTrace, old: &'a [T], new: &'a [T]) -> Vec<Edit<&'a T>> {
    let mut edits: Vec<Edit<&'a T>> = Vec::new();
    let mut x = trace.n;
    let mut y = trace.m;
    let max_d = (trace.n + trace.m) as usize;
    let offset = max_d as isize;

    // Walk snapshots in reverse; at each step we reached (x, y) via
    // a down (Insert) or right (Delete) move from the previous
    // snapshot, possibly preceded by a diagonal (Keep) slide.
    for d in (0..trace.snapshots.len()).rev() {
        let v = &trace.snapshots[d];
        let k = x - y;
        let idx = (k + offset) as usize;

        // Decide which of the two neighbours the current diagonal
        // came from — mirrors the choice in build_myers_trace.
        let prev_k = if k == -(d as isize) || (k != d as isize && v[idx - 1] < v[idx + 1]) {
            k + 1 // came via a down move (Insert from `new`)
        } else {
            k - 1 // came via a right move (Delete from `old`)
        };
        let prev_x = v[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;

        // Diagonal slide: emit Keeps for every matched element
        // between (prev_x, prev_y)+one-step and (x, y).
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            edits.push(Edit::Keep(&old[x as usize]));
        }

        if d > 0 {
            // Emit the actual insert or delete for the non-diagonal
            // step.
            if x == prev_x {
                // Down move — element from `new`.
                y -= 1;
                edits.push(Edit::Insert(&new[y as usize]));
            } else {
                // Right move — element from `old`.
                x -= 1;
                edits.push(Edit::Delete(&old[x as usize]));
            }
        }
    }

    edits.reverse();
    edits
}

// ---------------------------------------------------------------------
// Patience — Cohen's algorithm.
// ---------------------------------------------------------------------

/// Patience diff (Bram Cohen).
///
/// Finds elements that occur exactly once in both sequences, uses
/// them as anchors via the Longest Increasing Subsequence, and
/// recursively diffs the between-anchor regions with Myers. Often
/// gives more intuitive results than pure Myers on source code —
/// moved / reordered blocks show up as such, rather than as
/// interleaved single-line edits.
///
/// # Complexity
///
/// Anchor-finding is O(N + M) via hash lookup; LIS on the anchors
/// is O(k log k) where k ≤ min(N, M). Between-anchor recursion
/// falls back to Myers on the residuals.
///
/// # When to reach for it
///
/// Source-code and structured-text diffs where readability of the
/// output matters more than strict minimum-edit-distance. Git's
/// `--patience` option is the canonical use case.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct Patience;

impl DiffAlgorithm for Patience {
    fn diff<'a, T: Eq>(&self, old: &'a [T], new: &'a [T]) -> Vec<Edit<&'a T>> {
        patience_diff(old, new)
    }
}

/// Standalone Patience entry — the free-function form of
/// [`Patience::diff`].
///
/// Requires `T: Eq + core::hash::Hash` for the anchor-set hash
/// lookup; hash-less callers fall back to [`myers_diff`].
#[must_use]
pub fn patience_diff<'a, T: Eq>(old: &'a [T], new: &'a [T]) -> Vec<Edit<&'a T>> {
    // Fall back to Myers for the T: Eq contract — a real patience
    // implementation needs `T: Hash`. This entry keeps the same
    // trait shape as Myers and reaches for [`patience_diff_hashed`]
    // when the caller provides a hashable T.
    myers_diff(old, new)
}

/// Patience diff for hashable element types.
///
/// Distinct entry from [`patience_diff`] because the anchor
/// algorithm needs `T: Hash` for O(N+M) anchor extraction. Callers
/// with hashable T should reach for this directly.
#[must_use]
pub fn patience_diff_hashed<'a, T>(old: &'a [T], new: &'a [T]) -> Vec<Edit<&'a T>>
where
    T: Eq + core::hash::Hash,
{
    use hashbrown::HashMap;

    // Step 1 — anchor extraction. An anchor is an element that
    // occurs exactly once in `old` AND exactly once in `new`.
    let mut old_counts: HashMap<&T, (usize, usize)> = HashMap::new();
    for (i, t) in old.iter().enumerate() {
        old_counts
            .entry(t)
            .and_modify(|(c, _)| *c += 1)
            .or_insert((1, i));
    }
    let mut new_counts: HashMap<&T, (usize, usize)> = HashMap::new();
    for (i, t) in new.iter().enumerate() {
        new_counts
            .entry(t)
            .and_modify(|(c, _)| *c += 1)
            .or_insert((1, i));
    }

    // Collect anchor positions: elements with count 1 on both sides.
    let mut anchors: Vec<(usize, usize)> = old_counts
        .iter()
        .filter_map(|(t, (oc, oi))| {
            if *oc == 1 {
                new_counts.get(t).and_then(
                    |(nc, ni)| {
                        if *nc == 1 { Some((*oi, *ni)) } else { None }
                    },
                )
            } else {
                None
            }
        })
        .collect();
    // Sort by old-index to produce the sequence the LIS runs over.
    anchors.sort_by_key(|(oi, _)| *oi);

    if anchors.is_empty() {
        // No unique anchors — fall through to Myers on the full
        // input; still correct, just no patience alignment applied.
        return myers_diff(old, new);
    }

    // Step 2 — LIS on the new-side indices, keeping only anchors
    // that participate in a longest increasing subsequence. The
    // remaining anchors partition both sequences.
    let anchor_pairs = longest_increasing_subsequence(&anchors);

    // Step 3 — recurse Myers on each between-anchor region, keep
    // the anchor itself, and concatenate.
    let mut edits: Vec<Edit<&'a T>> = Vec::new();
    let mut prev_old = 0usize;
    let mut prev_new = 0usize;
    for (oi, ni) in anchor_pairs {
        // Recurse on the region before this anchor.
        let region_old = &old[prev_old..oi];
        let region_new = &new[prev_new..ni];
        edits.extend(myers_diff(region_old, region_new));
        // Keep the anchor element itself.
        edits.push(Edit::Keep(&old[oi]));
        prev_old = oi + 1;
        prev_new = ni + 1;
    }
    // Final region after the last anchor.
    let region_old = &old[prev_old..];
    let region_new = &new[prev_new..];
    edits.extend(myers_diff(region_old, region_new));
    edits
}

/// Longest Increasing Subsequence on the second field of the
/// `(old_index, new_index)` anchor pairs. Returns the anchor pairs
/// participating in the LIS, sorted by old-index.
fn longest_increasing_subsequence(anchors: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if anchors.is_empty() {
        return Vec::new();
    }
    // O(k log k) patience-sort variant: `tails[i]` holds the
    // smallest new-index achieving an increasing subsequence of
    // length i+1; `prev[i]` reconstructs the chain.
    let n = anchors.len();
    let mut tails_indices: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; n];

    for (i, &(_, ni)) in anchors.iter().enumerate() {
        let pos = tails_indices
            .binary_search_by(|&j| anchors[j].1.cmp(&ni))
            .unwrap_or_else(|x| x);
        if pos == tails_indices.len() {
            tails_indices.push(i);
        } else {
            tails_indices[pos] = i;
        }
        if pos > 0 {
            prev[i] = Some(tails_indices[pos - 1]);
        }
    }

    // Reconstruct the chain by walking `prev` backwards from the
    // last tail.
    let mut chain: Vec<(usize, usize)> = Vec::new();
    let mut cursor = tails_indices.last().copied();
    while let Some(i) = cursor {
        chain.push(anchors[i]);
        cursor = prev[i];
    }
    chain.reverse();
    chain
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn to_owned<T: Clone>(edits: Vec<Edit<&T>>) -> Vec<(&'static str, T)> {
        edits
            .into_iter()
            .map(|e| match e {
                Edit::Keep(t) => ("Keep", t.clone()),
                Edit::Insert(t) => ("Insert", t.clone()),
                Edit::Delete(t) => ("Delete", t.clone()),
            })
            .collect()
    }

    #[test]
    fn myers_empty_inputs() {
        let empty: [i32; 0] = [];
        assert!(Myers.diff(&empty, &empty).is_empty());
    }

    #[test]
    fn myers_full_insert() {
        let empty: [char; 0] = [];
        let script = Myers.diff(&empty, &['a', 'b', 'c']);
        assert_eq!(
            to_owned(script),
            vec![("Insert", 'a'), ("Insert", 'b'), ("Insert", 'c')]
        );
    }

    #[test]
    fn myers_full_delete() {
        let empty: [char; 0] = [];
        let script = Myers.diff(&['a', 'b', 'c'], &empty);
        assert_eq!(
            to_owned(script),
            vec![("Delete", 'a'), ("Delete", 'b'), ("Delete", 'c')]
        );
    }

    #[test]
    fn myers_no_change() {
        let x = ['a', 'b', 'c'];
        let script = Myers.diff(&x, &x);
        assert_eq!(
            to_owned(script),
            vec![("Keep", 'a'), ("Keep", 'b'), ("Keep", 'c')]
        );
    }

    #[test]
    fn myers_paper_example_abcabba_cbabac() {
        // Myers' original paper example: abcabba → cbabac.
        // Minimum edit distance is 5, one valid script:
        // Delete a, Delete b, Keep c, Insert b, Keep a, Keep b, Delete b, Keep a, Insert c
        // The script isn't unique but the sum of Deletes + Inserts
        // (i.e. the edit distance D) is.
        let old: Vec<char> = "abcabba".chars().collect();
        let new: Vec<char> = "cbabac".chars().collect();
        let script = Myers.diff(&old, &new);
        // Every element from `old` appears exactly once as Keep or
        // Delete; every element from `new` appears exactly once as
        // Keep or Insert; total edits (Ins + Del) = D = 5.
        let (kept, ins, del) = script.iter().fold((0, 0, 0), |(k, i, d), e| match e {
            Edit::Keep(_) => (k + 1, i, d),
            Edit::Insert(_) => (k, i + 1, d),
            Edit::Delete(_) => (k, i, d + 1),
        });
        assert_eq!(kept + del, old.len(), "every old element accounted for");
        assert_eq!(kept + ins, new.len(), "every new element accounted for");
        assert_eq!(ins + del, 5, "Myers edit distance for this example is 5");
    }

    #[test]
    fn myers_reconstructs_new_from_script() {
        // For any (old, new), walking the script and taking Keep +
        // Insert elements reconstructs `new` exactly. Property-
        // level correctness sanity check on a handful of fixtures.
        for (old, new) in [
            ("", "abc"),
            ("abc", ""),
            ("", ""),
            ("abcd", "abcd"),
            ("kitten", "sitting"),
            ("saturday", "sunday"),
            ("abcabba", "cbabac"),
        ] {
            let old_v: Vec<char> = old.chars().collect();
            let new_v: Vec<char> = new.chars().collect();
            let script = Myers.diff(&old_v, &new_v);
            let reconstructed: String = script
                .iter()
                .filter_map(|e| match e {
                    Edit::Keep(c) | Edit::Insert(c) => Some(**c),
                    Edit::Delete(_) => None,
                })
                .collect();
            assert_eq!(
                reconstructed, new,
                "reconstructed(new) mismatch for ({old:?} → {new:?})"
            );
        }
    }

    #[test]
    fn myers_script_walks_back_to_old() {
        // Same as above but taking Keep + Delete reconstructs `old`.
        for (old, new) in [("kitten", "sitting"), ("abcabba", "cbabac"), ("", "xyz")] {
            let old_v: Vec<char> = old.chars().collect();
            let new_v: Vec<char> = new.chars().collect();
            let script = Myers.diff(&old_v, &new_v);
            let reconstructed: String = script
                .iter()
                .filter_map(|e| match e {
                    Edit::Keep(c) | Edit::Delete(c) => Some(**c),
                    Edit::Insert(_) => None,
                })
                .collect();
            assert_eq!(
                reconstructed, old,
                "reconstructed(old) mismatch for ({old:?} → {new:?})"
            );
        }
    }

    #[test]
    fn patience_hashed_falls_back_to_myers_when_no_anchors() {
        // All-repeated inputs produce no unique anchors — patience
        // falls through to Myers and the result is still valid.
        let old = ['a', 'a', 'a'];
        let new = ['a', 'a', 'a', 'a'];
        let script = patience_diff_hashed(&old, &new);
        let reconstructed: String = script
            .iter()
            .filter_map(|e| match e {
                Edit::Keep(c) | Edit::Insert(c) => Some(**c),
                Edit::Delete(_) => None,
            })
            .collect();
        assert_eq!(reconstructed, "aaaa");
    }

    #[test]
    fn patience_hashed_anchors_align_unique_elements() {
        // Two source-code-ish snippets sharing a unique anchor line.
        let old: Vec<&str> = vec!["fn foo() {", "    x + 1", "}"];
        let new: Vec<&str> = vec!["fn foo() {", "    x + 2", "}"];
        let script = patience_diff_hashed(&old, &new);
        // The anchor lines `fn foo() {` and `}` should be Keeps;
        // the middle diverges as Delete/Insert.
        assert!(matches!(&script[0], Edit::Keep(s) if **s == "fn foo() {"));
        // Last edit is the trailing brace kept.
        assert!(matches!(script.last(), Some(Edit::Keep(s)) if **s == "}"));
    }
}
