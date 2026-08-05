//! Rolling-row Levenshtein kernel — the production `O(m · n)` time,
//! `O(min(m, n))` space implementation.
//!
//! The full DP matrix contains one row per prefix of `a` and one column per
//! prefix of `b`. When only the final cell is wanted, the entire matrix does
//! not need to be retained: cell `d[i][j]` depends only on
//! `d[i-1][j-1]`, `d[i-1][j]`, and `d[i][j-1]`, so it is enough to keep the
//! previous row and rebuild the current row in place.
//!
//! This module further reduces the buffer to a *single* row plus one scalar
//! carrying the "diagonal" cell, giving `min(m, n) + 1` cells of scratch.
//! The shorter input is always chosen as the inner dimension so that the
//! buffer is as small as possible regardless of the argument order.

use comparand_core::{Distance, Workspace};

use crate::workspace::LevenshteinWorkspace;

/// Computes the unit-cost Levenshtein distance between `a` and `b` using a
/// single rolling row backed by `ws`.
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
pub fn distance_rolling_rows_with_workspace<T: Eq>(
    a: &[T],
    b: &[T],
    ws: &mut LevenshteinWorkspace,
) -> Distance<u32> {
    // Choose the shorter side as the inner dimension so the scratch buffer is
    // `min(m, n) + 1` regardless of the caller's argument order.
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let m = long.len();
    let n = short.len();

    if n == 0 {
        return Distance::new(u32::try_from(m).expect("input length exceeds u32::MAX"));
    }

    // Grow scratch to n + 1 cells; we index into it as the "previous row".
    ws.ensure_capacity(n + 1);
    let row = ws.buffer_mut(n + 1);

    // Initial row: transforming an empty prefix of `long` into a prefix of
    // `short` of length j costs j insertions.
    for (j, cell) in row.iter_mut().enumerate() {
        *cell = u32::try_from(j).expect("input length exceeds u32::MAX");
    }

    for i in 1..=m {
        // At the start of row i, `row[j]` still holds d[i-1][j]. We overwrite
        // it in place, using `prev_diag` to remember d[i-1][j-1] between
        // iterations.
        let mut prev_diag = row[0];
        row[0] = u32::try_from(i).expect("input length exceeds u32::MAX");

        for j in 1..=n {
            let cost = u32::from(long[i - 1] != short[j - 1]);
            let deletion = row[j] + 1; // d[i-1][j] + 1
            let insertion = row[j - 1] + 1; // d[i][j-1] + 1
            let substitution = prev_diag + cost; // d[i-1][j-1] + cost
            let curr = deletion.min(insertion).min(substitution);

            // Slide the diagonal along before overwriting.
            prev_diag = row[j];
            row[j] = curr;
        }
    }

    Distance::new(row[n])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::full_matrix::distance_full_matrix;

    fn rolling(a: &[u8], b: &[u8]) -> u32 {
        let mut ws = LevenshteinWorkspace::new();
        distance_rolling_rows_with_workspace(a, b, &mut ws).into_inner()
    }

    #[test]
    fn empty_pair_is_zero() {
        assert_eq!(rolling(b"", b""), 0);
    }

    #[test]
    fn one_side_empty_is_other_length() {
        assert_eq!(rolling(b"", b"hello"), 5);
        assert_eq!(rolling(b"hello", b""), 5);
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(rolling(b"abcdef", b"abcdef"), 0);
    }

    #[test]
    fn kitten_sitting_is_three() {
        assert_eq!(rolling(b"kitten", b"sitting"), 3);
    }

    #[test]
    fn transposition_is_two() {
        assert_eq!(rolling(b"ab", b"ba"), 2);
    }

    #[test]
    fn argument_order_does_not_matter() {
        let a: &[u8] = b"quickly";
        let b: &[u8] = b"quick";
        assert_eq!(rolling(a, b), rolling(b, a));
    }

    #[test]
    fn workspace_is_reused_across_calls() {
        let mut ws = LevenshteinWorkspace::new();
        for _ in 0..8 {
            let d = distance_rolling_rows_with_workspace(b"kitten", b"sitting", &mut ws);
            assert_eq!(d.into_inner(), 3);
        }
        // A comparison of length-6 to length-7 requires 7 cells of scratch
        // (min(6, 7) + 1). The workspace should be at least that big and
        // should not have grown further.
        assert!(ws.capacity() >= 7);
    }

    #[test]
    fn matches_full_matrix_on_canonical_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"a", b"b"),
            (b"abc", b"xyz"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"distance", b"difference"),
            (b"aaaaaaa", b"aaaaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            assert_eq!(
                distance_full_matrix(a, b),
                rolling(a, b),
                "rolling-rows disagreed with full-matrix oracle on ({a:?}, {b:?})"
            );
        }
    }
}
