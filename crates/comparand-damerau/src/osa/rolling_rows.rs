//! Rolling-row OSA kernel — the production `O(m · n)` time,
//! `O(min(m, n))` space implementation.
//!
//! OSA's DP cell `d[i][j]` depends on `d[i-1][j-1]`, `d[i-1][j]`,
//! `d[i][j-1]`, *and* — when the transposition branch applies —
//! `d[i-2][j-2]`. That extra two-row reach is the one thing that
//! distinguishes OSA's rolling implementation from Levenshtein's: two rows
//! of scratch are not enough, three are needed.
//!
//! This module keeps three rows of `min(m, n) + 1` cells, backed by a
//! single caller-owned [`OsaWorkspace`], and rotates the "which physical
//! row is which logical row" mapping by `i % 3` on every iteration.
//! Rotating indices is materially cheaper than shuffling three `Vec`s
//! around and stays `#![forbid(unsafe_code)]`-safe.

use comparand_core::{Distance, Workspace};

use crate::workspace::OsaWorkspace;

/// Computes the Optimal String Alignment distance between `a` and `b`
/// using three rolling rows backed by `ws`.
///
/// The workspace is grown to `3 · (min(a.len(), b.len()) + 1)` cells if
/// needed, and left at that capacity on return so repeated calls of the
/// same size perform no further allocation.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the DP recurrence is universally written with `a`, `b`, `m`, `n`, `i`, `j`; renaming would put a translation layer between the code and its textbook derivation"
)]
#[allow(
    clippy::similar_names,
    reason = "`prev_off` and `prev2_off` name the two rows above the current row (i-1 and i-2); the pair-shaped naming is the clearest way to express that they are the two components of a three-row rolling window"
)]
pub fn distance_rolling_rows_with_workspace<T: Eq>(
    a: &[T],
    b: &[T],
    ws: &mut OsaWorkspace,
) -> Distance<u32> {
    // Choose the shorter side as the inner dimension so the scratch buffer
    // is `3 · (min(m, n) + 1)` regardless of the caller's argument order.
    // OSA is symmetric, so `osa(long, short) == osa(short, long) == osa(a, b)`.
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let m = long.len();
    let n = short.len();

    if n == 0 {
        return Distance::new(u32::try_from(m).expect("input length exceeds u32::MAX"));
    }

    let stride = n + 1;
    ws.ensure_capacity(3 * stride);
    let buf = ws.buffer_mut(3 * stride);

    // Row 0: `d[0][j] = j`. This lives in the physical slot `(0 % 3) * stride`.
    for (j, cell) in buf[..=n].iter_mut().enumerate() {
        *cell = u32::try_from(j).expect("input length exceeds u32::MAX");
    }

    for i in 1..=m {
        let curr_off = (i % 3) * stride;
        let prev_off = ((i + 3 - 1) % 3) * stride;
        // `prev2_off` is only *read* when `i >= 2`; the value is meaningful
        // starting then. We compute it unconditionally to keep the loop
        // body branchless.
        let prev2_off = ((i + 3 - 2) % 3) * stride;

        // Cell `d[i][0] = i` — leftmost column boundary.
        buf[curr_off] = u32::try_from(i).expect("input length exceeds u32::MAX");

        for j in 1..=n {
            let cost = u32::from(long[i - 1] != short[j - 1]);
            let deletion = buf[prev_off + j] + 1;
            let insertion = buf[curr_off + (j - 1)] + 1;
            let substitution = buf[prev_off + (j - 1)] + cost;
            let mut best = deletion.min(insertion).min(substitution);

            if i >= 2 && j >= 2 && long[i - 1] == short[j - 2] && long[i - 2] == short[j - 1] {
                let transposition = buf[prev2_off + (j - 2)] + 1;
                best = best.min(transposition);
            }

            buf[curr_off + j] = best;
        }
    }

    Distance::new(buf[(m % 3) * stride + n])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osa::full_matrix::distance_full_matrix;

    fn rolling(a: &[u8], b: &[u8]) -> u32 {
        let mut ws = OsaWorkspace::new();
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
    fn adjacent_transposition_is_one() {
        assert_eq!(rolling(b"ab", b"ba"), 1);
    }

    #[test]
    fn kitten_sitting_is_three() {
        assert_eq!(rolling(b"kitten", b"sitting"), 3);
    }

    #[test]
    fn ca_abc_is_three_under_osa() {
        assert_eq!(rolling(b"ca", b"abc"), 3);
    }

    #[test]
    fn argument_order_does_not_matter() {
        let a: &[u8] = b"quickly";
        let b: &[u8] = b"quick";
        assert_eq!(rolling(a, b), rolling(b, a));
    }

    #[test]
    fn workspace_is_reused_across_calls() {
        let mut ws = OsaWorkspace::new();
        for _ in 0..8 {
            let d = distance_rolling_rows_with_workspace(b"kitten", b"sitting", &mut ws);
            assert_eq!(d.into_inner(), 3);
        }
        // 3 rows of (min(6, 7) + 1) = 7 cells → 21 cells minimum.
        assert!(ws.capacity() >= 21);
    }

    #[test]
    fn matches_full_matrix_on_canonical_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"a", b"b"),
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"abc", b"xyz"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"distance", b"difference"),
            (b"aaaaaaa", b"aaaaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
            (b"abcd", b"badc"),
        ] {
            assert_eq!(
                distance_full_matrix(a, b),
                rolling(a, b),
                "rolling-rows disagreed with full-matrix oracle on ({a:?}, {b:?})"
            );
        }
    }
}
