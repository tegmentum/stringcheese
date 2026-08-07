//! SIMD-accelerated polynomial-hash slice-batch backend.
//!
//! Compiled only when both `simd` and `alloc` are enabled — `alloc`
//! because it lives under [`crate::fingerprint::polynomial`], which is
//! itself `alloc`-gated for the streaming hash's circular buffer.
//!
//! # Real vectorized kernel via block reformulation
//!
//! The polynomial hash's per-byte recurrence
//!
//! ```text
//! state = (state * BASE + byte - leaving * BASE^window) mod PRIME
//! ```
//!
//! is strictly sequential — each step depends on the previous via the
//! `state * BASE` scale. Unrolled over any `k`-byte run from
//! `state_0`, however, it collapses into the closed form
//!
//! ```text
//! state_k = state_0 * BASE^k + Σ_{i=0..k}  bytes[i] * BASE^(k-1-i)  (mod PRIME)
//! ```
//!
//! and the streaming hash's window-eviction identity guarantees the
//! digest after feeding `L >= window` bytes depends only on the last
//! `window` bytes — the same digest is reproduced by feeding just that
//! tail into a fresh `state = 0`. See the sibling
//! [`crate::fingerprint::rabin::simd`] backend for the effective-slice
//! truncation derivation shared with this kernel.
//!
//! Once the effective slice is chosen, the backends fold in
//! `BLOCK_LEN = 16`-byte blocks: the running-state scale becomes a
//! single per-block `state * PK_BLOCK` where `PK_BLOCK = BASE^16 mod
//! PRIME`, and the trailing sum vectorizes across lanes. Each
//! per-byte coefficient `pk = BASE^(15-i) mod PRIME` fits in 61 bits
//! — too wide for a straight 32-bit lane multiply — so the scalar
//! reference precomputes it split into `pk_hi = pk >> 32` (≤ 29 bits)
//! and `pk_lo = pk & 0xFFFF_FFFF` (32 bits). A byte × coefficient
//! product then expresses as two independent 32×32 → 64 multiplies,
//! both fitting in a u64 lane, that every arch backend issues through
//! its native widening multiply intrinsic:
//!
//! * `x86_avx2` — 4-lane `_mm256_mul_epu32` (`VPMULUDQ`), one AVX2
//!   fused multiply-accumulate pair per 4 bytes. AVX2 is baseline for
//!   the widest x86 dispatch here; `avx512ifma` (`_mm256_madd52lo_epu64`)
//!   would collapse the split into a single fused instruction and is
//!   listed as deferred future work in the AVX2 file's docs.
//! * `x86_sse2` — 2-lane `_mm_mul_epu32` (`PMULUDQ`), the SSE2
//!   baseline sibling. No runtime sub-feature detection is required
//!   because `PMULUDQ` is part of SSE2 itself.
//! * `aarch64_neon` — 2-lane `vmull_u32`, the NEON widening 32×32 →
//!   64 multiply. NEON is baseline for `aarch64`, so no
//!   sub-feature dispatch is needed.
//! * `wasm_simd128` — 2-lane `i64x2_mul`; wasm SIMD128 does not
//!   surface a dedicated 32×32 → 64 widening multiply, but with both
//!   inputs bounded to 32 bits the low-64 result of `i64x2_mul` is
//!   bit-identical to the widening product — see the file's docs for
//!   the bound derivation.
//!
//! # Byte-identical contract
//!
//! Every backend returns the same `u64` digest a scalar
//! [`RollingHash::roll`][crate::fingerprint::RollingHash::roll] loop
//! over the same input would produce. The differential tests below
//! and inside each arch file anchor every backend to that reference
//! across short inputs, block boundaries (15/16/17/31/32/33/63/64/65/
//! 127/128/129), window-sized inputs, larger blobs, and the
//! degenerate `window = 0` no-eviction mode.
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

    #[test]
    fn dispatcher_matches_scalar_reference_across_window_zero() {
        // Window `0` is the degenerate no-eviction mode — every byte
        // contributes to the state. Verify the effective-slice
        // truncation in the vector kernels short-circuits correctly
        // to "keep the whole slice" in that case.
        for &size in &[0usize, 1, 7, 8, 9, 15, 16, 17, 63, 64, 65, 128, 1024] {
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
