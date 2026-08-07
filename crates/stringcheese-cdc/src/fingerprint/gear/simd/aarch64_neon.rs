//! NEON-gated Gear-hash slice-batch backend for `aarch64`.
//!
//! Compiled only on `aarch64`. NEON is baseline for `aarch64`, so this
//! branch is always a valid target when the crate is built for that
//! architecture; the dispatcher still checks
//! `is_aarch64_feature_detected!("neon")` for uniformity with the x86
//! branches.
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
//! The block sum in turn has a natural 2-lane SIMD shape when written as
//! a Horner recurrence over 32 groups of 2 bytes:
//!
//! ```text
//! acc = (acc << 2) + [ G[b_{2k}]<<1, G[b_{2k+1}]<<0 ]
//! ```
//!
//! After 32 iterations, summing the two `u64` lanes reproduces the
//! block sum bit-for-bit. Below 64 bytes the kernel defers to the
//! scalar loop, and any `len % 64` tail after the last full block is
//! consumed by the same scalar recurrence — both share the reference
//! `state = (state << 1) + G[byte]` that anchors the differential
//! contract.
//!
//! # Implementation
//!
//! Each 2-byte group is read as two scalar byte loads (NEON has no
//! gather instruction; the 2048-byte `GEAR_TABLE` is too large for
//! `vqtbl4q_u8` and would need a manual byte-slice-then-recombine
//! anyway, so the cost model favours plain byte-indexed loads on the
//! integer side). The two `u64` cells are packed into a `uint64x2_t`
//! via `vsetq_lane_u64`, pre-shifted by `[1, 0]` with the variable-
//! per-lane `vshlq_u64`, and folded into an accumulator whose Horner
//! advance is a constant `vshlq_n_u64::<2>`. `vaddq_u64` sums into the
//! accumulator; `vaddvq_u64` (or a pair of `vgetq_lane_u64` +
//! wrapping-add) closes with the horizontal sum.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::aarch64::{
    int64x2_t, uint64x2_t, vaddq_u64, vdupq_n_s64, vdupq_n_u64, vgetq_lane_u64, vsetq_lane_s64,
    vsetq_lane_u64, vshlq_n_u64, vshlq_u64,
};

use super::scalar;
use crate::fingerprint::gear::GEAR_TABLE;

/// Gear block width used by the block-reformulation kernel — 64 bytes,
/// the point at which the `state << k` decay term wipes the accumulator
/// for a `u64` state.
const BLOCK_LEN: usize = 64;

/// NEON-gated Gear-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the ABI, but the dispatcher still checks
/// `is_aarch64_feature_detected!("neon")` to keep every dispatch branch
/// uniform.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn digest_of_slice(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    if len < BLOCK_LEN {
        return scalar::digest_of_slice(bytes);
    }

    let base = bytes.as_ptr();
    let mut state: u64 = 0;
    let full_blocks = len / BLOCK_LEN;

    // Each block wipes the accumulator (state << 64 == 0 for u64), so
    // the state after block `b` is exactly the block hash of that block.
    // Iterating over every full block preserves the scalar
    // one-block-at-a-time semantics and gives the compiler a straight
    // inner loop; the mathematical identity means intermediate blocks
    // are algebraically redundant, but iterating still costs O(n/64)
    // SIMD-kernel invocations, matching what a byte-parallel kernel is
    // expected to do.
    for b in 0..full_blocks {
        // SAFETY: `b * BLOCK_LEN + BLOCK_LEN <= full_blocks * BLOCK_LEN <= len`,
        // so `base.add(b * BLOCK_LEN)` addresses a valid 64-byte run.
        // NEON is upheld by this function's target-feature context.
        state = unsafe { block_hash_neon(base.add(b * BLOCK_LEN)) };
    }

    let tail_start = full_blocks * BLOCK_LEN;
    for &byte in &bytes[tail_start..] {
        state = (state << 1).wrapping_add(GEAR_TABLE[byte as usize]);
    }

    state
}

/// NEON kernel: compute the block hash of exactly 64 bytes starting at
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
/// * NEON must be available at run time.
#[target_feature(enable = "neon")]
unsafe fn block_hash_neon(block_ptr: *const u8) -> u64 {
    // Per-lane pre-shift vector: lane 0 by 1, lane 1 by 0. NEON's
    // `vshlq_u64` takes a signed count per lane where positive means
    // left; we build the int64x2_t by lane-setting `1` into lane 0 of a
    // zero splat.
    let shift_vec: int64x2_t = vsetq_lane_s64::<0>(1, vdupq_n_s64(0));

    let mut acc: uint64x2_t = vdupq_n_u64(0);

    for k in 0..32 {
        // SAFETY: `k * 2 + 2 <= 64`, and `block_ptr` is valid for 64
        // consecutive byte reads by this function's contract.
        let b0 = unsafe { *block_ptr.add(k * 2) };
        let b1 = unsafe { *block_ptr.add(k * 2 + 1) };

        // Byte values are always in `0..=255`, well within the 256-entry
        // `GEAR_TABLE`; index safety is upheld by the type of `u8`.
        let g0 = GEAR_TABLE[b0 as usize];
        let g1 = GEAR_TABLE[b1 as usize];

        let pair: uint64x2_t = vsetq_lane_u64::<1>(g1, vsetq_lane_u64::<0>(g0, vdupq_n_u64(0)));
        // Pre-shift each lane by [1, 0].
        let shifted: uint64x2_t = vshlq_u64(pair, shift_vec);

        // Horner step: acc = (acc << 2) + shifted.
        acc = vshlq_n_u64::<2>(acc);
        acc = vaddq_u64(acc, shifted);
    }

    // Horizontal sum. `vaddvq_u64` exists on some NEON revisions but is
    // avoided here in favour of two lane extractions plus a wrapping
    // add — both compile to the same one-cycle sequence at `-O2` and
    // keep the source uniform with the wasm and x86 backends.
    let lane0 = vgetq_lane_u64::<0>(acc);
    let lane1 = vgetq_lane_u64::<1>(acc);
    lane0.wrapping_add(lane1)
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
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { digest_of_slice(input) };
            assert_eq!(simd, reference(input), "on {input:?}");
        }
    }

    #[test]
    fn matches_scalar_reference_at_block_boundaries() {
        // Explicitly test the boundary between the SIMD block path and
        // the scalar tail — the whole point of the reformulation. See
        // the AVX2 sibling for the boundary-size rationale.
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { digest_of_slice(&input) };
            assert_eq!(simd, reference(&input), "at boundary size={size}");
        }
    }
}
