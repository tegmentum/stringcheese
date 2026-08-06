//! AVX2-gated Myers Levenshtein kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. The [`distance`] entry
//! point is marked `#[target_feature(enable = "avx2")]` so that the
//! compiler is free to emit AVX2 intrinsics for the tight inner loop of
//! the scalar Myers kernel it currently delegates to — most notably the
//! Peq table build, which is a purely data-parallel scatter that the
//! backend can lift into vector stores.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher in
//! [`crate::levenshtein::simd`] gates every call on
//! `is_x86_feature_detected!("avx2")`, so the precondition is met by
//! construction; call sites outside the dispatcher must uphold the same
//! contract.
//!
//! # Correctness first, per-arch SIMD Myers as follow-up
//!
//! The single-word Myers inner loop is a small handful of scalar
//! `u64` operations; genuine AVX2 acceleration would come from a
//! 256-bit block variant (four u64 lanes per column update) with
//! explicit inter-lane carry propagation. That variant is intricate
//! enough that landing it separately — after this dispatch scaffolding
//! is in place — is the safer sequencing. Wiring the AVX2 target-feature
//! now means the same public API keeps working when the block variant
//! lands.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use super::myers_scalar;

/// AVX2-gated Levenshtein distance for byte-slice inputs.
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
    // itself `unsafe`. The delegated call to `myers_scalar::distance`
    // performs no unsafe operations of its own; the AVX2 context is
    // there to let the compiler vectorize the delegated code's tight
    // scalar loops if it chooses.
    myers_scalar::distance(a, b)
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
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { distance(a, b) };
            let scalar = myers_scalar::distance(a, b);
            assert_eq!(simd, scalar, "avx2 disagreed with scalar on ({a:?}, {b:?})");
        }
    }
}
