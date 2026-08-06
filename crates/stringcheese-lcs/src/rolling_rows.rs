//! Rolling-row LCS kernel — the production `O(m · n)` time,
//! `O(min(m, n))` space implementation.
//!
//! The full DP matrix contains one row per prefix of `a` and one column per
//! prefix of `b`. When only the final cell is wanted, the entire matrix does
//! not need to be retained: cell `d[i][j]` depends only on `d[i-1][j-1]`,
//! `d[i-1][j]`, and `d[i][j-1]`, so it is enough to keep the previous row
//! and rebuild the current row in place.
//!
//! This module further reduces the buffer to a *single* row plus one scalar
//! carrying the "diagonal" cell, giving `min(m, n) + 1` cells of scratch.
//! The shorter input is always chosen as the inner dimension so that the
//! buffer is as small as possible regardless of the argument order.

use stringcheese_core::{Distance, Score};

use crate::workspace::LcsWorkspace;

/// Computes the length of the longest common subsequence between `a` and
/// `b` using a single rolling row backed by `ws`.
///
/// The workspace is grown to `min(a.len(), b.len()) + 1` cells if needed,
/// and left at that capacity on return so repeated calls of the same size
/// perform no further allocation.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the DP recurrence is universally written with `a`, `b`, `m`, `n`, `i`, `j`; renaming would put a translation layer between the code and its textbook derivation"
)]
pub fn lcs_length_rolling_rows_with_workspace<T: Eq>(
    a: &[T],
    b: &[T],
    ws: &mut LcsWorkspace,
) -> Score<u32> {
    // Choose the shorter side as the inner dimension so the scratch buffer
    // is `min(m, n) + 1` regardless of the caller's argument order. LCS is
    // symmetric in its inputs, so swapping is observationally invisible.
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let m = long.len();
    let n = short.len();

    // Length is zero whenever either side is empty; skip allocation.
    if n == 0 {
        return Score::new(0);
    }

    // Enforce the u32 length invariant up front, matching the oracle.
    let _ = u32::try_from(m).expect("input length exceeds u32::MAX");

    // Grow scratch to n + 1 cells; the buffer holds the "previous row".
    // Initialize to zero because the DP's boundary row is
    // `d[0][*] = 0`.
    let row = ws.buffer_mut(n + 1);
    for cell in row.iter_mut() {
        *cell = 0;
    }

    for i in 1..=m {
        // At the start of row i, `row[j]` still holds d[i-1][j]. We
        // overwrite it in place, using `prev_diag` to remember
        // d[i-1][j-1] between iterations.
        let mut prev_diag = row[0];
        // `d[i][0] = 0` for all i — no update is needed but the
        // assignment makes the invariant "row[0] holds d[i][0]" explicit.
        row[0] = 0;

        for j in 1..=n {
            let curr = if long[i - 1] == short[j - 1] {
                // Match: extend the diagonal predecessor by one.
                prev_diag + 1
            } else {
                // Mismatch: take the better of dropping one symbol from
                // either side. `row[j]` = d[i-1][j]; `row[j-1]` = d[i][j-1]
                // (already updated earlier in this row).
                row[j].max(row[j - 1])
            };

            // Slide the diagonal along before overwriting.
            prev_diag = row[j];
            row[j] = curr;
        }
    }

    Score::new(row[n])
}

/// Computes the LCS distance between `a` and `b` — the minimum number of
/// insertions plus deletions to transform one into the other — using the
/// rolling-rows kernel backed by `ws`.
///
/// This is computed as `|a| + |b| - 2 · lcs(a, b)`; see the module
/// documentation of [`crate::full_matrix`] for the derivation.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols, or if
/// `|a| + |b|` would overflow `u32`.
#[must_use]
pub fn lcs_distance_rolling_rows_with_workspace<T: Eq>(
    a: &[T],
    b: &[T],
    ws: &mut LcsWorkspace,
) -> Distance<u32> {
    let lcs = lcs_length_rolling_rows_with_workspace(a, b, ws).into_inner();
    let m = u32::try_from(a.len()).expect("input length exceeds u32::MAX");
    let n = u32::try_from(b.len()).expect("input length exceeds u32::MAX");
    let sum = m.checked_add(n).expect("|a| + |b| exceeds u32::MAX");
    let twice_lcs = lcs
        .checked_mul(2)
        .expect("2 * lcs(a, b) exceeds u32::MAX (unreachable if lcs ≤ min(|a|, |b|))");
    Distance::new(sum - twice_lcs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::full_matrix::{lcs_distance_full_matrix, lcs_length_full_matrix};
    use stringcheese_core::Workspace;

    fn rolling_len(a: &[u8], b: &[u8]) -> u32 {
        let mut ws = LcsWorkspace::new();
        lcs_length_rolling_rows_with_workspace(a, b, &mut ws).into_inner()
    }

    fn rolling_dist(a: &[u8], b: &[u8]) -> u32 {
        let mut ws = LcsWorkspace::new();
        lcs_distance_rolling_rows_with_workspace(a, b, &mut ws).into_inner()
    }

    #[test]
    fn empty_pair_length_is_zero() {
        assert_eq!(rolling_len(b"", b""), 0);
    }

    #[test]
    fn one_side_empty_length_is_zero() {
        assert_eq!(rolling_len(b"", b"hello"), 0);
        assert_eq!(rolling_len(b"hello", b""), 0);
    }

    #[test]
    fn identical_length_is_input_length() {
        assert_eq!(rolling_len(b"abcdef", b"abcdef"), 6);
    }

    #[test]
    fn textbook_abcbdab_bdcab_length_is_four() {
        assert_eq!(rolling_len(b"ABCBDAB", b"BDCAB"), 4);
    }

    #[test]
    fn textbook_agcat_gac_length_is_two() {
        assert_eq!(rolling_len(b"AGCAT", b"GAC"), 2);
    }

    #[test]
    fn argument_order_does_not_matter_for_length() {
        let a: &[u8] = b"quickly";
        let b: &[u8] = b"quick";
        assert_eq!(rolling_len(a, b), rolling_len(b, a));
    }

    #[test]
    fn workspace_is_reused_across_calls() {
        let mut ws = LcsWorkspace::new();
        for _ in 0..8 {
            let s = lcs_length_rolling_rows_with_workspace(b"ABCBDAB", b"BDCAB", &mut ws);
            assert_eq!(s.into_inner(), 4);
        }
        // The comparison uses min(7, 5) + 1 = 6 scratch cells.
        assert!(ws.capacity() >= 6);
    }

    #[test]
    fn matches_full_matrix_on_canonical_length_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"a", b"b"),
            (b"abc", b"xyz"),
            (b"ABCBDAB", b"BDCAB"),
            (b"AGCAT", b"GAC"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"aaaaaaa", b"aaaaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            assert_eq!(
                lcs_length_full_matrix(a, b).into_inner(),
                rolling_len(a, b),
                "rolling-rows disagreed with full-matrix oracle on length ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn matches_full_matrix_on_canonical_distance_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"abc", b"abcd"),
            (b"abcd", b"abc"),
            (b"abcd", b"abed"),
            (b"AGCAT", b"GAC"),
            (b"kitten", b"sitting"),
        ] {
            assert_eq!(
                lcs_distance_full_matrix(a, b).into_inner(),
                rolling_dist(a, b),
                "rolling-rows disagreed with full-matrix oracle on distance ({a:?}, {b:?})"
            );
        }
    }
}
