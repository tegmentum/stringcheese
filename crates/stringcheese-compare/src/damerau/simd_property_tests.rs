//! Property-based differential tests for the SIMD OSA backend.
//!
//! Compiled only under `--features simd` and only on non-wasm hosts (the
//! `proptest` transitive dep is not wasm-friendly, matching the pattern
//! used by the sibling [`crate::damerau::property_tests`] module).
//!
//! The claim under test is bit-for-bit agreement between the SIMD entry
//! points and the crate's `full_matrix` oracle. If any SIMD backend
//! disagrees with the oracle on any generated input, the whole story of
//! "SIMD as a transparent accelerator" is broken; this file is the
//! guard.

use proptest::prelude::*;

use crate::damerau::osa::full_matrix::distance_full_matrix;
use crate::damerau::osa::simd;
use crate::damerau::osa::simd::scalar;

/// Byte-slice strategy over the full byte alphabet, capped at 128 bytes.
///
/// 128 bytes crosses both the single-word Myers boundary (m ≤ 64) and
/// what will eventually be the block-Myers threshold (m > 64), so both
/// paths of a future bit-parallel OSA are exercised by the same test.
fn arb_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=128)
}

/// Long-side strategy — up to 512 bytes — to catch any state that might
/// only misbehave after many column iterations.
fn arb_bytes_long() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=512)
}

/// Small-alphabet strategy — a three-symbol byte alphabet — that raises
/// the frequency of adjacent-transposition matches. Small alphabets are
/// the stress case for OSA's transposition branch.
fn arb_bytes_small_alphabet() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'b'), Just(b'c')], 0..=128)
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

    /// The scalar SIMD-shaped OSA backend must agree with the
    /// full-matrix oracle; this is the anchor every arch-specific
    /// backend chains through.
    #[test]
    fn scalar_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let observed = scalar::distance(&a, &b);
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "scalar SIMD-shaped disagreed with oracle");
    }

    /// Long-side coverage: many column iterations against a long text.
    #[test]
    fn scalar_matches_oracle_on_long_text(
        pattern in arb_bytes(),
        text in arb_bytes_long(),
    ) {
        let observed = scalar::distance(&pattern, &text);
        let expected = distance_full_matrix(&pattern, &text);
        prop_assert_eq!(observed, expected, "scalar SIMD-shaped disagreed on long text");
    }

    /// Small-alphabet coverage: transposition branch fires often, so a
    /// bug in that branch would surface here more readily than on the
    /// random-byte strategy.
    #[test]
    fn scalar_matches_oracle_small_alphabet(
        a in arb_bytes_small_alphabet(),
        b in arb_bytes_small_alphabet(),
    ) {
        let observed = scalar::distance(&a, &b);
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "scalar SIMD-shaped disagreed on small-alphabet input");
    }

    /// Symmetry: OSA is symmetric in its inputs. The SIMD kernel picks
    /// one side to iterate over, so a symmetry violation would indicate
    /// that the choice matters (a bug).
    #[test]
    fn scalar_is_symmetric(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(
            scalar::distance(&a, &b),
            scalar::distance(&b, &a),
            "scalar SIMD-shaped disagreed with itself under argument-swap"
        );
    }

    /// The AVX2 backend, when available on the host, must agree with
    /// the scalar SIMD-shaped kernel on every input.
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
        let simd_result = unsafe { simd::x86_avx2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "avx2 disagreed with scalar");
    }

    /// SSE2 differential — SSE2 is baseline on x86_64.
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
        let simd_result = unsafe { simd::x86_sse2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
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
        let simd_result = unsafe { simd::aarch64_neon::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "neon disagreed with scalar");
    }
}
