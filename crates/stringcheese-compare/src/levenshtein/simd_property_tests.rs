//! Property-based differential tests for the SIMD Levenshtein backend.
//!
//! Compiled only under `--features simd` and only on non-wasm hosts (the
//! `proptest` transitive dep is not wasm-friendly, matching the pattern
//! used by the sibling [`crate::levenshtein::property_tests`] module).
//!
//! The claim under test is bit-for-bit agreement between the SIMD entry
//! points and the crate's `full_matrix` oracle. If any SIMD backend
//! disagrees with the oracle on any generated input, the whole story of
//! "SIMD as a transparent accelerator" is broken; this file is the
//! guard.

use proptest::prelude::*;

use crate::levenshtein::full_matrix::distance_full_matrix;
use crate::levenshtein::simd;
use crate::levenshtein::simd::myers_scalar;

/// Byte-slice strategy over the full byte alphabet, capped at 128 bytes.
///
/// Full byte alphabet exercises the Peq table's every entry, not just a
/// small subset. 128 bytes crosses both the single-word Myers boundary
/// (m ≤ 64) and the rolling-rows fallback (m > 64), so both paths are
/// exercised by the same test.
fn arb_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=128)
}

/// Long-side strategy — up to 512 bytes — to catch any Myers state that
/// might only misbehave after many column iterations.
fn arb_bytes_long() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=512)
}

proptest! {
    /// The runtime-dispatching [`simd::distance`] must agree with the
    /// full-matrix oracle on every generated input pair.
    #[test]
    fn dispatcher_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let observed = simd::distance(&a, &b);
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "simd::distance disagreed with oracle");
    }

    /// The scalar Myers backend must agree with the full-matrix oracle;
    /// this is the anchor every arch-specific backend chains through.
    #[test]
    fn scalar_myers_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let observed = myers_scalar::distance(&a, &b);
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "myers_scalar disagreed with oracle");
    }

    /// Long-side coverage: many text columns processed through the same
    /// small Peq state. A miscarry inside `single_word` would surface
    /// here rather than on the short inputs.
    #[test]
    fn scalar_myers_matches_oracle_on_long_text(
        pattern in arb_bytes(),
        text in arb_bytes_long(),
    ) {
        let observed = myers_scalar::distance(&pattern, &text);
        let expected = distance_full_matrix(&pattern, &text);
        prop_assert_eq!(observed, expected, "myers_scalar disagreed with oracle on long text");
    }

    /// Distance must be symmetric — Myers's algorithm is not obviously
    /// symmetric in its input arguments (it packs the pattern side into
    /// words), so a symmetry violation would indicate that the
    /// shorter-as-pattern selection isn't giving equivalent results.
    #[test]
    fn scalar_myers_is_symmetric(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(
            myers_scalar::distance(&a, &b),
            myers_scalar::distance(&b, &a),
            "myers_scalar disagreed with itself under argument-swap"
        );
    }

    /// The AVX2 backend, when available on the host, must agree with
    /// the scalar Myers on every input. This is the arch-specific
    /// differential that catches any drift between the SIMD-gated
    /// wrapper and the shared scalar core.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !is_x86_feature_detected!("avx2") {
            return Ok(());
        }
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        let simd_result = unsafe { simd::myers_x86_avx2::distance(&a, &b) };
        let scalar_result = myers_scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "avx2 disagreed with scalar");
    }

    /// SSE2 differential — SSE2 is baseline on x86_64, so this branch
    /// runs on every x86_64 host.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn sse2_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !is_x86_feature_detected!("sse2") {
            return Ok(());
        }
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        let simd_result = unsafe { simd::myers_x86_sse2::distance(&a, &b) };
        let scalar_result = myers_scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "sse2 disagreed with scalar");
    }

    /// NEON differential — NEON is baseline on aarch64.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn neon_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return Ok(());
        }
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        let simd_result = unsafe { simd::myers_aarch64_neon::distance(&a, &b) };
        let scalar_result = myers_scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "neon disagreed with scalar");
    }
}
