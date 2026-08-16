//! Property-based differential tests for the SIMD Jaro backend.
//!
//! Compiled only under `--features simd` and only on non-wasm hosts (the
//! `proptest` transitive dep is not wasm-friendly, matching the pattern
//! used by the sibling [`crate::jaro::property_tests`] module).
//!
//! The claim under test is bit-for-bit agreement between the SIMD entry
//! points and the crate's generic Jaro kernel. If any SIMD backend
//! disagrees with the generic kernel on any generated input, the whole
//! story of "SIMD as a transparent accelerator" is broken; this file is
//! the guard.

use proptest::prelude::*;

use crate::jaro::jaro::jaro_similarity;
use crate::jaro::simd;
use crate::jaro::simd::scalar;

/// Byte-slice strategy over the full byte alphabet, capped at 128 bytes.
///
/// The 128-byte cap crosses both the packed-bitmap single-word boundary
/// (positions 0..64 vs 64..128) and the amenability threshold (32), so
/// both hot-path shapes are exercised by the same test.
fn arb_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=128)
}

/// Long-side strategy — up to 512 bytes — to catch any bitmap state that
/// might only misbehave after many outer-loop iterations.
fn arb_bytes_long() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..=255, 0..=512)
}

/// Small-alphabet strategy — a three-symbol byte alphabet — that raises
/// the frequency of transposition-inducing runs. Small alphabets are the
/// stress case for Jaro's matching-window bookkeeping.
fn arb_bytes_small_alphabet() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'b'), Just(b'c')], 0..=128)
}

proptest! {
    /// The runtime-dispatching [`simd::similarity`] must agree with the
    /// generic Jaro kernel on every generated input pair.
    #[test]
    fn dispatcher_matches_generic(a in arb_bytes(), b in arb_bytes()) {
        let observed = simd::similarity(&a, &b);
        let expected = jaro_similarity(&a, &b);
        prop_assert_eq!(
            observed.to_bits(),
            expected.to_bits(),
            "simd::similarity disagreed with generic Jaro"
        );
    }

    /// The scalar SIMD-shaped backend must agree with the generic Jaro
    /// kernel; this is the anchor every arch-specific backend chains
    /// through.
    #[test]
    fn scalar_matches_generic(a in arb_bytes(), b in arb_bytes()) {
        let observed = scalar::similarity(&a, &b);
        let expected = jaro_similarity(&a, &b);
        prop_assert_eq!(
            observed.to_bits(),
            expected.to_bits(),
            "SIMD-shaped scalar disagreed with generic Jaro"
        );
    }

    /// Long-side coverage: many outer-loop iterations against a long
    /// text. A miscarry inside the window bookkeeping would surface
    /// here rather than on the short inputs.
    #[test]
    fn scalar_matches_generic_on_long_text(
        pattern in arb_bytes(),
        text in arb_bytes_long(),
    ) {
        let observed = scalar::similarity(&pattern, &text);
        let expected = jaro_similarity(&pattern, &text);
        prop_assert_eq!(
            observed.to_bits(),
            expected.to_bits(),
            "SIMD-shaped scalar disagreed on long text"
        );
    }

    /// Small-alphabet coverage: many candidate matches inside every
    /// window, exercising the "not-already-matched" bitmap logic hard.
    #[test]
    fn scalar_matches_generic_small_alphabet(
        a in arb_bytes_small_alphabet(),
        b in arb_bytes_small_alphabet(),
    ) {
        let observed = scalar::similarity(&a, &b);
        let expected = jaro_similarity(&a, &b);
        prop_assert_eq!(
            observed.to_bits(),
            expected.to_bits(),
            "SIMD-shaped scalar disagreed on small-alphabet input"
        );
    }

    /// Symmetry: Jaro is symmetric in its inputs. The SIMD kernel
    /// picks one side to iterate over, so a symmetry violation would
    /// indicate that the choice matters (a bug).
    #[test]
    fn scalar_is_symmetric(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(
            scalar::similarity(&a, &b).to_bits(),
            scalar::similarity(&b, &a).to_bits(),
            "SIMD-shaped scalar disagreed with itself under argument-swap"
        );
    }

    /// The AVX2 backend, when available on the host, must agree with the
    /// scalar SIMD-shaped kernel on every input. This is the
    /// arch-specific differential that catches any drift between the
    /// SIMD-gated wrapper and the shared scalar core.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !is_x86_feature_detected!("avx2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd_result = unsafe { simd::x86_avx2::similarity(&a, &b) };
        let scalar_result = scalar::similarity(&a, &b);
        prop_assert_eq!(
            simd_result.to_bits(),
            scalar_result.to_bits(),
            "avx2 disagreed with scalar"
        );
    }

    /// SSE2 differential — SSE2 is baseline on x86_64, so this branch
    /// runs on every x86_64 host.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn sse2_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !is_x86_feature_detected!("sse2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd_result = unsafe { simd::x86_sse2::similarity(&a, &b) };
        let scalar_result = scalar::similarity(&a, &b);
        prop_assert_eq!(
            simd_result.to_bits(),
            scalar_result.to_bits(),
            "sse2 disagreed with scalar"
        );
    }

    /// NEON differential — NEON is baseline on aarch64.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn neon_matches_scalar(a in arb_bytes(), b in arb_bytes()) {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd_result = unsafe { simd::aarch64_neon::similarity(&a, &b) };
        let scalar_result = scalar::similarity(&a, &b);
        prop_assert_eq!(
            simd_result.to_bits(),
            scalar_result.to_bits(),
            "neon disagreed with scalar"
        );
    }
}
