//! Property-based differential tests for the SIMD Hamming backend.
//!
//! Compiled only under `--features simd` and only on non-wasm hosts (the
//! `proptest` transitive dep is not wasm-friendly, matching the pattern
//! used by the sibling [`crate::hamming::property_tests`] module).
//!
//! The claim under test is bit-for-bit agreement between the SIMD entry
//! points and the crate's generic Hamming kernel. If any SIMD backend
//! disagrees with the generic kernel on any generated equal-length input,
//! the whole story of "SIMD as a transparent accelerator" is broken;
//! this file is the guard.

use proptest::prelude::*;

use crate::hamming::kernel::hamming_distance;
use crate::hamming::simd;
use crate::hamming::simd::scalar;

/// Equal-length byte-slice pair strategy over the full byte alphabet,
/// capped at 200 bytes. The cap spans the 32-byte AVX2 block width
/// several times over, so tail-handling and multi-block paths both fire.
fn arb_equal_length_pair() -> impl Strategy<Value = (alloc::vec::Vec<u8>, alloc::vec::Vec<u8>)> {
    (0usize..=200).prop_flat_map(|n| {
        (
            proptest::collection::vec(0u8..=255, n..=n),
            proptest::collection::vec(0u8..=255, n..=n),
        )
    })
}

/// Long-side strategy — up to 2048 bytes — to catch any accumulator
/// state that might only misbehave after many block iterations (in
/// particular the NEON `vaddlvq_u8` widening add whose result would
/// wrap silently if we accidentally used the u8 sibling).
fn arb_equal_length_long() -> impl Strategy<Value = (alloc::vec::Vec<u8>, alloc::vec::Vec<u8>)> {
    (0usize..=2048).prop_flat_map(|n| {
        (
            proptest::collection::vec(0u8..=255, n..=n),
            proptest::collection::vec(0u8..=255, n..=n),
        )
    })
}

proptest! {
    /// The runtime-dispatching [`simd::distance`] must agree with the
    /// generic Hamming kernel on every generated equal-length pair.
    #[test]
    fn dispatcher_matches_generic((a, b) in arb_equal_length_pair()) {
        let observed = simd::distance(&a, &b);
        let expected = hamming_distance(&a, &b).into_inner();
        prop_assert_eq!(observed, expected, "simd::distance disagreed with generic Hamming");
    }

    /// The scalar SIMD-shaped backend must agree with the generic
    /// Hamming kernel; this is the anchor every arch-specific backend
    /// chains through.
    #[test]
    fn scalar_matches_generic((a, b) in arb_equal_length_pair()) {
        let observed = scalar::distance(&a, &b);
        let expected = hamming_distance(&a, &b).into_inner();
        prop_assert_eq!(observed, expected, "SIMD-shaped scalar disagreed with generic Hamming");
    }

    /// Long-input coverage: many block iterations. A miscarry inside a
    /// backend's block accumulator (e.g. NEON's u8/u16 confusion) would
    /// surface here rather than on the short inputs.
    #[test]
    fn dispatcher_matches_generic_on_long((a, b) in arb_equal_length_long()) {
        let observed = simd::distance(&a, &b);
        let expected = hamming_distance(&a, &b).into_inner();
        prop_assert_eq!(observed, expected, "simd::distance disagreed on long input");
    }

    /// Cutoff correctness for the dispatcher: for any equal-length pair
    /// and any cutoff, the SIMD dispatcher's returned value must map to
    /// the same `Within` / `Exceeded` shape the generic kernel produces.
    #[test]
    fn dispatcher_within_matches_generic(
        (a, b) in arb_equal_length_pair(),
        cutoff in 0u32..300,
    ) {
        let raw = simd::distance_within(&a, &b, cutoff);
        let exact = hamming_distance(&a, &b).into_inner();
        if exact <= cutoff {
            // Within-cutoff: SIMD must return the exact count.
            prop_assert_eq!(raw, exact, "within-cutoff disagreed with exact distance");
        } else {
            // Exceeded: SIMD must return a value strictly greater than
            // cutoff. The exact value is a "may-terminate-early" number
            // above the cutoff, not necessarily equal to `exact`.
            prop_assert!(raw > cutoff, "exceeded-cutoff returned {raw} for cutoff {cutoff}");
        }
    }

    /// The AVX2 backend, when available on the host, must agree with the
    /// scalar SIMD-shaped kernel on every equal-length input.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn avx2_matches_scalar((a, b) in arb_equal_length_pair()) {
        if !is_x86_feature_detected!("avx2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd_result = unsafe { simd::x86_avx2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "avx2 disagreed with scalar");
    }

    /// SSE2 differential — SSE2 is baseline on x86_64, so this branch
    /// runs on every x86_64 host.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn sse2_matches_scalar((a, b) in arb_equal_length_pair()) {
        if !is_x86_feature_detected!("sse2") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd_result = unsafe { simd::x86_sse2::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "sse2 disagreed with scalar");
    }

    /// NEON differential — NEON is baseline on aarch64.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn neon_matches_scalar((a, b) in arb_equal_length_pair()) {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd_result = unsafe { simd::aarch64_neon::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "neon disagreed with scalar");
    }

    /// NEON differential on long inputs — the widening horizontal add
    /// (`vaddlvq_u8`) is the load-bearing choice for correctness on
    /// long inputs; a regression to the non-widening `vaddvq_u8` sibling
    /// would silently wrap after ~16 fully-matching blocks. This test
    /// exercises the many-block regime that would catch such a
    /// regression.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn neon_matches_scalar_on_long((a, b) in arb_equal_length_long()) {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return Ok(());
        }
        #[allow(
            unsafe_code,
            reason = "SIMD intrinsic wrappers are unsafe by declaration; the CPU-feature check above upholds the precondition"
        )]
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd_result = unsafe { simd::aarch64_neon::distance(&a, &b) };
        let scalar_result = scalar::distance(&a, &b);
        prop_assert_eq!(simd_result, scalar_result, "neon disagreed with scalar on long input");
    }
}
