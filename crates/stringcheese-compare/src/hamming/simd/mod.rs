//! SIMD-accelerated Hamming distance backend.
//!
//! This module is compiled only when the `simd` feature is enabled. It
//! provides a byte-slice-oriented Hamming kernel and dispatches at run
//! time to the best backend the host CPU supports.
//!
//! Hamming distance over equal-length byte slices is the simplest SIMD
//! target in the compare crate: for each pair of blocks, count the byte
//! positions where the two blocks differ. The kernels here all boil down
//! to `load` + `cmpeq_epi8` + `movemask` (or the arch equivalent) +
//! `count_ones` per block; the tail (fewer than one block) runs as a
//! scalar byte loop.
//!
//! # Public surface
//!
//! Callers interact with this backend through:
//!
//! * `distance` — the runtime-dispatching entry point that picks the
//!   best available backend for the host and delegates to it. This is
//!   what [`crate::hamming::algorithm::Hamming::distance_bytes`] calls
//!   when the `simd` feature is on and `is_byte_amenable_for_hamming`
//!   is satisfied.
//! * `distance_within` — the cutoff-aware sibling. Every arch backend
//!   returns the exact mismatch count when it is at most `cutoff`, or a
//!   value strictly greater than `cutoff` (a sentinel meaning "exceeded")
//!   when the true count is above. The caller is responsible for mapping
//!   the sentinel to [`stringcheese_core::BoundedDistance::Exceeded`].
//! * `is_byte_amenable_for_hamming` — the guard used by the public API
//!   to decide whether an input pair is a good fit for the SIMD backend.
//!   Very short inputs (below the block width on both sides) stay on the
//!   scalar path.
//!
//! # Backends
//!
//! * `scalar` — portable SIMD-shaped scalar Hamming. Always compiled;
//!   the reference against which every arch-specific backend is
//!   differentially tested.
//! * `x86_avx2` — AVX2-gated, compiled only on `x86_64`. 32-byte block
//!   width; `_mm256_cmpeq_epi8` + `_mm256_movemask_epi8` + `count_ones`.
//! * `x86_sse2` — SSE2-gated, compiled only on `x86_64`. 16-byte block
//!   width; `_mm_cmpeq_epi8` + `_mm_movemask_epi8` + `count_ones`.
//! * `aarch64_neon` — NEON-gated, compiled only on `aarch64`. 16-byte
//!   block width; `vceqq_u8` + `vshrq_n_u8::<7>` + `vaddlvq_u8` (the
//!   widening horizontal add — the load-bearing NEON idiom, since NEON
//!   has no direct byte-lane movemask).
//! * `wasm_simd128` — wasm SIMD128-gated. 16-byte block width;
//!   `u8x16_eq` + `u8x16_bitmask` + `count_ones`. Compiled only on
//!   `wasm32` and only when the `simd128` target-feature is enabled.
//!
//! # `unsafe` policy
//!
//! The compare crate uses `#![deny(unsafe_code)]` at its root. This
//! module is one of the documented exceptions (the others being
//! [`crate::levenshtein::simd`], [`crate::jaro::simd`], and
//! [`crate::damerau::osa::simd`]): every arch-specific backend carries a
//! module-scoped `#[allow(unsafe_code)]` because `#[target_feature]`-gated
//! functions are `unsafe fn` by rustc's declaration. Every `unsafe fn`
//! and every `unsafe` block in this module tree carries a comment naming
//! the safety precondition. The dispatcher in this file establishes the
//! CPU-feature preconditions before every call, so the arch-specific
//! `unsafe fn`s are always invoked with their contracts upheld.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics require unsafe by declaration; every unsafe fn and every unsafe block here has a safety comment naming the CPU-feature precondition the dispatcher upholds"
)]

pub mod scalar;

#[cfg(target_arch = "x86_64")]
pub mod x86_avx2;
#[cfg(target_arch = "x86_64")]
pub mod x86_sse2;

#[cfg(target_arch = "aarch64")]
pub mod aarch64_neon;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub mod wasm_simd128;

/// Minimum input length below which the SIMD backend is not worth the
/// setup cost.
///
/// Hamming's per-call setup is essentially nil (no bitmap allocation, no
/// Peq table), so the threshold is just the widest arch block width in
/// the tree (32 for AVX2). Below that the scalar path is at least as
/// fast and doesn't pay for the runtime-dispatch branch.
const HAMMING_MIN_LEN: usize = 32;

/// Returns `true` iff the input pair is a good candidate for the SIMD
/// Hamming backend.
///
/// The current criterion is a length threshold: both inputs must be at
/// least `HAMMING_MIN_LEN` (32) bytes. Hamming requires equal-length
/// inputs, so the check is effectively on `a.len()` alone once the
/// upstream length-mismatch guard has fired.
///
/// The check goes through [`crate::simd_dispatch::is_byte_amenable`] so
/// every SIMD sub-tree in this crate shares the same viability shape.
#[inline]
#[must_use]
pub fn is_byte_amenable_for_hamming(a: &[u8], b: &[u8]) -> bool {
    crate::simd_dispatch::is_byte_amenable(a, b, HAMMING_MIN_LEN)
}

/// Runtime-dispatching Hamming distance for equal-length byte-slice
/// inputs.
///
/// Picks the best backend for the host CPU and delegates. The result is
/// bit-for-bit identical to what [`crate::hamming::kernel::hamming_distance`]
/// would produce on the same inputs — this is asserted by the
/// differential tests below and by the SIMD-specific property tests in
/// `hamming/simd_property_tests.rs`.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
#[must_use]
pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::distance requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: is_x86_feature_detected!("avx2") returned true,
            // so the AVX2 target-feature precondition of
            // `x86_avx2::distance` holds.
            return unsafe { x86_avx2::distance(a, b) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: is_x86_feature_detected!("sse2") returned true,
            // so the SSE2 target-feature precondition of
            // `x86_sse2::distance` holds. SSE2 is baseline for x86_64,
            // so this branch always runs on that target.
            return unsafe { x86_sse2::distance(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: is_aarch64_feature_detected!("neon") returned
            // true, so the NEON target-feature precondition of
            // `aarch64_neon::distance` holds. NEON is baseline for
            // aarch64.
            return unsafe { aarch64_neon::distance(a, b) };
        }
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        // SAFETY: wasm-SIMD feature detection is a compile-time gate,
        // not a runtime one — the `target_feature = "simd128"` cfg on
        // this block is the same predicate that guards
        // `wasm_simd128`'s module-level compilation, so if this branch
        // is compiled the intrinsics inside are guaranteed legal for
        // any engine that accepts the module.
        return unsafe { wasm_simd128::distance(a, b) };
    }
    #[allow(
        unreachable_code,
        reason = "the wasm32+simd128 cfg-branch above returns unconditionally when compiled; on hosts where that branch is stripped this call is the fallthrough"
    )]
    scalar::distance(a, b)
}

/// Runtime-dispatching Hamming distance with an early-termination cutoff.
///
/// Returns the exact mismatch count when it is at most `cutoff`, or a
/// value strictly greater than `cutoff` (a sentinel meaning "exceeded")
/// when the true count is above. Callers should compare the returned
/// value against `cutoff` and map to [`stringcheese_core::BoundedDistance`]
/// accordingly.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
#[must_use]
pub fn distance_within(a: &[u8], b: &[u8], cutoff: u32) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::distance_within requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            // SAFETY: see `distance`.
            return unsafe { x86_avx2::distance_within(a, b, cutoff) };
        }
        if is_x86_feature_detected!("sse2") {
            // SAFETY: see `distance`.
            return unsafe { x86_sse2::distance_within(a, b, cutoff) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            // SAFETY: see `distance`.
            return unsafe { aarch64_neon::distance_within(a, b, cutoff) };
        }
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        // SAFETY: see `distance`.
        return unsafe { wasm_simd128::distance_within(a, b, cutoff) };
    }
    #[allow(
        unreachable_code,
        reason = "the wasm32+simd128 cfg-branch above returns unconditionally when compiled; on hosts where that branch is stripped this call is the fallthrough"
    )]
    scalar::distance_within(a, b, cutoff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hamming::kernel::hamming_distance;

    #[test]
    fn dispatcher_matches_generic_kernel_on_canonical_pairs() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b"a"),
            (b"karolin", b"kathrin"),
            (b"karolin", b"kerstin"),
            (b"1011101", b"1001001"),
            (b"abc", b"xyz"),
            (
                b"the quick brown fox jumps over the lazy dog",
                b"the quick brown fox leaps over the lazy dog",
            ),
        ];
        for (a, b) in cases {
            let simd = distance(a, b);
            let generic = hamming_distance(a, b).into_inner();
            assert_eq!(
                simd, generic,
                "simd::distance disagreed with generic kernel on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn scalar_matches_generic_kernel_on_canonical_pairs() {
        let cases: &[(&[u8], &[u8])] = &[(b"", b""), (b"karolin", b"kathrin"), (b"abc", b"xyz")];
        for (a, b) in cases {
            let s = scalar::distance(a, b);
            let generic = hamming_distance(a, b).into_inner();
            assert_eq!(s, generic, "on ({a:?}, {b:?})");
        }
    }

    #[test]
    fn is_byte_amenable_for_hamming_rejects_short_inputs() {
        assert!(!is_byte_amenable_for_hamming(b"", b""));
        assert!(!is_byte_amenable_for_hamming(b"abc", b"def"));
        // 32 bytes on both sides is exactly the threshold.
        let long = &b"abcdefghijklmnopqrstuvwxyz012345"[..];
        assert_eq!(long.len(), 32);
        assert!(is_byte_amenable_for_hamming(long, long));
    }

    #[test]
    fn dispatcher_within_below_cutoff_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..200u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[50] ^= 0x02;
        b[100] ^= 0x04;
        let simd = distance_within(&a, &b, 10);
        assert_eq!(simd, 3);
    }

    #[test]
    fn dispatcher_within_above_cutoff_returns_sentinel() {
        let a = alloc::vec![0u8; 200];
        let b = alloc::vec![0xffu8; 200];
        let simd = distance_within(&a, &b, 5);
        assert!(simd > 5, "expected exceeded sentinel, got {simd}");
    }
}
