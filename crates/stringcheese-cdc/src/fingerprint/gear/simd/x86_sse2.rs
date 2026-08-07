//! SSE2-gated Gear-hash slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. This is the SSE2 fallback selected by the
//! [dispatcher][super::digest_of_slice] when AVX2 is unavailable — every
//! `x86_64` CPU has SSE2 as part of the baseline ABI, so this branch is
//! always a valid target.
//!
//! # Kernel shape
//!
//! Gear's `state = (state << 1) + G[byte]` recurrence is strictly
//! sequential: `state_{n+1}` depends on `state_n`, so there is no natural
//! per-byte SIMD parallelism. This backend therefore consumes the byte
//! slice sequentially and relies on the compiler to use the SSE2
//! target-feature context established by `#[target_feature(enable = "sse2")]`
//! for whatever the enabled ISA can express — table loads, prefetching,
//! and register scheduling. See the [module docs][super] for the
//! trade-off and the reason a bit-parallel wide-block replacement is
//! documented follow-up work rather than shipped here.
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

/// SSE2-gated Gear-hash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` this is
/// guaranteed by the ABI, but the dispatcher still checks
/// `is_x86_feature_detected!("sse2")` to keep every dispatch branch
/// uniform.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn digest_of_slice(bytes: &[u8]) -> u64 {
    // The recurrence is sequential; delegating to the portable scalar
    // core inside this `#[target_feature]` context lets the compiler
    // generate SSE2-flavoured code (better register allocation and
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
        if !is_x86_feature_detected!("sse2") {
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
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { digest_of_slice(input) };
            assert_eq!(simd, reference(input), "on {input:?}");
        }
    }
}
