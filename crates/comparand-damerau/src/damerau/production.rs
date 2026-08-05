//! Full-Damerau production kernel — the `O(m · n)` time,
//! `O((m + 1) · (n + 1))` space implementation.
//!
//! This kernel computes the same Lowrance-Wagner recurrence as the oracle
//! ([`crate::damerau::full_matrix`]), but replaces the oracle's linear scan
//! for the "last position of `b[j-1]` in `a`" with a
//! `HashMap<&T, usize>` lookup — one entry per unique symbol seen in `a`,
//! updated at the end of each outer-loop iteration. The lookup is
//! amortized `O(1)`, giving the DP its textbook `O(m · n)` bound.
//!
//! # Trait bounds
//!
//! The auxiliary structure keys on `&T`, so the production kernel requires
//! `T: Eq + Hash` (as opposed to the oracle's `T: Eq`). That divergence is
//! deliberate: it is exactly what makes the oracle and production kernels
//! *structurally* different — they compute the same recurrence but locate
//! the transposition source through independent code paths, which is what
//! makes the differential test between them meaningful.
//!
//! # `std` gate
//!
//! `HashMap` lives in `std::collections`, not `alloc`, so this module is
//! gated on the crate's `std` feature. Under `--no-default-features
//! --features alloc` the OSA kernels and the Damerau oracle are still
//! available; the Damerau production kernel and the [`crate::Damerau`]
//! trait impl are not. This is the smallest gate that avoids taking on a
//! `hashbrown` dependency for the alloc-only configuration.
//!
//! # Workspace
//!
//! The DP matrix is held in the caller-owned [`DamerauWorkspace`], grown
//! to `(m + 1) · (n + 1)` cells and left at that capacity across calls. The
//! `HashMap` itself is *not* held in the workspace: its keys `&T` borrow
//! into the input slices for that call, so it cannot outlive the call
//! frame. Allocating a fresh `HashMap` per call is dominated by the DP
//! matrix cost in the batch-comparison workloads this workspace targets.

use core::hash::Hash;
use std::collections::HashMap;

use comparand_core::{Distance, Workspace};

use crate::workspace::DamerauWorkspace;

/// Computes the full (unrestricted) Damerau-Levenshtein distance between
/// `a` and `b` using the Lowrance-Wagner recurrence and a hash-table
/// auxiliary lookup, reusing `ws` as scratch across the call.
///
/// The workspace is grown to `(a.len() + 1) · (b.len() + 1)` cells if
/// needed, and left at that capacity on return so repeated calls of the
/// same size perform no further allocation.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols, or if
/// `a.len() + b.len()` exceeds `u32::MAX`.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names `a`, `b`, `m`, `n`, `d`, `i`, `j`, `k`, `l` follow Lowrance and Wagner's original 1975 notation; renaming would put a translation layer between the code and the paper it implements"
)]
pub fn distance_production_with_workspace<T: Eq + Hash>(
    a: &[T],
    b: &[T],
    ws: &mut DamerauWorkspace,
) -> Distance<u32> {
    let m = a.len();
    let n = b.len();

    if m == 0 {
        return Distance::new(u32::try_from(n).expect("input length exceeds u32::MAX"));
    }
    if n == 0 {
        return Distance::new(u32::try_from(m).expect("input length exceeds u32::MAX"));
    }

    let stride = n + 1;
    ws.ensure_capacity((m + 1) * stride);
    let d = ws.buffer_mut((m + 1) * stride);

    // Boundary conditions: `d[i][0] = i`, `d[0][j] = j`.
    for i in 0..=m {
        d[i * stride] = u32::try_from(i).expect("input length exceeds u32::MAX");
    }
    for (j, cell) in d[..=n].iter_mut().enumerate() {
        *cell = u32::try_from(j).expect("input length exceeds u32::MAX");
    }
    // The remaining cells were zero-filled or left dirty by a previous call;
    // every one of them will be overwritten by the DP loop below before it
    // is read, so no explicit reset is needed.

    // `da`: last row where each symbol of `a` was seen. Empty at the start;
    // grows to at most `|Σ ∩ a|` unique entries by the end.
    let mut da: HashMap<&T, usize> = HashMap::new();

    for i in 1..=m {
        // `db`: the largest column j' < j seen *this row* where
        // `a[i-1] == b[j'-1]`. Reset at the start of each row.
        let mut db: usize = 0;

        for j in 1..=n {
            // `k`: last row where `b[j-1]` was seen in `a` (0 if never).
            let k = da.get(&b[j - 1]).copied().unwrap_or(0);
            let l = db;

            let cost = if a[i - 1] == b[j - 1] {
                db = j;
                0u32
            } else {
                1u32
            };

            let substitution = d[(i - 1) * stride + (j - 1)] + cost;
            let insertion = d[i * stride + (j - 1)] + 1;
            let deletion = d[(i - 1) * stride + j] + 1;
            let mut best = substitution.min(insertion).min(deletion);

            if k > 0 && l > 0 {
                let base = d[(k - 1) * stride + (l - 1)];
                let gap = (i - k - 1) + 1 + (j - l - 1);
                let gap_u32 = u32::try_from(gap).expect("transposition gap exceeds u32::MAX");
                let transposition = base.saturating_add(gap_u32);
                best = best.min(transposition);
            }

            d[i * stride + j] = best;
        }

        // Register (or update) the most recent row where `a[i-1]` appeared.
        da.insert(&a[i - 1], i);
    }

    Distance::new(d[m * stride + n])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damerau::full_matrix::distance_full_matrix;

    fn production(a: &[u8], b: &[u8]) -> u32 {
        let mut ws = DamerauWorkspace::new();
        distance_production_with_workspace(a, b, &mut ws).into_inner()
    }

    #[test]
    fn empty_pair_is_zero() {
        assert_eq!(production(b"", b""), 0);
    }

    #[test]
    fn one_side_empty_is_other_length() {
        assert_eq!(production(b"", b"hello"), 5);
        assert_eq!(production(b"hello", b""), 5);
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(production(b"abcdef", b"abcdef"), 0);
    }

    #[test]
    fn adjacent_transposition_is_one() {
        assert_eq!(production(b"ab", b"ba"), 1);
    }

    #[test]
    fn ca_abc_is_two() {
        assert_eq!(production(b"ca", b"abc"), 2);
    }

    #[test]
    fn multiple_transpositions() {
        assert_eq!(production(b"abcd", b"badc"), 2);
    }

    #[test]
    fn workspace_reuse_matches_fresh_workspace() {
        let mut ws = DamerauWorkspace::new();
        let a: &[u8] = b"prefix-common-tail-A";
        let b: &[u8] = b"prefix-common-tail-B";
        let d1 = distance_production_with_workspace(a, b, &mut ws).into_inner();
        // Call several times; ensure capacity is not lost between calls and
        // that dirty tail cells don't corrupt the answer.
        for _ in 0..8 {
            let d = distance_production_with_workspace(a, b, &mut ws).into_inner();
            assert_eq!(d, d1);
        }
        assert!(ws.capacity() >= (a.len() + 1) * (b.len() + 1));
    }

    #[test]
    fn matches_oracle_on_canonical_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"a", b"b"),
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"abcd", b"badc"),
            (b"abc", b"xyz"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"aaaaaaa", b"aaaaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            assert_eq!(
                distance_full_matrix(a, b),
                production(a, b),
                "production disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }
}
