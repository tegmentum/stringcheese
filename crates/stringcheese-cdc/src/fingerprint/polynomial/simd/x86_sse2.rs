//! SSE2-gated polynomial-hash slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. Ships a real vectorized kernel built on
//! 2-lane `_mm_mul_epu32` (`PMULUDQ`) multiply-accumulate — SSE2 has
//! surfaced the 32×32 → 64 lane multiply since day one, so no runtime
//! sub-feature detection is required beyond the SSE2 baseline itself.
//!
//! # Kernel shape
//!
//! Identical block-form reformulation as the AVX2 sibling — see [the
//! module docs][super] for the full derivation. This backend simply
//! halves the SIMD lane width: two 32×32 → 64 multiplies per SIMD
//! instruction instead of four, so `BLOCK_LEN = 16` bytes yield eight
//! SIMD chunks instead of four. The scalar tail, effective-slice
//! truncation, and per-block Mersenne reduction are all shared with
//! the AVX2 sibling backend.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because
//! `#[target_feature(enable = ...)]` functions have a documented
//! precondition — the enabled ISA feature must be present at run
//! time. On `x86_64` SSE2 is guaranteed by the ABI; the dispatcher
//! still checks `is_x86_feature_detected!("sse2")` for consistency
//! with the other arch branches.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::x86_64::{
    __m128i, _mm_add_epi64, _mm_cvtsi128_si64, _mm_loadu_si128, _mm_mul_epu32, _mm_setzero_si128,
    _mm_unpackhi_epi64,
};

use super::scalar;
use crate::fingerprint::polynomial::BASE;

/// SSE2-gated polynomial-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` SSE2 is
/// guaranteed by the ABI; the dispatcher still checks
/// `is_x86_feature_detected!("sse2")` for consistency with the other
/// arch branches.
#[target_feature(enable = "sse2")]
#[must_use]
#[allow(
    clippy::cast_ptr_alignment,
    reason = "`_mm_loadu_si128` accepts any-alignment pointers by contract — the `cast::<__m128i>()` reinterpretation is only a type change, not an alignment claim"
)]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    let effective_start = if window == 0 {
        0
    } else {
        bytes.len().saturating_sub(window)
    };
    let effective = &bytes[effective_start..];
    let len = effective.len();

    if len < scalar::BLOCK_LEN {
        return scalar::scalar_from_zero(effective);
    }

    let full_blocks = len / scalar::BLOCK_LEN;
    let mut state: u64 = 0;

    // SAFETY: this function is `#[target_feature(enable = "sse2")]`, so
    // every SSE2 intrinsic invoked below has its ISA precondition
    // upheld by the enclosing call context. Each SIMD chunk reads 2
    // bytes from `effective` at offset `block_start + chunk_off + 2 <=
    // full_blocks * BLOCK_LEN <= len`, and 16 bytes (2 x u64) from the
    // static coefficient tables at offset `chunk * 2 + 2 <= BLOCK_LEN =
    // 16`. All reads stay within their buffers.
    unsafe {
        let hi_ptr = scalar::COEFF_HI.as_ptr().cast::<__m128i>();
        let lo_ptr = scalar::COEFF_LO.as_ptr().cast::<__m128i>();

        for b in 0..full_blocks {
            let block_start = b * scalar::BLOCK_LEN;

            let mut hi_acc = _mm_setzero_si128();
            let mut lo_acc = _mm_setzero_si128();

            // 8 SIMD chunks × 2 lanes = 16 bytes per block.
            for chunk in 0..(scalar::BLOCK_LEN / 2) {
                let off = block_start + chunk * 2;
                // Widen 2 bytes into 2 u64 lanes (each byte in the low
                // 32 bits of its lane).
                let b_wide: [u64; 2] = [u64::from(effective[off]), u64::from(effective[off + 1])];
                let b_v = _mm_loadu_si128(b_wide.as_ptr().cast::<__m128i>());

                let coeff_hi_v = _mm_loadu_si128(hi_ptr.add(chunk));
                let coeff_lo_v = _mm_loadu_si128(lo_ptr.add(chunk));

                // 2-lane 32×32 → 64 unsigned multiplies. Same
                // arithmetic bound as the AVX2 sibling, halved in
                // parallelism.
                let hi_prod = _mm_mul_epu32(b_v, coeff_hi_v);
                let lo_prod = _mm_mul_epu32(b_v, coeff_lo_v);

                hi_acc = _mm_add_epi64(hi_acc, hi_prod);
                lo_acc = _mm_add_epi64(lo_acc, lo_prod);
            }

            let hi_sum = horizontal_sum_u64x2(hi_acc);
            let lo_sum = horizontal_sum_u64x2(lo_acc);

            let block_sum_u128 = (u128::from(hi_sum) << 32) + u128::from(lo_sum);
            let block_sum = scalar::reduce_mod(block_sum_u128);

            let state_scaled = scalar::mul_mod(state, scalar::PK_BLOCK);
            state = scalar::add_mod(state_scaled, block_sum);
        }
    }

    // Scalar tail: length in `[0, BLOCK_LEN)` by construction.
    let tail_start = full_blocks * scalar::BLOCK_LEN;
    for &b in &effective[tail_start..] {
        state = scalar::add_mod(scalar::mul_mod(state, BASE), u64::from(b));
    }

    state
}

/// Horizontal sum of the two `u64` lanes of an `__m128i`.
///
/// # Safety
///
/// SSE2 must be available at run time. Enforced by the enclosing call
/// context via `#[target_feature]`.
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn horizontal_sum_u64x2(v: __m128i) -> u64 {
    // All intrinsics below are `safe fn` in `core::arch::x86_64` — pure
    // register-to-register shuffles and adds with no memory access and
    // no ISA precondition beyond the `#[target_feature]` on this fn.
    // No inner `unsafe` block is required; `unsafe_op_in_unsafe_fn`
    // would flag it if one were.
    let hi = _mm_unpackhi_epi64(v, v);
    let sum = _mm_add_epi64(v, hi);
    // `_mm_cvtsi128_si64` returns i64; the polynomial-hash accumulator
    // is a bit pattern where signed/unsigned is a reinterpretation,
    // not a value change.
    #[allow(
        clippy::cast_sign_loss,
        reason = "`_mm_cvtsi128_si64` returns i64 by intrinsic signature; the value is a u64 bit pattern"
    )]
    let out = _mm_cvtsi128_si64(sum) as u64;
    out
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test inputs use `(i as u8)` and `>> shift as u8` patterns to derive deterministic pseudo-random bytes from small counters; `size` is bounded well below `u32::MAX`, so truncation cannot occur"
)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::polynomial::PolynomialHash;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = PolynomialHash::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for &window in &[1usize, 8, 32, 64, 100] {
            let cases: &[&[u8]] = &[
                b"",
                b"a",
                b"the quick brown fox jumps over the lazy dog",
                &[0u8; 128],
                &[0xFFu8; 200],
            ];
            for &input in cases {
                // SAFETY: is_x86_feature_detected!("sse2") returned true.
                let simd = unsafe { digest_of_slice(window, input) };
                assert_eq!(
                    simd,
                    reference(window, input),
                    "on {input:?} window {window}"
                );
            }
        }
    }

    #[test]
    fn matches_scalar_reference_at_block_boundaries() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for &window in &[8usize, 64, 128, 512] {
            for &size in &[
                1usize, 2, 3, 7, 8, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129,
            ] {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "deterministic pseudo-random byte via low-bits truncation of a mixed u32"
                )]
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(1) >> 16) as u8)
                    .collect();
                // SAFETY: is_x86_feature_detected!("sse2") returned true.
                let simd = unsafe { digest_of_slice(window, &input) };
                assert_eq!(
                    simd,
                    reference(window, &input),
                    "at boundary size={size} window={window}"
                );
            }
        }
    }

    #[test]
    fn matches_scalar_reference_across_window_zero() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for &size in &[0usize, 1, 15, 16, 17, 63, 64, 65, 128, 1024] {
            let input: alloc::vec::Vec<u8> =
                (0..size).map(|i| (i as u8).wrapping_mul(17)).collect();
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { digest_of_slice(0, &input) };
            assert_eq!(simd, reference(0, &input), "window=0 size={size}");
        }
    }
}
