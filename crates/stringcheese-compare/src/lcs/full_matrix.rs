//! Full-matrix LCS oracle.
//!
//! This module contains the deliberately-slow reference implementations the
//! rest of the crate is validated against. Two entry points are provided:
//!
//! * [`lcs_length_full_matrix`] — the length of the longest common
//!   subsequence, computed by the textbook DP.
//! * [`lcs_distance_full_matrix`] — the LCS-distance metric,
//!   `|a| + |b| - 2 · lcs(a, b)`, computed from the same matrix.
//!
//! Both functions compute the full `(m+1) × (n+1)` dynamic-programming
//! matrix, exactly as it appears in every textbook. The recurrence is the
//! "no substitution" specialization of Wagner and Fischer's (1974)
//! string-to-string correction DP; see the crate-level `References` section
//! for the full citation. The recurrence is:
//!
//! ```text
//! lcs[i][j] = 0                              if i == 0 or j == 0
//!           = lcs[i-1][j-1] + 1              if a[i-1] == b[j-1]
//!           = max(lcs[i-1][j], lcs[i][j-1])  otherwise
//! ```
//!
//! # Complexity
//!
//! `O(m · n)` time. `O(m · n)` space. Both dimensions are proportional to
//! input length — this kernel exists to be *correct*, not to be *fast*. The
//! production kernel is [`crate::lcs::rolling_rows`], which uses `O(min(m, n))`
//! auxiliary space with the same time complexity.
//!
//! # Role
//!
//! Two conditions make an oracle useful:
//!
//! 1. It is written from a different structural formulation than the code
//!    it validates. The full-matrix form and the rolling-row form share
//!    only the recurrence itself; they differ in loop structure, indexing,
//!    and buffer management. A shared bug in the recurrence would be caught
//!    by canonical test vectors; independent bugs in the two structures
//!    cancel out only accidentally.
//! 2. It is trivial to inspect by eye. Any deviation from the textbook form
//!    should be readily visible during review.
//!
//! # Generic sequences
//!
//! Both kernels operate on any `&[T]` where `T: Eq`. String comparisons
//! pick their representation (bytes, `char`s, grapheme clusters, tokens)
//! at the call site — the kernel itself takes no view on which
//! representation is semantically correct.

use alloc::vec;

use stringcheese_core::{Distance, Score};

/// Computes the length of the longest common subsequence of `a` and `b`
/// using the full dynamic-programming matrix.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols. Callers with
/// inputs approaching that scale should use a bit-parallel or Hunt–Szymanski
/// implementation rather than a `u32` DP kernel.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names `a`, `b`, `m`, `n`, `d`, `i`, `j` are the standard mathematical notation used in every textbook derivation of this algorithm; renaming them for clippy would obscure the oracle's role as a direct translation of that derivation"
)]
pub fn lcs_length_full_matrix<T: Eq>(a: &[T], b: &[T]) -> Score<u32> {
    let m = a.len();
    let n = b.len();

    // Fast paths that also let the main loop assume both inputs are
    // non-empty. LCS length of anything with an empty side is zero.
    if m == 0 || n == 0 {
        return Score::new(0);
    }

    // A row-major dense matrix `d[i][j]`, laid out flat for
    // cache-friendliness — the ORACLE remains one-line-per-recurrence-branch
    // below, so the flattening is a mechanical change, not a clarity loss.
    let cols = n + 1;
    let mut d = vec![0u32; (m + 1) * cols];

    // Boundary conditions: `lcs[0][*] = lcs[*][0] = 0` are already set by
    // the zero-initialization of `d` above; no explicit fill is needed.

    // Interior cells: match extends the previous diagonal by one; mismatch
    // takes the better of dropping one symbol from either side.
    for i in 1..=m {
        for j in 1..=n {
            let cell = if a[i - 1] == b[j - 1] {
                d[(i - 1) * cols + (j - 1)] + 1
            } else {
                let up = d[(i - 1) * cols + j];
                let left = d[i * cols + (j - 1)];
                up.max(left)
            };
            d[i * cols + j] = cell;
        }
    }

    let length = d[m * cols + n];
    // A subsequence of two length-`min(m, n)` inputs cannot exceed either
    // length; check the invariant in debug builds so a broken future
    // refactor is caught at test time rather than by a downstream property.
    debug_assert!(u64::from(length) <= m as u64);
    debug_assert!(u64::from(length) <= n as u64);
    Score::new(length)
}

/// Computes the LCS distance between `a` and `b` — the minimum number of
/// single-symbol insertions plus deletions needed to transform `a` into
/// `b`.
///
/// This is computed as `|a| + |b| - 2 · lcs(a, b)`, using
/// [`lcs_length_full_matrix`] for the LCS length. The subtraction cannot
/// underflow: `lcs(a, b) ≤ min(|a|, |b|) ≤ (|a| + |b|) / 2`, so
/// `2 · lcs(a, b) ≤ |a| + |b|`.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols, or if
/// `|a| + |b|` would overflow `u32`.
#[must_use]
pub fn lcs_distance_full_matrix<T: Eq>(a: &[T], b: &[T]) -> Distance<u32> {
    let lcs = lcs_length_full_matrix(a, b).into_inner();
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

    fn len(a: &[u8], b: &[u8]) -> u32 {
        lcs_length_full_matrix(a, b).into_inner()
    }

    fn dist(a: &[u8], b: &[u8]) -> u32 {
        lcs_distance_full_matrix(a, b).into_inner()
    }

    #[test]
    fn empty_pair_length_is_zero() {
        assert_eq!(len(b"", b""), 0);
    }

    #[test]
    fn one_side_empty_length_is_zero() {
        assert_eq!(len(b"", b"hello"), 0);
        assert_eq!(len(b"hello", b""), 0);
    }

    #[test]
    fn identical_length_is_input_length() {
        assert_eq!(len(b"abcdef", b"abcdef"), 6);
    }

    #[test]
    fn disjoint_alphabet_length_is_zero() {
        assert_eq!(len(b"abc", b"xyz"), 0);
    }

    #[test]
    fn textbook_abcbdab_bdcab_length_is_four() {
        // Cormen et al., "Introduction to Algorithms" 3rd ed., section 15.4:
        // LCS of "ABCBDAB" and "BDCAB" is length 4 (BCAB or BDAB).
        assert_eq!(len(b"ABCBDAB", b"BDCAB"), 4);
    }

    #[test]
    fn textbook_agcat_gac_length_is_two() {
        // Common textbook worked example: the LCS of "AGCAT" and "GAC" has
        // length 2 (e.g. "GA" or "AC" or "GC").
        assert_eq!(len(b"AGCAT", b"GAC"), 2);
    }

    #[test]
    fn empty_pair_distance_is_zero() {
        assert_eq!(dist(b"", b""), 0);
    }

    #[test]
    fn one_side_empty_distance_is_other_length() {
        assert_eq!(dist(b"", b"hello"), 5);
        assert_eq!(dist(b"hello", b""), 5);
    }

    #[test]
    fn insertion_distance_is_one() {
        // LCS("abc", "abcd") = 3. Distance = 3 + 4 - 2*3 = 1.
        assert_eq!(dist(b"abc", b"abcd"), 1);
    }

    #[test]
    fn substitution_distance_is_two() {
        // LCS("abcd", "abed") = 3 ("abd"). Distance = 4 + 4 - 6 = 2.
        // Contrast with Levenshtein, which would return 1.
        assert_eq!(dist(b"abcd", b"abed"), 2);
    }

    #[test]
    fn agcat_gac_distance_is_four() {
        // LCS = 2, distance = 5 + 3 - 4 = 4.
        assert_eq!(dist(b"AGCAT", b"GAC"), 4);
    }

    #[test]
    fn generic_over_char() {
        let a: &[char] = &['c', 'a', 'f', 'é'];
        let b: &[char] = &['c', 'a', 'f', 'e'];
        // The LCS is "caf" — the diacritic differs at char granularity.
        assert_eq!(lcs_length_full_matrix(a, b).into_inner(), 3);
        // Distance is 4 + 4 - 6 = 2 (delete é, insert e).
        assert_eq!(lcs_distance_full_matrix(a, b).into_inner(), 2);
    }

    #[test]
    fn generic_over_i32_tokens() {
        let a: &[i32] = &[10, 20, 30, 40];
        let b: &[i32] = &[10, 25, 30, 40];
        assert_eq!(lcs_length_full_matrix(a, b).into_inner(), 3);
        assert_eq!(lcs_distance_full_matrix(a, b).into_inner(), 2);
    }
}
