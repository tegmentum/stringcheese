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
//! # The rolling recurrence is sequential — but a 64-byte block is not
//!
//! Buzhash's per-byte update is
//!
//! ```text
//! state = state.rotate_left(1)
//!       ^ (if full) T[leaving].rotate_left(window mod 64)
//!       ^ T[byte]
//! ```
//!
//! Folding the two table terms into a single per-index contribution
//!
//! ```text
//! contrib_i = T[bytes[i]] ^ (if i >= window then T[bytes[i-window]].rotate_left(window mod 64) else 0)
//! ```
//!
//! makes each step `state = state.rotate_left(1) ^ contrib_i`. Unrolled
//! over `k` bytes, the recurrence collapses to the closed form
//!
//! ```text
//! state_k = state_0.rotate_left(k)
//!         ^ Σ_{i=0..k}  contrib_i.rotate_left(k-1-i)
//! ```
//!
//! At `k = 64` — the `u64` rotate cycle length — `state_0.rotate_left(64)`
//! reduces to `state_0` unchanged. Each 64-byte block therefore folds
//! into the running state as a simple XOR:
//!
//! ```text
//! state_after_block = state_before_block ^ blockhash
//! blockhash = Σ_{i=0..64} contrib_i.rotate_left(63 - i)
//! ```
//!
//! The block sum has a natural Horner-form SIMD shape — for the
//! AVX2 4-lane kernel, groups of 4 bytes:
//!
//! ```text
//! acc = acc.rotate_left(4) ^ [ ROL(c[4k+0], 3), ROL(c[4k+1], 2),
//!                              ROL(c[4k+2], 1), ROL(c[4k+3], 0) ]
//! ```
//!
//! and for the NEON / wasm-SIMD 2-lane kernels, groups of 2 bytes:
//!
//! ```text
//! acc = acc.rotate_left(2) ^ [ ROL(c[2k+0], 1), ROL(c[2k+1], 0) ]
//! ```
//!
//! XOR-reducing the two/four `u64` lanes at the end reproduces the
//! block sum bit-for-bit. Below 64 bytes the kernel defers to the
//! scalar loop (there is no prior state to preserve through a 64-bit
//! rotate cycle), and any `len % 64` tail after the last full block is
//! consumed by the same scalar recurrence — both share the
//! `state = state.rotate_left(1) ^ contrib_i` core that anchors the
//! differential contract.
//!
//! # Public surface
//!
//! * [`digest_of_slice`] — the runtime-dispatching entry point. Feeds
//!   a fresh `Buzhash::new(window)` with `bytes` byte-by-byte and
//!   returns the final `digest()`.
//!
//! # Backends
//!
//! * `scalar` — portable single-`u64` Buzhash loop. Always compiled;
//!   the reference against which every arch-specific backend is
//!   differentially tested.
//! * `x86_avx2` — AVX2-gated, compiled only on `x86_64`. Real block-
//!   reformulation kernel over four `u64` lanes: `_mm256_loadu_si256`
//!   for the per-block contribution loads, per-lane variable rotate
//!   via `_mm256_sllv_epi64` + `_mm256_srlv_epi64` + `_mm256_or_si256`,
//!   constant 4-bit rotate for the Horner advance, and `_mm256_xor_si256`
//!   for the fold. 16 iterations per 64-byte block.
//! * `x86_sse2` — SSE2-gated, compiled only on `x86_64`. Deliberately
//!   scalar under `target_feature(sse2)`; see that module's docs for
//!   why (no gather; no per-lane variable shift below AVX2). AVX2 is
//!   the wide x86 branch the dispatcher prefers.
//! * `aarch64_neon` — NEON-gated, compiled only on `aarch64`. Real
//!   block-reformulation kernel over two `u64` lanes: `vld1q_u64` for
//!   the block-contribution loads, per-lane variable rotate via
//!   `vshlq_u64` (positive and negative counts for left / logical-right)
//!   OR-combined with `vorrq_u64`, constant 2-bit rotate for the
//!   Horner advance, and `veorq_u64` for the fold. 32 iterations per
//!   64-byte block.
//! * `wasm_simd128` — wasm SIMD128-gated, compiled only when the
//!   `simd128` target-feature is enabled at build time. Real block-
//!   reformulation kernel over two `u64` lanes: the per-lane pre-rotate
//!   `[1, 0]` is factored out into a scalar-side `g0.rotate_left(1)`
//!   pack (wasm's `u64x2_shl` applies a single scalar shift uniformly
//!   to both lanes), and the Horner advance / fold use `u64x2_shl(_, 2)`
//!   / `u64x2_shr(_, 62)` / `v128_or` / `v128_xor`. 32 iterations per
//!   64-byte block.
//!
//! # `unsafe` policy
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
    fn dispatcher_matches_scalar_reference_at_word_bits_boundary() {
        // (c') `WORD_BITS`-adjacent input sizes — Buzhash's block
        //     reformulation exploits the identity `ROL(x, 64) == x` on a
        //     `u64` rotate, so 63 / 64 / 65-byte inputs are the exact
        //     lengths that straddle the block boundary. Every backend
        //     must agree with the scalar reference on all three, for
        //     every window that could put a full block into the
        //     windowed phase.
        for &window in &[1usize, 4, 8, 16, 32, 63, 64, 65, 100] {
            for &size in &[63usize, 64, 65] {
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| {
                        let m = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                        (m >> 24) as u8
                    })
                    .collect();
                assert_eq!(
                    digest_of_slice(window, &input),
                    scalar_reference(window, &input),
                    "at word-bits boundary size={size} window={window}"
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
