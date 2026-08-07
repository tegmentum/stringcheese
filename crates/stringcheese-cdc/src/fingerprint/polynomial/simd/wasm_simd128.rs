//! wasm SIMD128-gated polynomial-hash slice-batch backend for `wasm32`.
//!
//! Compiled only on `wasm32` targets and only when the `simd128`
//! target-feature is enabled at compile time. Unlike `x86_64` and
//! `aarch64`, wasm has no runtime CPU-feature detection: whether the
//! SIMD opcodes are legal is a property of the wasm engine executing
//! the module. Callers control the choice via
//! `RUSTFLAGS=-C target-feature=+simd128` at build time, and the
//! dispatcher in [`super`] compiles this path in or out with a
//! matching `#[cfg(target_feature = "simd128")]` gate.
//!
//! # Kernel shape — 16-byte block folding via `i64x2_mul`
//!
//! Same block-form derivation as the sibling backends — see [the
//! module docs][super] for the full treatment. wasm SIMD128 exposes
//! `i64x2_mul` for a 2-lane 64×64 → 64 (low-half) multiply; when both
//! inputs are bounded to 32 bits the result of `i64x2_mul` is exactly
//! the 32×32 → 64 widening product this kernel needs. Both the byte
//! lane (≤ 8 bits, widened to u64) and the coefficient lane (≤ 32
//! bits by construction — see the scalar reference's `COEFF_HI` /
//! `COEFF_LO` block-form constants) satisfy that bound, so the
//! reduction is byte-identical to the AVX2 and NEON siblings.
//!
//! # Implementation
//!
//! Each block issues eight `i64x2_mul` + `i64x2_add` pairs — one per
//! 2-byte SIMD chunk — for each of `hi_acc` and `lo_acc`. Bytes are
//! packed into a `v128` via `u64x2(byte0 as u64, byte1 as u64)`, and
//! coefficients load straight from the static `COEFF_HI[chunk*2..]` /
//! `COEFF_LO[chunk*2..]` `u64` slices with `v128_load`. Horizontal
//! reduction extracts the two lanes and sums scalar-side; the block
//! sum reassembles as `(hi_sum << 32) + lo_sum` in u128 for the
//! Mersenne reduction.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` for parity with the sibling
//! SSE2/AVX2/NEON backends' `#[target_feature]`-gated signature, even
//! though on wasm the target feature is a compile-time property
//! rather than a runtime precondition. On `wasm32` with
//! `target_feature = "simd128"` this function is unconditionally safe
//! to call.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::wasm32::{
    i64x2_add, i64x2_mul, u64x2, u64x2_extract_lane, u64x2_splat, v128, v128_load,
};

use super::scalar;
use crate::fingerprint::polynomial::BASE;

/// wasm SIMD128-gated polynomial-hash digest of a byte slice.
///
/// # Safety
///
/// See the module-level safety note — on `wasm32 + simd128` this is
/// unconditionally safe.
#[target_feature(enable = "simd128")]
#[must_use]
#[allow(
    clippy::cast_ptr_alignment,
    reason = "`v128_load` accepts any-alignment pointers by contract — the `cast::<v128>()` reinterpretation is only a type change, not an alignment claim"
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

    // SAFETY: this function is `#[target_feature(enable = "simd128")]`,
    // upheld here by the compile-time cfg gate. Each SIMD chunk reads
    // 2 bytes from `effective` at offset `block_start + chunk*2 + 2 <=
    // full_blocks * BLOCK_LEN <= len`, and 16 bytes (2 x u64) from the
    // static coefficient tables at offset `chunk * 2 + 2 <= BLOCK_LEN
    // = 16`. `v128_load` requires a valid pointer to at least 16
    // bytes; both loads satisfy that.
    unsafe {
        let hi_ptr = scalar::COEFF_HI.as_ptr();
        let lo_ptr = scalar::COEFF_LO.as_ptr();

        for b in 0..full_blocks {
            let block_start = b * scalar::BLOCK_LEN;

            let mut hi_acc: v128 = u64x2_splat(0);
            let mut lo_acc: v128 = u64x2_splat(0);

            // 8 SIMD chunks × 2 lanes = 16 bytes per block.
            for chunk in 0..(scalar::BLOCK_LEN / 2) {
                let off = block_start + chunk * 2;
                // Pack 2 bytes into a v128 (each byte in its own u64
                // lane's low 32 bits). `u64x2(a, b)` places `a` in
                // lane 0 and `b` in lane 1.
                let b_v: v128 = u64x2(u64::from(effective[off]), u64::from(effective[off + 1]));

                let coeff_hi_v: v128 = v128_load(hi_ptr.add(chunk * 2).cast::<v128>());
                let coeff_lo_v: v128 = v128_load(lo_ptr.add(chunk * 2).cast::<v128>());

                // 2-lane 64×64 → 64 (low-half) multiplies. All inputs
                // fit in 32 bits by construction (bytes ≤ 8 bits,
                // coefficients ≤ 32 bits), so the low-64 result is
                // exactly the 32×32 → 64 widening product the AVX2 and
                // NEON siblings compute with dedicated widening ops.
                let hi_prod = i64x2_mul(b_v, coeff_hi_v);
                let lo_prod = i64x2_mul(b_v, coeff_lo_v);

                hi_acc = i64x2_add(hi_acc, hi_prod);
                lo_acc = i64x2_add(lo_acc, lo_prod);
            }

            let hi_sum =
                u64x2_extract_lane::<0>(hi_acc).wrapping_add(u64x2_extract_lane::<1>(hi_acc));
            let lo_sum =
                u64x2_extract_lane::<0>(lo_acc).wrapping_add(u64x2_extract_lane::<1>(lo_acc));

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
        for &window in &[1usize, 8, 32, 64, 100] {
            let cases: &[&[u8]] = &[
                b"",
                b"a",
                b"the quick brown fox jumps over the lazy dog",
                &[0u8; 128],
                &[0xFFu8; 200],
            ];
            for &input in cases {
                // SAFETY: this file is only compiled under `wasm32 +
                // simd128`, so the target-feature precondition holds
                // by build-time cfg.
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
                // SAFETY: `wasm32 + simd128` cfg upholds the
                // target-feature precondition.
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
        for &size in &[0usize, 1, 15, 16, 17, 63, 64, 65, 128, 1024] {
            let input: alloc::vec::Vec<u8> =
                (0..size).map(|i| (i as u8).wrapping_mul(17)).collect();
            // SAFETY: `wasm32 + simd128` cfg upholds the
            // target-feature precondition.
            let simd = unsafe { digest_of_slice(0, &input) };
            assert_eq!(simd, reference(0, &input), "window=0 size={size}");
        }
    }
}
