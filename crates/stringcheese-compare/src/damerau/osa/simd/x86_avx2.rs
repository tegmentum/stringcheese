//! AVX2-gated OSA (restricted Damerau-Levenshtein) kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. AVX2 doubles the
//! wide-block Hyyrö-OSA register width from SSE2's 128 to 256 bits, so
//! patterns up to `m = 256` bytes now stay on the bit-parallel path.
//!
//! # Algorithm
//!
//! The kernel implements Hyyrö's bit-parallel OSA algorithm — Myers's
//! word-parallel Levenshtein extended with an extra bit-vector `Pm_old`
//! (the previous column's `Peq[text[j-1]]`) and a diagonal-zero vector
//! `D0`. See Hyyrö (2003), "Bit-parallel approximate string matching
//! algorithms with transposition" (SPIRE 2003).
//!
//! Concretely:
//!
//! * For `m ≤ 64` the AVX2 register width is wasted; this path delegates
//!   to [`super::scalar`] which pays a smaller Peq-table build cost.
//! * For `64 < m ≤ 128` we use the SSE2 sibling backend — SSE2 is a
//!   strict subset of AVX2, so calling it from an AVX2-gated function is
//!   safe. A 128-bit state is a better fit than a 256-bit state with
//!   half the lanes idle.
//! * For `128 < m ≤ 256` we pack `Pv`, `Mv`, `D0`, `Pm_old`, and each
//!   `Peq[c]` entry into `__m256i` values (four `u64` lanes) and run the
//!   Hyyrö inner loop with a 256-bit integer add and a 256-bit
//!   shift-left-by-one supplying the cross-lane carries.
//! * For `m > 256` the pattern no longer fits in one AVX2 register;
//!   this path delegates back to [`super::scalar`].
//!
//! # AVX2-specific carry mechanics
//!
//! Identical to the Levenshtein AVX2 backend (see
//! `crate::levenshtein::simd::myers_x86_avx2`): the full 256-bit add is
//! done by storing to `[u64; 4]`, chaining an `overflowing_add` across
//! the four lanes, and reloading. The full shift-left-by-1 uses
//! `_mm256_slli_epi64` for the per-lane part; the carry lane comes from
//! `_mm256_srli_epi64(_, 63)` shifted one 64-bit lane to the left via
//! `_mm256_permute4x64_epi64`, then masked to zero the wrap-around lane.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher gates every call on
//! `is_x86_feature_detected!("avx2")`.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "AVX2 intrinsics (`_mm256_setr_epi64x`, `_mm256_set1_epi64x`) take `i64`; the wide-block Myers state is unsigned by construction, so `as i64` / `as u64` at those boundaries is a pure bit-transmute, not a numeric conversion"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "every pointer cast in this module feeds an *unaligned* AVX2 load/store (`_mm256_loadu_si256`, `_mm256_storeu_si256`), which by contract accepts any-alignment pointers; the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]
#![allow(
    clippy::similar_names,
    reason = "the Hyyrö recurrence has structurally similar names — `hp`/`hn`, `hp_shifted`/`hn_shifted`, `d0_or_hp`/`d0_or_vp` — that map 1:1 to the paper's variables; renaming them to be more distinct would put a translation layer between the code and the derivation"
)]

use core::arch::x86_64::{
    __m256i, _mm256_and_si256, _mm256_andnot_si256, _mm256_loadu_si256, _mm256_or_si256,
    _mm256_permute4x64_epi64, _mm256_set1_epi64x, _mm256_setr_epi64x, _mm256_setzero_si256,
    _mm256_slli_epi64, _mm256_srli_epi64, _mm256_storeu_si256, _mm256_xor_si256,
};

use super::{scalar, x86_sse2};

/// Machine-word width used by the scalar single-word path.
const W_SCALAR: usize = 64;

/// Register width in bits for the SSE2 sibling path.
const W_SSE2: usize = 128;

/// Widest pattern length the AVX2 wide-block path handles.
const W_AVX2: usize = 256;

/// AVX2-gated OSA distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure AVX2 is available on the running CPU. The
/// dispatcher in the parent [`super`] module guarantees this via
/// `is_x86_feature_detected!("avx2")`.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    let (pattern, text) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let m = pattern.len();
    let n = text.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }

    if m <= W_SCALAR {
        return scalar::distance(a, b);
    }

    if m <= W_SSE2 {
        // SAFETY: AVX2 is a strict superset of SSE2 — every CPU that
        // provides AVX2 also provides SSE2 — so the SSE2 backend's
        // `#[target_feature(enable = "sse2")]` precondition is upheld
        // by this function's AVX2 target-feature context.
        return unsafe { x86_sse2::distance(a, b) };
    }

    if m > W_AVX2 {
        return scalar::distance(a, b);
    }

    // SAFETY: AVX2 target-feature context established by this function's
    // `#[target_feature(enable = "avx2")]` upholds the AVX2 precondition
    // of `wide_block_256`.
    unsafe { wide_block_256(pattern, text, m) }
}

/// Wide-block Hyyrö-OSA for `128 < m ≤ 256`. Every bit-vector variable
/// lives in a `__m256i` (four `u64` lanes); carry propagation between
/// lanes is explicit.
///
/// # Safety
///
/// The caller must ensure AVX2 is available.
#[target_feature(enable = "avx2")]
unsafe fn wide_block_256(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SSE2 && m <= W_AVX2);

    // Peq stored as `[u64; 256 × 4]`, four u64s per byte value. Single
    // unaligned 256-bit load per symbol.
    let mut peq: alloc::vec::Vec<u64> = alloc::vec![0u64; 1024];
    for (i, &c) in pattern.iter().enumerate() {
        let idx = (c as usize) * 4 + i / W_SCALAR;
        let bit = i % W_SCALAR;
        peq[idx] |= 1u64 << bit;
    }

    let msb_bit = m - 1;
    let msb_lane = msb_bit / W_SCALAR;
    let msb_lane_bit = msb_bit % W_SCALAR;
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    // SAFETY: AVX2 target-feature context established by the containing
    // `#[target_feature(enable = "avx2")]` — every intrinsic below is
    // AVX2.
    unsafe {
        let all_ones = _mm256_set1_epi64x(-1);
        let mut pv = all_ones;
        let mut mv = _mm256_setzero_si256();
        let mut d0 = _mm256_setzero_si256();
        let mut pm_old = _mm256_setzero_si256();
        let one_lo = _mm256_setr_epi64x(1, 0, 0, 0);
        let hi_lanes_mask = _mm256_setr_epi64x(0, -1i64, -1i64, -1i64);

        let mut scratch_a = [0u64; 4];
        let mut scratch_b = [0u64; 4];

        for &c in text {
            let pm_ptr = peq.as_ptr().add((c as usize) * 4).cast::<__m256i>();
            let pm_j = _mm256_loadu_si256(pm_ptr);

            // Transposition contribution:
            //   tr = shl1((~D0_prev) & pm_j) & pm_old
            let not_d0_and_pm = _mm256_andnot_si256(d0, pm_j);
            let tr_shifted = shl1_256(not_d0_and_pm, hi_lanes_mask);
            let tr = _mm256_and_si256(tr_shifted, pm_old);

            // Myers D0 with `| vn | tr`:
            //   d0 = (((pm_j & vp) + vp) ^ vp) | pm_j | vn | tr
            let pm_and_vp = _mm256_and_si256(pm_j, pv);
            let sum = add256(pm_and_vp, pv, &mut scratch_a, &mut scratch_b);
            let xor = _mm256_xor_si256(sum, pv);
            let d0_new = _mm256_or_si256(_mm256_or_si256(xor, pm_j), _mm256_or_si256(mv, tr));

            // Hp = vn | ~(d0 | vp); Hn = d0 & vp
            let d0_or_vp = _mm256_or_si256(d0_new, pv);
            let hp = _mm256_or_si256(mv, _mm256_andnot_si256(d0_or_vp, all_ones));
            let hn = _mm256_and_si256(d0_new, pv);

            if bit_at(hp, msb_lane, msb_lane_bit, &mut scratch_a) {
                score += 1;
            }
            if bit_at(hn, msb_lane, msb_lane_bit, &mut scratch_a) {
                score -= 1;
            }

            // hp = (hp << 1) | 1; hn <<= 1
            let hp_shifted = _mm256_or_si256(shl1_256(hp, hi_lanes_mask), one_lo);
            let hn_shifted = shl1_256(hn, hi_lanes_mask);

            // vp = hn_shifted | ~(d0 | hp_shifted); vn = hp_shifted & d0
            let d0_or_hp = _mm256_or_si256(d0_new, hp_shifted);
            let pv_new = _mm256_or_si256(hn_shifted, _mm256_andnot_si256(d0_or_hp, all_ones));
            let mv_new = _mm256_and_si256(hp_shifted, d0_new);

            d0 = d0_new;
            pm_old = pm_j;
            pv = pv_new;
            mv = mv_new;
        }

        score
    }
}

/// 256-bit big-integer add, wrapping at 2^256.
///
/// # Safety
///
/// AVX2 must be available.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn add256(
    a: __m256i,
    b: __m256i,
    scratch_a: &mut [u64; 4],
    scratch_b: &mut [u64; 4],
) -> __m256i {
    unsafe {
        _mm256_storeu_si256(scratch_a.as_mut_ptr().cast::<__m256i>(), a);
        _mm256_storeu_si256(scratch_b.as_mut_ptr().cast::<__m256i>(), b);
        let mut out = [0u64; 4];
        let mut carry = 0u64;
        for i in 0..4 {
            let (s1, c1) = scratch_a[i].overflowing_add(scratch_b[i]);
            let (s2, c2) = s1.overflowing_add(carry);
            out[i] = s2;
            carry = u64::from(c1) | u64::from(c2);
        }
        _mm256_loadu_si256(out.as_ptr().cast::<__m256i>())
    }
}

/// Shift a full 256-bit value left by one bit; the outgoing bit 255 is
/// dropped.
///
/// # Safety
///
/// AVX2 must be available.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn shl1_256(v: __m256i, hi_lanes_mask: __m256i) -> __m256i {
    let shifted = _mm256_slli_epi64(v, 1);
    let top_bits = _mm256_srli_epi64(v, 63);
    // Rotate `top_bits` one 64-bit lane toward higher indices, then
    // mask out the wrap-around lane. Same technique as the
    // Levenshtein AVX2 backend — see its `shl1_256` for the immediate
    // derivation.
    let rotated = _mm256_permute4x64_epi64::<0b_10_01_00_11>(top_bits);
    let carry = _mm256_and_si256(rotated, hi_lanes_mask);
    _mm256_or_si256(shifted, carry)
}

/// Extract bit `(lane * 64 + lane_bit)` from a 256-bit vector.
///
/// # Safety
///
/// AVX2 must be available.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn bit_at(v: __m256i, lane: usize, lane_bit: usize, scratch: &mut [u64; 4]) -> bool {
    debug_assert!(lane < 4 && lane_bit < W_SCALAR);
    unsafe {
        _mm256_storeu_si256(scratch.as_mut_ptr().cast::<__m256i>(), v);
    }
    (scratch[lane] >> lane_bit) & 1 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !is_x86_feature_detected!("avx2") {
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
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { distance(a, b) };
            let sc = scalar::distance(a, b);
            assert_eq!(simd, sc, "avx2 disagreed with scalar on ({a:?}, {b:?})");
        }
    }

    /// Boundary at `m = 129` — first pattern length past the SSE2
    /// wide-block cutoff. Exercises the 256-bit wide-block code path.
    #[test]
    fn wide_block_matches_scalar_at_m_129() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..129u8).collect();
        let mut b = a.clone();
        b.swap(64, 65);
        b[128] ^= 0x01;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 256` — exact register width.
    #[test]
    fn wide_block_matches_scalar_at_m_256() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..=255u8).collect();
        let mut b = a.clone();
        b.swap(0, 1);
        b.swap(127, 128);
        b[255] ^= 0x80;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 257` — first pattern length past the AVX2 wide-
    /// block range. Delegates to the scalar rolling-rows fallback.
    #[test]
    fn wide_block_delegates_past_m_256() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..257u32)
            .map(|i| u8::try_from(i & 0xff).unwrap())
            .collect();
        let mut b = a.clone();
        b[256] ^= 0x01;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Differential across a range spanning both wide-block sizes and
    /// the delegation boundaries.
    #[test]
    fn differential_across_lengths() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for m in 1..=300usize {
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
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd1 = unsafe { distance(&a, &b) };
            let sc1 = scalar::distance(&a, &b);
            assert_eq!(simd1, sc1, "at m={m} on (a, b)");
            let simd2 = unsafe { distance(&a, &text_ext) };
            let sc2 = scalar::distance(&a, &text_ext);
            assert_eq!(simd2, sc2, "at m={m} on (a, text_ext)");
        }
    }
}
