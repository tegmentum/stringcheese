//! AVX2-gated Rabin-fingerprint slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. See the [module docs][super] for the
//! kernel shape and the shared `unsafe` policy.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use super::scalar;

/// AVX2-gated Rabin-fingerprint digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure AVX2 is available.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    scalar::digest_of_slice(window, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::rabin::RabinFingerprint;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = RabinFingerprint::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for &window in &[1usize, 8, 32, 64, 100] {
            let cases: &[&[u8]] = &[
                b"",
                b"a",
                b"the quick brown fox jumps over the lazy dog",
                &[0u8; 128],
                &[0xFFu8; 200],
            ];
            for &input in cases {
                // SAFETY: is_x86_feature_detected!("avx2") returned true.
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
