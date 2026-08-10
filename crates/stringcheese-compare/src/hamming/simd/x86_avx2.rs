//! AVX2-gated Hamming kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. AVX2 doubles the byte
//! throughput of the SSE2 sibling by walking 32-byte blocks per iteration
//! instead of 16.
//!
//! # Algorithm
//!
//! Hamming distance over byte slices is the count of positions at which
//! the two slices differ. The kernel is a direct block-wise byte compare:
//!
//! * Load a 32-byte block from each side with `_mm256_loadu_si256`.
//! * Compare with `_mm256_cmpeq_epi8` to get a per-lane 0xff/0x00 result
//!   (0xff where the two bytes match, 0x00 where they differ).
//! * Reduce to a 32-bit mask with `_mm256_movemask_epi8` (bit i = 1 iff
//!   lane i matched), then compute the mismatch count for the block as
//!   `BLOCK - matches.count_ones()`. No shuffle-based popcount LUT is
//!   needed — the movemask + `count_ones` idiom is one instruction per
//!   block plus a `popcnt`, which is faster than a byte-shuffle LUT on
//!   every AVX2-capable CPU.
//! * Handle the tail (fewer than 32 bytes) with a scalar byte loop.
//!
//! # Safety
//!
//! [`distance`] and [`distance_within`] are `unsafe fn` because
//! `#[target_feature(enable = ...)]` functions have a documented
//! precondition — the enabled ISA feature must be present at run time.
//! The dispatcher in [`crate::hamming::simd`] gates every call on
//! `is_x86_feature_detected!("avx2")`, so the precondition is met by
//! construction; call sites outside the dispatcher must uphold the same
//! contract.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_possible_wrap,
    reason = "`_mm256_set1_epi8` and byte-broadcast helpers take `i8`; the byte reinterpretation is a bit-transmute, not a numeric conversion"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "every pointer cast in this module feeds an *unaligned* AVX2 load (`_mm256_loadu_si256`), which by contract accepts any-alignment `*const __m256i`; the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]

use core::arch::x86_64::{__m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8};

/// AVX2 block width in bytes — one `__m256i` per iteration of the inner
/// loop.
const BLOCK: usize = 32;

/// AVX2 block width as `u32` — used inside the hot loop for the
/// `BLOCK - matches` mismatch count, where both operands are `u32`.
const BLOCK_U32: u32 = 32;

/// AVX2-gated Hamming distance for equal-length byte slices.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// The caller must ensure AVX2 is available on the running CPU. The
/// dispatcher in the parent [`super`] module guarantees this via
/// `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::x86_avx2::distance requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: `off + BLOCK <= len` guarantees both 32-byte reads
        // stay inside their respective slices; AVX2 is enabled by this
        // function's `#[target_feature]`; the pointer casts feed unaligned
        // loads, which accept any alignment. The loadu intrinsics are
        // the only unsafe ones here — cmpeq and movemask are safe under
        // an AVX2 target-feature context on Rust 1.87+.
        let va = unsafe { _mm256_loadu_si256(a.as_ptr().add(off).cast::<__m256i>()) };
        let vb = unsafe { _mm256_loadu_si256(b.as_ptr().add(off).cast::<__m256i>()) };
        let eq = _mm256_cmpeq_epi8(va, vb);
        let match_mask = _mm256_movemask_epi8(eq).cast_unsigned();
        let matches = match_mask.count_ones();
        // Each block contributes `BLOCK - matches` mismatches. `BLOCK` is
        // 32, `matches` in 0..=32; the subtraction cannot underflow.
        mismatches = mismatches.saturating_add(BLOCK_U32 - matches);
        off += BLOCK;
    }
    // Tail: fewer than BLOCK bytes left. A masked SIMD load would work
    // too, but a scalar loop over at most 31 bytes is negligible and
    // keeps the branch cheap.
    while off < len {
        if a[off] != b[off] {
            mismatches = mismatches.saturating_add(1);
        }
        off += 1;
    }
    mismatches
}

/// AVX2-gated Hamming distance with an early-termination cutoff. Returns
/// the exact mismatch count when it is at most `cutoff`, or a value
/// strictly greater than `cutoff` (a sentinel meaning "exceeded") when the
/// true count is above.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// The caller must ensure AVX2 is available on the running CPU.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn distance_within(a: &[u8], b: &[u8], cutoff: u32) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::x86_avx2::distance_within requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: see `distance`.
        let va = unsafe { _mm256_loadu_si256(a.as_ptr().add(off).cast::<__m256i>()) };
        let vb = unsafe { _mm256_loadu_si256(b.as_ptr().add(off).cast::<__m256i>()) };
        let eq = _mm256_cmpeq_epi8(va, vb);
        let match_mask = _mm256_movemask_epi8(eq).cast_unsigned();
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
        if !is_x86_feature_detected!("avx2") {
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
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { distance(a, b) };
            let scalar_ref = scalar::distance(a, b);
            assert_eq!(
                simd, scalar_ref,
                "avx2 disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }

    /// Sweep every length across the block boundaries: 31/32/33 (single
    /// block edge), 63/64/65 (two-block boundary), and 127/128/129.
    #[test]
    fn differential_across_lengths() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for len in 0..=200usize {
            let a: alloc::vec::Vec<u8> = (0..len)
                .map(|i| u8::try_from(i & 0xff).unwrap().wrapping_mul(31))
                .collect();
            let mut b = a.clone();
            // Flip a few bytes to force mismatches at various positions.
            for &pos in &[0usize, 15, 16, 31, 32, 63, 64, 100, 127, 128] {
                if pos < len {
                    b[pos] ^= 0x5A;
                }
            }
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { distance(&a, &b) };
            let scalar_ref = scalar::distance(&a, &b);
            assert_eq!(simd, scalar_ref, "at len={len}");
        }
    }

    #[test]
    fn distance_within_matches_scalar_below_cutoff() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..100u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[50] ^= 0x02;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { distance_within(&a, &b, 10) };
        assert_eq!(simd, 2);
    }

    #[test]
    fn distance_within_reports_exceeded_above_cutoff() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        // All 100 bytes differ.
        let a = alloc::vec![0u8; 100];
        let b = alloc::vec![0xffu8; 100];
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { distance_within(&a, &b, 5) };
        assert!(simd > 5);
    }
}
