//! NEON-gated Rabin-fingerprint slice-batch backend for `aarch64`.
//!
//! Compiled only on `aarch64`. This module ships two internal kernels
//! selected by a runtime CPU-feature check:
//!
//! * When the crypto extension is available it dispatches to the
//!   private `digest_of_slice_pmull` kernel — a real vectorized
//!   kernel built on 8-byte block folding through `vmull_p64` (the
//!   `PMULL` polynomial 64x64 → 128 multiply).
//! * Otherwise it falls back to the portable scalar core inside a
//!   `neon` target-feature context. `AES` is used as the runtime
//!   detection proxy for `PMULL` because on `ARMv8-A` the crypto
//!   extension groups AES, PMULL, SHA-1, and SHA-2; every `AES`-
//!   capable `ARMv8` CPU also supports `PMULL`, and Rust's stable
//!   `is_aarch64_feature_detected!` exposes `"aes"` as the canonical
//!   runtime check.
//!
//! # Kernel shape — 8-byte block folding via `vmull_p64`
//!
//! Rabin's per-byte rolling recurrence collapses over any 8-byte run
//! into
//!
//! ```text
//! state_{k+8} = state_k * x^64 + u64_be(bytes[k..k+8])  (mod P)
//! ```
//!
//! and `state_k * x^64 mod P` — using `x^64 ≡ LOW_P (mod P)` with
//! `LOW_P = 0x1B` — is exactly one carry-less multiply, followed by a
//! four-bit `SHIFT_TABLE` second-fold. See the `x86_sse2` sibling
//! module for the derivation shared with the `pclmulqdq` kernel; the
//! NEON form differs only in which intrinsic issues the multiply.
//!
//! `vmull_p64(a, b)` returns a `p128` — an opaque polynomial-128 type
//! — that we reinterpret as a `uint64x2_t` pair and split with
//! `vgetq_lane_u64::<0>` / `vgetq_lane_u64::<1>`. The low 64 bits are
//! the state's `state * LOW_P` low half; the high 64 bits (in fact
//! only the low four bits of that high half) feed
//! `SHIFT_TABLE[high as usize]` for the second fold.
//!
//! # Runtime detection: `aes` as `PMULL` proxy
//!
//! `is_aarch64_feature_detected!("aes")` returns true exactly when the
//! CPU advertises the `ARMv8` crypto extension. That extension groups
//! `PMULL` with `AES`; there is no separate `"pmull"` string in
//! stable Rust's set, and no `ARMv8-A` CPU ships one without the other.
//! The `#[target_feature]` attribute needs `"neon,aes"` because
//! `vmull_p64` is documented against that pair of features in
//! `core::arch::aarch64`.
//!
//! # Safety
//!
//! Every `unsafe fn` here is `#[target_feature(...)]`-declared. The
//! `neon`-only entry point is safe once the dispatcher confirms NEON
//! (guaranteed by the `aarch64` ABI); the `aes,neon` inner kernel is
//! only called after `is_aarch64_feature_detected!("aes")` returns
//! true. See the [module docs][super] for the shared `unsafe` policy.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::aarch64::{uint64x2_t, vgetq_lane_u64, vmull_p64};

use super::scalar;

/// Block size for the block-folding kernel — 8 bytes is the natural
/// unit for a `u64`-wide state.
const BLOCK: usize = 8;

/// NEON-gated Rabin-fingerprint digest of a byte slice.
///
/// Runtime-detects `aes` (as `PMULL` proxy) on the first line and
/// dispatches to the real vectorized kernel when present; otherwise
/// falls back to the portable scalar core inside this `neon`
/// target-feature context.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` NEON is
/// guaranteed by the ABI; the dispatcher still checks
/// `is_aarch64_feature_detected!("neon")` for consistency with the
/// other arch branches.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    if std::arch::is_aarch64_feature_detected!("aes") {
        // SAFETY: is_aarch64_feature_detected!("aes") just returned
        // true; on ARMv8-A the crypto extension groups PMULL with
        // AES, so `vmull_p64` is legal. NEON is upheld by this
        // function's own target-feature context.
        return unsafe { digest_of_slice_pmull(window, bytes) };
    }
    scalar::digest_of_slice(window, bytes)
}

/// PMULL-gated Rabin-fingerprint digest of a byte slice.
///
/// Implements the 8-byte block-folding kernel documented at the module
/// level.
///
/// # Safety
///
/// * NEON and the ARMv8-A crypto extension (`PMULL`, detected as
///   `"aes"`) must both be available at run time.
#[target_feature(enable = "neon,aes")]
#[must_use]
unsafe fn digest_of_slice_pmull(window: usize, bytes: &[u8]) -> u64 {
    // Effective slice: only the last `window` bytes affect the
    // streaming digest (see the SSE2 sibling's derivation shared with
    // the roll-out cancellation identity). Window `0` is degenerate —
    // no eviction ever fires, so every byte contributes.
    let effective_start = if window == 0 {
        0
    } else {
        bytes.len().saturating_sub(window)
    };
    let effective = &bytes[effective_start..];
    let len = effective.len();

    if len < BLOCK {
        return scalar_from_zero(effective);
    }

    let full_blocks = len / BLOCK;
    let mut state: u64 = 0;

    // SAFETY: this function is `#[target_feature(enable = "neon,aes")]`;
    // every intrinsic below has its ISA precondition upheld by the
    // enclosing call context. The pointer arithmetic reads a byte
    // block at `base_ptr.add(b * BLOCK)` where
    // `b * BLOCK + BLOCK <= full_blocks * BLOCK <= len`.
    unsafe {
        let base_ptr = effective.as_ptr();
        for b in 0..full_blocks {
            let chunk_ptr = base_ptr.add(b * BLOCK).cast::<[u8; 8]>();
            let chunk = u64::from_be_bytes(chunk_ptr.read_unaligned());

            // `state * x^64 mod P`: since `x^64 ≡ LOW_P (mod P)`,
            // this is `state * LOW_P mod P`. The polynomial-128 result
            // splits into a low 64-bit polynomial and a high sub-4-bit
            // polynomial; the second fold uses the same SHIFT_TABLE
            // the streaming path already builds.
            let product = vmull_p64(state, super::super::LOW_P);
            // `vmull_p64` returns an opaque `p128`; reinterpret as a
            // pair of `u64` lanes via `uint64x2_t` and extract each
            // lane directly with NEON's per-lane accessor. This keeps
            // the fold's high-half extraction in the SIMD register
            // file and avoids a scalar `u128` shift.
            let product_u64x2: uint64x2_t = core::mem::transmute(product);
            let low64 = vgetq_lane_u64::<0>(product_u64x2);
            let high64 = vgetq_lane_u64::<1>(product_u64x2);

            // high64 fits in 4 bits (state degree ≤ 63, LOW_P degree 4
            // → product degree ≤ 67, so bits 64..67 populate high64).
            // The `usize::try_from` cannot fail on the 64-bit targets
            // this backend compiles for; use it anyway to satisfy the
            // conservative clippy `cast_possible_truncation` lint on
            // hypothetical 32-bit builds.
            let idx = usize::try_from(high64).expect("high64 fits in 4 bits");
            state = low64 ^ super::super::SHIFT_TABLE[idx] ^ chunk;
        }
    }

    // Scalar tail: length in `[0, BLOCK)` by construction.
    let tail_start = full_blocks * BLOCK;
    for &b in &effective[tail_start..] {
        let high = (state >> 56) as usize;
        state = (state << 8) ^ u64::from(b) ^ super::super::SHIFT_TABLE[high];
    }

    state
}

/// Runs the scalar Rabin recurrence over `bytes` from `state = 0`, no
/// window bookkeeping. Used by [`digest_of_slice_pmull`] on inputs
/// shorter than one block; the effective-slice truncation upstream
/// has already collapsed the streaming eviction into a fresh state.
#[inline]
fn scalar_from_zero(bytes: &[u8]) -> u64 {
    let mut state: u64 = 0;
    for &b in bytes {
        let high = (state >> 56) as usize;
        state = (state << 8) ^ u64::from(b) ^ super::super::SHIFT_TABLE[high];
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::rabin::RabinFingerprint;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = RabinFingerprint::new(window);
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
        // 7 is below one block (fully scalar). 8 is exactly one
        // block, no tail. 9 is one block plus a 1-byte tail. 63 is
        // seven blocks plus a 7-byte tail. 64 is eight blocks, no
        // tail. 65 is eight blocks plus a 1-byte tail.
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for &window in &[8usize, 64, 128, 512] {
            for &size in &[7usize, 8, 9, 15, 16, 17, 63, 64, 65, 127, 128, 129] {
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
    fn pmull_kernel_matches_scalar_when_available() {
        // Directly hit the inner pmull kernel — the entry-point test
        // above dispatches on runtime detection and may route to
        // scalar on hosts without AES/PMULL; this test asserts the
        // kernel itself is bit-identical on every host that can run
        // it.
        if !std::arch::is_aarch64_feature_detected!("aes") {
            return;
        }
        for &window in &[1usize, 8, 32, 64, 100, 1024] {
            for &size in &[0usize, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 4096] {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "deterministic pseudo-random byte via low-bits truncation of a mixed u64"
                )]
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| ((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 24) as u8)
                    .collect();
                // SAFETY: is_aarch64_feature_detected!("aes") returned true.
                let simd = unsafe { digest_of_slice_pmull(window, &input) };
                assert_eq!(
                    simd,
                    reference(window, &input),
                    "pmull kernel diverged at size={size} window={window}"
                );
            }
        }
    }
}
