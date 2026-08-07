//! AVX2-gated Gear-hash slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. This is the widest x86 backend
//! [the dispatcher][super::digest_of_slice] can select; it falls back to
//! the [SSE2 sibling][super::x86_sse2] on hosts without AVX2.
//!
//! # Kernel shape
//!
//! Gear's `state = (state << 1) + G[byte]` recurrence is strictly
//! sequential, so this backend consumes the byte slice sequentially
//! inside the AVX2 target-feature context. See the [module docs][super]
//! for why the initial cut ships the same core under AVX2 rather than a
//! hand-written wide-block kernel — a bit-parallel replacement is
//! documented follow-up work.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher checks
//! `is_x86_feature_detected!("avx2")` before every call.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// AVX2-gated Gear-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure AVX2 is available (see the module-level
/// safety note).
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn digest_of_slice(bytes: &[u8]) -> u64 {
    // The recurrence is sequential; delegating to the portable scalar
    // core inside this `#[target_feature]` context lets the compiler
    // generate AVX2-flavoured code (better register allocation and
    // scheduling than the module-default target features permit).
    scalar::digest_of_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::gear::GearHash;

    fn reference(bytes: &[u8]) -> u64 {
        let mut h = GearHash::new(64);
        for &b in bytes {
            h.roll(b);
        }
        h.state()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let cases: &[&[u8]] = &[
            b"",
            b"a",
            b"the quick brown fox jumps over the lazy dog",
            &[0u8; 128],
            &[0xFFu8; 200],
        ];
        for &input in cases {
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { digest_of_slice(input) };
            assert_eq!(simd, reference(input), "on {input:?}");
        }
    }
}
