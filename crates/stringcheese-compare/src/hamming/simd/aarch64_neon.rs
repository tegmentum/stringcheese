//! NEON-gated Hamming kernel for `aarch64`.
//!
//! This module compiles only on `aarch64` targets. NEON is part of the
//! aarch64 baseline, so the dispatcher's `is_aarch64_feature_detected!`
//! check is defensive rather than gating.
//!
//! # Algorithm
//!
//! Same shape as the SSE2 sibling with a 16-byte block width and NEON's
//! native movemask alternative:
//!
//! * Load a 16-byte block from each side with `vld1q_u8`.
//! * Compare with `vceqq_u8` (0xff where equal, 0x00 where different).
//! * Shift each lane right by 7 with `vshrq_n_u8::<7>` to collapse 0xff to
//!   1 and 0x00 to 0 — one bit of information per lane.
//! * Widen-and-reduce with `vaddlvq_u8`, which sums the 16 u8 lanes into
//!   a single `u16` value in `0..=16` (the block's match count).
//! * Block mismatch count = `BLOCK - matches`.
//! * Tail (fewer than 16 bytes) runs as a scalar byte loop.
//!
//! `vaddlvq_u8` is the load-bearing choice here: it's the widening
//! horizontal add (vs. `vaddvq_u8` which returns `u8` and would wrap
//! silently for a fully-matched block). The result fits in `u16` because
//! 16 lanes × 1 bit each ≤ 16.
//!
//! # Safety
//!
//! [`distance`] and [`distance_within`] are `unsafe fn` because
//! `#[target_feature(enable = "neon")]` functions have a documented
//! precondition — NEON must be enabled at run time. On `aarch64` NEON is
//! guaranteed by the standard ABI; the dispatcher checks it anyway for
//! uniformity across architectures.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use core::arch::aarch64::{vaddlvq_u8, vceqq_u8, vld1q_u8, vshrq_n_u8};

/// NEON block width in bytes — one `uint8x16_t` per iteration of the
/// inner loop.
const BLOCK: usize = 16;

/// NEON block width as `u32` — used inside the hot loop for the
/// `BLOCK - matches` mismatch count.
const BLOCK_U32: u32 = 16;

/// NEON-gated Hamming distance for equal-length byte slices.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the standard ABI; the dispatcher still checks
/// `std::arch::is_aarch64_feature_detected!("neon")` for uniformity.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::aarch64_neon::distance requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: `off + BLOCK <= len` guarantees both 16-byte reads
        // stay inside their respective slices; NEON is enabled by
        // this function's `#[target_feature]`. The vld1q_u8 loads are
        // the only unsafe intrinsics here — vceqq_u8, vshrq_n_u8, and
        // vaddlvq_u8 are safe under a NEON target-feature context on
        // Rust 1.87+.
        let va = unsafe { vld1q_u8(a.as_ptr().add(off)) };
        let vb = unsafe { vld1q_u8(b.as_ptr().add(off)) };
        let eq = vceqq_u8(va, vb);
        // 0xff → 1, 0x00 → 0 per lane.
        let ones = vshrq_n_u8::<7>(eq);
        // Widening horizontal add — sums 16 u8 lanes (each 0 or 1) into
        // a single `u16` in `0..=16`.
        let matches = u32::from(vaddlvq_u8(ones));
        mismatches = mismatches.saturating_add(BLOCK_U32 - matches);
        off += BLOCK;
    }
    while off < len {
        if a[off] != b[off] {
            mismatches = mismatches.saturating_add(1);
        }
        off += 1;
    }
    mismatches
}

/// NEON-gated Hamming distance with an early-termination cutoff.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// The caller must ensure NEON is available.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn distance_within(a: &[u8], b: &[u8], cutoff: u32) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::aarch64_neon::distance_within requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: see `distance`.
        let va = unsafe { vld1q_u8(a.as_ptr().add(off)) };
        let vb = unsafe { vld1q_u8(b.as_ptr().add(off)) };
        let eq = vceqq_u8(va, vb);
        let ones = vshrq_n_u8::<7>(eq);
        let matches = u32::from(vaddlvq_u8(ones));
        mismatches = mismatches.saturating_add(BLOCK_U32 - matches);
        if mismatches > cutoff {
            return mismatches;
        }
        off += BLOCK;
    }
    while off < len {
        if a[off] != b[off] {
            mismatches = mismatches.saturating_add(1);
            if mismatches > cutoff {
                return mismatches;
            }
        }
        off += 1;
    }
    mismatches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hamming::simd::scalar;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b"a"),
            (b"karolin", b"kathrin"),
            (b"1011101", b"1001001"),
            (b"abc", b"xyz"),
        ];
        for (a, b) in cases {
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { distance(a, b) };
            let scalar_ref = scalar::distance(a, b);
            assert_eq!(
                simd, scalar_ref,
                "neon disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn differential_across_lengths() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for len in 0..=200usize {
            let a: alloc::vec::Vec<u8> = (0..len)
                .map(|i| u8::try_from(i & 0xff).unwrap().wrapping_mul(31))
                .collect();
            let mut b = a.clone();
            for &pos in &[0usize, 7, 15, 16, 31, 32, 63, 64, 100, 127] {
                if pos < len {
                    b[pos] ^= 0x5A;
                }
            }
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { distance(&a, &b) };
            let scalar_ref = scalar::distance(&a, &b);
            assert_eq!(simd, scalar_ref, "at len={len}");
        }
    }

    /// Long fully-matched input — every block contributes `BLOCK` matches
    /// (which is 16, well below `u8::MAX` per lane but the widening add
    /// is what makes the whole-block match count representable).
    #[test]
    fn long_identical_inputs_report_zero() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a = alloc::vec![0x42u8; 4096];
        let b = a.clone();
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance(&a, &b) };
        assert_eq!(simd, 0);
    }

    /// Long fully-mismatched input — every block contributes zero matches.
    #[test]
    fn long_disjoint_inputs_report_length() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a = alloc::vec![0x00u8; 4096];
        let b = alloc::vec![0xffu8; 4096];
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance(&a, &b) };
        assert_eq!(simd, 4096);
    }

    #[test]
    fn distance_within_matches_scalar_below_cutoff() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..100u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[50] ^= 0x02;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance_within(&a, &b, 10) };
        assert_eq!(simd, 2);
    }

    #[test]
    fn distance_within_reports_exceeded_above_cutoff() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a = alloc::vec![0u8; 100];
        let b = alloc::vec![0xffu8; 100];
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance_within(&a, &b, 5) };
        assert!(simd > 5);
    }
}
