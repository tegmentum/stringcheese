//! SSE2-gated Buzhash slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. This is the SSE2 fallback selected by
//! the [dispatcher][super::digest_of_slice] when AVX2 is unavailable —
//! every `x86_64` CPU has SSE2 as part of the baseline ABI, so this
//! branch is always a valid target.
//!
//! # Kernel shape — scalar under `target_feature(sse2)`
//!
//! The [AVX2][super::x86_avx2] and `aarch64_neon` sibling backends
//! ship a real vectorized kernel built on the Buzhash block
//! reformulation — see their module docs for the derivation. This
//! SSE2 backend deliberately does **not**: SSE2 has no
//! `_mm_i32gather_epi64` (that arrived in AVX2) and no per-lane
//! variable shift (`_mm_sllv_epi64` and `_mm_srlv_epi64` are AVX2 as
//! well), so the per-lane pre-rotate the block form needs cannot be
//! expressed on SSE2 without partial emulation via scalar-side moves.
//! `_mm_slli_epi64` and `_mm_srli_epi64` share a scalar count across
//! both lanes, which suffices for the Horner-step 2-bit rotate but not
//! the `[3, 2, 1, 0]` per-lane pre-rotate an AVX2 4-lane kernel uses.
//! A partly-emulated kernel on SSE2 hardware in 2026 costs more than
//! the vector reduction saves, so this backend keeps the scalar
//! `state = state.rotate_left(1) ^ contrib_i` core and lets the
//! compiler generate SSE2-flavoured code inside the
//! `#[target_feature]` context. AVX2 is the wide x86 branch the
//! dispatcher prefers when available.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. On `x86_64` SSE2 is guaranteed by the
//! ABI; the dispatcher still checks `is_x86_feature_detected!("sse2")`
//! for consistency with the other arch branches.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// SSE2-gated Buzhash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` SSE2 is
/// guaranteed by the ABI; the dispatcher still checks
/// `is_x86_feature_detected!("sse2")` for consistency with the other
/// arch branches.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    // The recurrence is sequential; delegating to the portable scalar
    // core inside this `#[target_feature]` context lets the compiler
    // generate SSE2-flavoured code (better register allocation and
    // scheduling than the module-default target features permit).
    scalar::digest_of_slice(window, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::buzhash::Buzhash;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = Buzhash::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for &window in &[0usize, 1, 8, 32, 64, 100] {
            let cases: &[&[u8]] = &[
                b"",
                b"a",
                b"the quick brown fox jumps over the lazy dog",
                &[0u8; 128],
                &[0xFFu8; 200],
            ];
            for &input in cases {
                // SAFETY: is_x86_feature_detected!("sse2") returned true.
                let simd = unsafe { digest_of_slice(window, input) };
                assert_eq!(
                    simd,
                    reference(window, input),
                    "on {input:?} window {window}"
                );
            }
        }
    }
}
