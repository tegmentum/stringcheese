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
//!
//! # Autovectorisation-friendly loop shape
//!
//! The hot inner loop is written to make LLVM's autovectoriser's job as easy
//! as possible: state that would otherwise be re-loaded from the row buffer
//! every iteration (the "carry" cell `d[i][j-1]` and the outer symbol
//! `long[i-1]`) is hoisted into scalar locals; the inner loop then walks
//! `row[1..=n]` and `short[..]` in lockstep via a `zip` iterator, which is
//! the idiom that lets LLVM elide the per-iteration bounds checks. Compared
//! to the naive `for j in 1..=n { row[j] = … row[j-1] … row[j] … }` shape,
//! this cuts the inner loop from three loads + one store per cell to one
//! load + one store, and unblocks the same autovectorisation that
//! `strsim::levenshtein`'s tight scalar loop gets today.

use stringcheese_core::{Distance, Workspace};

use crate::levenshtein::workspace::LevenshteinWorkspace;

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
// `#[inline]` (rather than `#[inline(always)]`) lets rustc still choose not
// to inline into a huge caller — the trait-object dispatch path through
// `DistanceMetric::distance` genuinely does not want this monomorphised
// into every callsite. Within a single crate the inline attribute is
// enough for LLVM to see through the wrapper and produce a specialised
// inner loop for each concrete `T`, which is what unblocks
// autovectorisation on the ASCII-byte path.
#[inline]
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
    // `short` of length j costs j insertions. `n` is bounded by `u32::MAX`
    // (checked up-front) so every `j <= n` cast to `u32` is in range.
    let _ = u32::try_from(m).expect("input length exceeds u32::MAX");
    let _ = u32::try_from(n).expect("input length exceeds u32::MAX");
    for (j, cell) in row.iter_mut().enumerate() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "n was already checked to fit in u32; j <= n"
        )]
        {
            *cell = j as u32;
        }
    }

    // Split the initialisation cell off the front so the inner loop can walk
    // `row_body` and `short` in lockstep — a same-length `zip` is the shape
    // LLVM's bounds-check elision recognises. `n >= 1` here so the split is
    // always OK.
    let (row_head, row_body) = row
        .split_first_mut()
        .expect("row has at least one cell since n >= 1");
    debug_assert_eq!(row_body.len(), n);

    let mut i: u32 = 0;
    for long_i in long {
        i += 1;
        // `prev_diag` = d[i-1][j-1] at the top of iteration j (initially
        // d[i-1][0] = i-1, which is exactly what `row_head` holds before we
        // overwrite it with the new row's leftmost cell d[i][0] = i).
        let mut prev_diag = *row_head;
        *row_head = i;
        // `curr` = d[i][j-1] going into iteration j (initially d[i][0] = i).
        let mut curr = i;

        for (cell, short_j) in row_body.iter_mut().zip(short.iter()) {
            let cost = u32::from(long_i != short_j);
            // Single load of d[i-1][j] per iteration. The old code loaded
            // this twice (once for `deletion`, once for `prev_diag`) which
            // was a CSE opportunity LLVM did not always take.
            let up = *cell;
            let deletion = up + 1;
            let insertion = curr + 1;
            let substitution = prev_diag + cost;
            curr = deletion.min(insertion).min(substitution);
            prev_diag = up;
            *cell = curr;
        }
        // `curr` now holds d[i][n]; also stored in `row_body[n-1]`.
    }

    // `n >= 1` here, so `row_body` is non-empty and `last()` is `Some`.
    Distance::new(*row_body.last().expect("row_body is non-empty since n >= 1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levenshtein::full_matrix::distance_full_matrix;

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
