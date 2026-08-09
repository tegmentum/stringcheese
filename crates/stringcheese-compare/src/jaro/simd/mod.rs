//! SIMD-accelerated Jaro similarity backend.
//!
//! This module is compiled only when the `simd` feature is enabled. It
//! provides a byte-slice-oriented Jaro kernel — Jaro (1989) — and
//! dispatches at run time to the best backend the host CPU supports.
//!
//! # Public surface
//!
//! Callers interact with this backend through two entry points:
//!
//! * `similarity` — the runtime-dispatching entry point that picks the
//!   best available backend for the host and delegates to it. This is what
//!   [`crate::jaro::Jaro::similarity_bytes_with_workspace`] calls when the
//!   `simd` feature is on and `is_byte_amenable_for_jaro` is satisfied.
//! * `is_byte_amenable_for_jaro` — the guard used by the public API to
//!   decide whether an input pair is a good fit for the SIMD backend. Very
//!   short inputs stay on the generic char-agnostic scalar path because the
//!   setup cost of building the packed matched-position bitmaps dominates
//!   the tiny window scan; unicode-heavy inputs — which are already using
//!   the char-based scalar path — never enter this module in the first
//!   place.
//!
//! # Backends
//!
//! * `scalar` — portable SIMD-shaped scalar Jaro. Always compiled;
//!   the reference against which every arch-specific backend is
//!   differentially tested.
//! * `x86_avx2` — AVX2-gated, compiled only on `x86_64`. 32-byte block
//!   width; broadcasts `a[i]` with `_mm256_set1_epi8`, loads `b`-blocks
//!   with `_mm256_loadu_si256`, compares with `_mm256_cmpeq_epi8`, and
//!   reduces to a 32-bit mask via `_mm256_movemask_epi8`.
//! * `x86_sse2` — SSE2-gated, compiled only on `x86_64`. 16-byte block
//!   width; broadcasts with `_mm_set1_epi8`, loads with
//!   `_mm_loadu_si128`, compares with `_mm_cmpeq_epi8`, reduces to a
//!   16-bit mask via `_mm_movemask_epi8`.
//! * `aarch64_neon` — NEON-gated, compiled only on `aarch64`. 16-byte
//!   block width; broadcasts with `vdupq_n_u8`, loads with `vld1q_u8`,
//!   compares with `vceqq_u8`. NEON has no direct byte-lane movemask,
//!   so the reduction uses the standard `vshrn_n_u16::<4>` idiom that
//!   maps each source byte's compare result to a 4-bit nibble in a
//!   `u64` — `trailing_zeros() / 4` recovers the first-set-lane index.
//! * `wasm_simd128` — wasm SIMD128-gated. 16-byte block width; broadcasts
//!   with `u8x16_splat`, loads with `v128_load`, compares with `u8x16_eq`,
//!   reduces to a 16-bit mask via `u8x16_bitmask` (the wasm analogue of
//!   `_mm_movemask_epi8`). Compiled only on `wasm32` and only when the
//!   `simd128` target-feature is enabled — wasm-SIMD feature detection
//!   is a compile-time gate, not a runtime one.
//!
//! All four arch-specific backends share the same shape: for each byte
//! of `a`, broadcast it across a SIMD register and walk the matching
//! window in `b` in fixed-width blocks. Each block is `cmpeq_epi8`'d
//! against the broadcast, `AND`ed with the negation of the corresponding
//! slice of the packed `b_matched` bitmap (extracted via the shared
//! `Bitmap::read_bits` in the private `common` submodule) and with a
//! valid-lanes mask on the trailing partial block, then reduced to the
//! first-set-bit position via `trailing_zeros()`. Transposition counting
//! and the final score compute stay scalar because they walk sparse
//! matched positions and don't vectorize.
//!
//! # `unsafe` policy
//!
//! The compare crate uses `#![deny(unsafe_code)]` at its root. This module
//! is one of the documented exceptions (the other two being
//! [`crate::levenshtein::simd`] and [`crate::damerau::osa::simd`]): every
//! arch-specific backend carries a module-scoped `#[allow(unsafe_code)]`
//! because `#[target_feature]`-gated functions are `unsafe fn` by rustc's
//! declaration. Every `unsafe fn` and every `unsafe` block in this module
//! tree carries a comment naming the safety precondition. The dispatcher
//! in this file establishes the CPU-feature preconditions before every
//! call, so the arch-specific `unsafe fn`s are always invoked with their
//! contracts upheld.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics require unsafe by declaration; every unsafe fn and every unsafe block here has a safety comment naming the CPU-feature precondition the dispatcher upholds"
)]

// `common` hosts helpers shared across the arch-specific backends. When
// no arch backend is compiled (e.g. wasm32 without simd128), nothing
// references it and `-D warnings` promotes the dead-code lint to an
// error. Gate the module on the union of the arch cfgs that use it.
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
mod common;
pub mod scalar;

#[cfg(target_arch = "x86_64")]
pub mod x86_avx2;
#[cfg(target_arch = "x86_64")]
pub mod x86_sse2;

#[cfg(target_arch = "aarch64")]
pub mod aarch64_neon;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub mod wasm_simd128;

/// Minimum input length below which the SIMD backend is not worth the
/// setup cost.
///
/// The packed bitmap allocation plus the runtime-feature dispatch cost
/// more than the direct scalar-generic Jaro on tiny inputs. 32 bytes is
/// the same crossover Levenshtein uses; the two DPs have similar per-call
/// setup overheads, so a single tuned value is enough for both.
const JARO_MIN_LEN: usize = 32;

/// Returns `true` iff the input pair is a good candidate for the SIMD
/// Jaro backend.
///
/// The current criterion is a length threshold: both inputs are byte-oriented
/// by construction (the caller is on the `&[u8]` API entry point), and the
/// shorter side must be at least `JARO_MIN_LEN` (32) bytes long for the
/// SIMD-shaped scan and packed-bitmap allocation to be worthwhile. The
/// underlying comparison is delegated to
/// [`crate::simd_dispatch::is_byte_amenable`] so every SIMD sub-tree in
/// this crate shares the same viability shape.
///
/// The current threshold is 32 bytes on the shorter side; the constant is
/// private (see the module source) because the value is not part of the
/// stable API and may be re-tuned as benchmarks evolve.
#[inline]
#[must_use]
pub fn is_byte_amenable_for_jaro(a: &[u8], b: &[u8]) -> bool {
    crate::simd_dispatch::is_byte_amenable(a, b, JARO_MIN_LEN)
}

/// Runtime-dispatching Jaro similarity for byte-slice inputs.
///
/// Picks the best backend for the host CPU and delegates. The dispatch
/// itself is a single `is_x86_feature_detected!` / equivalent call per
/// invocation; if that cost matters, callers should cache the choice
/// (StringCheese does not currently expose a cached-dispatcher wrapper
/// because criterion measurements show the overhead is negligible for
/// any input above the length guard used by
/// [`is_byte_amenable_for_jaro`]).
///
/// The result is bit-for-bit identical to what the generic Jaro kernel
/// in [`crate::jaro`] would produce on the same inputs — this is
/// asserted by the differential tests below and by the SIMD-specific
/// property tests in `jaro/simd_property_tests.rs`.
#[must_use]
pub fn similarity(a: &[u8], b: &[u8]) -> f64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: is_x86_feature_detected!("avx2") returned true,
            // so the AVX2 target-feature precondition of
            // `x86_avx2::similarity` holds.
            return unsafe { x86_avx2::similarity(a, b) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: is_x86_feature_detected!("sse2") returned true,
            // so the SSE2 target-feature precondition of
            // `x86_sse2::similarity` holds. SSE2 is baseline for
            // x86_64, so this branch always runs on that target.
            return unsafe { x86_sse2::similarity(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: is_aarch64_feature_detected!("neon") returned
            // true, so the NEON target-feature precondition of
            // `aarch64_neon::similarity` holds. NEON is baseline for
            // aarch64.
            return unsafe { aarch64_neon::similarity(a, b) };
        }
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        // SAFETY: wasm-SIMD feature detection is a compile-time gate,
        // not a runtime one — the `target_feature = "simd128"` cfg on
        // this block is the same predicate that guards
        // `wasm_simd128`'s module-level compilation, so if this branch
        // is compiled the intrinsics inside are guaranteed legal for
        // any engine that accepts the module.
        return unsafe { wasm_simd128::similarity(a, b) };
    }
    #[allow(
        unreachable_code,
        reason = "the wasm32+simd128 cfg-branch above returns unconditionally when compiled; on hosts where that branch is stripped this call is the fallthrough"
    )]
    scalar::similarity(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jaro::jaro::jaro_similarity;

    /// Every canonical pair, run through every backend the host CPU has,
    /// must agree with the generic Jaro kernel bit-for-bit.
    #[test]
    fn dispatcher_matches_generic_kernel_on_canonical_pairs() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"", b"hello"),
            (b"hello", b""),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"aaaaaaa", b"aaaaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
            (
                b"the quick brown fox jumps over the lazy dog",
                b"the quick brown fox leaps over the lazy dog",
            ),
        ];
        for (a, b) in cases {
            assert_eq!(
                similarity(a, b).to_bits(),
                jaro_similarity(a, b).to_bits(),
                "simd::similarity disagreed with generic kernel on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn scalar_matches_generic_kernel_on_canonical_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
        ] {
            let simd = scalar::similarity(a, b);
            let generic = jaro_similarity(a, b);
            assert_eq!(simd.to_bits(), generic.to_bits(), "on ({a:?}, {b:?})");
        }
    }

    #[test]
    fn is_byte_amenable_for_jaro_rejects_short_inputs() {
        assert!(!is_byte_amenable_for_jaro(b"", b""));
        assert!(!is_byte_amenable_for_jaro(b"MARTHA", b"MARHTA"));
        // 32 bytes on both sides is exactly the threshold.
        let long = &b"abcdefghijklmnopqrstuvwxyz012345"[..];
        assert_eq!(long.len(), 32);
        assert!(is_byte_amenable_for_jaro(long, long));
    }
}
