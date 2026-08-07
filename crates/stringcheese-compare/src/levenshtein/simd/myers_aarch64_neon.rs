//! NEON-gated Myers Levenshtein kernel for `aarch64`.
//!
//! This module compiles only on `aarch64` targets. NEON is part of the
//! aarch64 baseline, so the dispatcher's `is_aarch64_feature_detected!`
//! check is defensive rather than gating.
//!
//! # Algorithm
//!
//! The kernel implements Myers's bit-parallel edit-distance algorithm
//! (Myers, JACM 1999, §4) extended to a 128-bit column state via Hyyrö's
//! wide-block reformulation (Hyyrö, 2003). Concretely:
//!
//! * For patterns of length `m ≤ 64` the NEON register width is wasted;
//!   this path delegates straight to [`super::myers_scalar`], which uses
//!   a single `u64` and pays the smaller Peq-table build cost.
//! * For `64 < m ≤ 128` we pack `Pv`, `Mv`, and each `Peq[c]` entry into
//!   `uint64x2_t` values and run the same six-op inner loop with a
//!   128-bit integer add and a 128-bit shift-left-by-one supplying the
//!   cross-lane carries.
//! * For `m > 128` the pattern no longer fits in one NEON register; this
//!   path delegates back to [`super::myers_scalar`] which in turn falls
//!   back to a rolling-rows DP.
//!
//! # NEON-specific carry mechanics
//!
//! NEON's `vaddq_u64` is per-lane 64-bit and does not surface carries.
//! The full-width 128-bit add is done by extracting the two `u64` halves
//! with `vgetq_lane_u64`, chaining an `overflowing_add`, and reassembling
//! with `vsetq_lane_u64`. The full-width shift-left-by-1 uses `vshlq_n_u64`
//! for the per-lane shift, `vshrq_n_u64` to isolate each lane's bit-63,
//! and `vextq_u64(zero, carry, 1)` to align the low lane's bit-63 into
//! the high lane's bit-0.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. On `aarch64`, NEON is guaranteed by the
//! standard ABI; the dispatcher checks it anyway for uniformity across
//! architectures.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]

use core::arch::aarch64::{
    uint64x2_t, vandq_u64, vdupq_n_u64, veorq_u64, vextq_u64, vgetq_lane_u64, vld1q_u64, vorrq_u64,
    vsetq_lane_u64, vshlq_n_u64, vshrq_n_u64,
};

use super::myers_scalar;

/// Machine-word width used by the scalar single-word path.
const W_SCALAR: usize = 64;

/// Widest pattern length the NEON wide-block path can handle.
const W_NEON: usize = 128;

/// NEON-gated Levenshtein distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the standard ABI, but the dispatcher still checks
/// `std::arch::is_aarch64_feature_detected!("neon")` for uniformity.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols — matches
/// the panic contract of the crate's other DP kernels.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    let (pattern, text) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let m = pattern.len();
    let n = text.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }

    if m <= W_SCALAR || m > W_NEON {
        // Below the wide-block break-even the scalar single-word path
        // wins; above the register width the pattern doesn't fit in one
        // 128-bit block, so delegate to the scalar module which handles
        // both cases.
        return myers_scalar::distance(a, b);
    }

    // SAFETY: the NEON target-feature attribute on this function upholds
    // the NEON precondition of `wide_block_128`.
    unsafe { wide_block_128(pattern, text, m) }
}

/// Wide-block Myers for `64 < m ≤ 128`. Every state variable lives in a
/// `uint64x2_t`; carry propagation between the two 64-bit lanes is
/// explicit.
///
/// # Safety
///
/// The caller must ensure NEON is available.
#[target_feature(enable = "neon")]
unsafe fn wide_block_128(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SCALAR && m <= W_NEON);

    // Peq is stored as an interleaved `[u64; 256 × 2]`, low lane then
    // high lane per byte value. This layout lets the inner loop pull
    // each `Peq[c]` with a single 128-bit load.
    let mut peq: alloc::vec::Vec<u64> = alloc::vec![0u64; 512];
    for (i, &c) in pattern.iter().enumerate() {
        let idx = (c as usize) * 2 + i / W_SCALAR;
        let bit = i % W_SCALAR;
        peq[idx] |= 1u64 << bit;
    }

    let hi_mask: u64 = if m == W_NEON {
        u64::MAX
    } else {
        (1u64 << (m - W_SCALAR)) - 1
    };
    let msb_bit = m - 1;
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    // SAFETY: NEON target-feature context established by the containing
    // `#[target_feature(enable = "neon")]` — every intrinsic below is
    // NEON.
    unsafe {
        // Initial Pv = [u64::MAX, hi_mask] (lane 0 is low bits).
        let mut pv = vsetq_lane_u64::<1>(hi_mask, vdupq_n_u64(u64::MAX));
        let mut mv = vdupq_n_u64(0);
        let all_ones = vdupq_n_u64(u64::MAX);
        // Constant vector [1, 0] to inject `1` into bit 0 of shifted Ph.
        let one_lo = vsetq_lane_u64::<0>(1, vdupq_n_u64(0));

        for &c in text {
            let eq_ptr = peq.as_ptr().add((c as usize) * 2);
            let eq = vld1q_u64(eq_ptr);

            let xv = vorrq_u64(eq, mv);

            let eq_and_pv = vandq_u64(eq, pv);
            let sum = add128(eq_and_pv, pv);
            let xh = vorrq_u64(veorq_u64(sum, pv), eq);

            let ph = vorrq_u64(mv, veorq_u64(vorrq_u64(xh, pv), all_ones));
            let mh = vandq_u64(pv, xh);

            if bit_at(ph, msb_bit) {
                score += 1;
            }
            if bit_at(mh, msb_bit) {
                score -= 1;
            }

            let ph_shifted = vorrq_u64(shl1(ph), one_lo);
            let mh_shifted = shl1(mh);

            pv = vorrq_u64(mh_shifted, veorq_u64(vorrq_u64(xv, ph_shifted), all_ones));
            mv = vandq_u64(ph_shifted, xv);
        }

        score
    }
}

/// 128-bit big-integer add, wrapping at 2^128.
///
/// # Safety
///
/// NEON must be available.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn add128(a: uint64x2_t, b: uint64x2_t) -> uint64x2_t {
    // Every NEON intrinsic used here is stably safe on aarch64 — the
    // outer `unsafe fn` covers only the `#[target_feature]` precondition,
    // which the caller upholds.
    let a_lo = vgetq_lane_u64::<0>(a);
    let a_hi = vgetq_lane_u64::<1>(a);
    let b_lo = vgetq_lane_u64::<0>(b);
    let b_hi = vgetq_lane_u64::<1>(b);
    let (s_lo, carry) = a_lo.overflowing_add(b_lo);
    let s_hi = a_hi.wrapping_add(b_hi).wrapping_add(u64::from(carry));
    let out = vsetq_lane_u64::<0>(s_lo, vdupq_n_u64(0));
    vsetq_lane_u64::<1>(s_hi, out)
}

/// Shift a full 128-bit value left by one bit; the outgoing bit 127 is
/// dropped.
///
/// # Safety
///
/// NEON must be available.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn shl1(v: uint64x2_t) -> uint64x2_t {
    // Every NEON intrinsic used here is stably safe on aarch64 — the
    // outer `unsafe fn` covers only the `#[target_feature]` precondition,
    // which the caller upholds.
    //
    // Per-lane 64-bit shift-left by 1.
    let shifted = vshlq_n_u64::<1>(v);
    // Extract bit 63 of each lane into bit 0 of that lane.
    let top_bits = vshrq_n_u64::<63>(v);
    // `vextq_u64(zero, top_bits, 1)` concatenates
    // `[zero_lo, zero_hi, top_bits_lo, top_bits_hi]` and reads two lanes
    // starting from index 1, giving `[zero_hi, top_bits_lo]` — the low
    // lane's bit-63 placed at the high lane's bit-0.
    let zero = vdupq_n_u64(0);
    let carry = vextq_u64::<1>(zero, top_bits);
    vorrq_u64(shifted, carry)
}

/// Extract bit `i` (0..128) from a 128-bit vector.
///
/// # Safety
///
/// NEON must be available.
#[inline]
#[target_feature(enable = "neon")]
unsafe fn bit_at(v: uint64x2_t, i: usize) -> bool {
    debug_assert!(i < W_NEON);
    // Every NEON intrinsic used here is stably safe on aarch64 — the
    // outer `unsafe fn` covers only the `#[target_feature]` precondition,
    // which the caller upholds.
    if i < W_SCALAR {
        (vgetq_lane_u64::<0>(v) >> i) & 1 != 0
    } else {
        (vgetq_lane_u64::<1>(v) >> (i - W_SCALAR)) & 1 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { distance(a, b) };
            let scalar = myers_scalar::distance(a, b);
            assert_eq!(simd, scalar, "neon disagreed with scalar on ({a:?}, {b:?})");
        }
    }

    /// Boundary at `m = 65` — first pattern length past the scalar
    /// single-word cutoff. Exercises the wide-block code path.
    #[test]
    fn wide_block_matches_scalar_at_m_65() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b[64] ^= 0x01;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
        assert_eq!(simd, 1);
    }

    /// Boundary at `m = 128` — exact register width. Exercises the
    /// `m == W_NEON` special-case for the initial `Pv`.
    #[test]
    fn wide_block_matches_scalar_at_m_128() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x80;
        b[127] ^= 0x80;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Boundary at `m = 129` — first pattern length past the NEON wide-
    /// block range. Delegates to the scalar rolling-rows fallback.
    #[test]
    fn wide_block_delegates_past_m_128() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..129u8).collect();
        let mut b = a.clone();
        b[128] ^= 0x01;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Differential across the full m ∈ [1, 200] range against
    /// scalar Myers. Every pattern length must agree bit-for-bit.
    #[test]
    fn differential_across_lengths() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for m in 1..=200usize {
            let a: alloc::vec::Vec<u8> = (0..m)
                .map(|i| u8::try_from(i & 0xff).unwrap().wrapping_mul(31))
                .collect();
            let mut b = a.clone();
            if !b.is_empty() {
                b[m / 2] ^= 0x5A;
            }
            let text_ext: alloc::vec::Vec<u8> = (0..(m + 17))
                .map(|i| {
                    u8::try_from(i & 0xff)
                        .unwrap()
                        .wrapping_mul(17)
                        .wrapping_add(3)
                })
                .collect();
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd1 = unsafe { distance(&a, &b) };
            let scalar1 = myers_scalar::distance(&a, &b);
            assert_eq!(simd1, scalar1, "at m={m} on (a, b)");
            // SAFETY: same as above.
            let simd2 = unsafe { distance(&a, &text_ext) };
            let scalar2 = myers_scalar::distance(&a, &text_ext);
            assert_eq!(simd2, scalar2, "at m={m} on (a, text_ext)");
        }
    }
}
