//! SIMD-accelerated polynomial-hash slice-batch backend.
//!
//! Compiled only when both `simd` and `alloc` are enabled — `alloc`
//! because it lives under [`crate::fingerprint::polynomial`], which is
//! itself `alloc`-gated for the streaming hash's circular buffer.
//!
//! # The rolling recurrence is strictly sequential
//!
//! The polynomial hash's per-byte update is
//!
//! ```text
//! state = (state * BASE + byte - leaving * BASE^window) mod PRIME
//! ```
//!
//! Each step depends on the previous state via the multiplication, and
//! the modular reduction on `u128` intermediate products has no natural
//! per-byte SIMD form on baseline SSE2/NEON/wasm-SIMD. Baseline vector
//! ISAs do not surface a 64×64 → 128 multiply lane primitive (AVX-512
//! IFMA is the first ISA that does, and requires runtime detection that
//! is out of scope for this initial cut). This backend therefore
//! consumes the byte slice sequentially inside a `#[target_feature]`
//! context — the same shape [`crate::fingerprint::gear::simd`] uses.
//!
//! The **byte-identical contract** is what this initial cut preserves:
//! every backend returns the same `u64` digest a scalar
//! [`RollingHash::roll`][crate::fingerprint::RollingHash::roll] loop
//! over the same input would produce. The differential tests below
//! anchor every backend to that reference.
//!
//! # Public surface
//!
//! * [`digest_of_slice`] — the runtime-dispatching entry point. Feeds
//!   a fresh `PolynomialHash::new(window)` with `bytes` byte-by-byte
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

/// Runtime-dispatching polynomial-hash digest for byte-slice inputs.
///
/// Feeds `bytes` byte-by-byte into a fresh
/// `PolynomialHash::new(window)` and returns the final `digest()`.
/// Byte-for-byte identical to a scalar
/// [`RollingHash::roll`][crate::fingerprint::RollingHash::roll] loop
/// over the same input.
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
    use crate::fingerprint::polynomial::PolynomialHash;

    fn scalar_reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = PolynomialHash::new(window);
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
}
