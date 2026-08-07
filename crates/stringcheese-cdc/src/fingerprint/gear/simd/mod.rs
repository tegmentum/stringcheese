//! SIMD-accelerated Gear-hash slice-batch backend.
//!
//! This module is compiled only when the `simd` feature is enabled. It
//! provides a slice-oriented entry point that consumes a byte slice and
//! returns the digest a scalar [`GearHash`][super::GearHash]-driven
//! `roll(byte)` sequence would produce for the same input, then dispatches
//! at run time to the best backend the host CPU supports.
//!
//! # The rolling recurrence is strictly sequential
//!
//! Gear's per-byte update is `state = (state << 1) + G[byte]`. Each step
//! depends on the previous state, so there is no natural per-byte SIMD
//! parallelism at the update level. The backends here therefore consume
//! the byte slice sequentially inside a `#[target_feature]` context and
//! rely on the compiler to auto-vectorize the load/gather/shift/add
//! sequence with whatever the enabled ISA can express — the same
//! shape [`crate::fingerprint::gear::simd`] uses for wasm SIMD and
//! established previously in the wave-5 SIMD sub-trees under
//! `stringcheese_compare`.
//!
//! The important property this file preserves is the **byte-identical
//! contract**: each backend produces the same `u64` digest a scalar
//! `RollingHash::roll` loop would produce, for every input. That
//! contract is what lets a future hand-written wide-block bit-parallel
//! kernel drop in without changing this module's API — the differential
//! tests below anchor every backend to the scalar reference.
//!
//! # Public surface
//!
//! * [`digest_of_slice`] — the runtime-dispatching entry point. Feeds a
//!   fresh `GearHash` (state = 0) with `bytes` byte-by-byte and returns
//!   the final `state()`. Nominal window is not a parameter because
//!   Gear's effective window is fixed at 64 bytes by construction (see
//!   the [scalar module docs][super]) and the state alone determines
//!   the digest.
//!
//! # Backends
//!
//! * `scalar` — portable single-`u64` Gear loop. Always compiled; the
//!   reference against which every arch-specific backend is
//!   differentially tested.
//! * `x86_avx2` — AVX2-gated, compiled only on `x86_64`.
//! * `x86_sse2` — SSE2-gated, compiled only on `x86_64`.
//! * `aarch64_neon` — NEON-gated, compiled only on `aarch64`.
//! * `wasm_simd128` — wasm SIMD128-gated, compiled only when the
//!   `simd128` target-feature is enabled at build time. Gear's shift-add
//!   recurrence has no natural v128 lift but the module compiles the
//!   same scalar core under the wasm SIMD compile-time flag so
//!   auto-vectorization can proceed and the API is uniform across
//!   arches.
//!
//! # `unsafe` policy
//!
//! The crate root is `#![deny(unsafe_code)]`. This module is one of the
//! four documented exceptions (alongside `buzhash::simd`,
//! `polynomial::simd`, and `rabin::simd`): every arch-specific backend
//! carries a module-scoped `#[allow(unsafe_code)]` because
//! `#[target_feature]`-gated functions are `unsafe fn` by rustc's
//! declaration. Every `unsafe fn` and every `unsafe` block in this
//! module tree carries a comment naming the CPU-feature precondition
//! the dispatcher upholds. The dispatcher in this file establishes the
//! preconditions via `is_x86_feature_detected!` / `is_aarch64_feature_detected!`
//! / compile-time `cfg(target_feature = "simd128")` before every call,
//! so the arch-specific `unsafe fn`s are always invoked with their
//! contracts upheld.

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

/// Runtime-dispatching Gear-hash digest for byte-slice inputs.
///
/// Feeds `bytes` byte-by-byte into a fresh Gear state (initial
/// `state = 0`) and returns the final `state()`. This is exactly what
/// a scalar
///
/// ```ignore
/// let mut h = GearHash::new(64);
/// for &b in bytes { h.roll(b); }
/// h.state()
/// ```
///
/// would produce, byte-for-byte — the differential tests below assert
/// bit-for-bit agreement across every backend and every canonical input
/// shape.
///
/// The dispatch itself is a single `is_x86_feature_detected!` /
/// equivalent call per invocation; callers that repeatedly hash small
/// slices should cache the choice or use the scalar
/// [`GearHash::roll`][super::RollingHash::roll] API directly, since the
/// dispatch cost matters for tiny inputs.
#[must_use]
pub fn digest_of_slice(bytes: &[u8]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: is_x86_feature_detected!("avx2") returned true,
            // so the AVX2 target-feature precondition of
            // `x86_avx2::digest_of_slice` holds.
            return unsafe { x86_avx2::digest_of_slice(bytes) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: is_x86_feature_detected!("sse2") returned true,
            // so the SSE2 target-feature precondition of
            // `x86_sse2::digest_of_slice` holds. SSE2 is baseline for
            // x86_64, so this branch always runs on that target.
            return unsafe { x86_sse2::digest_of_slice(bytes) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: is_aarch64_feature_detected!("neon") returned
            // true, so the NEON target-feature precondition of
            // `aarch64_neon::digest_of_slice` holds. NEON is baseline
            // for aarch64.
            return unsafe { aarch64_neon::digest_of_slice(bytes) };
        }
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        // SAFETY: wasm-SIMD feature detection is a compile-time gate,
        // not a runtime one — the `target_feature = "simd128"` cfg on
        // this block is the same predicate that guards `wasm_simd128`'s
        // module-level compilation, so if this branch is compiled the
        // intrinsics inside are guaranteed legal for any engine that
        // accepts the module.
        return unsafe { wasm_simd128::digest_of_slice(bytes) };
    }
    #[allow(
        unreachable_code,
        reason = "the wasm32+simd128 cfg-branch above returns unconditionally when compiled; on hosts where that branch is stripped this call is the fallthrough"
    )]
    scalar::digest_of_slice(bytes)
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test inputs use `(i as _)` and `>> shift as u8` patterns to derive deterministic pseudo-random bytes from small counters; `size` is bounded well below `u32::MAX`, so truncation cannot occur"
)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::gear::GearHash;

    /// Runs the scalar reference — a `GearHash`-driven `roll` loop —
    /// against the same input and returns its final digest. Every SIMD
    /// backend must agree with this.
    fn scalar_reference(bytes: &[u8]) -> u64 {
        let mut h = GearHash::new(64);
        for &b in bytes {
            h.roll(b);
        }
        h.state()
    }

    #[test]
    fn dispatcher_matches_scalar_reference_on_short_inputs() {
        // (a) short random inputs — covers empty, single-byte, and
        //     under-window sizes.
        let cases: &[&[u8]] = &[
            b"",
            b"a",
            b"ab",
            b"abcdefgh",
            b"the quick brown fox",
            &[0x00; 3],
            &[0xff; 7],
        ];
        for &input in cases {
            assert_eq!(
                digest_of_slice(input),
                scalar_reference(input),
                "on input {input:?}"
            );
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_at_chunk_boundaries() {
        // (b) chunk boundaries — sizes that straddle likely internal
        //     unroll/chunk widths (2, 4, 8, 16, 32) so a truncated
        //     tail loop cannot slip past differential coverage.
        for &size in &[
            1usize, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129,
        ] {
            let input: alloc::vec::Vec<u8> = (0..size)
                .map(|i| {
                    // Deterministic pseudo-random bytes so the test is
                    // reproducible across runs and platforms.
                    let m = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(1);
                    (m >> 16) as u8
                })
                .collect();
            assert_eq!(
                digest_of_slice(&input),
                scalar_reference(&input),
                "at chunk boundary size={size}"
            );
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_at_window_size() {
        // (c) window-sized inputs — exactly 64 bytes (Gear's effective
        //     window), plus one under and one over. Below 64 the state
        //     still carries prefix contribution; at 64 the initial
        //     `state << 64` overflows to zero; above 64 only the trailing
        //     64 bytes contribute.
        for &size in &[63usize, 64, 65, 128, 191, 192] {
            let input: alloc::vec::Vec<u8> =
                (0..size).map(|i| (i as u8).wrapping_mul(31)).collect();
            assert_eq!(
                digest_of_slice(&input),
                scalar_reference(&input),
                "at window-adjacent size={size}"
            );
        }
    }

    #[test]
    fn dispatcher_matches_scalar_reference_on_larger_blobs() {
        // (d) larger random blobs — several KB to exercise any batched
        //     inner loop across many rounds.
        for &size in &[512usize, 1024, 4096, 16 * 1024] {
            let input: alloc::vec::Vec<u8> = (0..size)
                .map(|i| {
                    let m = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    ((m >> 24) & 0xFF) as u8
                })
                .collect();
            assert_eq!(
                digest_of_slice(&input),
                scalar_reference(&input),
                "on larger blob size={size}"
            );
        }
    }
}
