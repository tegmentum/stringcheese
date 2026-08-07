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
//! * `x86_avx2` — AVX2-gated, compiled only on `x86_64`.
//! * `x86_sse2` — SSE2-gated, compiled only on `x86_64`.
//! * `aarch64_neon` — NEON-gated, compiled only on `aarch64`.
//!
//! The three arch-specific backends currently share the scalar
//! implementation under a `#[target_feature(enable = "...")]` context;
//! this puts the runtime-dispatch scaffolding in place and lets the
//! compiler use the enabled ISA for its own auto-vectorization of the
//! window-scan inner loop. A true wide-block Jaro window scan using
//! 128-bit / 256-bit byte-lane compares (SSE2 `_mm_cmpeq_epi8`, AVX2
//! `_mm256_cmpeq_epi8`, NEON `vceqq_u8`) with a packed-bitmap mask and
//! `movemask` / `vaddvq_u8` reduction is documented as follow-up work —
//! landing it does not require any API change.
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

mod common;
pub mod scalar;

#[cfg(target_arch = "x86_64")]
pub mod x86_avx2;
#[cfg(target_arch = "x86_64")]
pub mod x86_sse2;

#[cfg(target_arch = "aarch64")]
pub mod aarch64_neon;

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
