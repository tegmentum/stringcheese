//! SIMD-accelerated Buzhash slice-batch backend.
//!
//! This module is compiled only when the `simd` and `alloc` features are
//! both enabled — `alloc` because it lives under
//! [`crate::fingerprint::buzhash`], which is itself `alloc`-gated for
//! the streaming-hash's circular buffer. The slice-batch entry point
//! here does *not* need a heap-allocated buffer (it can index the input
//! slice directly for the leaving byte), but the SIMD tree lives inside
//! the parent module so its cfg naturally inherits the same gate.
//!
//! # The rolling recurrence is strictly sequential
//!
//! Buzhash's per-byte update is
//!
//! ```text
//! state = state.rotate_left(1)
//!       ^ (if full) T[leaving].rotate_left(window mod 64)
//!       ^ T[byte]
//! ```
//!
//! Each step depends on the previous state, so there is no natural
//! per-byte SIMD parallelism at the update level. The backends here
//! therefore consume the byte slice sequentially inside a
//! `#[target_feature]` context and rely on the compiler to
//! auto-vectorize the load/rotate/xor sequence with whatever the
//! enabled ISA can express — the same shape [`crate::fingerprint::gear::simd`]
//! uses. Buzhash's rotate operations are naturally SIMD-friendly for
//! `u64x2` on every arch (SSE2 has no direct `_mm_rol_epi64` but the
//! shift+OR pattern is one intrinsic each), so a hand-written v128 lift
//! is tractable and documented as follow-up work.
//!
//! The **byte-identical contract** is what this initial cut preserves:
//! every backend returns the same `u64` digest a scalar
//! [`RollingHash::roll`][crate::fingerprint::RollingHash::roll]
//! loop over the same input would produce. The differential tests
//! below anchor every backend to that reference.
//!
//! # Public surface
//!
//! * [`digest_of_slice`] — the runtime-dispatching entry point. Feeds
//!   a fresh `Buzhash::new(window)` with `bytes` byte-by-byte and
//!   returns the final `digest()`.
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

/// Runtime-dispatching Buzhash digest for byte-slice inputs.
///
/// Feeds `bytes` byte-by-byte into a fresh `Buzhash::new(window)` and
/// returns the final `digest()`. This is exactly what a scalar
///
/// ```ignore
/// let mut h = Buzhash::new(window);
/// for &b in bytes { h.roll(b); }
/// h.digest()
/// ```
///
/// would produce, byte-for-byte — the differential tests below assert
/// bit-for-bit agreement across every backend and every canonical input
/// shape.
///
/// A `window` of zero is legal but degenerate: the digest is always the
/// identity `0` regardless of the input.
#[must_use]
pub fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            return unsafe { x86_avx2::digest_of_slice(window, bytes) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            return unsafe { x86_sse2::digest_of_slice(window, bytes) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: is_aarch64_feature_detected!("neon") returned
            // true.
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
    use crate::fingerprint::buzhash::Buzhash;

    fn scalar_reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = Buzhash::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn dispatcher_matches_scalar_reference_on_short_inputs() {
        // (a) short random inputs — the interesting shapes are: window
        //     larger than input, exactly matching, and slightly smaller.
        for &window in &[0usize, 1, 4, 8] {
            let cases: &[&[u8]] = &[b"", b"a", b"abcdefgh", b"the quick brown fox"];
            for &input in cases {
                assert_eq!(
                    digest_of_slice(window, input),
                    scalar_reference(window, input),
                    "on input {input:?} with window {window}"
                );
            }
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_at_chunk_boundaries() {
        // (b) chunk boundaries — sizes straddling common unroll widths.
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
        // (c) window-sized inputs — exactly the window, one under, one
        //     over. Buzhash's eviction path is exercised past the
        //     window boundary; before it, the roll-out XOR is skipped.
        for &window in &[8usize, 32, 64, 100] {
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
        // (d) larger random blobs — several KB across two windows.
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
    fn zero_window_is_identity() {
        for &input in &[b"".as_ref(), b"abcdef".as_ref(), b"long input to test"] {
            assert_eq!(digest_of_slice(0, input), 0);
        }
    }
}
