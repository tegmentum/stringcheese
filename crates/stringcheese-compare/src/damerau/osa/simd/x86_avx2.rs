//! AVX2-gated OSA (restricted Damerau-Levenshtein) kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. The [`distance`] entry
//! point is marked `#[target_feature(enable = "avx2")]` so that the
//! compiler is free to emit AVX2 intrinsics for the tight inner loop of
//! the scalar SIMD-shaped OSA kernel it currently delegates to — the
//! three-row rolling DP has a data-parallel per-cell recurrence that the
//! backend can lift into vectorized min / add / compare sequences over
//! the u32 row.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature must
//! be present at run time. The dispatcher in
//! [`crate::damerau::osa::simd`] gates every call on
//! `is_x86_feature_detected!("avx2")`, so the precondition is met by
//! construction; call sites outside the dispatcher must uphold the same
//! contract.
//!
//! # Correctness first, per-arch bit-parallel OSA as follow-up
//!
//! Real per-arch acceleration for OSA would move to Hyyrö (2003)
//! bit-parallel OSA — Myers's word-parallel Levenshtein with an extra
//! bit-vector carrying the transposition-match state between adjacent
//! columns. A wide-block form (256-bit blocks on AVX2) is the natural
//! next step. Landing it separately — after this dispatch scaffolding is
//! in place — is the safer sequencing. Wiring the AVX2 target-feature now
//! means the same public API keeps working when the bit-parallel form
//! lands.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use super::scalar;

/// AVX2-gated OSA distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure AVX2 is available on the running CPU. The
/// dispatcher in the parent [`super`] module guarantees this via
/// `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    // SAFETY: The target-feature attribute lifts the enclosed code into
    // an AVX2 execution context — this is what makes the function
    // itself `unsafe`. The delegated call performs no unsafe operations
    // of its own; the AVX2 context is there to let the compiler
    // vectorize the delegated code's tight scalar loops if it chooses.
    scalar::distance(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !is_x86_feature_detected!("avx2") {
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
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { distance(a, b) };
            let sc = scalar::distance(a, b);
            assert_eq!(simd, sc, "avx2 disagreed with scalar on ({a:?}, {b:?})");
        }
    }
}
