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
//! * `x86_avx2` — AVX2-gated Hyyrö-OSA wide-block, compiled only on
//!   `x86_64`. Handles patterns of length `128 < m ≤ 256` with a
//!   256-bit register (four `u64` lanes), delegates `64 < m ≤ 128` to
//!   the SSE2 sibling (SSE2 is a strict subset of AVX2, so a 128-bit
//!   state is a better fit than a half-idle 256-bit one), and
//!   shorter/longer patterns to the scalar path.
//! * `x86_sse2` — SSE2-gated Hyyrö-OSA wide-block, compiled only on
//!   `x86_64`. Handles `64 < m ≤ 128` with a 128-bit register (two
//!   `u64` lanes); shorter/longer patterns delegate to the scalar path.
//! * `aarch64_neon` — NEON-gated Hyyrö-OSA wide-block for
//!   `64 < m ≤ 128`, same shape as SSE2 with NEON's `vextq_u64`-based
//!   cross-lane carry. Compiled only on `aarch64`.
//! * `wasm_simd128` — wasm SIMD128-gated Hyyrö-OSA wide-block for
//!   `64 < m ≤ 128`. Two `u64` lanes per `v128`, same shape as
//!   SSE2/NEON but with lane-extract/insert used for the cross-lane
//!   carry (wasm SIMD has no direct `_mm_slli_si128`-style
//!   whole-register byte shift on the u64 lane dimension). Compiled
//!   only on `wasm32` and only when the `simd128` target-feature is
//!   enabled — wasm-SIMD feature detection is a compile-time gate,
//!   not a runtime one.
//!
//! Every arch-specific backend implements Hyyrö's (2003) bit-parallel
//! OSA recurrence — Myers's word-parallel Levenshtein extended with an
//! extra bit-vector `Pm_old` (the previous column's `Peq[text[j-1]]`)
//! and a diagonal-zero vector `D0` — packed into the arch's widest
//! integer register. See Hyyrö, H. (2003), "Bit-parallel approximate
//! string matching algorithms with transposition" (SPIRE 2003,
//! LNCS 2857, 95-107). The scalar backend deliberately keeps the
//! rolling-rows DP form as the differential-test anchor; the
//! arch-specific backends are checked bit-for-bit against it in the
//! module-local tests and in the property tests at
//! `crates/stringcheese-compare/src/damerau/simd_property_tests.rs`.
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

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub mod wasm_simd128;

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
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        // SAFETY: wasm-SIMD feature detection is a compile-time gate,
        // not a runtime one — the `target_feature = "simd128"` cfg on
        // this block is the same predicate that guards
        // `wasm_simd128`'s module-level compilation, so if this branch
        // is compiled the intrinsics inside are guaranteed legal for
        // any engine that accepts the module.
        return unsafe { wasm_simd128::distance(a, b) };
    }
    #[allow(
        unreachable_code,
        reason = "the wasm32+simd128 cfg-branch above returns unconditionally when compiled; on hosts where that branch is stripped this call is the fallthrough"
    )]
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
