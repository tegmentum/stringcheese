//! SIMD-accelerated Rabin-fingerprint slice-batch backend.
//!
//! Compiled only when both `simd` and `alloc` are enabled — `alloc`
//! because it lives under [`crate::fingerprint::rabin`], which is
//! itself `alloc`-gated for the streaming hash's roll-out table.
//!
//! # Real vectorized `GF(2)` reduction on hardware carry-less multiply
//!
//! Rabin's per-byte update over `GF(2)` polynomial arithmetic is
//!
//! ```text
//! state = (state << 8) ^ byte ^ SHIFT_TABLE[state >> 56]
//!       ^ (if full) roll_out_table[leaving]
//! ```
//!
//! Each step depends on the previous state via the shift and the
//! high-byte-driven table lookup, so there is no natural *per-byte*
//! SIMD parallelism at the update level. Unrolled over any 8-byte
//! run, however, the recurrence collapses to the closed form
//!
//! ```text
//! state_{k+8} = state_k * x^64 + u64_be(bytes[k..k+8])   (mod P)
//! ```
//!
//! and — since `P(x) = x^64 + LOW_P` with `LOW_P = 0x1B` — the
//! reduction `state_k * x^64 mod P` is exactly one carry-less
//! multiply, followed by a four-bit second fold through the same
//! byte-indexed `SHIFT_TABLE` the streaming path already builds. The
//! arch backends here express that step in hardware:
//!
//! * `x86_sse2` — `pclmulqdq` gated. `_mm_clmulepi64_si128` performs
//!   the 8-byte block fold in one instruction; the second fold reduces
//!   the at-most-four-bit high half via a `SHIFT_TABLE[high]` scalar
//!   lookup. Runtime-detects `pclmulqdq` on entry and falls back to
//!   the portable scalar core when absent.
//! * `x86_avx2` — AVX2-gated entry point that delegates into the
//!   SSE2 sibling's `pclmulqdq` kernel when `pclmulqdq` is present
//!   (still one carry-less multiply per 8-byte block, same real
//!   vectorized reduction as the SSE2 branch) and falls back to
//!   scalar otherwise. A `VPCLMULQDQ` 2-way parallel path is
//!   deferred until the workspace MSRV moves past 1.89 — see the
//!   AVX2 file's docs for the derivation waiting there.
//! * `aarch64_neon` — `PMULL` gated (runtime-detected as `"aes"` on
//!   the ARMv8-A crypto extension, which groups `AES`, `PMULL`,
//!   `SHA-1`, and `SHA-2`). `vmull_p64` supplies the 8-byte block
//!   fold's carry-less multiply; the second fold is the same
//!   `SHIFT_TABLE` scalar lookup. Runtime-detects `aes` on entry and
//!   falls back to a NEON-context scalar core when the crypto
//!   extension is absent.
//! * `wasm_simd128` — wasm SIMD128 has no carry-less multiply
//!   primitive on integer lanes, so this backend ships as the scalar
//!   core inside a `simd128` target-feature context. See the file's
//!   docs for the ISA constraint that keeps the fold on the scalar
//!   path.
//!
//! # Effective-slice window truncation
//!
//! The streaming Rabin-hash's roll-out-table cancellation guarantees
//! that the digest after feeding `L` bytes with window `W` depends
//! only on the last `min(L, W)` bytes when `W > 0`, and on every
//! byte when `W == 0` (degenerate no-eviction mode). The vectorized
//! kernels exploit this: they truncate the input up front to the
//! effective slice, then process it from `state = 0` via the block
//! form. The truncation is byte-identical to the streaming reference
//! by construction; every backend's differential tests anchor the
//! full pipeline (truncation + block fold + scalar tail) against a
//! scalar `RollingHash::roll` loop over the un-truncated input.
//!
//! # Public surface
//!
//! * [`digest_of_slice`] — the runtime-dispatching entry point. Feeds
//!   a fresh `RabinFingerprint::new(window)` with `bytes` byte-by-byte
//!   and returns the final `digest()`.
//!
//! # Backends and `unsafe` policy
//!
//! Layout and `unsafe` policy mirror [`crate::fingerprint::gear::simd`];
//! see the docs there for the shared trade-off and the invariant every
//! backend function's safety comment references.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics require unsafe by declaration; every unsafe fn and every unsafe block here has a safety comment naming the CPU-feature precondition the dispatcher upholds"
)]

pub mod scalar;

#[cfg(target_arch = "x86_64")]
pub mod x86_avx2;
#[cfg(target_arch = "x86_64")]
pub mod x86_sse2;

#[cfg(target_arch = "aarch64")]
pub mod aarch64_neon;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub mod wasm_simd128;

/// Runtime-dispatching Rabin-fingerprint digest for byte-slice inputs.
///
/// Feeds `bytes` byte-by-byte into a fresh
/// `RabinFingerprint::new(window)` and returns the final `digest()`.
/// Byte-for-byte identical to a scalar
/// [`RollingHash::roll`][crate::fingerprint::RollingHash::roll] loop
/// over the same input.
#[must_use]
pub fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: is_x86_feature_detected!("avx2") returned true;
            // the AVX2 entry point checks `vpclmulqdq`/`pclmulqdq`
            // internally and falls back accordingly.
            return unsafe { x86_avx2::digest_of_slice(window, bytes) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: is_x86_feature_detected!("sse2") returned true;
            // the SSE2 entry point checks `pclmulqdq` internally and
            // falls back to scalar when absent.
            return unsafe { x86_sse2::digest_of_slice(window, bytes) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: is_aarch64_feature_detected!("neon") returned
            // true; the NEON entry point checks `aes` (PMULL proxy)
            // internally and falls back to scalar when absent.
            return unsafe { aarch64_neon::digest_of_slice(window, bytes) };
        }
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        // SAFETY: wasm-SIMD feature detection is a compile-time gate.
        return unsafe { wasm_simd128::digest_of_slice(window, bytes) };
    }
    #[allow(
        unreachable_code,
        reason = "the wasm32+simd128 cfg-branch above returns unconditionally when compiled; on hosts where that branch is stripped this call is the fallthrough"
    )]
    scalar::digest_of_slice(window, bytes)
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test inputs use `(i as _)` and `>> shift as u8` patterns to derive deterministic pseudo-random bytes from small counters; `size` is bounded well below `u32::MAX`, so truncation cannot occur"
)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::rabin::RabinFingerprint;

    fn scalar_reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = RabinFingerprint::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn dispatcher_matches_scalar_reference_on_short_inputs() {
        for &window in &[1usize, 4, 8] {
            let cases: &[&[u8]] = &[b"", b"a", b"abcdefgh", b"the quick brown fox"];
            for &input in cases {
                assert_eq!(
                    digest_of_slice(window, input),
                    scalar_reference(window, input),
                    "input {input:?} window {window}"
                );
            }
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_at_chunk_boundaries() {
        for &window in &[8usize, 32, 64] {
            for &size in &[
                1usize, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129,
            ] {
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| {
                        let m = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(11);
                        (m >> 16) as u8
                    })
                    .collect();
                assert_eq!(
                    digest_of_slice(window, &input),
                    scalar_reference(window, &input),
                    "size={size} window={window}"
                );
            }
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_at_window_size() {
        for &window in &[1usize, 8, 32, 64, 100] {
            for size in [window.saturating_sub(1), window, window + 1, window * 2 + 3] {
                let input: alloc::vec::Vec<u8> =
                    (0..size).map(|i| (i as u8).wrapping_mul(31)).collect();
                assert_eq!(
                    digest_of_slice(window, &input),
                    scalar_reference(window, &input),
                    "size={size} window={window}"
                );
            }
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_on_larger_blobs() {
        for &window in &[32usize, 128] {
            for &size in &[512usize, 1024, 4096, 16 * 1024] {
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| {
                        let m = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                        ((m >> 24) & 0xFF) as u8
                    })
                    .collect();
                assert_eq!(
                    digest_of_slice(window, &input),
                    scalar_reference(window, &input),
                    "size={size} window={window}"
                );
            }
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_across_window_zero() {
        // Window `0` is the degenerate no-eviction mode — every byte
        // contributes to the state. Verify the effective-slice
        // truncation in the vector kernels short-circuits correctly
        // to "keep the whole slice" in that case.
        for &size in &[0usize, 1, 7, 8, 9, 63, 64, 65, 128, 1024] {
            let input: alloc::vec::Vec<u8> =
                (0..size).map(|i| (i as u8).wrapping_mul(17)).collect();
            assert_eq!(
                digest_of_slice(0, &input),
                scalar_reference(0, &input),
                "window=0 size={size}"
            );
        }
    }
}
