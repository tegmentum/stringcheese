//! NEON-gated polynomial-hash slice-batch backend for `aarch64`.
//!
//! Compiled only on `aarch64`. Ships a real vectorized kernel built on
//! `vmull_u32` — the NEON widening 32×32 → 64 multiply — over the
//! block-form reformulation documented in [the module docs][super] and
//! the [scalar reference][super::scalar].
//!
//! # Kernel shape — 16-byte block folding via `vmull_u32`
//!
//! Same derivation as the AVX2 sibling: for each 16-byte block the
//! kernel evaluates
//!
//! ```text
//! BLOCK_SUM = Σ_{i=0..16}  bytes[i] * pk[15-i]  (integer sum)
//! state    <- state * PK_BLOCK + BLOCK_SUM  (mod PRIME)
//! ```
//!
//! with each coefficient `pk = BASE^(15-i) mod PRIME` split into
//! high/low 32-bit halves and accumulated into `hi_acc` and `lo_acc`
//! independently. NEON offers a native widening 32×32 → 64 multiply
//! (`vmull_u32`) that consumes a `uint32x2_t` pair and returns a
//! `uint64x2_t`, so the byte and coefficient inputs stay in 32-bit
//! lanes end-to-end. The Horner-style accumulation uses `vaddq_u64`;
//! the horizontal 2→1 reduce at end-of-block extracts each lane
//! scalar-side and sums.
//!
//! # Implementation
//!
//! Each block issues eight `vmull_u32` + `vaddq_u64` pairs — one per
//! 2-byte SIMD chunk — for each of `hi_acc` and `lo_acc`. Bytes are
//! widened to `uint32x2_t` scalar-side (`vcreate_u32` from a packed
//! u64 assembly of the two u32-widened bytes) and coefficients are
//! loaded from the static `COEFF_HI[chunk*2..chunk*2+2]` /
//! `COEFF_LO[chunk*2..chunk*2+2]` `u64` slices, then narrowed with
//! `vmovn_u64` — every stored `u64` in those tables has the high 32
//! bits zero by construction, so the narrowing is lossless.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because
//! `#[target_feature(enable = ...)]` functions have a documented
//! precondition — the enabled ISA feature must be present at run
//! time. On `aarch64` NEON is guaranteed by the ABI; the dispatcher
//! still checks `is_aarch64_feature_detected!("neon")` for
//! consistency with the x86 branches.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::aarch64::{
    uint64x2_t, vaddq_u64, vcreate_u32, vdupq_n_u64, vgetq_lane_u64, vld1q_u64, vmovn_u64,
    vmull_u32,
};

use super::scalar;
use crate::fingerprint::polynomial::BASE;

/// NEON-gated polynomial-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` NEON is
/// guaranteed by the ABI; the dispatcher still checks
/// `is_aarch64_feature_detected!("neon")` for uniformity with the x86
/// branches.
#[target_feature(enable = "neon")]
#[must_use]
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

    // SAFETY: this function is `#[target_feature(enable = "neon")]`, so
    // every NEON intrinsic invoked below has its ISA precondition
    // upheld by the enclosing call context. Each SIMD chunk reads 2
    // bytes from `effective` at offset `block_start + chunk*2 + 2 <=
    // full_blocks * BLOCK_LEN <= len`, and 16 bytes (2 x u64) from the
    // static coefficient tables at offset `chunk * 2 + 2 <= BLOCK_LEN =
    // 16`.
    unsafe {
        let hi_ptr = scalar::COEFF_HI.as_ptr();
        let lo_ptr = scalar::COEFF_LO.as_ptr();

        for b in 0..full_blocks {
            let block_start = b * scalar::BLOCK_LEN;

            let mut hi_acc: uint64x2_t = vdupq_n_u64(0);
            let mut lo_acc: uint64x2_t = vdupq_n_u64(0);

            // 8 SIMD chunks × 2 lanes = 16 bytes per block.
            for chunk in 0..(scalar::BLOCK_LEN / 2) {
                let off = block_start + chunk * 2;
                // Pack 2 bytes into a `uint32x2_t` (each byte in its
                // own u32 lane). `vcreate_u32` takes a `u64`
                // interpreted as the little-endian bit pattern of the
                // two u32 lanes: lane 0 = low 32 bits, lane 1 = high
                // 32 bits.
                let byte_lo = u64::from(effective[off]);
                let byte_hi = u64::from(effective[off + 1]);
                let b_v = vcreate_u32(byte_lo | (byte_hi << 32));

                // Load the 2-u64 slice of coefficients (each with the
                // high 32 bits zero by construction) and narrow to
                // `uint32x2_t` — the narrowing is lossless per
                // [`super::scalar::COEFF_HI`]'s invariant.
                let coeff_hi_wide = vld1q_u64(hi_ptr.add(chunk * 2));
                let coeff_lo_wide = vld1q_u64(lo_ptr.add(chunk * 2));
                let coeff_hi_v = vmovn_u64(coeff_hi_wide);
                let coeff_lo_v = vmovn_u64(coeff_lo_wide);

                // Widening 2-lane 32×32 → 64 multiplies. Result bounds
                // match the AVX2 sibling: each `hi_prod` lane ≤ 2^37,
                // each `lo_prod` lane ≤ 2^40. Sum of 8 chunks stays
                // under ~2^40 (hi) / ~2^43 (lo) per lane — well within
                // the 64-bit lane bound.
                let hi_prod = vmull_u32(b_v, coeff_hi_v);
                let lo_prod = vmull_u32(b_v, coeff_lo_v);

                hi_acc = vaddq_u64(hi_acc, hi_prod);
                lo_acc = vaddq_u64(lo_acc, lo_prod);
            }

            let hi_sum = vgetq_lane_u64::<0>(hi_acc).wrapping_add(vgetq_lane_u64::<1>(hi_acc));
            let lo_sum = vgetq_lane_u64::<0>(lo_acc).wrapping_add(vgetq_lane_u64::<1>(lo_acc));

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
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
                // SAFETY: is_aarch64_feature_detected!("neon") returned true.
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
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
                // SAFETY: is_aarch64_feature_detected!("neon") returned true.
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
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for &size in &[0usize, 1, 15, 16, 17, 63, 64, 65, 128, 1024] {
            let input: alloc::vec::Vec<u8> =
                (0..size).map(|i| (i as u8).wrapping_mul(17)).collect();
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { digest_of_slice(0, &input) };
            assert_eq!(simd, reference(0, &input), "window=0 size={size}");
        }
    }
}
