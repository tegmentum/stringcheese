//! Scalar, SIMD-shaped Jaro similarity kernel for byte-slice inputs.
//!
//! This is the SIMD backend's portable fallback and the reference against
//! which every arch-specific SIMD implementation is differentially tested.
//! It computes the same numeric result as the generic Jaro kernel in
//! [`crate::jaro`], but rewrites the matching-window
//! scan into a form that maps cleanly to vector loads — the inner loop
//! walks a contiguous slice of `b` and compares each cell against a
//! broadcast `a[i]` plus a "not-already-matched" bitmap mask. On a
//! scalar-only host that shape lowers to a tight scalar loop; the
//! four sibling arch backends (`x86_avx2`, `x86_sse2`, `aarch64_neon`,
//! `wasm_simd128`) lift the same shape
//! into arch-specific byte-lane compares (`_mm256_cmpeq_epi8` on AVX2,
//! `_mm_cmpeq_epi8` on SSE2, `vceqq_u8` on NEON, `u8x16_eq` on wasm
//! SIMD128) with the matching mask-reduction (`_mm256_movemask_epi8`,
//! `_mm_movemask_epi8`, NEON's `vshrn_n_u16::<4>` nibble movemask, and
//! `u8x16_bitmask` respectively), without any structural change to the
//! algorithm.
//!
//! # Algorithm
//!
//! Jaro (1989) defines the similarity as
//!
//! ```text
//!     jaro(a, b) = 0                                            if m = 0
//!                = (m/|a| + m/|b| + (m - t)/m) / 3              otherwise
//! ```
//!
//! where `m` is the number of matches inside a window of half-radius
//! `w = max(|a|, |b|) / 2 - 1`, and `t` is the number of transpositions
//! among the matched positions. See
//! [`crate::jaro`] for the full derivation; this module is the
//! byte-slice-only, SIMD-shaped restatement.
//!
//! # Scope
//!
//! Byte-slice inputs (`&[u8]`) only. Unicode-scalar callers (`&[char]`)
//! stay on the generic Jaro kernel path (see [`crate::jaro`]) because
//! the SIMD window scan assumes fixed-width symbols that fit in a byte
//! lane.
//!
//! # Bitmap layout
//!
//! `b_matched` is a packed bit-vector (one bit per position of `b`)
//! rather than a `Vec<bool>` of one byte per position. Packing lets the
//! four arch backends AND the bitmap against the block's comparison-mask
//! word directly — the shared `common::Bitmap::read_bits` hands
//! back the exact slice of `b_matched` corresponding to a block's window
//! range as one integer — and it halves the memory traffic of the scalar
//! path too.
//!
//! # Reference
//!
//! - Jaro, M. A. (1989). "Advances in Record-Linkage Methodology as
//!   Applied to Matching the 1985 Census of Tampa, Florida." *Journal of
//!   the American Statistical Association*, 84(406), 414-420.
//!   <https://doi.org/10.1080/01621459.1989.10478785>

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};

/// Computes the Jaro similarity between byte-slice inputs `a` and `b`.
///
/// Returns the same `f64` in `[0.0, 1.0]` that the generic Jaro kernel
/// in [`crate::jaro`] would return on the same inputs. The two-empty and
/// one-empty boundary conventions match the scalar kernel bit-for-bit.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "`a`, `b`, `m`, `t`, `w` are the canonical notation Jaro's paper uses for the two inputs, the match count, the transposition count, and the matching-window half-radius; renaming for clippy would obscure the direct correspondence with the published definition"
)]
#[allow(
    clippy::needless_range_loop,
    reason = "the index `i` is used to compute the matching-window bounds `[i-w, i+w+1)` in `b` and to index the `a_matched` bitmap; converting to `a.iter().enumerate()` would obscure that the loop drives the window offset, not just the read of `a[i]`"
)]
pub fn similarity(a: &[u8], b: &[u8]) -> f64 {
    let len_a = a.len();
    let len_b = b.len();

    // Boundary conventions — must match the generic Jaro kernel exactly.
    if len_a == 0 && len_b == 0 {
        return 1.0;
    }
    if len_a == 0 || len_b == 0 {
        return 0.0;
    }

    // Matching-window half-radius `w = max(|a|, |b|) / 2 - 1`. Saturating
    // subtraction handles the small-input case (max_len <= 1 collapses to
    // a window of zero, meaning "match at the same index only").
    let max_len = max(len_a, len_b);
    let window = (max_len / 2).saturating_sub(1);

    // Packed bit-vectors: one bit per position, tracking which positions
    // have already been consumed by a match. Packing (rather than a
    // Vec<bool>) is what lets an arch-specific backend AND the bitmap
    // against a comparison-mask word directly; on the scalar host it also
    // halves the memory traffic vs. a byte-per-position representation.
    let mut a_matched = Bitmap::new(len_a);
    let mut b_matched = Bitmap::new(len_b);

    let mut matches: usize = 0;
    for i in 0..len_a {
        // Clamp `start` to `len_b` so the window is empty (rather than
        // out-of-bounds) once `i` is far enough past the end of `b`
        // that no valid position could match. `Range::is_empty` on the
        // eventual `start..end` handles the far-past-end case cleanly
        // for the outer generic kernel, but slice indexing panics
        // whenever `start > slice.len()`, hence the explicit clamp.
        let start = i.saturating_sub(window).min(len_b);
        let end = min(len_b, i + window + 1);

        // The vectorizable step: broadcast a[i] and scan b[start..end]
        // for the first position that (a) equals the broadcast byte and
        // (b) is not yet marked in b_matched. `find_match_in_window`
        // wraps that scan; the four sibling arch backends implement the
        // same signature with a `cmpeq_epi8` / `vceqq_u8` / `u8x16_eq`
        // lane compare, an AND against the packed b_matched slice
        // (fetched through `common::Bitmap::read_bits`), and a
        // first-set-bit reduction, without changing the surrounding
        // bookkeeping.
        if let Some(j) = find_match_in_window(a[i], &b[start..end], &b_matched, start) {
            a_matched.set(i);
            b_matched.set(j);
            matches += 1;
        }
    }

    if matches == 0 {
        return 0.0;
    }

    // Transpositions: walk the matched positions in both sequences in
    // order and count disagreements; the transposition count is half the
    // disagreement count.
    let mut disagreements: usize = 0;
    let mut k: usize = 0;
    for i in 0..len_a {
        if !a_matched.get(i) {
            continue;
        }
        while !b_matched.get(k) {
            k += 1;
        }
        if a[i] != b[k] {
            disagreements += 1;
        }
        k += 1;
    }
    let transpositions = disagreements / 2;

    // Same cast-precision-loss allowance as the generic kernel: usize up
    // to 2^53 fits exactly in an f64 mantissa; Jaro is never applied to
    // inputs approaching that scale.
    #[allow(
        clippy::cast_precision_loss,
        reason = "inputs approaching 2^53 symbols exceed every practical Jaro use; the cast is exact for anything smaller"
    )]
    let m = matches as f64;
    #[allow(clippy::cast_precision_loss, reason = "see above")]
    let a_len_f = len_a as f64;
    #[allow(clippy::cast_precision_loss, reason = "see above")]
    let b_len_f = len_b as f64;
    #[allow(clippy::cast_precision_loss, reason = "see above")]
    let t = transpositions as f64;

    (m / a_len_f + m / b_len_f + (m - t) / m) / 3.0
}

/// Scan `window` for the first byte equal to `needle` whose absolute
/// position in `b` (given by `base + local_index`) is not yet marked in
/// `b_matched`.
///
/// Returns the absolute position in `b` on success, `None` if no match
/// remains in the window. Structuring the scan as a function on a
/// contiguous slice is what makes the SIMD lowering direct: the four
/// arch backends load a vector-register block of `window`, compare
/// against a broadcast `needle`, AND against the corresponding slice of
/// `b_matched` (via `common::Bitmap::read_bits`) plus a valid-
/// lanes mask on the trailing partial block, and reduce to the first
/// set bit — all without touching the outer loop.
#[inline]
fn find_match_in_window(
    needle: u8,
    window: &[u8],
    b_matched: &Bitmap,
    base: usize,
) -> Option<usize> {
    for (local, &candidate) in window.iter().enumerate() {
        let absolute = base + local;
        if b_matched.get(absolute) {
            continue;
        }
        if candidate == needle {
            return Some(absolute);
        }
    }
    None
}

/// Packed bit-vector, one bit per position, backed by a flat `Vec<u64>`.
///
/// Chosen over `Vec<bool>` because the arch-specific backends want to
/// AND a comparison mask against a contiguous 64-bit word of the
/// bitmap directly. On the scalar host the packed form also halves
/// memory traffic vs. a byte-per-position representation.
struct Bitmap {
    words: Vec<u64>,
}

impl Bitmap {
    #[inline]
    fn new(bits: usize) -> Self {
        // `bits.div_ceil(64)` is the number of 64-bit words needed to
        // hold `bits` positions; every word is zero on construction so
        // every position starts unmarked.
        let words = bits.div_ceil(64);
        Self {
            words: vec![0u64; words],
        }
    }

    #[inline]
    fn get(&self, i: usize) -> bool {
        let (word, bit) = (i >> 6, i & 63);
        (self.words[word] >> bit) & 1 == 1
    }

    #[inline]
    fn set(&mut self, i: usize) {
        let (word, bit) = (i >> 6, i & 63);
        self.words[word] |= 1u64 << bit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_empty_is_one() {
        assert_eq!(similarity(b"", b"").to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn one_empty_is_zero() {
        assert_eq!(similarity(b"", b"abc").to_bits(), 0.0_f64.to_bits());
        assert_eq!(similarity(b"abc", b"").to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn identical_is_one() {
        assert_eq!(
            similarity(b"kitten", b"kitten").to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn martha_marhta_matches_published_value() {
        let s = similarity(b"MARTHA", b"MARHTA");
        assert!((s - 17.0_f64 / 18.0_f64).abs() < 1e-12);
    }

    #[test]
    fn dixon_dicksonx_matches_published_value() {
        // Winkler's canonical Jaro example. Four matches (D, I, O, N —
        // X falls outside the matching window of half-radius
        // `max(5, 8) / 2 - 1 = 3`), zero transpositions.
        let s = similarity(b"DIXON", b"DICKSONX");
        let expected = (4.0_f64 / 5.0_f64 + 4.0_f64 / 8.0_f64 + (4.0_f64 - 0.0) / 4.0_f64) / 3.0;
        assert!((s - expected).abs() < 1e-12);
    }

    #[test]
    fn matches_generic_kernel_on_canonical_pairs() {
        use crate::jaro::jaro::jaro_similarity;
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"", b"z"),
            (b"a", b""),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"aaaaaaaa", b"aaaaaaaa"),
            (b"abcdefghij", b"jihgfedcba"),
            (
                b"the quick brown fox jumps over the lazy dog",
                b"the quick brown fox leaps over the lazy dog",
            ),
        ];
        for (a, b) in cases {
            let got = similarity(a, b);
            let want = jaro_similarity(a, b);
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "SIMD-shaped scalar disagreed with generic kernel on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn symmetric_on_canonical_pairs() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (
                b"the quick brown fox jumps over the lazy dog",
                b"the quick brown fox leaps over the lazy dog",
            ),
        ];
        for (a, b) in cases {
            assert_eq!(
                similarity(a, b).to_bits(),
                similarity(b, a).to_bits(),
                "similarity is not symmetric on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn boundary_length_8_matches_generic() {
        // Bottom of the amenability threshold.
        use crate::jaro::jaro::jaro_similarity;
        let a: alloc::vec::Vec<u8> = (0..8u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        assert_eq!(
            similarity(&a, &b).to_bits(),
            jaro_similarity(&a, &b).to_bits()
        );
    }

    #[test]
    fn boundary_length_64_matches_generic() {
        // Exactly one packed bitmap word.
        use crate::jaro::jaro::jaro_similarity;
        let a: alloc::vec::Vec<u8> = (0..64u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x80;
        b[63] ^= 0x80;
        assert_eq!(
            similarity(&a, &b).to_bits(),
            jaro_similarity(&a, &b).to_bits()
        );
    }

    #[test]
    fn boundary_length_65_crosses_word() {
        // One position past the single-word boundary — checks the
        // multi-word bitmap path.
        use crate::jaro::jaro::jaro_similarity;
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b[64] ^= 0x01;
        assert_eq!(
            similarity(&a, &b).to_bits(),
            jaro_similarity(&a, &b).to_bits()
        );
    }

    #[test]
    fn very_asymmetric_lengths_match_generic() {
        // Regression: a very short `b` against a much longer `a` used
        // to trigger an out-of-bounds slice index — the `start` cursor
        // could exceed `len_b` for `i` far past the last position in
        // `b`, and slice indexing panics whenever `start > slice.len()`.
        // The generic kernel avoids the slice by iterating the raw
        // `start..end` range directly; this kernel now clamps `start`
        // to `len_b` before slicing.
        use crate::jaro::jaro::jaro_similarity;
        let a = alloc::vec![0u8; 83];
        let b = alloc::vec![0u8; 1];
        let expected = jaro_similarity(&a, &b);
        let observed = similarity(&a, &b);
        assert_eq!(observed.to_bits(), expected.to_bits());
    }

    #[test]
    fn boundary_length_128_matches_generic() {
        use crate::jaro::jaro::jaro_similarity;
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[63] ^= 0x02;
        b[64] ^= 0x04;
        b[127] ^= 0x08;
        assert_eq!(
            similarity(&a, &b).to_bits(),
            jaro_similarity(&a, &b).to_bits()
        );
    }
}
