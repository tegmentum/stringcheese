//! Scalar, SIMD-shaped Hamming kernel for byte-slice inputs.
//!
//! This module is the SIMD backend's portable fallback and the reference
//! against which every arch-specific SIMD implementation is differentially
//! tested. It computes the same mismatch count as the generic
//! [`crate::hamming::kernel::hamming_distance`], but rewrites the fold as a
//! block-wise byte compare that mirrors the shape every arch-specific
//! backend actually runs:
//!
//! * Walk the two input slices in fixed-width blocks (16 or 32 bytes on
//!   the vector backends; here the block is a plain scalar chunk).
//! * For each block, count the byte positions at which the two slices
//!   differ.
//! * Handle the tail (fewer than `BLOCK` bytes) with the same scalar
//!   comparison, which — because this file *is* the scalar reference — is
//!   just a byte-by-byte loop.
//!
//! # Overflow
//!
//! The accumulator is `u32` with saturating adds, matching the return
//! shape of the generic kernel. Inputs longer than `u32::MAX` bytes
//! (~4 GiB) saturate rather than wrap.
//!
//! # Panics
//!
//! Panics if `a.len() != b.len()`, matching the generic kernel's
//! equal-length precondition. The dispatcher upholds this before calling
//! any SIMD backend, and this scalar reference does the same for
//! symmetry.

/// Scalar reference kernel: counts mismatching byte positions between two
/// equal-length slices.
///
/// Bit-for-bit equivalent to what the generic kernel returns on the same
/// inputs. Serves as the differential-test oracle for every arch-specific
/// backend in this SIMD sub-tree.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
#[must_use]
pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::scalar::distance requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let mut mismatches: u32 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        if x != y {
            mismatches = mismatches.saturating_add(1);
        }
    }
    mismatches
}

/// Scalar reference kernel with an early-termination cutoff. Returns the
/// exact mismatch count when it is at most `cutoff`, or `cutoff + 1` (a
/// sentinel meaning "exceeded") when the true count is strictly greater.
///
/// The sentinel-return shape matches what the arch-specific backends can
/// cheaply produce: they process one block at a time and can stop as soon
/// as the running count exceeds `cutoff`, without needing to compute the
/// exact final count.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
#[must_use]
pub fn distance_within(a: &[u8], b: &[u8], cutoff: u32) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::scalar::distance_within requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let mut mismatches: u32 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        if x != y {
            mismatches = mismatches.saturating_add(1);
            if mismatches > cutoff {
                return mismatches;
            }
        }
    }
    mismatches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pair_is_zero() {
        assert_eq!(distance(b"", b""), 0);
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(distance(b"abcdef", b"abcdef"), 0);
    }

    #[test]
    fn all_differ_equals_length() {
        assert_eq!(distance(b"abc", b"xyz"), 3);
    }

    #[test]
    fn canonical_pairs_match_hand_counts() {
        assert_eq!(distance(b"karolin", b"kathrin"), 3);
        assert_eq!(distance(b"karolin", b"kerstin"), 3);
        assert_eq!(distance(b"1011101", b"1001001"), 2);
    }

    #[test]
    #[should_panic(expected = "equal-length")]
    fn panics_on_unequal_length() {
        let _ = distance(b"abc", b"abcd");
    }

    #[test]
    fn within_reports_exact_below_cutoff() {
        assert_eq!(distance_within(b"karolin", b"kathrin", 5), 3);
    }

    #[test]
    fn within_reports_sentinel_above_cutoff() {
        // True distance = 3, cutoff = 2 → returns 3 (> cutoff = "exceeded").
        assert_eq!(distance_within(b"karolin", b"kathrin", 2), 3);
    }

    #[test]
    fn within_at_cutoff_is_exact() {
        // True distance = 3, cutoff = 3 → returns 3 (== cutoff, not exceeded).
        assert_eq!(distance_within(b"karolin", b"kathrin", 3), 3);
    }
}
