//! AVX2-gated Gear-hash slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. This is the widest x86 backend
//! [the dispatcher][super::digest_of_slice] can select; it falls back to
//! the [SSE2 sibling][super::x86_sse2] on hosts without AVX2.
//!
//! # Kernel shape — block reformulation
//!
//! Gear's per-byte recurrence `state_{n+1} = (state_n << 1) + G[b_n]`
//! unrolls, over any k-byte run, into the closed form
//!
//! ```text
//! state_k = state_0 << k  +  Σ_{i=0..k}  G[b_i] << (k-1-i)
//! ```
//!
//! When `k = 64` the leading `state_0 << 64` term evaporates in `u64`
//! arithmetic — every bit of the prior state has shifted out — and the
//! recurrence collapses to `state_64 = Σ_{i=0..64} G[b_i] << (63-i)`.
//! That sum is independent of `state_0`, so a 64-byte block can be
//! computed in isolation and dropped straight into the running state.
//! The block sum in turn has a natural 4-lane SIMD shape when written as
//! a Horner recurrence over 16 groups of 4 bytes:
//!
//! ```text
//! acc = (acc << 4) + [ G[b_{4k}]<<3,  G[b_{4k+1}]<<2,
//!                      G[b_{4k+2}]<<1, G[b_{4k+3}]<<0 ]
//! ```
//!
//! After 16 iterations, summing the four `u64` lanes reproduces the
//! block sum bit-for-bit. Below 64 bytes the kernel defers to the
//! scalar loop, and any `len % 64` tail after the last full block is
//! consumed by the same scalar recurrence — both share the reference
//! `state = (state << 1) + G[byte]` that anchors the differential
//! contract.
//!
//! # Implementation
//!
//! Each 4-byte group is loaded as one unaligned `u32`, widened to four
//! `i32` byte-value indices via `_mm_cvtepu8_epi32`, and gathered from
//! the 256-entry `GEAR_TABLE` with `_mm256_i32gather_epi64` (scale 8 for
//! the `u64` cell width). A constant `_mm256_sllv_epi64` supplies the
//! per-lane `[3, 2, 1, 0]` pre-shift; `_mm256_slli_epi64::<4>` advances
//! the accumulator between iterations; `_mm256_add_epi64` folds the
//! group into place. The horizontal sum at the end reduces the four
//! `u64` lanes to a single digest with `_mm256_extracti128_si256`
//! plus one `_mm_add_epi64` and two `_mm_extract_epi64` extractions.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher checks
//! `is_x86_feature_detected!("avx2")` before every call.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "the `*const u32` cast in `block_hash_avx2` feeds `read_unaligned`, which by contract accepts any-alignment pointers; the clippy lint doesn't know the read tolerates under-alignment"
)]

use core::arch::x86_64::{
    __m128i, _mm_add_epi64, _mm_cvtepu8_epi32, _mm_cvtsi32_si128, _mm_extract_epi64,
    _mm256_add_epi64, _mm256_castsi256_si128, _mm256_extracti128_si256, _mm256_i32gather_epi64,
    _mm256_setr_epi64x, _mm256_setzero_si256, _mm256_slli_epi64, _mm256_sllv_epi64,
};

use super::scalar;
use crate::fingerprint::gear::GEAR_TABLE;

/// Gear block width used by the block-reformulation kernel — 64 bytes,
/// the point at which the `state << k` decay term wipes the accumulator
/// for a `u64` state.
const BLOCK_LEN: usize = 64;

/// AVX2-gated Gear-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure AVX2 is available (see the module-level
/// safety note).
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn digest_of_slice(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    if len < BLOCK_LEN {
        // Below one block the reformulation gives no lift over the
        // scalar recurrence — the block wipe would fold in state_0
        // that hasn't been established yet.
        return scalar::digest_of_slice(bytes);
    }

    let base = bytes.as_ptr();
    let mut state: u64 = 0;
    let full_blocks = len / BLOCK_LEN;

    // Each block wipes the accumulator (state << 64 == 0 for u64), so
    // the state after block `b` is exactly `block_hash_avx2(bytes[b*64 .. (b+1)*64])`.
    // Iterating gives the compiler a straight inner loop over full
    // blocks — equivalent in output to skipping to the last block
    // directly, but preserves the scalar semantics one-block-at-a-time
    // and keeps the benchmark shape informative.
    for b in 0..full_blocks {
        // SAFETY: `b * BLOCK_LEN + BLOCK_LEN <= full_blocks * BLOCK_LEN <= len`,
        // so `base.add(b * BLOCK_LEN)` addresses a valid 64-byte run.
        // AVX2 is upheld by this function's target-feature context.
        state = unsafe { block_hash_avx2(base.add(b * BLOCK_LEN)) };
    }

    // Scalar tail — length in `[0, 64)` by construction.
    let tail_start = full_blocks * BLOCK_LEN;
    for &byte in &bytes[tail_start..] {
        state = (state << 1).wrapping_add(GEAR_TABLE[byte as usize]);
    }

    state
}

/// AVX2 kernel: compute the block hash of exactly 64 bytes starting at
/// `block_ptr`.
///
/// Returns `Σ_{i=0..64} G[b_i] << (63 - i)` in wrapping `u64` arithmetic
/// — the closed form the scalar recurrence collapses to over a 64-byte
/// window when the initial state is zero (see the module docs).
///
/// # Safety
///
/// * `block_ptr` must be a valid pointer for reads of 64 consecutive
///   bytes.
/// * AVX2 must be available at run time.
#[target_feature(enable = "avx2")]
unsafe fn block_hash_avx2(block_ptr: *const u8) -> u64 {
    let gear_ptr = GEAR_TABLE.as_ptr().cast::<i64>();

    // Per-lane pre-shift: lane 0 by 3, lane 1 by 2, lane 2 by 1, lane 3
    // by 0 — matches the Horner unroll documented in the module.
    // `_mm256_setr_epi64x` packs its arguments in ascending-lane order,
    // so lane 0 receives the first argument.
    //
    // SAFETY: this function is `#[target_feature(enable = "avx2")]`,
    // so every AVX2 intrinsic invoked below has its ISA precondition
    // upheld by the enclosing call context. The pointer arithmetic
    // (`block_ptr.add(k * 4).cast::<u32>().read_unaligned()`) stays in
    // bounds because `k * 4 + 4 <= 64` and `block_ptr` is valid for
    // 64 consecutive byte reads by this function's contract. The
    // gather uses byte-valued indices in `0..=255`, which are well
    // within the 256-entry `GEAR_TABLE`.
    unsafe {
        let shift_vec = _mm256_setr_epi64x(3, 2, 1, 0);

        let mut acc = _mm256_setzero_si256();

        // 16 iterations × 4 bytes = 64 bytes. Manual unrolling here
        // yields no measurable benefit over the compiler's loop
        // unrolling at `-O2`/`-O3` and keeps the source auditable
        // against the derivation.
        for k in 0..16 {
            // Load 4 bytes as one unaligned little-endian `u32`. The
            // `read_unaligned` contract accepts any-alignment pointers;
            // clippy's `cast_ptr_alignment` warning is silenced at the
            // module level for this same reason.
            let bytes4 = block_ptr.add(k * 4).cast::<u32>().read_unaligned();

            // Splat the 32 bits into the low lane of an xmm, then
            // widen the four bytes to four i32 gather indices.
            // `_mm_cvtepu8_epi32` (SSE4.1) is available under AVX2 by
            // feature-set superset: every CPU that reports AVX2 also
            // reports SSE4.1.
            #[allow(
                clippy::cast_possible_wrap,
                reason = "`_mm_cvtsi32_si128` takes an i32 by intrinsic signature; the reinterpretation to signed is a bit pattern change with no arithmetic meaning — the four packed bytes are widened to independent unsigned lanes by `_mm_cvtepu8_epi32` on the next line"
            )]
            let bytes4_signed = bytes4 as i32;
            let packed: __m128i = _mm_cvtsi32_si128(bytes4_signed);
            let indices: __m128i = _mm_cvtepu8_epi32(packed);

            // Gather four `u64` values from `GEAR_TABLE`, one per byte
            // value. Scale is `8` because the table cells are `u64`.
            let gathered = _mm256_i32gather_epi64::<8>(gear_ptr, indices);

            // Pre-shift each lane by its Horner exponent [3, 2, 1, 0].
            let shifted = _mm256_sllv_epi64(gathered, shift_vec);

            // Horner step: acc = (acc << 4) + shifted.
            acc = _mm256_slli_epi64::<4>(acc);
            acc = _mm256_add_epi64(acc, shifted);
        }

        // Horizontal sum of the four `u64` lanes.
        let high_128 = _mm256_extracti128_si256::<1>(acc);
        let low_128 = _mm256_castsi256_si128(acc);
        let pair = _mm_add_epi64(low_128, high_128);
        // `_mm_extract_epi64` yields `i64`; reinterpret as `u64` for
        // the wrapping add — wrapping is the whole recurrence.
        #[allow(
            clippy::cast_sign_loss,
            reason = "`_mm_extract_epi64` returns i64 by intrinsic signature; Gear state is a bit pattern where signed/unsigned is a re-interpretation, not a value change"
        )]
        let lane0 = _mm_extract_epi64::<0>(pair) as u64;
        #[allow(
            clippy::cast_sign_loss,
            reason = "`_mm_extract_epi64` returns i64 by intrinsic signature; Gear state is a bit pattern where signed/unsigned is a re-interpretation, not a value change"
        )]
        let lane1 = _mm_extract_epi64::<1>(pair) as u64;
        lane0.wrapping_add(lane1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::gear::GearHash;

    fn reference(bytes: &[u8]) -> u64 {
        let mut h = GearHash::new(64);
        for &b in bytes {
            h.roll(b);
        }
        h.state()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let cases: &[&[u8]] = &[
            b"",
            b"a",
            b"the quick brown fox jumps over the lazy dog",
            &[0u8; 128],
            &[0xFFu8; 200],
        ];
        for &input in cases {
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { digest_of_slice(input) };
            assert_eq!(simd, reference(input), "on {input:?}");
        }
    }

    #[test]
    fn matches_scalar_reference_at_block_boundaries() {
        // Explicitly test the boundary between the SIMD block path and
        // the scalar tail — the whole point of the reformulation. 63
        // is below the block threshold (fully scalar). 64 is exactly
        // one block, no tail. 65 is one block plus a 1-byte tail. 127
        // is one block plus a 63-byte tail. 128 is two blocks, no
        // tail. 129 is two blocks plus a 1-byte tail.
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for &size in &[63usize, 64, 65, 127, 128, 129] {
            let input: alloc::vec::Vec<u8> = (0..size)
                .map(|i| {
                    #[allow(
                        clippy::cast_possible_truncation,
                        reason = "deterministic pseudo-random byte via low-bits truncation of a mixed u32"
                    )]
                    let m = ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(1) >> 16) as u8;
                    m
                })
                .collect();
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { digest_of_slice(&input) };
            assert_eq!(simd, reference(&input), "at boundary size={size}");
        }
    }
}
