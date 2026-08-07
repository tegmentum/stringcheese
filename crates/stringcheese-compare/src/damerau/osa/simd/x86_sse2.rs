//! SSE2-gated OSA (restricted Damerau-Levenshtein) kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. It is the SSE2 fallback
//! selected by the dispatcher when AVX2 is unavailable — every `x86_64`
//! CPU has SSE2 as part of the baseline ABI, so this branch is always a
//! valid target.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature must
//! be present at run time. On `x86_64`, SSE2 is guaranteed by the ABI;
//! the dispatcher checks it anyway for consistency with the other arch
//! branches.
//!
//! # Correctness first, per-arch bit-parallel OSA as follow-up
//!
//! See the sibling AVX2 module for the reasoning behind the current
//! scalar-delegation shape. A 128-bit-block Hyyrö-style bit-parallel OSA
//! is the natural next step; it belongs in a standalone commit alongside
//! its differential tests.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use super::scalar;

/// SSE2-gated OSA distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` this is
/// guaranteed by the ABI, but the dispatcher still checks
/// `is_x86_feature_detected!("sse2")` to keep every dispatch branch
/// uniform.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    // SAFETY: The target-feature attribute lifts the enclosed code into
    // an SSE2 execution context — this is what makes the function itself
    // `unsafe`. The delegated call performs no unsafe operations.
    scalar::distance(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !is_x86_feature_detected!("sse2") {
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
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { distance(a, b) };
            let sc = scalar::distance(a, b);
            assert_eq!(simd, sc, "sse2 disagreed with scalar on ({a:?}, {b:?})");
        }
    }
}
