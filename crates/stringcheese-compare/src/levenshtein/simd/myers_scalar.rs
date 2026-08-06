//! Scalar Myers 1999 bit-parallel Levenshtein kernel.
//!
//! This is the SIMD backend's portable fallback and the reference against
//! which every arch-specific SIMD implementation is differentially tested.
//! It computes the same numeric result as
//! [`crate::levenshtein::rolling_rows`], but in one 64-bit machine word per
//! block of 64 pattern positions rather than one `u32` per cell — that alone
//! is a factor-of-64 reduction in cell updates on the tight inner loop for
//! any pattern that fits in a single word.
//!
//! # Algorithm
//!
//! Myers (1999) reformulates the classical Wagner–Fischer edit-distance DP
//! in terms of the *differences* between adjacent DP cells rather than the
//! cell values themselves. Because a unit-cost DP cell can only differ from
//! its neighbours by −1, 0, or +1, four bit-vectors — one per direction and
//! sign — suffice to encode a whole column. All four fit in a single
//! machine word for patterns up to `w` symbols (`w = 64` here), and the
//! recurrence between successive columns reduces to a handful of bitwise
//! operations plus one addition. See:
//!
//! * G. Myers, "A Fast Bit-Vector Algorithm for Approximate String Matching
//!   Based on Dynamic Programming", *Journal of the ACM* 46(3), 1999,
//!   pp. 395–415. <https://doi.org/10.1145/316542.316550>
//!
//! # Scope
//!
//! Byte-slice inputs (`&[u8]`) only. The Peq lookup table is indexed by
//! byte, so the algorithm assumes a 256-symbol alphabet. Unicode-scalar
//! (`char`) inputs would need either a hash-backed Peq or a per-symbol
//! restructuring of the table; those callers stay on the char-based
//! rolling-rows path in [`crate::levenshtein::algorithm`].
//!
//! # Single-word vs. multi-word patterns
//!
//! The single-word variant is exact for patterns of length ≤ 64. Longer
//! patterns need Myers's block extension (§5 of the 1999 paper) or
//! Hyyrö's presentation of the same — block Myers is future work; the
//! current implementation transparently falls back to a scalar rolling
//! rows for any pattern that doesn't fit in one word. The fallback keeps
//! correctness while the block variant is being landed and validated.

use alloc::vec;

/// Machine word width used by the bit-parallel kernel. Fixed at 64.
const W: usize = 64;

/// Computes the unit-cost Levenshtein distance between `a` and `b`.
///
/// The shorter side is treated as the pattern and packed into 64-bit words;
/// the longer side is the text processed one symbol per outer-loop
/// iteration. Choosing the shorter side as the pattern minimizes the
/// number of blocks (and therefore the cost per column) regardless of the
/// caller's argument order.
///
/// Returns the same `u32` distance that
/// [`crate::levenshtein::rolling_rows`] would return on the same inputs.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols — matching
/// the panic contract of the crate's other DP kernels.
#[must_use]
pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    // Pick the shorter side as the pattern. Myers's algorithm is
    // symmetric — d(a, b) = d(b, a) — so this only affects the shape of
    // the work, not the answer.
    let (pattern, text) = if a.len() <= b.len() { (a, b) } else { (b, a) };

    let m = pattern.len();
    let n = text.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }

    if m <= W {
        return single_word(pattern, text);
    }

    // Block Myers is future work; fall back to a scalar rolling-rows DP
    // so long-pattern correctness is never at risk. This inlined
    // rolling-rows avoids reintroducing a `LevenshteinWorkspace` dependency
    // and keeps the SIMD module self-contained.
    fallback_rolling_rows(pattern, text)
}

/// Single-word Myers 1999 for patterns of length `1 ≤ m ≤ 64`.
///
/// Follows the pseudocode of §4 of the paper verbatim: `Pv` and `Mv` are
/// the vertical positive- and negative-delta bit-vectors for the current
/// column, `score` tracks `d[m][j]` incrementally, and `msb` selects the
/// bit that governs the score update at the bottom of each column.
#[inline]
fn single_word(pattern: &[u8], text: &[u8]) -> u32 {
    let m = pattern.len();
    debug_assert!((1..=W).contains(&m));

    // Peq[c] has bit `i` set iff pattern[i] == c. 256 entries covers the
    // full byte alphabet; the table is stack-allocated and cheap to build.
    let mut peq: [u64; 256] = [0u64; 256];
    for (i, &c) in pattern.iter().enumerate() {
        peq[c as usize] |= 1u64 << i;
    }

    // Pv starts as `1^m` — every one of the m pattern positions has a
    // +1 vertical delta because d[i][0] = i. Mv starts at zero.
    let mut pv: u64 = if m == W { !0u64 } else { (1u64 << m) - 1 };
    let mut mv: u64 = 0;
    let msb: u64 = 1u64 << (m - 1);

    // score tracks d[m][j]. d[m][0] = m.
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    for &c in text {
        let eq = peq[c as usize];
        let xv = eq | mv;
        // The addition `(Eq & Pv) + Pv` computes the "leftmost match"
        // that Myers's derivation uses to produce the horizontal-delta
        // bit-vectors; overflow of a single u64 is intentional here and
        // is what makes the algorithm bit-parallel in the first place.
        let xh = (((eq & pv).wrapping_add(pv)) ^ pv) | eq;

        let mut ph = mv | !(xh | pv);
        let mut mh = pv & xh;

        if ph & msb != 0 {
            score += 1;
        }
        if mh & msb != 0 {
            score -= 1;
        }

        ph = (ph << 1) | 1;
        mh <<= 1;

        pv = mh | !(xv | ph);
        mv = ph & xv;
    }

    score
}

/// Scalar rolling-rows DP used when the pattern does not fit in a single
/// word. This is a local copy of the recurrence — it deliberately does
/// not go through [`crate::levenshtein::rolling_rows`] so that the SIMD
/// path stays independently exercised in its own tests without creating
/// a cross-kernel oracle cycle.
#[inline]
fn fallback_rolling_rows(pattern: &[u8], text: &[u8]) -> u32 {
    // Pattern is the shorter side per the caller's convention. Keep it
    // as the inner (row) dimension so the buffer is `m + 1` u32s.
    let m = pattern.len();
    let n = text.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }

    let mut row: alloc::vec::Vec<u32> = vec![0u32; m + 1];
    for (i, cell) in row.iter_mut().enumerate() {
        *cell = u32::try_from(i).expect("input length exceeds u32::MAX");
    }

    for j in 1..=n {
        let mut prev_diag = row[0];
        row[0] = u32::try_from(j).expect("input length exceeds u32::MAX");
        for i in 1..=m {
            let cost = u32::from(pattern[i - 1] != text[j - 1]);
            let deletion = row[i] + 1;
            let insertion = row[i - 1] + 1;
            let substitution = prev_diag + cost;
            let curr = deletion.min(insertion).min(substitution);
            prev_diag = row[i];
            row[i] = curr;
        }
    }

    row[m]
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn kitten_sitting_is_three() {
        assert_eq!(distance(b"kitten", b"sitting"), 3);
    }

    #[test]
    fn transposition_is_two() {
        assert_eq!(distance(b"ab", b"ba"), 2);
    }

    #[test]
    fn all_different_equals_max_length() {
        assert_eq!(distance(b"abc", b"xyz"), 3);
    }

    #[test]
    fn argument_order_does_not_matter() {
        assert_eq!(
            distance(b"quickly", b"quick"),
            distance(b"quick", b"quickly")
        );
    }

    #[test]
    fn boundary_length_63_matches_scalar() {
        // Just inside the single-word cutoff.
        let a: alloc::vec::Vec<u8> = (0..63u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[40] ^= 0x02;
        assert_eq!(distance(&a, &b), 2);
    }

    #[test]
    fn boundary_length_64_matches_scalar() {
        // Exactly on the single-word boundary; the `m == W` branch is
        // where the mask `(1 << m) - 1` would overflow if not special-cased.
        let a: alloc::vec::Vec<u8> = (0..64u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x80;
        b[63] ^= 0x80;
        assert_eq!(distance(&a, &b), 2);
    }

    #[test]
    fn boundary_length_65_uses_fallback() {
        // One symbol beyond the single-word cutoff triggers the
        // rolling-rows fallback; the answer must still be correct.
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b[64] ^= 0x01;
        assert_eq!(distance(&a, &b), 1);
    }

    #[test]
    fn matches_full_matrix_on_canonical_pairs() {
        use crate::levenshtein::full_matrix::distance_full_matrix;
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
                distance(a, b),
                "myers_scalar disagreed with full-matrix oracle on ({a:?}, {b:?})"
            );
        }
    }
}
