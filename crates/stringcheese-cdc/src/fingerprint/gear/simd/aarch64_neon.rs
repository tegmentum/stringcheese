//! NEON-gated Gear-hash slice-batch backend for `aarch64`.
//!
//! Compiled only on `aarch64`. NEON is baseline for `aarch64`, so this
//! branch is always a valid target when the crate is built for that
//! architecture; the dispatcher still checks
//! `is_aarch64_feature_detected!("neon")` for uniformity with the x86
//! branches.
//!
//! # Kernel shape
//!
//! Gear's `state = (state << 1) + G[byte]` recurrence is strictly
//! sequential, so this backend consumes the byte slice sequentially
//! inside the NEON target-feature context. See the [module docs][super]
//! for why the initial cut ships the same core under NEON rather than a
//! hand-written wide-block kernel — a bit-parallel replacement is
//! documented follow-up work.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// NEON-gated Gear-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the ABI, but the dispatcher still checks
/// `is_aarch64_feature_detected!("neon")` to keep every dispatch branch
/// uniform.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn digest_of_slice(bytes: &[u8]) -> u64 {
    // The recurrence is sequential; delegating to the portable scalar
    // core inside this `#[target_feature]` context lets the compiler
    // generate NEON-flavoured code (better register allocation and
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
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { digest_of_slice(input) };
            assert_eq!(simd, reference(input), "on {input:?}");
        }
    }
}
