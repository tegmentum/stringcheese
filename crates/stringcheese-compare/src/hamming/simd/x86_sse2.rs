//! SSE2-gated Hamming kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. SSE2 is part of the
//! `x86_64` baseline ABI, so the dispatcher's `is_x86_feature_detected!`
//! check is defensive rather than gating.
//!
//! # Algorithm
//!
//! Same shape as the AVX2 sibling with a 16-byte block width:
//!
//! * Load a 16-byte block from each side with `_mm_loadu_si128`.
//! * Compare with `_mm_cmpeq_epi8` (0xff where equal, 0x00 where different).
//! * Reduce to a 16-bit mask with `_mm_movemask_epi8`.
//! * Block mismatch count = `BLOCK - matches.count_ones()`.
//! * Tail (fewer than 16 bytes) runs as a scalar byte loop.
//!
//! # Safety
//!
//! See the AVX2 sibling — the same `#[target_feature]` precondition
//! applies here, gated by the dispatcher.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_possible_wrap,
    reason = "the byte reinterpretation is a bit-transmute, not a numeric conversion"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "every pointer cast in this module feeds an *unaligned* SSE2 load (`_mm_loadu_si128`), which by contract accepts any-alignment `*const __m128i`; the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]

use core::arch::x86_64::{__m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8};

/// SSE2 block width in bytes — one `__m128i` per iteration of the inner
/// loop.
const BLOCK: usize = 16;

/// SSE2 block width as `u32` — used inside the hot loop for the
/// `BLOCK - matches` mismatch count.
const BLOCK_U32: u32 = 16;

/// SSE2-gated Hamming distance for equal-length byte slices.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` this is
/// guaranteed by the ABI; the dispatcher still checks for uniformity
/// with the other arch branches.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::x86_sse2::distance requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: `off + BLOCK <= len` guarantees both 16-byte reads
        // stay inside their respective slices; SSE2 is enabled by this
        // function's `#[target_feature]`; the pointer casts feed unaligned
        // loads, which accept any alignment. The loadu intrinsics are
        // the only unsafe ones here — cmpeq and movemask are safe under
        // an SSE2 target-feature context on Rust 1.87+.
        let va = unsafe { _mm_loadu_si128(a.as_ptr().add(off).cast::<__m128i>()) };
        let vb = unsafe { _mm_loadu_si128(b.as_ptr().add(off).cast::<__m128i>()) };
        let eq = _mm_cmpeq_epi8(va, vb);
        // `_mm_movemask_epi8` returns an `i32` whose low 16 bits are the
        // per-lane MSB of `eq` (1 iff the lane compared equal). The
        // upper 16 bits are zero — `cast_unsigned` reinterprets and
        // `count_ones` runs on the u32 form.
        let match_mask = _mm_movemask_epi8(eq).cast_unsigned();
        let matches = match_mask.count_ones();
        mismatches = mismatches.saturating_add(BLOCK_U32 - matches);
        off += BLOCK;
    }
    while off < len {
        if a[off] != b[off] {
            mismatches = mismatches.saturating_add(1);
        }
        off += 1;
    }
    mismatches
}

/// SSE2-gated Hamming distance with an early-termination cutoff.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// The caller must ensure SSE2 is available.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn distance_within(a: &[u8], b: &[u8], cutoff: u32) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::x86_sse2::distance_within requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: see `distance`.
        let va = unsafe { _mm_loadu_si128(a.as_ptr().add(off).cast::<__m128i>()) };
        let vb = unsafe { _mm_loadu_si128(b.as_ptr().add(off).cast::<__m128i>()) };
        let eq = _mm_cmpeq_epi8(va, vb);
        let match_mask = _mm_movemask_epi8(eq).cast_unsigned();
        let matches = match_mask.count_ones();
        mismatches = mismatches.saturating_add(BLOCK_U32 - matches);
        if mismatches > cutoff {
            return mismatches;
        }
        off += BLOCK;
    }
    while off < len {
        if a[off] != b[off] {
            mismatches = mismatches.saturating_add(1);
            if mismatches > cutoff {
                return mismatches;
            }
        }
        off += 1;
    }
    mismatches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hamming::simd::scalar;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b"a"),
            (b"karolin", b"kathrin"),
            (b"1011101", b"1001001"),
            (b"abc", b"xyz"),
        ];
        for (a, b) in cases {
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { distance(a, b) };
            let scalar_ref = scalar::distance(a, b);
            assert_eq!(
                simd, scalar_ref,
                "sse2 disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn differential_across_lengths() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for len in 0..=200usize {
            let a: alloc::vec::Vec<u8> = (0..len)
                .map(|i| u8::try_from(i & 0xff).unwrap().wrapping_mul(31))
                .collect();
            let mut b = a.clone();
            for &pos in &[0usize, 7, 15, 16, 31, 32, 63, 64, 100, 127] {
                if pos < len {
                    b[pos] ^= 0x5A;
                }
            }
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { distance(&a, &b) };
            let scalar_ref = scalar::distance(&a, &b);
            assert_eq!(simd, scalar_ref, "at len={len}");
        }
    }

    #[test]
    fn distance_within_matches_scalar_below_cutoff() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..100u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[50] ^= 0x02;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { distance_within(&a, &b, 10) };
        assert_eq!(simd, 2);
    }

    #[test]
    fn distance_within_reports_exceeded_above_cutoff() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a = alloc::vec![0u8; 100];
        let b = alloc::vec![0xffu8; 100];
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { distance_within(&a, &b, 5) };
        assert!(simd > 5);
    }
}
