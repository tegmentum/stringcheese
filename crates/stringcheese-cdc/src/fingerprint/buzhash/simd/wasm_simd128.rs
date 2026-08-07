//! wasm SIMD128-gated Buzhash slice-batch backend for `wasm32`.
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
//! # Kernel shape — block reformulation
//!
//! Buzhash's per-byte recurrence
//! `state_{n+1} = state_n.rotate_left(1) ^ contrib_n`, where
//! `contrib_n = T[bytes[n]] ^ (n >= window ? T[bytes[n-window]].rotate_left(window mod 64) : 0)`,
//! unrolls over any `k`-byte run into the closed form
//!
//! ```text
//! state_k = state_0.rotate_left(k)
//!         ^ Σ_{i=0..k}  contrib_i.rotate_left(k-1-i)
//! ```
//!
//! At `k = 64` the leading `state_0.rotate_left(64)` reduces to `state_0`
//! unchanged (a `u64` rotate cycles at 64 bits), so a 64-byte block
//! folds into the running state as a simple XOR:
//!
//! ```text
//! state_after = state_before ^ blockhash
//! blockhash   = Σ_{i=0..64} contrib_i.rotate_left(63 - i)
//! ```
//!
//! The block sum has a natural 2-lane SIMD shape when written as a
//! Horner recurrence over 32 groups of 2 bytes:
//!
//! ```text
//! acc = acc.rotate_left(2) ^ [ ROL(c[2k+0], 1), ROL(c[2k+1], 0) ]
//! ```
//!
//! After 32 iterations, XOR-reducing the two `u64` lanes reproduces
//! the block sum bit-for-bit. Below 64 bytes the kernel defers to the
//! scalar loop, and any `len % 64` tail after the last full block is
//! consumed by the same scalar recurrence.
//!
//! # Implementation
//!
//! wasm SIMD128 has no gather, and `u64x2_shl` / `u64x2_shr` each take
//! a single scalar shift count applied uniformly to both lanes — no
//! per-lane variable shift is available. The kernel therefore
//! computes contributions scalar-side into a 64-entry buffer, applies
//! the per-lane `[1, 0]` pre-rotate scalar-side (a single
//! `g0.rotate_left(1)` before packing lane 0; lane 1's rotate is the
//! identity), and packs the pair with `u64x2(_, _)`. The Horner
//! advance uses a constant 2-bit rotate composed of `u64x2_shl(_, 2)`
//! + `u64x2_shr(_, 62)` + `v128_or`; the fold uses `v128_xor`. Two
//! `u64x2_extract_lane` calls plus a scalar `^` close with the
//! horizontal reduction.
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
    u64x2, u64x2_extract_lane, u64x2_shl, u64x2_shr, u64x2_splat, v128, v128_or, v128_xor,
};

use super::scalar;
use crate::fingerprint::buzhash::BUZ_TABLE;

/// Buzhash block width used by the block-reformulation kernel — 64
/// bytes, the point at which the `state.rotate_left(k)` decay term
/// cycles back to `state` for a `u64` state.
const BLOCK_LEN: usize = 64;

/// wasm SIMD128-gated Buzhash digest of a byte slice.
///
/// # Safety
///
/// See the module-level safety note — on `wasm32 + simd128` this is
/// unconditionally safe.
#[target_feature(enable = "simd128")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    if window == 0 {
        return 0;
    }

    let len = bytes.len();
    if len < BLOCK_LEN {
        return scalar::digest_of_slice(window, bytes);
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "`window % 64` is in 0..64 and always fits in a u32"
    )]
    let window_rot = (window % 64) as u32;

    let mut state: u64 = 0;
    let full_blocks = len / BLOCK_LEN;

    for b in 0..full_blocks {
        let start = b * BLOCK_LEN;
        let contribs = compute_contribs(window, window_rot, bytes, start);
        // SAFETY: `contribs` is a valid stack-allocated `[u64; 64]`
        // whose 512 bytes are fully initialized above; wasm SIMD128 is
        // upheld by this function's target-feature context (a
        // compile-time cfg on this backend).
        let block_hash = unsafe { block_hash_wasm(&contribs) };
        state ^= block_hash;
    }

    let tail_start = full_blocks * BLOCK_LEN;
    for i in tail_start..len {
        let new_contrib = BUZ_TABLE[bytes[i] as usize];
        if i >= window {
            let leaving = bytes[i - window];
            let leaving_contrib = BUZ_TABLE[leaving as usize].rotate_left(window_rot);
            state = state.rotate_left(1) ^ leaving_contrib ^ new_contrib;
        } else {
            state = state.rotate_left(1) ^ new_contrib;
        }
    }

    state
}

/// Precomputes the per-index contribution values for one 64-byte block.
///
/// `contribs[i] = T[bytes[start+i]] ^ (start+i >= window ? T[bytes[start+i-window]].rotate_left(window_rot) : 0)`.
#[inline]
fn compute_contribs(
    window: usize,
    window_rot: u32,
    bytes: &[u8],
    start: usize,
) -> [u64; BLOCK_LEN] {
    let mut contribs = [0u64; BLOCK_LEN];
    for (i, slot) in contribs.iter_mut().enumerate() {
        let pos = start + i;
        let new_contrib = BUZ_TABLE[bytes[pos] as usize];
        if pos >= window {
            let leaving = bytes[pos - window];
            let leaving_contrib = BUZ_TABLE[leaving as usize].rotate_left(window_rot);
            *slot = new_contrib ^ leaving_contrib;
        } else {
            *slot = new_contrib;
        }
    }
    contribs
}

/// wasm SIMD128 kernel: compute the block hash of exactly 64
/// contributions.
///
/// Returns `Σ_{i=0..64} contribs[i].rotate_left(63 - i)` under XOR —
/// the closed form the Buzhash recurrence collapses to over a 64-byte
/// window when the leading `state_0.rotate_left(64)` term cycles back
/// to `state_0` (see the module docs).
///
/// # Safety
///
/// * `simd128` must be enabled at compile time (upheld by the parent
///   module's cfg gate).
#[target_feature(enable = "simd128")]
unsafe fn block_hash_wasm(contribs: &[u64; BLOCK_LEN]) -> u64 {
    let mut acc: v128 = u64x2_splat(0);

    // 32 iterations × 2 contribs = 64.
    for k in 0..32 {
        let g0 = contribs[k * 2];
        let g1 = contribs[k * 2 + 1];

        // Pre-rotate lane 0 by 1 in scalar — `u64x2_shl` applies a
        // single shift to both lanes, so the per-lane `[1, 0]`
        // pre-rotate pattern is factored out into this pack. Lane 1's
        // rotate-by-zero is the identity.
        let pair: v128 = u64x2(g0.rotate_left(1), g1);

        // Uniform 2-bit rotate for the Horner advance.
        let al = u64x2_shl(acc, 2);
        let ar = u64x2_shr(acc, 62);
        let advanced = v128_or(al, ar);

        acc = v128_xor(advanced, pair);
    }

    let lane0 = u64x2_extract_lane::<0>(acc);
    let lane1 = u64x2_extract_lane::<1>(acc);
    lane0 ^ lane1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::buzhash::Buzhash;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = Buzhash::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        for &window in &[0usize, 1, 8, 32, 64, 100] {
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
    fn matches_scalar_reference_at_word_bits_boundary() {
        // Explicitly test the block boundary — the whole point of the
        // reformulation. 63 is below one block (fully scalar). 64 is
        // exactly one block. 65 is one block plus a 1-byte tail. 127 /
        // 128 / 129 straddle the second-block boundary.
        for &window in &[1usize, 4, 8, 32, 64, 100, 200] {
            for &size in &[63usize, 64, 65, 127, 128, 129] {
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| {
                        #[allow(
                            clippy::cast_possible_truncation,
                            reason = "deterministic pseudo-random byte via low-bits truncation of a mixed u32"
                        )]
                        let m =
                            ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(1) >> 16) as u8;
                        m
                    })
                    .collect();
                // SAFETY: `wasm32 + simd128` compile-time cfg upholds
                // the target-feature precondition.
                let simd = unsafe { digest_of_slice(window, &input) };
                assert_eq!(
                    simd,
                    reference(window, &input),
                    "at boundary size={size} window={window}"
                );
            }
        }
    }
}
