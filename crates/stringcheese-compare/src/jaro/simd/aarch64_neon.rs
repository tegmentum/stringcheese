//! NEON-gated Jaro kernel for `aarch64`.
//!
//! This module compiles only on `aarch64` targets. NEON is part of the
//! aarch64 baseline, so the dispatcher's `is_aarch64_feature_detected!`
//! check is defensive rather than gating.
//!
//! # Safety
//!
//! [`similarity`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature must
//! be present at run time. On `aarch64`, NEON is guaranteed by the standard
//! ABI; the dispatcher checks it anyway for uniformity across architectures.
//!
//! # Correctness first, per-arch SIMD Jaro as follow-up
//!
//! Same shape as the x86 siblings: this module currently delegates to
//! [`super::scalar`] under a NEON target-feature context. A 128-bit-block
//! window scan using NEON's `vdupq_n_u8` broadcast, `vceqq_u8` byte-lane
//! compare, and the standard "shift-narrow + `vaddvq_u8`" horizontal-reduce
//! sequence is the natural next step and belongs in a standalone commit.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use super::scalar;

/// NEON-gated Jaro similarity for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the standard ABI, but the dispatcher still checks
/// `std::arch::is_aarch64_feature_detected!("neon")` for uniformity.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn similarity(a: &[u8], b: &[u8]) -> f64 {
    // SAFETY: The target-feature attribute lifts the enclosed code into
    // a NEON execution context — this is what makes the function itself
    // `unsafe`. The delegated call performs no unsafe operations.
    scalar::similarity(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { similarity(a, b) };
            let sc = scalar::similarity(a, b);
            assert_eq!(
                simd.to_bits(),
                sc.to_bits(),
                "neon disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }
}
