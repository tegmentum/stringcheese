//! AVX2-gated polynomial-hash slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. Ships a real vectorized kernel built on
//! 4-lane `_mm256_mul_epu32` (`VPMULUDQ`) multiply-accumulate over the
//! block-form reformulation documented in the [module docs][super]
//! and the [scalar reference][super::scalar]. Falls back to the
//! [portable scalar core][super::scalar::digest_of_slice] for inputs
//! shorter than one block.
//!
//! # Kernel shape — 16-byte block folding via `VPMULUDQ`
//!
//! The polynomial hash's per-byte recurrence
//!
//! ```text
//! state = (state * BASE + byte - leaving * BASE^window) mod PRIME
//! ```
//!
//! collapses over any `k`-byte run from `state_0` into the closed form
//!
//! ```text
//! state_k = state_0 * BASE^k
//!         + Σ_{i=0..k}  bytes[i] * BASE^(k-1-i)  (mod PRIME)
//! ```
//!
//! and the streaming digest's window-eviction identity guarantees that
//! for `bytes.len() > window` the same digest is reproduced by feeding
//! just the last `window` bytes into a fresh `state = 0`. See the
//! [sibling Rabin backend][crate::fingerprint::rabin::simd] for the
//! effective-slice truncation derivation shared with this kernel.
//!
//! Once the effective slice is chosen, the kernel folds it in
//! `BLOCK_LEN = 16`-byte blocks:
//!
//! ```text
//! state_after = state_before * BASE^16 + BLOCK_SUM  (mod PRIME)
//! BLOCK_SUM   = Σ_{i=0..16}  bytes[i] * BASE^(15-i)
//! ```
//!
//! Each per-byte coefficient `pk = BASE^(15-i) mod PRIME` fits in 61
//! bits — too wide for a straight 32-bit lane multiply — so the
//! [scalar reference][super::scalar] precomputes it split into
//! `pk_hi = pk >> 32` (≤ 29 bits) and `pk_lo = pk & 0xFFFF_FFFF` (32
//! bits). Then `byte * pk = (byte * pk_hi) * 2^32 + byte * pk_lo`,
//! with each half-product fitting comfortably in a 64-bit lane:
//! `byte * pk_hi ≤ 255 * (2^29 - 1) < 2^37`, and `byte * pk_lo ≤ 255 *
//! (2^32 - 1) < 2^40`. Accumulating 16 such half-products per block
//! stays under `~2^41` and `~2^44` respectively — well within u64.
//!
//! # Implementation
//!
//! The kernel processes one 16-byte block per iteration. For each
//! block:
//!
//! * Bytes 0..4 are widened to a 4-lane `__m256i` (each byte in the
//!   low 32 bits of its u64 lane) and multiplied by the corresponding
//!   `COEFF_HI[0..4]` and `COEFF_LO[0..4]` slices via
//!   `_mm256_mul_epu32`. `hi_acc` and `lo_acc` are advanced by
//!   `_mm256_add_epi64`. The same pattern repeats for bytes 4..8,
//!   8..12, and 12..16.
//! * A horizontal 4→1 add on each of `hi_acc` and `lo_acc` yields
//!   scalar `hi_sum` and `lo_sum` u64s.
//! * The block sum reassembles as `(hi_sum << 32) + lo_sum` in u128
//!   and reduces once through the Mersenne trick.
//! * The running state advances as `state = state * PK_BLOCK +
//!   block_sum` mod `PRIME`.
//!
//! Any `effective_len % BLOCK_LEN` tail after the last full block is
//! consumed by the portable scalar recurrence — same reduction, same
//! byte-identical contract with the streaming reference.
//!
//! # Why not AVX-512 IFMA?
//!
//! `_mm256_madd52lo_epu64` / `_mm256_madd52hi_epu64` would collapse
//! the split-half-product accumulation into a single fused 52-bit
//! multiply-add per lane, with a natural fit for the 61-bit Mersenne
//! coefficient. That intrinsic family requires the AVX-512 IFMA
//! (`avx512ifma`) feature, which needs runtime detection and a
//! separate widened kernel; deferred until the workspace picks up the
//! rest of the AVX-512 dispatch surface.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because
//! `#[target_feature(enable = ...)]` functions have a documented
//! precondition — the enabled ISA feature must be present at run
//! time. The dispatcher checks `is_x86_feature_detected!("avx2")`
//! before every call.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::x86_64::{
    __m128i, __m256i, _mm_add_epi64, _mm_cvtsi128_si64, _mm_unpackhi_epi64, _mm256_add_epi64,
    _mm256_castsi256_si128, _mm256_extracti128_si256, _mm256_loadu_si256, _mm256_mul_epu32,
    _mm256_setzero_si256,
};

use super::scalar;
use crate::fingerprint::polynomial::BASE;

/// AVX2-gated polynomial-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure AVX2 is available.
#[target_feature(enable = "avx2")]
#[must_use]
#[allow(
    clippy::cast_ptr_alignment,
    reason = "`_mm256_loadu_si256` accepts any-alignment pointers by contract — the `cast::<__m256i>()` reinterpretation is only a type change, not an alignment claim"
)]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    // Effective slice: only the last `window` bytes affect the
    // streaming digest (window-eviction identity). Window `0` is
    // degenerate — no eviction ever fires, so every byte contributes.
    let effective_start = if window == 0 {
        0
    } else {
        bytes.len().saturating_sub(window)
    };
    let effective = &bytes[effective_start..];
    let len = effective.len();

    if len < scalar::BLOCK_LEN {
        // Below one block, the block reformulation offers no lift;
        // fall through to the scalar recurrence directly (still inside
        // the target-feature context for consistent codegen).
        return scalar::scalar_from_zero(effective);
    }

    let full_blocks = len / scalar::BLOCK_LEN;
    let mut state: u64 = 0;

    // SAFETY: this function is `#[target_feature(enable = "avx2")]`, so
    // every AVX2 intrinsic invoked below has its ISA precondition
    // upheld by the enclosing call context. Each of the four SIMD
    // reads in the loop covers 4 bytes at offset `block_start +
    // chunk_off`, with `block_start + BLOCK_LEN <= full_blocks *
    // BLOCK_LEN <= len`; each coefficient load reads 32 bytes (4 x
    // u64) from the static [`scalar::COEFF_HI`] / [`scalar::COEFF_LO`]
    // tables at offset `chunk * 4 + 4 <= BLOCK_LEN = 16`. All reads
    // stay within their buffers.
    unsafe {
        let hi_ptr = scalar::COEFF_HI.as_ptr().cast::<__m256i>();
        let lo_ptr = scalar::COEFF_LO.as_ptr().cast::<__m256i>();

        for b in 0..full_blocks {
            let block_start = b * scalar::BLOCK_LEN;

            let mut hi_acc = _mm256_setzero_si256();
            let mut lo_acc = _mm256_setzero_si256();

            // 4 SIMD chunks × 4 lanes = 16 bytes per block. Manual
            // unrolling would obscure the derivation without helping
            // codegen at `-O2`/`-O3`.
            for chunk in 0..(scalar::BLOCK_LEN / 4) {
                let off = block_start + chunk * 4;
                // Widen 4 bytes into 4 u64 lanes (each byte in the low
                // 32 bits of its lane). A tiny stack-allocated `[u64;
                // 4]` compiles to a handful of movs plus a single
                // `_mm256_loadu_si256`; explicit `VPMOVZXBQ` from
                // memory would spill into more instructions on modern
                // uarches because of the 4-byte load width.
                let b_wide: [u64; 4] = [
                    u64::from(effective[off]),
                    u64::from(effective[off + 1]),
                    u64::from(effective[off + 2]),
                    u64::from(effective[off + 3]),
                ];
                let b_v = _mm256_loadu_si256(b_wide.as_ptr().cast::<__m256i>());

                let coeff_hi_v = _mm256_loadu_si256(hi_ptr.add(chunk));
                let coeff_lo_v = _mm256_loadu_si256(lo_ptr.add(chunk));

                // 4-lane 32×32 → 64 unsigned multiplies. Each output
                // lane holds one `byte_i * coeff_half_i` product,
                // bounded as documented in the module intro.
                let hi_prod = _mm256_mul_epu32(b_v, coeff_hi_v);
                let lo_prod = _mm256_mul_epu32(b_v, coeff_lo_v);

                hi_acc = _mm256_add_epi64(hi_acc, hi_prod);
                lo_acc = _mm256_add_epi64(lo_acc, lo_prod);
            }

            let hi_sum = horizontal_sum_u64x4(hi_acc);
            let lo_sum = horizontal_sum_u64x4(lo_acc);

            // Reassemble `hi_sum * 2^32 + lo_sum` in u128 for the
            // single Mersenne reduction of the block. The sum bound is
            // `~2^73 + ~2^44 < 2^74`, well within `reduce_mod`'s
            // documented two-step convergence range.
            let block_sum_u128 = (u128::from(hi_sum) << 32) + u128::from(lo_sum);
            let block_sum = scalar::reduce_mod(block_sum_u128);

            // Advance the running state by one block's worth of scale.
            let state_scaled = scalar::mul_mod(state, scalar::PK_BLOCK);
            state = scalar::add_mod(state_scaled, block_sum);
        }
    }

    // Scalar tail: length in `[0, BLOCK_LEN)` by construction. Uses the
    // same per-byte recurrence the block form was derived from.
    let tail_start = full_blocks * scalar::BLOCK_LEN;
    for &b in &effective[tail_start..] {
        state = scalar::add_mod(scalar::mul_mod(state, BASE), u64::from(b));
    }

    state
}

/// Horizontal sum of the four `u64` lanes of an `__m256i`.
///
/// # Safety
///
/// AVX2 must be available at run time. Enforced by the enclosing call
/// context via `#[target_feature]`.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn horizontal_sum_u64x4(v: __m256i) -> u64 {
    // All intrinsics below are `safe fn` in `core::arch::x86_64` — pure
    // register-to-register shuffles and adds with no memory access and
    // no ISA precondition beyond the `#[target_feature]` on this fn.
    // No inner `unsafe` block is required; `unsafe_op_in_unsafe_fn`
    // would flag it if one were.
    let hi: __m128i = _mm256_extracti128_si256::<1>(v);
    let lo: __m128i = _mm256_castsi256_si128(v);
    let sum2 = _mm_add_epi64(lo, hi); // 2 u64 lanes
    let hi2 = _mm_unpackhi_epi64(sum2, sum2); // move lane 1 → lane 0
    let final_sum = _mm_add_epi64(sum2, hi2);
    // `_mm_cvtsi128_si64` returns i64; the polynomial-hash accumulator
    // is a bit pattern where signed/unsigned is a reinterpretation,
    // not a value change.
    #[allow(
        clippy::cast_sign_loss,
        reason = "`_mm_cvtsi128_si64` returns i64 by intrinsic signature; the value is a u64 bit pattern"
    )]
    let out = _mm_cvtsi128_si64(final_sum) as u64;
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
        if !is_x86_feature_detected!("avx2") {
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
                // SAFETY: is_x86_feature_detected!("avx2") returned true.
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
        // Explicitly test the boundary between the vectorized block
        // path and the scalar tail — the whole point of the
        // reformulation. 15 is below one block (fully scalar). 16 is
        // exactly one block, no tail. 17 is one block plus a 1-byte
        // tail. 63/64/65/127/128/129 straddle several block boundaries
        // and the workspace-standard chunk boundaries.
        if !is_x86_feature_detected!("avx2") {
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
                // SAFETY: is_x86_feature_detected!("avx2") returned true.
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
        // Window `0` is the degenerate no-eviction mode — every byte
        // contributes to the state. Verify the effective-slice
        // truncation short-circuits correctly to "keep the whole
        // slice" in that case.
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for &size in &[0usize, 1, 15, 16, 17, 63, 64, 65, 128, 1024] {
            let input: alloc::vec::Vec<u8> =
                (0..size).map(|i| (i as u8).wrapping_mul(17)).collect();
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { digest_of_slice(0, &input) };
            assert_eq!(simd, reference(0, &input), "window=0 size={size}");
        }
    }
}
