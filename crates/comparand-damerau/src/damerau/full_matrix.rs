//! Full-matrix full-Damerau oracle (Lowrance-Wagner 1975 recurrence).
//!
//! This module contains the deliberately-slow reference implementation the
//! production Damerau kernel is validated against. It computes the full
//! `(m+1) × (n+1)` dynamic-programming matrix and locates the "last
//! position of `b[j-1]` in `a`" via a **linear scan** through the previous
//! rows, rather than a hash-table lookup. That choice keeps the trait bound
//! at `T: Eq` (matching the OSA kernels and Levenshtein), and gives the
//! oracle a genuine structural difference from the production kernel — the
//! two locate the transposition source through independent code paths.
//!
//! # Complexity
//!
//! `O(m² · n)` time in the worst case (the linear scan is at most
//! `O(m)` per cell). `O(m · n)` space. This kernel exists to be *correct*,
//! not to be *fast*.
//!
//! # Recurrence
//!
//! The recurrence is Lowrance and Wagner's, restated at the `(m+1) × (n+1)`
//! matrix layout without the enclosing sentinel border used in some
//! textbook presentations (the transposition candidate is guarded by
//! explicit `k > 0 && l > 0` checks rather than by a `+∞` sentinel row):
//!
//! ```text
//!   d[i][j] = min(
//!       d[i-1][j]     + 1,                 // deletion
//!       d[i][j-1]     + 1,                 // insertion
//!       d[i-1][j-1]   + cost,              // substitution
//!       d[k-1][l-1]   + (i-k-1) + 1 + (j-l-1)   // transposition
//!   )
//! ```
//!
//! where `k` is the largest `i' < i` with `a[i'-1] == b[j-1]` (or `0` if
//! no such `i'` exists), and `l` is the largest `j' < j` seen *this row*
//! with `a[i-1] == b[j'-1]` (also `0` if none). The transposition branch
//! is only taken when both `k > 0` and `l > 0`.
//!
//! # Generic sequences
//!
//! The kernel operates on any `&[T]` where `T: Eq`. The linear-scan
//! auxiliary lookup is what makes the `Eq`-only bound possible.

use alloc::vec;

/// Computes the full (unrestricted) Damerau-Levenshtein distance between
/// `a` and `b` using the Lowrance-Wagner recurrence and a linear scan for
/// the transposition auxiliary lookup.
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
pub fn distance_full_matrix<T: Eq>(a: &[T], b: &[T]) -> u32 {
    let m = a.len();
    let n = b.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }
    if n == 0 {
        return u32::try_from(m).expect("input length exceeds u32::MAX");
    }

    let stride = n + 1;
    let mut d = vec![0u32; (m + 1) * stride];

    // Boundary conditions: `d[i][0] = i`, `d[0][j] = j`.
    for i in 0..=m {
        d[i * stride] = u32::try_from(i).expect("input length exceeds u32::MAX");
    }
    for (j, cell) in d[..=n].iter_mut().enumerate() {
        *cell = u32::try_from(j).expect("input length exceeds u32::MAX");
    }

    for i in 1..=m {
        // `db`: the largest column j' < j seen *this row* where
        // `a[i-1] == b[j'-1]`. Reset at the start of each row.
        let mut db: usize = 0;

        for j in 1..=n {
            // Linear scan for `k`: the largest row i' < i where
            // `a[i'-1] == b[j-1]`, or 0 if no such row exists. Scanning
            // from i-1 downward returns the *most recent* match first.
            let mut k: usize = 0;
            for i2 in (1..i).rev() {
                if a[i2 - 1] == b[j - 1] {
                    k = i2;
                    break;
                }
            }
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

            // Transposition branch: only meaningful when both `k > 0` and
            // `l > 0`. When either is zero the pseudocode reads a sentinel
            // cell at `d[-1][.]` or `d[.][-1]` that is set to `m + n`; the
            // explicit guard here achieves the same effect without the
            // sentinel row/column.
            if k > 0 && l > 0 {
                let base = d[(k - 1) * stride + (l - 1)];
                // Since `k <= i-1` and `l <= j-1`, both `(i - k - 1)` and
                // `(j - l - 1)` are non-negative usize. Their sum plus 1
                // fits comfortably in u32 when `m + n <= u32::MAX`, which
                // the empty-input guards implicitly assumed.
                let gap = (i - k - 1) + 1 + (j - l - 1);
                let gap_u32 = u32::try_from(gap).expect("transposition gap exceeds u32::MAX");
                let transposition = base.saturating_add(gap_u32);
                best = best.min(transposition);
            }

            d[i * stride + j] = best;
        }
    }

    d[m * stride + n]
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
        assert_eq!(distance_full_matrix(b"ab", b"ba"), 1);
    }

    #[test]
    fn kitten_sitting_is_three() {
        assert_eq!(distance_full_matrix(b"kitten", b"sitting"), 3);
    }

    #[test]
    fn ca_abc_is_two_under_full_damerau() {
        // The distinguishing example versus OSA (which scores this pair as
        // 3). Full Damerau: transpose "ca" → "ac" (cost 1), then insert
        // "b" between them → "abc" (cost 1). Total 2.
        assert_eq!(distance_full_matrix(b"ca", b"abc"), 2);
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
    fn generic_over_char() {
        let a: &[char] = &['c', 'a', 'f', 'é'];
        let b: &[char] = &['c', 'a', 'f', 'e'];
        assert_eq!(distance_full_matrix(a, b), 1);
    }

    #[test]
    fn generic_over_i32_tokens() {
        let a: &[i32] = &[10, 20, 30, 40];
        let b: &[i32] = &[10, 25, 30, 40];
        assert_eq!(distance_full_matrix(a, b), 1);
    }

    #[test]
    fn multiple_transpositions() {
        // "abcd" -> "badc": swap (a, b), swap (c, d). Two transpositions;
        // Damerau counts each as one operation, so distance = 2.
        assert_eq!(distance_full_matrix(b"abcd", b"badc"), 2);
    }
}
