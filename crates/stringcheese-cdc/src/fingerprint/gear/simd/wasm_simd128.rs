//! wasm SIMD128-gated Gear-hash slice-batch backend for `wasm32`.
//!
//! Compiled only on `wasm32` targets and only when the `simd128`
//! target-feature is enabled at compile time. Unlike `x86_64` and
//! `aarch64`, wasm has no runtime CPU-feature detection: whether the
//! SIMD opcodes are legal is a property of the wasm engine executing
//! the module. Callers control the choice via
//! `RUSTFLAGS=-C target-feature=+simd128` at build time, and the
//! dispatcher in [`super`] compiles this path in or out with a matching
//! `#[cfg(target_feature = "simd128")]` gate.
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
//! The block sum has a natural 2-lane SIMD shape when written as a
//! Horner recurrence over 32 groups of 2 bytes:
//!
//! ```text
//! acc = (acc << 2) + [ G[b_{2k}]<<1, G[b_{2k+1}]<<0 ]
//! ```
//!
//! After 32 iterations, summing the two `u64` lanes reproduces the
//! block sum bit-for-bit. Below 64 bytes the kernel defers to the
//! scalar loop; any `len % 64` tail after the last full block is
//! consumed by the same scalar recurrence.
//!
//! # Implementation
//!
//! wasm SIMD128 has no gather, and `u64x2_shl` takes a single scalar
//! shift count that applies uniformly to both lanes — no per-lane
//! variable shift is available. The kernel therefore reads each byte
//! scalar-side, looks up the `u64` cell from `GEAR_TABLE`, pre-shifts
//! lane 0 by `1` in scalar (`g0 << 1`) so the `u64x2` values can be
//! packed with `u64x2(g0 << 1, g1)`, and folds them into the
//! accumulator with a constant `u64x2_shl(acc, 2)` Horner advance and
//! a `u64x2_add` combine. Two `u64x2_extract_lane` calls close with
//! the horizontal sum.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` for parity with the sibling
//! SSE2/AVX2/NEON backends' `#[target_feature]`-gated signature, even
//! though on wasm the target feature is a compile-time property rather
//! than a runtime precondition. On `wasm32` with
//! `target_feature = "simd128"` this function is unconditionally safe
//! to call.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::wasm32::{u64x2, u64x2_add, u64x2_extract_lane, u64x2_shl, u64x2_splat, v128};

use super::scalar;
use crate::fingerprint::gear::GEAR_TABLE;

/// Gear block width used by the block-reformulation kernel — 64 bytes,
/// the point at which the `state << k` decay term wipes the accumulator
/// for a `u64` state.
const BLOCK_LEN: usize = 64;

/// wasm SIMD128-gated Gear-hash digest of a byte slice.
///
/// # Safety
///
/// See the module-level safety note — on `wasm32 + simd128` this is
/// unconditionally safe.
#[target_feature(enable = "simd128")]
#[must_use]
pub unsafe fn digest_of_slice(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    if len < BLOCK_LEN {
        return scalar::digest_of_slice(bytes);
    }

    let base = bytes.as_ptr();
    let mut state: u64 = 0;
    let full_blocks = len / BLOCK_LEN;

    for b in 0..full_blocks {
        // SAFETY: `b * BLOCK_LEN + BLOCK_LEN <= full_blocks * BLOCK_LEN <= len`,
        // so `base.add(b * BLOCK_LEN)` addresses a valid 64-byte run.
        // wasm SIMD128 is upheld by this function's target-feature
        // context (a compile-time cfg on this backend).
        state = unsafe { block_hash_wasm(base.add(b * BLOCK_LEN)) };
    }

    let tail_start = full_blocks * BLOCK_LEN;
    for &byte in &bytes[tail_start..] {
        state = (state << 1).wrapping_add(GEAR_TABLE[byte as usize]);
    }

    state
}

/// wasm SIMD128 kernel: compute the block hash of exactly 64 bytes
/// starting at `block_ptr`.
///
/// Returns `Σ_{i=0..64} G[b_i] << (63 - i)` in wrapping `u64` arithmetic
/// — the closed form the scalar recurrence collapses to over a 64-byte
/// window when the initial state is zero (see the module docs).
///
/// # Safety
///
/// * `block_ptr` must be a valid pointer for reads of 64 consecutive
///   bytes.
/// * `simd128` must be enabled at compile time (upheld by the parent
///   module's cfg gate).
#[target_feature(enable = "simd128")]
unsafe fn block_hash_wasm(block_ptr: *const u8) -> u64 {
    let mut acc: v128 = u64x2_splat(0);

    for k in 0..32 {
        // SAFETY: `k * 2 + 2 <= 64`, and `block_ptr` is valid for 64
        // consecutive byte reads by this function's contract.
        let b0 = unsafe { *block_ptr.add(k * 2) };
        let b1 = unsafe { *block_ptr.add(k * 2 + 1) };

        // Byte values are always in `0..=255`, well within the 256-entry
        // `GEAR_TABLE`; index safety is upheld by the type of `u8`.
        let g0 = GEAR_TABLE[b0 as usize];
        let g1 = GEAR_TABLE[b1 as usize];

        // Pre-shift lane 0 by 1 in scalar — `u64x2_shl` applies a
        // single shift to both lanes, so the per-lane `[1, 0]` pattern
        // is factored out into this pack.
        let pair: v128 = u64x2(g0 << 1, g1);

        // Horner step: acc = (acc << 2) + pair.
        acc = u64x2_shl(acc, 2);
        acc = u64x2_add(acc, pair);
    }

    let lane0 = u64x2_extract_lane::<0>(acc);
    let lane1 = u64x2_extract_lane::<1>(acc);
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
        let cases: &[&[u8]] = &[
            b"",
            b"a",
            b"the quick brown fox jumps over the lazy dog",
            &[0u8; 128],
            &[0xFFu8; 200],
        ];
        for &input in cases {
            // SAFETY: this file is only compiled under `wasm32 +
            // simd128`, so the target-feature precondition holds by
            // build-time cfg.
            let simd = unsafe { digest_of_slice(input) };
            assert_eq!(simd, reference(input), "on {input:?}");
        }
    }

    #[test]
    fn matches_scalar_reference_at_block_boundaries() {
        // Explicitly test the boundary between the SIMD block path and
        // the scalar tail — the whole point of the reformulation. See
        // the AVX2 sibling for the boundary-size rationale.
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
            // SAFETY: `wasm32 + simd128` compile-time cfg upholds the
            // target-feature precondition.
            let simd = unsafe { digest_of_slice(&input) };
            assert_eq!(simd, reference(&input), "at boundary size={size}");
        }
    }
}
