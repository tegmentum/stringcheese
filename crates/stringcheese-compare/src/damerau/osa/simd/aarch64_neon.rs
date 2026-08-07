//! NEON-gated OSA (restricted Damerau-Levenshtein) kernel for `aarch64`.
//!
//! This module compiles only on `aarch64` targets. NEON is part of the
//! aarch64 baseline, so the dispatcher's `is_aarch64_feature_detected!`
//! check is defensive rather than gating.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature must
//! be present at run time. On `aarch64`, NEON is guaranteed by the standard
//! ABI; the dispatcher checks it anyway for uniformity across architectures.
//!
//! # Correctness first, per-arch bit-parallel OSA as follow-up
//!
//! Same shape as the x86 siblings: this module currently delegates to
//! [`super::scalar`] under a NEON target-feature context. A 128-bit-block
//! Hyyrö-style bit-parallel OSA using NEON's `vaddq_u64` / `vshlq_u64`
//! primitives is the natural next step and belongs in a standalone commit.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use super::scalar;

/// NEON-gated OSA distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the standard ABI, but the dispatcher still checks
/// `std::arch::is_aarch64_feature_detected!("neon")` for uniformity.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    // SAFETY: The target-feature attribute lifts the enclosed code into
    // a NEON execution context — this is what makes the function itself
    // `unsafe`. The delegated call performs no unsafe operations.
    scalar::distance(a, b)
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
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { distance(a, b) };
            let sc = scalar::distance(a, b);
            assert_eq!(simd, sc, "neon disagreed with scalar on ({a:?}, {b:?})");
        }
    }
}
