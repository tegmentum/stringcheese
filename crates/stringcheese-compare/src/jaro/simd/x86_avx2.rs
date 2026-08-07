//! AVX2-gated Jaro kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. The [`similarity`] entry
//! point is marked `#[target_feature(enable = "avx2")]` so that the
//! compiler is free to emit AVX2 intrinsics for the tight inner loop of
//! the scalar Jaro kernel it currently delegates to — most notably the
//! byte-broadcast + compare + horizontal-reduce window scan, which is a
//! textbook AVX2 lifting.
//!
//! # Safety
//!
//! [`similarity`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature must
//! be present at run time. The dispatcher in [`crate::jaro::simd`] gates
//! every call on `is_x86_feature_detected!("avx2")`, so the precondition
//! is met by construction; call sites outside the dispatcher must uphold
//! the same contract.
//!
//! # Correctness first, per-arch SIMD Jaro as follow-up
//!
//! The scalar Jaro window scan is a small loop of byte compares against
//! the "not-already-matched" bitmap. A genuine AVX2 lifting would load
//! 32 bytes of `b` per iteration into a `__m256i`, broadcast `a[i]` with
//! `_mm256_set1_epi8`, compare with `_mm256_cmpeq_epi8`, mask against the
//! corresponding bits of the packed bitmap, and reduce to the first set
//! bit via `_mm256_movemask_epi8` + `trailing_zeros`. That variant is
//! intricate enough that landing it separately — after this dispatch
//! scaffolding is in place — is the safer sequencing. Wiring the AVX2
//! target-feature now means the same public API keeps working when the
//! block variant lands.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use super::scalar;

/// AVX2-gated Jaro similarity for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure AVX2 is available on the running CPU. The
/// dispatcher in the parent [`super`] module guarantees this via
/// `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn similarity(a: &[u8], b: &[u8]) -> f64 {
    // SAFETY: The target-feature attribute lifts the enclosed code into
    // an AVX2 execution context — this is what makes the function
    // itself `unsafe`. The delegated call performs no unsafe operations
    // of its own; the AVX2 context is there to let the compiler
    // vectorize the delegated code's tight scalar loops if it chooses.
    scalar::similarity(a, b)
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
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { similarity(a, b) };
            let sc = scalar::similarity(a, b);
            assert_eq!(
                simd.to_bits(),
                sc.to_bits(),
                "avx2 disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }
}
