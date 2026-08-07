//! AVX2-gated Rabin-fingerprint slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. This is the widest x86 backend the
//! [dispatcher][super::digest_of_slice] can select on this
//! workspace's MSRV; it delegates to the [SSE2 sibling][super::x86_sse2]'s
//! `pclmulqdq` kernel when `pclmulqdq` is available and falls back to
//! the portable scalar core otherwise.
//!
//! # Why not `VPCLMULQDQ`?
//!
//! A 2-way parallel folding kernel built on
//! `_mm256_clmulepi64_epi128` (`VPCLMULQDQ`) would halve the fold
//! dependency chain relative to `_mm_clmulepi64_si128`, and the
//! math generalizes cleanly (an interleaved-init trick over two
//! sub-streams; a precomputed `x^128 mod P` fold constant). That
//! intrinsic was stabilized in Rust 1.89, however, and this
//! workspace's MSRV is 1.85 — the wide variant can't be used
//! without bumping MSRV. The SSE2 sibling's `digest_of_slice_pclmul`
//! kernel still delivers the real vectorized `GF(2)` reduction (one
//! `pclmulqdq` per 8-byte block, followed by the shared
//! `SHIFT_TABLE` high-nibble fold) that this AVX2 entry point
//! ultimately dispatches into. When the MSRV moves past 1.89 the
//! `vpclmulqdq` path can be added here as a narrower runtime-
//! detection branch above the current `pclmulqdq` branch, without
//! touching the dispatcher.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher checks
//! `is_x86_feature_detected!("avx2")` before every call. See the
//! [module docs][super] for the shared `unsafe` policy.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// AVX2-gated Rabin-fingerprint digest of a byte slice.
///
/// Runtime-detects `pclmulqdq` on entry and delegates to the SSE2
/// sibling's `pclmulqdq` kernel when present; otherwise falls back to
/// the portable scalar core inside this `avx2` target-feature context.
///
/// # Safety
///
/// The caller must ensure AVX2 is available. The dispatcher checks
/// `is_x86_feature_detected!("avx2")` before every call.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    if is_x86_feature_detected!("pclmulqdq") {
        // SAFETY: is_x86_feature_detected!("pclmulqdq") returned true.
        // SSE2 is a subset of AVX2 (baseline for `x86_64`), so the
        // sibling's target-feature precondition is upheld.
        return unsafe { super::x86_sse2::digest_of_slice_pclmul(window, bytes) };
    }
    scalar::digest_of_slice(window, bytes)
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
        // 7 is below one block (fully scalar). 8 is exactly one block,
        // no tail. 9 is one block plus a 1-byte tail. 63 is seven
        // blocks plus a 7-byte tail. 64 is eight blocks, no tail. 65
        // is eight blocks plus a 1-byte tail.
        if !is_x86_feature_detected!("avx2") {
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
}
