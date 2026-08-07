//! SSE2-gated OSA (restricted Damerau-Levenshtein) kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. It is the SSE2 fallback
//! selected by the dispatcher when AVX2 is unavailable — every `x86_64`
//! CPU has SSE2 as part of the baseline ABI, so this branch is always a
//! valid target.
//!
//! # Algorithm
//!
//! The kernel implements Hyyrö's bit-parallel OSA algorithm — Myers's
//! word-parallel Levenshtein extended with an extra bit-vector `Pm_old`
//! (the previous column's `Peq[text[j-1]]`) and a diagonal-zero vector
//! `D0`. See Hyyrö (2003), "Bit-parallel approximate string matching
//! algorithms with transposition" (SPIRE 2003) for the derivation, and
//! [`super::scalar`] for the rolling-rows reference.
//!
//! Concretely:
//!
//! * For patterns of length `m ≤ 64` the SSE2 register width is wasted;
//!   this path delegates to [`super::scalar`] which uses the rolling-rows
//!   DP for correctness anchor.
//! * For `64 < m ≤ 128` we pack `Pv`, `Mv`, `D0`, `Pm_old`, and each
//!   `Peq[c]` entry into `__m128i` values and run the six-operation
//!   Hyyrö inner loop with a 128-bit integer add and a 128-bit
//!   shift-left-by-one supplying the cross-lane carries.
//! * For `m > 128` the pattern no longer fits in one SSE2 register; this
//!   path delegates back to [`super::scalar`]. The AVX2 sibling extends
//!   the bit-parallel range to `m ≤ 256`.
//!
//! # SSE2-specific carry mechanics
//!
//! Identical to the Levenshtein SSE2 backend (see
//! `crate::levenshtein::simd::myers_x86_sse2`): SSE2's `_mm_add_epi64`
//! is per-lane 64-bit and doesn't surface carries, so we extract, chain
//! `overflowing_add`, and reassemble with `_mm_set_epi64x`. The full
//! 128-bit shift-left-by-1 uses `_mm_slli_epi64` for the per-lane shift
//! plus `_mm_srli_epi64(_, 63)` + `_mm_slli_si128(_, 8)` to move the
//! low lane's outgoing bit into the high lane's bit-0.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature must
//! be present at run time. On `x86_64`, SSE2 is guaranteed by the ABI;
//! the dispatcher checks it anyway for consistency with the other arch
//! branches.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "the SSE2 x86 intrinsics for 64-bit-lane values (`_mm_cvtsi128_si64`, `_mm_set_epi64x`) take/return `i64`; the wide-block Myers state is unsigned by construction, so `as u64` / `as i64` at those boundaries is a pure bit-transmute, not a numeric conversion"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "every pointer cast in this module feeds an *unaligned* SSE2 load (`_mm_loadu_si128`), which by contract accepts any-alignment `*const __m128i`; the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]
#![allow(
    clippy::similar_names,
    reason = "the Hyyrö recurrence has structurally similar names — `hp`/`hn`, `hp_shifted`/`hn_shifted`, `d0_or_hp`/`d0_or_vp` — that map 1:1 to the paper's variables; renaming them to be more distinct would put a translation layer between the code and the derivation"
)]

use core::arch::x86_64::{
    __m128i, _mm_and_si128, _mm_andnot_si128, _mm_cvtsi128_si64, _mm_loadu_si128, _mm_or_si128,
    _mm_set_epi64x, _mm_set1_epi64x, _mm_setzero_si128, _mm_slli_epi64, _mm_slli_si128,
    _mm_srli_epi64, _mm_srli_si128, _mm_xor_si128,
};

use super::scalar;

/// Machine-word width used by the scalar single-word paths.
const W_SCALAR: usize = 64;

/// Widest pattern length the SSE2 wide-block path handles.
const W_SSE2: usize = 128;

/// SSE2-gated OSA distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` this is
/// guaranteed by the ABI, but the dispatcher still checks
/// `is_x86_feature_detected!("sse2")` to keep every dispatch branch
/// uniform.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols — matches
/// the panic contract of the crate's other DP kernels.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    let (pattern, text) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let m = pattern.len();
    let n = text.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }

    if m <= W_SCALAR || m > W_SSE2 {
        // Below the wide-block break-even the scalar single-word path
        // wins; above the register width the pattern doesn't fit in one
        // 128-bit block. Fall back to the SIMD-shaped scalar OSA which
        // uses rolling-rows DP.
        return scalar::distance(a, b);
    }

    // SAFETY: SSE2 target-feature context established by this function's
    // `#[target_feature(enable = "sse2")]` upholds the SSE2 precondition
    // of `wide_block_128`.
    unsafe { wide_block_128(pattern, text, m) }
}

/// Wide-block Hyyrö-OSA for `64 < m ≤ 128`. Every bit-vector variable
/// lives in an `__m128i`; carry propagation between the two 64-bit
/// lanes is explicit.
///
/// # Safety
///
/// The caller must ensure SSE2 is available.
#[target_feature(enable = "sse2")]
unsafe fn wide_block_128(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SCALAR && m <= W_SSE2);

    // Peq stored as an interleaved `[u64; 256 × 2]` — low lane then
    // high lane per byte value. Single unaligned 128-bit load per
    // symbol.
    let mut peq: alloc::vec::Vec<u64> = alloc::vec![0u64; 512];
    for (i, &c) in pattern.iter().enumerate() {
        let idx = (c as usize) * 2 + i / W_SCALAR;
        let bit = i % W_SCALAR;
        peq[idx] |= 1u64 << bit;
    }

    let msb_bit = m - 1;
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    // SAFETY: SSE2 target-feature context established by the containing
    // `#[target_feature(enable = "sse2")]` — every intrinsic below is
    // SSE2.
    unsafe {
        // Hyyrö's OSA uses `Vp` initialized to all-ones (see the
        // rapidfuzz-rs `hyrroe2003` reference). High bits above the
        // pattern length are canceled by the mask on the score-update
        // step below.
        let mut pv = _mm_set1_epi64x(-1);
        let mut mv = _mm_setzero_si128();
        let mut d0 = _mm_setzero_si128();
        let mut pm_old = _mm_setzero_si128();
        let one_lo = _mm_set_epi64x(0, 1);

        for &c in text {
            let pm_ptr = peq.as_ptr().add((c as usize) * 2).cast::<__m128i>();
            let pm_j = _mm_loadu_si128(pm_ptr);

            // Transposition contribution:
            //   tr = shl1((~D0_prev) & pm_j) & pm_old
            let not_d0_and_pm = _mm_andnot_si128(d0, pm_j);
            let tr_shifted = shl1(not_d0_and_pm);
            let tr = _mm_and_si128(tr_shifted, pm_old);

            // Myers D0 with `| vn | tr`:
            //   d0 = (((pm_j & vp) + vp) ^ vp) | pm_j | vn | tr
            let pm_and_vp = _mm_and_si128(pm_j, pv);
            let sum = add128(pm_and_vp, pv);
            let xor = _mm_xor_si128(sum, pv);
            let d0_new = _mm_or_si128(_mm_or_si128(xor, pm_j), _mm_or_si128(mv, tr));

            // Hp = vn | ~(d0 | vp); Hn = d0 & vp
            let d0_or_vp = _mm_or_si128(d0_new, pv);
            let hp = _mm_or_si128(mv, _mm_andnot_si128(d0_or_vp, _mm_set1_epi64x(-1)));
            let hn = _mm_and_si128(d0_new, pv);

            // Score update via MSB check.
            if bit_at(hp, msb_bit) {
                score += 1;
            }
            if bit_at(hn, msb_bit) {
                score -= 1;
            }

            // hp = (hp << 1) | 1; hn <<= 1
            let hp_shifted = _mm_or_si128(shl1(hp), one_lo);
            let hn_shifted = shl1(hn);

            // vp = hn_shifted | ~(d0 | hp_shifted); vn = hp_shifted & d0
            let d0_or_hp = _mm_or_si128(d0_new, hp_shifted);
            let pv_new = _mm_or_si128(hn_shifted, _mm_andnot_si128(d0_or_hp, _mm_set1_epi64x(-1)));
            let mv_new = _mm_and_si128(hp_shifted, d0_new);

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
/// SSE2 must be available.
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn add128(a: __m128i, b: __m128i) -> __m128i {
    // Register-only SSE2 intrinsics — no memory access. The outer
    // `unsafe fn` covers only the `#[target_feature]` precondition,
    // which the caller upholds.
    unsafe {
        let a_lo = _mm_cvtsi128_si64(a) as u64;
        let a_hi = _mm_cvtsi128_si64(_mm_srli_si128(a, 8)) as u64;
        let b_lo = _mm_cvtsi128_si64(b) as u64;
        let b_hi = _mm_cvtsi128_si64(_mm_srli_si128(b, 8)) as u64;
        let (s_lo, carry) = a_lo.overflowing_add(b_lo);
        let s_hi = a_hi.wrapping_add(b_hi).wrapping_add(u64::from(carry));
        _mm_set_epi64x(s_hi as i64, s_lo as i64)
    }
}

/// Shift a full 128-bit value left by one bit; the outgoing bit 127 is
/// dropped.
///
/// # Safety
///
/// SSE2 must be available.
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn shl1(v: __m128i) -> __m128i {
    unsafe {
        let shifted = _mm_slli_epi64(v, 1);
        let top_bits = _mm_srli_epi64(v, 63);
        let carry = _mm_slli_si128(top_bits, 8);
        _mm_or_si128(shifted, carry)
    }
}

/// Extract bit `i` (0..128) from a 128-bit vector.
///
/// # Safety
///
/// SSE2 must be available.
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn bit_at(v: __m128i, i: usize) -> bool {
    debug_assert!(i < W_SSE2);
    unsafe {
        if i < W_SCALAR {
            let lo = _mm_cvtsi128_si64(v) as u64;
            (lo >> i) & 1 != 0
        } else {
            let hi = _mm_cvtsi128_si64(_mm_srli_si128(v, 8)) as u64;
            (hi >> (i - W_SCALAR)) & 1 != 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !is_x86_feature_detected!("sse2") {
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
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { distance(a, b) };
            let sc = scalar::distance(a, b);
            assert_eq!(simd, sc, "sse2 disagreed with scalar on ({a:?}, {b:?})");
        }
    }

    /// Boundary at `m = 65` — first pattern length past the scalar
    /// single-word cutoff. Exercises the wide-block code path.
    #[test]
    fn wide_block_matches_scalar_at_m_65() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b.swap(30, 31);
        b[64] ^= 0x01;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 128` — exact register width. Exercises the
    /// full-register initial `Pv`.
    #[test]
    fn wide_block_matches_scalar_at_m_128() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b.swap(0, 1);
        b[63] ^= 0x02;
        b.swap(64, 65);
        b[127] ^= 0x08;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 129` — first pattern length past the SSE2 wide-
    /// block range. Delegates to the scalar rolling-rows fallback.
    #[test]
    fn wide_block_delegates_past_m_128() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..129u8).collect();
        let mut b = a.clone();
        b[128] ^= 0x01;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Differential across the full m ∈ [1, 200] range against the
    /// SIMD-shaped scalar OSA. Every pattern length must agree
    /// bit-for-bit.
    #[test]
    fn differential_across_lengths() {
        if !is_x86_feature_detected!("sse2") {
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
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd1 = unsafe { distance(&a, &b) };
            let sc1 = scalar::distance(&a, &b);
            assert_eq!(simd1, sc1, "at m={m} on (a, b)");
            let simd2 = unsafe { distance(&a, &text_ext) };
            let sc2 = scalar::distance(&a, &text_ext);
            assert_eq!(simd2, sc2, "at m={m} on (a, text_ext)");
        }
    }

    /// Adjacent-transposition stress on a small alphabet — the
    /// transposition branch fires often, so Hyyrö's cross-column
    /// tracking is heavily exercised here.
    #[test]
    fn small_alphabet_transposition_stress() {
        if !is_x86_feature_detected!("sse2") {
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
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { distance(&a, &b) };
            let sc = scalar::distance(&a, &b);
            assert_eq!(simd, sc, "seed={seed}");
        }
    }
}
