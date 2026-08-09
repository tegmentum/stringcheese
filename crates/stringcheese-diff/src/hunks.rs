//! Group an edit script into contextual hunks.
//!
//! A [`Hunk`] is a run of edits surrounded by up to `context` Keep
//! elements on either side. Consecutive change regions closer than
//! `2 * context` merge; regions farther apart become separate
//! hunks — matches how `git diff -U<n>` groups output.

use alloc::vec::Vec;

use crate::Edit;

/// One contextual hunk of an edit script.
///
/// Carries the byte-or-element positions in `old` and `new` where
/// the hunk starts, plus the exact edit sequence — the unified-diff
/// writer consumes this shape directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk<T> {
    /// 0-based starting position in `old`.
    pub old_start: usize,
    /// Number of `old` elements this hunk spans (Keeps + Deletes).
    pub old_len: usize,
    /// 0-based starting position in `new`.
    pub new_start: usize,
    /// Number of `new` elements this hunk spans (Keeps + Inserts).
    pub new_len: usize,
    /// The subset of the original edit script covered by this
    /// hunk, in the same order it appeared in the full script.
    pub edits: Vec<Edit<T>>,
}

/// Group `script` into hunks with `context` lines of surrounding
/// Keep on either side of each change region.
///
/// A `context` of 3 matches git's default. Choose 0 for no
/// surrounding Keeps (only the changed lines themselves emit).
#[must_use]
pub fn hunks<T>(script: &[Edit<T>], context: usize) -> Vec<Hunk<T>>
where
    T: Clone,
{
    // Identify the indices of Change edits (Insert / Delete).
    let change_positions: Vec<usize> = script
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.is_change().then_some(i))
        .collect();

    if change_positions.is_empty() {
        return Vec::new();
    }

    // Walk change positions, grouping neighbours that fall within
    // `2 * context` of each other into the same hunk range.
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut cur_start = change_positions[0];
    let mut cur_end = change_positions[0];
    for &pos in &change_positions[1..] {
        if pos <= cur_end + 2 * context + 1 {
            cur_end = pos;
        } else {
            groups.push((cur_start, cur_end));
            cur_start = pos;
            cur_end = pos;
        }
    }
    groups.push((cur_start, cur_end));

    // For each group, expand outward by `context`, clip to the
    // script bounds, and collect the covered edits + old/new
    // position accounting.
    let mut out: Vec<Hunk<T>> = Vec::with_capacity(groups.len());
    for (lo, hi) in groups {
        let start = lo.saturating_sub(context);
        let end = (hi + context + 1).min(script.len()); // exclusive

        // Sum old- and new-side positions consumed by everything
        // BEFORE `start` — that's where this hunk begins in each.
        let mut old_pos = 0usize;
        let mut new_pos = 0usize;
        for e in &script[..start] {
            match e {
                Edit::Keep(_) => {
                    old_pos += 1;
                    new_pos += 1;
                }
                Edit::Delete(_) => old_pos += 1,
                Edit::Insert(_) => new_pos += 1,
            }
        }
        let old_start = old_pos;
        let new_start = new_pos;

        // Sum the hunk's own old- and new-side lengths.
        let mut old_len = 0usize;
        let mut new_len = 0usize;
        let mut edits: Vec<Edit<T>> = Vec::with_capacity(end - start);
        for e in &script[start..end] {
            match e {
                Edit::Keep(t) => {
                    old_len += 1;
                    new_len += 1;
                    edits.push(Edit::Keep(t.clone()));
                }
                Edit::Delete(t) => {
                    old_len += 1;
                    edits.push(Edit::Delete(t.clone()));
                }
                Edit::Insert(t) => {
                    new_len += 1;
                    edits.push(Edit::Insert(t.clone()));
                }
            }
        }

        out.push(Hunk {
            old_start,
            old_len,
            new_start,
            new_len,
            edits,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::{DiffAlgorithm, Myers};

    #[test]
    fn hunks_of_identical_input_is_empty() {
        let x = ['a', 'b', 'c'];
        let script = Myers.diff(&x, &x);
        assert!(hunks(&script, 3).is_empty());
    }

    #[test]
    fn hunks_groups_single_change_with_context() {
        // one Delete surrounded by Keeps — one hunk with 2-Keep
        // context on either side.
        let old = ['a', 'b', 'c', 'd', 'e'];
        let new = ['a', 'b', 'd', 'e']; // dropped 'c'
        let script = Myers.diff(&old, &new);
        let hs = hunks(&script, 1);
        assert_eq!(hs.len(), 1);
        let h = &hs[0];
        assert_eq!(h.old_start, 1); // start at 'b' (index 1) with context=1
        assert_eq!(h.old_len, 3); // b, c, d
        assert_eq!(h.new_start, 1);
        assert_eq!(h.new_len, 2); // b, d
    }

    #[test]
    fn hunks_far_apart_split() {
        // Two Deletes 10 elements apart — with context=1 they
        // become two hunks (not merged).
        let old: Vec<char> = vec!['x', 'a', 'a', 'a', 'a', 'a', 'a', 'a', 'a', 'a', 'y'];
        let new: Vec<char> = vec!['a', 'a', 'a', 'a', 'a', 'a', 'a', 'a', 'a'];
        let script = Myers.diff(&old, &new);
        let hs = hunks(&script, 1);
        assert_eq!(
            hs.len(),
            2,
            "distant changes don't merge with small context"
        );
    }
}
