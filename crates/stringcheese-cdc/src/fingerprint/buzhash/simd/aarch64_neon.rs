//! NEON-gated Buzhash slice-batch backend for `aarch64`.
//!
//! Compiled only on `aarch64`. See the [module docs][super] for the
//! kernel shape and the shared `unsafe` policy.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// NEON-gated Buzhash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` NEON is
/// guaranteed by the ABI; the dispatcher still checks
/// `is_aarch64_feature_detected!("neon")` for uniformity with the x86
/// branches.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
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
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
                // SAFETY: is_aarch64_feature_detected!("neon") returned true.
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
