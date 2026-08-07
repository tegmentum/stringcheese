//! SIMD-accelerated OSA (restricted Damerau-Levenshtein) backend.
//!
//! This module is compiled only when the `simd` feature is enabled. It
//! provides a byte-slice-oriented OSA kernel and dispatches at run time to
//! the best backend the host CPU supports.
//!
//! # Full unrestricted Damerau stays scalar
//!
//! Only OSA (`Osa`) gets a SIMD sub-tree — the full unrestricted
//! Damerau-Levenshtein kernel at [`crate::damerau::damerau`] uses a
//! HashMap-backed algorithm whose transposition-source lookup does not
//! vectorize under the Myers bit-parallel pattern. Adding SIMD to full
//! Damerau is deferred; see the module doc on
//! [`crate::damerau::damerau::production`] for the reasoning.
//!
//! # Public surface
//!
//! Callers interact with this backend through two entry points:
//!
//! * `distance` — the runtime-dispatching entry point that picks the
//!   best available backend for the host and delegates to it. This is what
//!   [`crate::damerau::Osa::distance_bytes_with_workspace`] calls when the
//!   `simd` feature is on and `is_byte_amenable_for_osa` is satisfied.
//! * `is_byte_amenable_for_osa` — the guard used by the public API to
//!   decide whether an input pair is a good fit for the SIMD backend.
//!   Very short inputs stay on the generic scalar rolling-rows path
//!   because the setup cost of the SIMD-shaped kernel dominates the tiny
//!   inner loop; unicode-scalar callers never enter this module because
//!   the byte-slice entry point is `&[u8]`-only.
//!
//! # Backends
//!
//! * `scalar` — portable SIMD-shaped scalar OSA (three-row rolling DP,
//!   self-contained heap buffer). Always compiled; the reference against
//!   which every arch-specific backend is differentially tested.
//! * `x86_avx2` — AVX2-gated, compiled only on `x86_64`.
//! * `x86_sse2` — SSE2-gated, compiled only on `x86_64`.
//! * `aarch64_neon` — NEON-gated, compiled only on `aarch64`.
//!
//! The three arch-specific backends currently share the scalar
//! implementation under a `#[target_feature(enable = "...")]` context;
//! this puts the runtime-dispatch scaffolding in place and lets the
//! compiler use the enabled ISA for its own auto-vectorization of the
//! rolling-rows inner loop. A true bit-parallel OSA in the shape of
//! Hyyrö (2003) is documented as follow-up work — landing it does not
//! require any API change.
//!
//! # `unsafe` policy
//!
//! The compare crate uses `#![deny(unsafe_code)]` at its root. This module
//! is one of the documented exceptions (alongside [`crate::levenshtein::simd`]
//! and [`crate::jaro::simd`]): every arch-specific backend carries a
//! module-scoped `#[allow(unsafe_code)]` because `#[target_feature]`-gated
//! functions are `unsafe fn` by rustc's declaration. Every `unsafe fn`
//! and every `unsafe` block in this module tree carries a comment naming
//! the safety precondition. The dispatcher in this file establishes the
//! CPU-feature preconditions before every call, so the arch-specific
//! `unsafe fn`s are always invoked with their contracts upheld.

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

/// Minimum input length below which the SIMD backend is not worth the
/// setup cost.
///
/// The self-contained three-row rolling-buffer allocation plus the
/// runtime-feature dispatch cost more than the direct scalar generic
/// rolling-rows kernel on tiny inputs. 32 bytes is the same crossover
/// Levenshtein and Jaro use; the three DPs have similar per-call setup
/// overheads, so a single tuned value is enough for all of them.
const OSA_MIN_LEN: usize = 32;

/// Returns `true` iff the input pair is a good candidate for the SIMD
/// OSA backend.
///
/// The current criterion is a length threshold: both inputs are byte-oriented
/// by construction (the caller is on the `&[u8]` API entry point), and the
/// shorter side must be at least `OSA_MIN_LEN` (32) bytes long for the
/// SIMD-shaped kernel's setup cost to be worthwhile. The underlying
/// comparison is delegated to [`crate::simd_dispatch::is_byte_amenable`]
/// so every SIMD sub-tree in this crate shares the same viability shape.
///
/// The current threshold is 32 bytes on the shorter side; the constant is
/// private (see the module source) because the value is not part of the
/// stable API and may be re-tuned as benchmarks evolve.
#[inline]
#[must_use]
pub fn is_byte_amenable_for_osa(a: &[u8], b: &[u8]) -> bool {
    crate::simd_dispatch::is_byte_amenable(a, b, OSA_MIN_LEN)
}

/// Runtime-dispatching OSA distance for byte-slice inputs.
///
/// Picks the best backend for the host CPU and delegates. The dispatch
/// itself is a single `is_x86_feature_detected!` / equivalent call per
/// invocation; if that cost matters, callers should cache the choice
/// (StringCheese does not currently expose a cached-dispatcher wrapper
/// because criterion measurements show the overhead is negligible for
/// any input above the length guard used by
/// [`is_byte_amenable_for_osa`]).
///
/// The result is bit-for-bit identical to what
/// [`crate::damerau::osa::rolling_rows::distance_rolling_rows_with_workspace`]
/// would produce on the same inputs — this is asserted by the differential
/// tests below and by the SIMD-specific property tests in
/// `damerau/simd_property_tests.rs`.
#[must_use]
pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: is_x86_feature_detected!("avx2") returned true,
            // so the AVX2 target-feature precondition of
            // `x86_avx2::distance` holds.
            return unsafe { x86_avx2::distance(a, b) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: is_x86_feature_detected!("sse2") returned true,
            // so the SSE2 target-feature precondition of
            // `x86_sse2::distance` holds. SSE2 is baseline for x86_64,
            // so this branch always runs on that target.
            return unsafe { x86_sse2::distance(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: is_aarch64_feature_detected!("neon") returned
            // true, so the NEON target-feature precondition of
            // `aarch64_neon::distance` holds. NEON is baseline for
            // aarch64.
            return unsafe { aarch64_neon::distance(a, b) };
        }
    }
    scalar::distance(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damerau::osa::full_matrix::distance_full_matrix;

    /// Every canonical pair, run through every backend the host CPU has,
    /// must agree with the full-matrix oracle.
    #[test]
    fn dispatcher_matches_oracle_on_canonical_pairs() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"", b"hello"),
            (b"hello", b""),
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"distance", b"difference"),
            (b"aaaaaaa", b"aaaaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
            (b"abcd", b"badc"),
            // A pair long enough to actually meet is_byte_amenable_for_osa.
            (
                b"the quick brown fox jumps over the lazy dog",
                b"the quikc brown fox leaps over the lazy dog",
            ),
        ];
        for (a, b) in cases {
            assert_eq!(
                distance(a, b),
                distance_full_matrix(a, b),
                "simd::distance disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn scalar_matches_full_matrix_on_canonical_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            let simd = scalar::distance(a, b);
            let oracle = distance_full_matrix(a, b);
            assert_eq!(simd, oracle, "on ({a:?}, {b:?})");
        }
    }

    #[test]
    fn is_byte_amenable_for_osa_rejects_short_inputs() {
        assert!(!is_byte_amenable_for_osa(b"", b""));
        assert!(!is_byte_amenable_for_osa(b"kitten", b"sitting"));
        // 32 bytes on both sides is exactly the threshold.
        let long = &b"abcdefghijklmnopqrstuvwxyz012345"[..];
        assert_eq!(long.len(), 32);
        assert!(is_byte_amenable_for_osa(long, long));
    }
}
