//! Scalar, SIMD-shaped OSA kernel for byte-slice inputs.
//!
//! This is the SIMD backend's portable fallback and the reference against
//! which every arch-specific SIMD implementation is differentially tested.
//! It computes the same numeric result as
//! [`crate::damerau::osa::rolling_rows::distance_rolling_rows_with_workspace`]
//! but is self-contained: it owns its own three-row rolling buffer on the
//! heap and takes no workspace argument, keeping the SIMD sub-tree
//! independently exercised without cross-kernel oracle coupling.
//!
//! # Algorithm
//!
//! OSA extends unit-cost Levenshtein with a single new operation — the
//! transposition of two adjacent symbols, cost `1` — subject to the
//! "no substring edited twice" restriction:
//!
//! ```text
//!   d[i][j] = min(
//!       d[i-1][j]   + 1,             // deletion
//!       d[i][j-1]   + 1,             // insertion
//!       d[i-1][j-1] + cost,          // substitution (cost = a[i-1] != b[j-1])
//!       d[i-2][j-2] + 1              // transposition (only if
//!                                    //   a[i-1] == b[j-2] and
//!                                    //   a[i-2] == b[j-1])
//!   )
//! ```
//!
//! The transposition branch reaches to `d[i-2][j-2]`, so the rolling
//! implementation needs three rows in scratch. This kernel keeps them in
//! a single flat `Vec<u32>` of `3 · (min(m, n) + 1)` cells and rotates
//! the "which physical row is which logical row" mapping by `i % 3` on
//! every iteration — the same tiny arithmetic the sibling
//! [`crate::damerau::osa::rolling_rows`] uses.
//!
//! # SIMD lifting is deferred
//!
//! A true bit-parallel OSA in the shape of Hyyrö (2003) —
//! Myers's word-parallel Levenshtein extended with an extra bit-vector
//! that carries the transposition-match state between adjacent columns —
//! is documented follow-up work. The recurrence is more delicate than
//! the classical Myers pattern (the transposition bit depends on the
//! *previous column's* equality mask, so the propagation needs a
//! per-column bookkeeping word), and getting it bit-for-bit right on
//! wide-block inputs demands its own dedicated commit alongside a
//! full differential test sweep. This module's current shape puts the
//! dispatch scaffolding in place and keeps correctness anchored on the
//! rolling-rows form; the arch-specific backends can be upgraded to
//! Hyyrö-style bit-parallel OSA behind the same public API.
//!
//! # Reference for the deferred bit-parallel form
//!
//! - Hyyrö, H. (2003). "Bit-parallel approximate string matching
//!   algorithms with transposition." *SPIRE 2003*, LNCS 2857, 95-107.
//!   <https://doi.org/10.1007/978-3-540-39984-1_8>

use alloc::vec;

/// Computes the OSA distance between byte-slice inputs `a` and `b`.
///
/// Returns the same `u32` distance that
/// [`crate::damerau::osa::rolling_rows`] would return on the same inputs.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols — matching
/// the panic contract of the crate's other DP kernels.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the DP recurrence is universally written with `a`, `b`, `m`, `n`, `i`, `j`; renaming would put a translation layer between the code and its textbook derivation"
)]
#[allow(
    clippy::similar_names,
    reason = "`prev_off` and `prev2_off` name the two rows above the current row (i-1 and i-2); the pair-shaped naming is the clearest way to express that they are the two components of a three-row rolling window"
)]
pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    // Choose the shorter side as the inner dimension so the scratch
    // buffer is `3 · (min(m, n) + 1)` regardless of the caller's argument
    // order. OSA is symmetric.
    let (long, short) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let m = long.len();
    let n = short.len();

    if n == 0 {
        return u32::try_from(m).expect("input length exceeds u32::MAX");
    }

    let stride = n + 1;
    let mut buf: alloc::vec::Vec<u32> = vec![0u32; 3 * stride];

    // Row 0: `d[0][j] = j`, in physical slot `(0 % 3) * stride = 0`.
    for (j, cell) in buf[..=n].iter_mut().enumerate() {
        *cell = u32::try_from(j).expect("input length exceeds u32::MAX");
    }

    for i in 1..=m {
        let curr_off = (i % 3) * stride;
        let prev_off = ((i + 3 - 1) % 3) * stride;
        // `prev2_off` is only read when `i >= 2`; computing it
        // unconditionally keeps the loop body branchless.
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

    buf[(m % 3) * stride + n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damerau::osa::full_matrix::distance_full_matrix;

    #[test]
    fn empty_pair_is_zero() {
        assert_eq!(distance(b"", b""), 0);
    }

    #[test]
    fn one_side_empty_is_other_length() {
        assert_eq!(distance(b"", b"hello"), 5);
        assert_eq!(distance(b"hello", b""), 5);
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(distance(b"abcdef", b"abcdef"), 0);
    }

    #[test]
    fn adjacent_transposition_is_one() {
        assert_eq!(distance(b"ab", b"ba"), 1);
    }

    #[test]
    fn kitten_sitting_is_three() {
        assert_eq!(distance(b"kitten", b"sitting"), 3);
    }

    #[test]
    fn ca_abc_is_three_under_osa() {
        // Distinguishing example vs full Damerau (which gives 2).
        assert_eq!(distance(b"ca", b"abc"), 3);
    }

    #[test]
    fn argument_order_does_not_matter() {
        assert_eq!(
            distance(b"quickly", b"quick"),
            distance(b"quick", b"quickly")
        );
    }

    #[test]
    fn boundary_length_63_matches_oracle() {
        // Just inside the (future) single-word Myers cutoff.
        let a: alloc::vec::Vec<u8> = (0..63u8).collect();
        let mut b = a.clone();
        b.swap(10, 11);
        b[40] ^= 0x02;
        assert_eq!(distance(&a, &b), distance_full_matrix(&a, &b));
    }

    #[test]
    fn boundary_length_64_matches_oracle() {
        // Exactly on the single-word Myers boundary.
        let a: alloc::vec::Vec<u8> = (0..64u8).collect();
        let mut b = a.clone();
        b.swap(0, 1);
        b.swap(62, 63);
        assert_eq!(distance(&a, &b), distance_full_matrix(&a, &b));
    }

    #[test]
    fn boundary_length_65_matches_oracle() {
        // One position past the single-word boundary — the future
        // block-Myers case.
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b[64] ^= 0x01;
        assert_eq!(distance(&a, &b), distance_full_matrix(&a, &b));
    }

    #[test]
    fn boundary_length_128_matches_oracle() {
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b.swap(0, 1);
        b[63] ^= 0x02;
        b.swap(64, 65);
        b[127] ^= 0x08;
        assert_eq!(distance(&a, &b), distance_full_matrix(&a, &b));
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
                distance(a, b),
                "SIMD-shaped scalar OSA disagreed with full-matrix oracle on ({a:?}, {b:?})"
            );
        }
    }
}
