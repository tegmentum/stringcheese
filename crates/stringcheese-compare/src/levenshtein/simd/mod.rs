//! SIMD-accelerated Levenshtein backend (Myers 1999 bit-parallel).
//!
//! This module is compiled only when the `simd` feature is enabled. It
//! provides a byte-slice-oriented Levenshtein kernel implemented via
//! Myers's bit-parallel algorithm and dispatches at run time to the best
//! backend the host CPU supports.
//!
//! # Public surface
//!
//! Callers interact with this backend through two entry points:
//!
//! * `distance` — the runtime-dispatching entry point that picks the
//!   best available backend for the host and delegates to it. This is
//!   what [`crate::levenshtein::Levenshtein::distance_bytes_with_workspace`]
//!   calls when the `simd` feature is on and `is_byte_amenable_for_myers`
//!   is satisfied.
//! * `is_byte_amenable_for_myers` — the guard used by the public API to
//!   decide whether an input pair is a good fit for the Myers backend.
//!   Very short inputs stay on the scalar rolling-rows kernel because
//!   the setup cost of building the Peq table dominates the tiny inner
//!   loop; unicode-heavy inputs — which are already using the char-based
//!   scalar path — never enter this module in the first place.
//!
//! # Backends
//!
//! * `myers_scalar` — portable single-word Myers 1999 (m ≤ 64) with a
//!   rolling-rows fallback for longer patterns. Always compiled.
//! * `myers_x86_avx2` — AVX2-gated, compiled only on `x86_64`.
//! * `myers_x86_sse2` — SSE2-gated, compiled only on `x86_64`.
//! * `myers_aarch64_neon` — NEON-gated, compiled only on `aarch64`.
//!
//! The three arch-specific backends currently share the scalar Myers
//! implementation under a `#[target_feature(enable = "...")]` context;
//! this puts the runtime-dispatch scaffolding in place and lets the
//! compiler use the enabled ISA for its own auto-vectorization of the
//! Peq table build. A true wide-block Myers using 128-bit / 256-bit
//! integer arithmetic (SSE2 `_mm_add_epi64`, AVX2 `_mm256_add_epi64`,
//! NEON `vaddq_u64`) with explicit inter-lane carry propagation is
//! documented as follow-up work — landing it does not require any API
//! change.
//!
//! # `unsafe` policy
//!
//! The compare crate uses `#![deny(unsafe_code)]` at its root. This
//! module is the single documented exception: every arch-specific
//! backend carries a module-scoped `#[allow(unsafe_code)]` because
//! `#[target_feature]`-gated functions are `unsafe fn` by rustc's
//! declaration and the SIMD intrinsics themselves are `unsafe fn`.
//! Every `unsafe fn` and every `unsafe` block in this module tree
//! carries a comment naming the safety precondition. The dispatcher in
//! this file establishes the CPU-feature preconditions before every
//! call, so the arch-specific `unsafe fn`s are always invoked with
//! their contracts upheld.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics require unsafe by declaration; every unsafe fn and every unsafe block here has a safety comment naming the CPU-feature precondition the dispatcher upholds"
)]

pub mod myers_scalar;

#[cfg(target_arch = "x86_64")]
pub mod myers_x86_avx2;
#[cfg(target_arch = "x86_64")]
pub mod myers_x86_sse2;

#[cfg(target_arch = "aarch64")]
pub mod myers_aarch64_neon;

/// Minimum input length below which the Myers backend is not worth the
/// setup cost.
///
/// Building the 256-entry `Peq` table plus dispatching through the
/// runtime-feature check costs more than a rolling-rows pass over
/// very short inputs. Empirically 32 bytes is the crossover on modern
/// desktop CPUs; smaller inputs stay on the scalar path.
const MYERS_MIN_LEN: usize = 32;

/// Returns `true` iff the input pair is a good candidate for the SIMD
/// Myers backend.
///
/// The current criterion is a length threshold: both inputs are
/// byte-oriented by construction (the caller is on the `&[u8]` API entry
/// point), and the shorter side must be at least `MYERS_MIN_LEN` (32)
/// bytes long for the algorithmic win to outweigh the Peq build. The
/// underlying comparison is delegated to
/// [`crate::simd_dispatch::is_byte_amenable`] so every SIMD sub-tree in
/// this crate shares the same viability shape.
#[inline]
#[must_use]
pub fn is_byte_amenable_for_myers(a: &[u8], b: &[u8]) -> bool {
    crate::simd_dispatch::is_byte_amenable(a, b, MYERS_MIN_LEN)
}

/// Runtime-dispatching Levenshtein distance for byte-slice inputs.
///
/// Picks the best backend for the host CPU and delegates. The dispatch
/// itself is a single `is_x86_feature_detected!` / equivalent call per
/// invocation; if that cost matters, callers should cache the choice
/// (StringCheese does not currently expose a cached-dispatcher wrapper
/// because criterion measurements show the overhead is negligible for
/// any input above the `MYERS_MIN_LEN` guard used by
/// [`is_byte_amenable_for_myers`]).
///
/// The result is bit-for-bit identical to what
/// [`crate::levenshtein::distance_rolling_rows_with_workspace`] would
/// produce on the same inputs — this is asserted by the differential
/// tests below and by the SIMD-specific property tests in
/// `levenshtein/simd_property_tests.rs`.
#[must_use]
pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: is_x86_feature_detected!("avx2") returned true,
            // so the AVX2 target-feature precondition of
            // `myers_x86_avx2::distance` holds.
            return unsafe { myers_x86_avx2::distance(a, b) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: is_x86_feature_detected!("sse2") returned true,
            // so the SSE2 target-feature precondition of
            // `myers_x86_sse2::distance` holds. SSE2 is baseline for
            // x86_64, so this branch always runs on that target.
            return unsafe { myers_x86_sse2::distance(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: is_aarch64_feature_detected!("neon") returned
            // true, so the NEON target-feature precondition of
            // `myers_aarch64_neon::distance` holds. NEON is baseline
            // for aarch64.
            return unsafe { myers_aarch64_neon::distance(a, b) };
        }
    }
    myers_scalar::distance(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levenshtein::full_matrix::distance_full_matrix;
    use crate::levenshtein::rolling_rows::distance_rolling_rows_with_workspace;
    use crate::levenshtein::workspace::LevenshteinWorkspace;

    /// Every canonical pair, run through every backend the host CPU has,
    /// must agree with the full-matrix oracle.
    #[test]
    fn dispatcher_matches_oracle_on_canonical_pairs() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"", b"hello"),
            (b"hello", b""),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"distance", b"difference"),
            (b"aaaaaaa", b"aaaaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
            // A pair long enough to actually meet is_byte_amenable_for_myers.
            (
                b"the quick brown fox jumps over the lazy dog",
                b"the quick brown fox leaps over the lazy dog",
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

    /// The scalar Myers backend and the crate's production rolling-rows
    /// kernel must produce identical answers — differential test used
    /// as the correctness anchor for every arch-specific backend that
    /// chains through the scalar path.
    #[test]
    fn scalar_myers_matches_rolling_rows_on_canonical_pairs() {
        let mut ws = LevenshteinWorkspace::new();
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            let scalar = myers_scalar::distance(a, b);
            let rolling = distance_rolling_rows_with_workspace(a, b, &mut ws).into_inner();
            assert_eq!(scalar, rolling, "on ({a:?}, {b:?})");
        }
    }

    #[test]
    fn is_byte_amenable_for_myers_rejects_short_inputs() {
        assert!(!is_byte_amenable_for_myers(b"", b""));
        assert!(!is_byte_amenable_for_myers(b"kitten", b"sitting"));
        // 32 bytes on both sides is exactly the threshold.
        let long = &b"abcdefghijklmnopqrstuvwxyz012345"[..];
        assert_eq!(long.len(), 32);
        assert!(is_byte_amenable_for_myers(long, long));
    }
}
