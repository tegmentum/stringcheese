//! Full-matrix OSA oracle.
//!
//! This module contains the deliberately-slow reference implementation the
//! rest of the OSA kernels are validated against. It computes the full
//! `(m+1) × (n+1)` dynamic-programming matrix, exactly as it appears in
//! every textbook derivation of "restricted Damerau-Levenshtein". It is
//! written for clarity: variable names match the standard notation and the
//! loop body is a one-to-one translation of the recurrence.
//!
//! # Complexity
//!
//! `O(m · n)` time. `O(m · n)` space. Both dimensions are proportional to
//! input length — this kernel exists to be *correct*, not to be *fast*. The
//! production kernel is [`crate::damerau::osa::rolling_rows`], which uses
//! `O(min(m, n))` auxiliary space with the same time complexity.
//!
//! # Role
//!
//! Two conditions make an oracle useful:
//!
//! 1. It is written from a different structural formulation than the code it
//!    validates. The full-matrix form and the rolling-row form share only the
//!    recurrence itself; they differ in loop structure, indexing, and buffer
//!    management. A shared bug in the recurrence would be caught by canonical
//!    test vectors; independent bugs in the two structures cancel out only
//!    accidentally.
//! 2. It is trivial to inspect by eye. Any deviation from the textbook form
//!    should be readily visible during review.
//!
//! # Generic sequences
//!
//! The kernel operates on any `&[T]` where `T: Eq`. String comparisons pick
//! their representation (bytes, `char`s, grapheme clusters, tokens) at the
//! call site — the kernel itself takes no view on which representation is
//! semantically correct.

use alloc::vec;

/// Computes the Optimal String Alignment distance between `a` and `b`
/// using the full dynamic-programming matrix.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols. Callers with
/// inputs approaching that scale should use a bit-parallel implementation
/// rather than a `u32` DP kernel.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names `a`, `b`, `m`, `n`, `d`, `i`, `j` are the standard mathematical notation used in every textbook derivation of this algorithm; renaming them for clippy would obscure the oracle's role as a direct translation of that derivation"
)]
pub fn distance_full_matrix<T: Eq>(a: &[T], b: &[T]) -> u32 {
    let m = a.len();
    let n = b.len();

    // Fast paths that also let the main loop assume both inputs are non-empty.
    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }
    if n == 0 {
        return u32::try_from(m).expect("input length exceeds u32::MAX");
    }

    // A row-major dense matrix `d[i][j]`, laid out flat for cache-friendliness
    // — the ORACLE remains one-line-per-recurrence-branch below, so the
    // flattening is a mechanical change, not a clarity loss.
    let cols = n + 1;
    let mut d = vec![0u32; (m + 1) * cols];

    // Boundary conditions: transforming an empty prefix into a prefix of
    // length k costs k insertions (or deletions, symmetrically).
    for i in 0..=m {
        d[i * cols] = u32::try_from(i).expect("input length exceeds u32::MAX");
    }
    for (j, cell) in d[..=n].iter_mut().enumerate() {
        *cell = u32::try_from(j).expect("input length exceeds u32::MAX");
    }

    // Interior cells: the standard Levenshtein three-way minimum, augmented
    // with the transposition candidate at `d[i-2][j-2] + 1` when the two
    // preceding symbols are a swapped adjacent pair.
    for i in 1..=m {
        for j in 1..=n {
            let cost = u32::from(a[i - 1] != b[j - 1]);
            let deletion = d[(i - 1) * cols + j] + 1;
            let insertion = d[i * cols + (j - 1)] + 1;
            let substitution = d[(i - 1) * cols + (j - 1)] + cost;
            let mut best = deletion.min(insertion).min(substitution);

            if i >= 2 && j >= 2 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                let transposition = d[(i - 2) * cols + (j - 2)] + 1;
                best = best.min(transposition);
            }

            d[i * cols + j] = best;
        }
    }

    d[m * cols + n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pair_is_zero() {
        assert_eq!(distance_full_matrix::<u8>(b"", b""), 0);
    }

    #[test]
    fn one_side_empty_is_other_length() {
        assert_eq!(distance_full_matrix(b"", b"hello"), 5);
        assert_eq!(distance_full_matrix(b"hello", b""), 5);
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(distance_full_matrix(b"abcdef", b"abcdef"), 0);
    }

    #[test]
    fn adjacent_transposition_is_one() {
        // The characteristic OSA/Damerau operation: a single adjacent swap
        // costs one, not two (which is what plain Levenshtein would give).
        assert_eq!(distance_full_matrix(b"ab", b"ba"), 1);
    }

    #[test]
    fn kitten_sitting_is_three() {
        // Same score as ordinary Levenshtein: no adjacent transposition
        // appears in the optimal edit sequence.
        assert_eq!(distance_full_matrix(b"kitten", b"sitting"), 3);
    }

    #[test]
    fn insertion_is_one() {
        assert_eq!(distance_full_matrix(b"cat", b"cats"), 1);
    }

    #[test]
    fn deletion_is_one() {
        assert_eq!(distance_full_matrix(b"cats", b"cat"), 1);
    }

    #[test]
    fn substitution_is_one() {
        assert_eq!(distance_full_matrix(b"cat", b"cot"), 1);
    }

    #[test]
    fn ca_abc_is_three_under_osa() {
        // The distinguishing example versus full Damerau, which scores this
        // pair as 2. Under OSA the "one edit per substring" restriction
        // forbids reusing the transposed characters in a subsequent
        // insertion, so the optimal path costs 3.
        assert_eq!(distance_full_matrix(b"ca", b"abc"), 3);
    }

    #[test]
    fn generic_over_char() {
        let a: &[char] = &['c', 'a', 'f', 'é'];
        let b: &[char] = &['c', 'a', 'f', 'e'];
        assert_eq!(distance_full_matrix(a, b), 1);
    }
}
