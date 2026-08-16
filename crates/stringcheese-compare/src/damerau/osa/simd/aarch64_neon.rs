//! NEON-gated OSA (restricted Damerau-Levenshtein) kernel for `aarch64`.
//!
//! This module compiles only on `aarch64` targets. NEON is part of the
//! aarch64 baseline, so the dispatcher's `is_aarch64_feature_detected!`
//! check is defensive rather than gating.
//!
//! # Algorithm
//!
//! The kernel implements Hyyrö's bit-parallel OSA algorithm — Myers's
//! word-parallel Levenshtein extended with an extra bit-vector `Pm_old`
//! (the previous column's `Peq[text[j-1]]`) and a diagonal-zero vector
//! `D0`. See Hyyrö (2003), "Bit-parallel approximate string matching
//! algorithms with transposition" (SPIRE 2003).
//!
//! * For `m ≤ 64` the NEON register width is wasted; this path delegates
//!   to [`super::scalar`].
//! * For `64 < m ≤ 128` we pack `Pv`, `Mv`, `D0`, `Pm_old`, and each
//!   `Peq[c]` entry into `uint64x2_t` values and run the Hyyrö inner
//!   loop with a 128-bit integer add and a 128-bit shift-left-by-one
//!   supplying the cross-lane carries.
//! * For `m > 128` the pattern no longer fits in one NEON register;
//!   this path delegates back to [`super::scalar`].
//!
//! # NEON-specific carry mechanics
//!
//! Identical to the Levenshtein NEON backend (see
//! `crate::levenshtein::simd::myers_aarch64_neon`): the full 128-bit
//! add uses `vgetq_lane_u64` + `overflowing_add` + `vsetq_lane_u64`, and
//! the shift-left-by-1 uses `vshlq_n_u64<1>` + `vshrq_n_u64<63>` +
//! `vextq_u64<1>` to move the low lane's outgoing bit into the high
//! lane's bit-0.
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
#![allow(
    clippy::similar_names,
    reason = "the Hyyrö recurrence has structurally similar names — `hp`/`hn`, `hp_shifted`/`hn_shifted`, `d0_or_hp`/`d0_or_vp` — that map 1:1 to the paper's variables; renaming them to be more distinct would put a translation layer between the code and the derivation"
)]

use core::arch::aarch64::{
    uint64x2_t, vandq_u64, vbicq_u64, vdupq_n_u64, veorq_u64, vextq_u64, vgetq_lane_u64, vld1q_u64,
    vorrq_u64, vsetq_lane_u64, vshlq_n_u64, vshrq_n_u64,
};

use super::scalar;

const W_SCALAR: usize = 64;
const W_NEON: usize = 128;

/// NEON-gated OSA distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the standard ABI, but the dispatcher still checks
/// `std::arch::is_aarch64_feature_detected!("neon")` for uniformity.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols.
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
        return scalar::distance(a, b);
    }

    // SAFETY: NEON target-feature context established by this function's
    // `#[target_feature(enable = "neon")]` upholds the NEON precondition
    // of `wide_block_128`.
    unsafe { wide_block_128(pattern, text, m) }
}

/// Wide-block Hyyrö-OSA for `64 < m ≤ 128`. Every bit-vector variable
/// lives in a `uint64x2_t`; carry propagation between the two 64-bit
/// lanes is explicit.
///
/// # Safety
///
/// The caller must ensure NEON is available.
#[target_feature(enable = "neon")]
unsafe fn wide_block_128(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SCALAR && m <= W_NEON);

    let mut peq: alloc::vec::Vec<u64> = alloc::vec![0u64; 512];
    for (i, &c) in pattern.iter().enumerate() {
        let idx = (c as usize) * 2 + i / W_SCALAR;
        let bit = i % W_SCALAR;
        peq[idx] |= 1u64 << bit;
    }

    let msb_bit = m - 1;
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    // SAFETY: NEON target-feature context established by the containing
    // `#[target_feature(enable = "neon")]` — every intrinsic below is
    // NEON.
    unsafe {
        let all_ones = vdupq_n_u64(u64::MAX);
        let mut pv = all_ones;
        let mut mv = vdupq_n_u64(0);
        let mut d0 = vdupq_n_u64(0);
        let mut pm_old = vdupq_n_u64(0);
        let one_lo = vsetq_lane_u64::<0>(1, vdupq_n_u64(0));

        for &c in text {
            let pm_ptr = peq.as_ptr().add((c as usize) * 2);
            let pm_j = vld1q_u64(pm_ptr);

            // Transposition contribution:
            //   tr = shl1((~D0_prev) & pm_j) & pm_old
            // NEON `vbicq_u64(a, b) = a & ~b`, so `~D0_prev & pm_j`
            // = `vbicq_u64(pm_j, D0_prev)`.
            let not_d0_and_pm = vbicq_u64(pm_j, d0);
            let tr_shifted = shl1(not_d0_and_pm);
            let tr = vandq_u64(tr_shifted, pm_old);

            // Myers D0 with `| vn | tr`:
            //   d0 = (((pm_j & vp) + vp) ^ vp) | pm_j | vn | tr
            let pm_and_vp = vandq_u64(pm_j, pv);
            let sum = add128(pm_and_vp, pv);
            let xor = veorq_u64(sum, pv);
            let d0_new = vorrq_u64(vorrq_u64(xor, pm_j), vorrq_u64(mv, tr));

            // Hp = vn | ~(d0 | vp); Hn = d0 & vp
            //   ~(d0 | vp) = vbicq_u64(all_ones, d0 | vp)
            let d0_or_vp = vorrq_u64(d0_new, pv);
            let hp = vorrq_u64(mv, vbicq_u64(all_ones, d0_or_vp));
            let hn = vandq_u64(d0_new, pv);

            if bit_at(hp, msb_bit) {
                score += 1;
            }
            if bit_at(hn, msb_bit) {
                score -= 1;
            }

            // hp = (hp << 1) | 1; hn <<= 1
            let hp_shifted = vorrq_u64(shl1(hp), one_lo);
            let hn_shifted = shl1(hn);

            // vp = hn_shifted | ~(d0 | hp_shifted); vn = hp_shifted & d0
            let d0_or_hp = vorrq_u64(d0_new, hp_shifted);
            let pv_new = vorrq_u64(hn_shifted, vbicq_u64(all_ones, d0_or_hp));
            let mv_new = vandq_u64(hp_shifted, d0_new);

            d0 = d0_new;
            pm_old = pm_j;
            pv = pv_new;
            mv = mv_new;
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
    // Every NEON intrinsic used here is safe on aarch64 — the outer
    // `unsafe fn` covers only the `#[target_feature]` precondition,
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
    // Register-only NEON intrinsics — see `add128`.
    let shifted = vshlq_n_u64::<1>(v);
    let top_bits = vshrq_n_u64::<63>(v);
    let zero = vdupq_n_u64(0);
    // `vextq_u64<1>(zero, top_bits)` concatenates
    // `[zero_lo, zero_hi, top_bits_lo, top_bits_hi]` and reads two
    // lanes starting from index 1, giving `[zero_hi, top_bits_lo]` —
    // the low lane's bit-63 placed at the high lane's bit-0.
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
    // Register-only NEON intrinsics — see `add128`.
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
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { distance(a, b) };
            let sc = scalar::distance(a, b);
            assert_eq!(simd, sc, "neon disagreed with scalar on ({a:?}, {b:?})");
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
        b.swap(30, 31);
        b[64] ^= 0x01;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 128` — exact register width.
    #[test]
    fn wide_block_matches_scalar_at_m_128() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b.swap(0, 1);
        b[63] ^= 0x02;
        b.swap(64, 65);
        b[127] ^= 0x08;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 129` — first pattern length past the NEON
    /// wide-block range. Delegates to the scalar rolling-rows fallback.
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
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Differential across the full m ∈ [1, 200] range against the
    /// SIMD-shaped scalar OSA. Every pattern length must agree
    /// bit-for-bit.
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
            if b.len() >= 2 {
                b.swap(m / 2, m / 2 - 1);
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
            let sc1 = scalar::distance(&a, &b);
            assert_eq!(simd1, sc1, "at m={m} on (a, b)");
            // SAFETY: same guard as `simd1` above — NEON is available.
            let simd2 = unsafe { distance(&a, &text_ext) };
            let sc2 = scalar::distance(&a, &text_ext);
            assert_eq!(simd2, sc2, "at m={m} on (a, text_ext)");
        }
    }

    /// Adjacent-transposition stress on a small alphabet.
    #[test]
    fn small_alphabet_transposition_stress() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for seed in 0u32..64 {
            let mut a = alloc::vec![0u8; 96];
            let mut b = alloc::vec![0u8; 96];
            for (i, cell) in a.iter_mut().enumerate() {
                *cell = ((seed.wrapping_add(u32::try_from(i).unwrap())) % 3) as u8;
            }
            for (i, cell) in b.iter_mut().enumerate() {
                *cell = ((seed
                    .wrapping_mul(7)
                    .wrapping_add(u32::try_from(i).unwrap().wrapping_mul(11)))
                    % 3) as u8;
            }
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { distance(&a, &b) };
            let sc = scalar::distance(&a, &b);
            assert_eq!(simd, sc, "seed={seed}");
        }
    }
}
