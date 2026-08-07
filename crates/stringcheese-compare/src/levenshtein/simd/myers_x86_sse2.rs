//! SSE2-gated Myers Levenshtein kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. It is the SSE2 fallback
//! selected by the dispatcher when AVX2 is unavailable — every `x86_64`
//! CPU has SSE2 as part of the baseline ABI, so this branch is always a
//! valid target.
//!
//! # Algorithm
//!
//! The kernel implements Myers's bit-parallel edit-distance algorithm
//! (Myers, JACM 1999, §4) extended to a 128-bit column state via Hyyrö's
//! wide-block reformulation (Hyyrö, 2003). Concretely:
//!
//! * For patterns of length `m ≤ 64` the SSE2 register width is wasted;
//!   this path delegates straight to [`super::myers_scalar`], which uses
//!   a single `u64` and pays the smaller Peq-table build cost.
//! * For `64 < m ≤ 128` we pack `Pv`, `Mv`, and each `Peq[c]` entry into
//!   `__m128i` values and run the same six-op inner loop with a 128-bit
//!   integer add and a 128-bit shift-left-by-one supplying the cross-lane
//!   carries.
//! * For `m > 128` the pattern no longer fits in one SSE2 register; this
//!   path delegates back to [`super::myers_scalar`] which in turn falls
//!   back to a rolling-rows DP. The AVX2 sibling extends the bit-parallel
//!   range to `m ≤ 256`.
//!
//! # SSE2-specific carry mechanics
//!
//! SSE2 provides `_mm_add_epi64` (per-lane 64-bit add, **no** cross-lane
//! carry) and `_mm_slli_si128` (byte-granular whole-register shift). The
//! full-width 128-bit add is done by extracting the two `u64` halves,
//! chaining an `overflowing_add`, and reassembling with `_mm_set_epi64x`.
//! The full-width shift-left-by-1 is done in three SSE2 instructions:
//! per-lane `_mm_slli_epi64(v, 1)`, per-lane `_mm_srli_epi64(v, 63)` to
//! isolate the outgoing bit-63s, and `_mm_slli_si128(..., 8)` to move the
//! low lane's carry into the high lane's low bit; a final `_mm_or_si128`
//! merges the two.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. On `x86_64`, SSE2 is guaranteed by the
//! ABI; the dispatcher checks it anyway for consistency with the other
//! arch branches.

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

use core::arch::x86_64::{
    __m128i, _mm_and_si128, _mm_cvtsi128_si64, _mm_loadu_si128, _mm_or_si128, _mm_set_epi64x,
    _mm_set1_epi64x, _mm_setzero_si128, _mm_slli_epi64, _mm_slli_si128, _mm_srli_epi64,
    _mm_srli_si128, _mm_xor_si128,
};

use super::myers_scalar;

/// Machine-word width used by the scalar single-word path. Any pattern of
/// length at most this many symbols is faster on the scalar kernel; the
/// SSE2 wide-block path only wins from `m = W_SCALAR + 1` upward.
const W_SCALAR: usize = 64;

/// Widest pattern length the SSE2 wide-block path can handle. Equal to the
/// SSE2 register width in bits — one `Pv`/`Mv` bit per pattern position.
const W_SSE2: usize = 128;

/// SSE2-gated Levenshtein distance for byte-slice inputs.
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
    // Pick the shorter side as the pattern — Myers is symmetric, and the
    // shorter side controls the number of blocks we need.
    let (pattern, text) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let m = pattern.len();
    let n = text.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }

    if m <= W_SCALAR || m > W_SSE2 {
        // Below the SSE2 wide-block break-even the scalar single-word
        // path wins; above the register width there aren't enough bits
        // for the wide-block state, so fall back to the scalar module
        // which handles both the m ≤ 64 fast path and the m > 64
        // rolling-rows fallback.
        return myers_scalar::distance(a, b);
    }

    // SAFETY: the SSE2 target-feature attribute on this function upholds
    // the SSE2 precondition of `wide_block_128`.
    unsafe { wide_block_128(pattern, text, m) }
}

/// Wide-block Myers for `64 < m ≤ 128`. Every state variable lives in an
/// `__m128i`; carry propagation between the two 64-bit lanes is explicit.
///
/// # Safety
///
/// The caller must ensure SSE2 is available.
#[target_feature(enable = "sse2")]
unsafe fn wide_block_128(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SCALAR && m <= W_SSE2);

    // Peq is stored as an interleaved `[u64; 256 × 2]`, low lane then high
    // lane per byte value. This layout lets the inner loop pull each
    // `Peq[c]` with a single unaligned 128-bit load.
    let mut peq: alloc::vec::Vec<u64> = alloc::vec![0u64; 512];
    for (i, &c) in pattern.iter().enumerate() {
        let idx = (c as usize) * 2 + i / W_SCALAR;
        let bit = i % W_SCALAR;
        peq[idx] |= 1u64 << bit;
    }

    // Initial Pv = 1^m (bits 0..m set). Low lane is always full (m > 64);
    // high lane fills the remaining `m - 64` bits.
    let hi_mask: u64 = if m == W_SSE2 {
        u64::MAX
    } else {
        (1u64 << (m - W_SCALAR)) - 1
    };
    let msb_bit = m - 1;
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    // SAFETY: SSE2 target-feature context established by the containing
    // `#[target_feature(enable = "sse2")]` — every intrinsic below is
    // SSE2.
    unsafe {
        // The `_mm_set_epi64x` argument order is (high, low): `hi_mask`
        // sits in the top 64 bits, all-ones in the low 64.
        let mut pv = _mm_set_epi64x(hi_mask as i64, -1i64);
        let mut mv = _mm_setzero_si128();
        let all_ones = _mm_set1_epi64x(-1);
        // Constant vector `[1, 0]` used to inject a `1` into bit 0 of the
        // shifted `Ph`.
        let one_lo = _mm_set_epi64x(0, 1);

        for &c in text {
            // `Peq[c]` — unaligned 128-bit load out of the interleaved
            // table.
            let eq_ptr = peq.as_ptr().add((c as usize) * 2).cast::<__m128i>();
            let eq = _mm_loadu_si128(eq_ptr);

            // Myers 1999 §4 inner loop, verbatim, in 128-bit form.
            let xv = _mm_or_si128(eq, mv);

            let eq_and_pv = _mm_and_si128(eq, pv);
            let sum = add128(eq_and_pv, pv);
            let xh = _mm_or_si128(_mm_xor_si128(sum, pv), eq);

            let ph = _mm_or_si128(mv, _mm_xor_si128(_mm_or_si128(xh, pv), all_ones));
            let mh = _mm_and_si128(pv, xh);

            if bit_at(ph, msb_bit) {
                score += 1;
            }
            if bit_at(mh, msb_bit) {
                score -= 1;
            }

            // Ph = (Ph << 1) | 1, Mh <<= 1
            let ph_shifted = _mm_or_si128(shl1(ph), one_lo);
            let mh_shifted = shl1(mh);

            pv = _mm_or_si128(
                mh_shifted,
                _mm_xor_si128(_mm_or_si128(xv, ph_shifted), all_ones),
            );
            mv = _mm_and_si128(ph_shifted, xv);
        }

        score
    }
}

/// 128-bit big-integer add, wrapping at 2^128.
///
/// # Safety
///
/// SSE2 must be available; the intrinsics used here are all SSE2.
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn add128(a: __m128i, b: __m128i) -> __m128i {
    // All SSE2 intrinsics used here operate on register values only —
    // no memory access — and are stably safe on x86_64. The outer
    // `unsafe fn` covers only the `#[target_feature]` precondition,
    // which the caller upholds.
    let a_lo = _mm_cvtsi128_si64(a) as u64;
    let a_hi = _mm_cvtsi128_si64(_mm_srli_si128(a, 8)) as u64;
    let b_lo = _mm_cvtsi128_si64(b) as u64;
    let b_hi = _mm_cvtsi128_si64(_mm_srli_si128(b, 8)) as u64;
    let (s_lo, carry) = a_lo.overflowing_add(b_lo);
    let s_hi = a_hi.wrapping_add(b_hi).wrapping_add(u64::from(carry));
    _mm_set_epi64x(s_hi as i64, s_lo as i64)
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
    // Register-only SSE2 intrinsics — see `add128`.
    //
    // Per-lane 64-bit shift-left by 1.
    let shifted = _mm_slli_epi64(v, 1);
    // Extract bit 63 of each lane into bit 0 of that lane.
    let top_bits = _mm_srli_epi64(v, 63);
    // Move the low lane's bit-63 into the high lane's bit-0 by
    // whole-register byte-shifting left by 8. The high lane's
    // outgoing bit-63 is discarded, as required.
    let carry = _mm_slli_si128(top_bits, 8);
    _mm_or_si128(shifted, carry)
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
    // Register-only SSE2 intrinsics — see `add128`.
    if i < W_SCALAR {
        let lo = _mm_cvtsi128_si64(v) as u64;
        (lo >> i) & 1 != 0
    } else {
        let hi = _mm_cvtsi128_si64(_mm_srli_si128(v, 8)) as u64;
        (hi >> (i - W_SCALAR)) & 1 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        // SSE2 is baseline for x86_64; if this test runs on x86_64 it
        // must be available.
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { distance(a, b) };
            let scalar = myers_scalar::distance(a, b);
            assert_eq!(simd, scalar, "sse2 disagreed with scalar on ({a:?}, {b:?})");
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
        b[64] ^= 0x01;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
        assert_eq!(simd, 1);
    }

    /// Boundary at `m = 128` — exact register width. Exercises the
    /// `m == W_SSE2` special-case for the initial `Pv`.
    #[test]
    fn wide_block_matches_scalar_at_m_128() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x80;
        b[127] ^= 0x80;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
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
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Differential across the full m ∈ [1, 200] range against
    /// scalar Myers. Every pattern length must agree bit-for-bit.
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
            if !b.is_empty() {
                b[m / 2] ^= 0x5A;
            }
            // Also vary text length distinctly.
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
            let scalar1 = myers_scalar::distance(&a, &b);
            assert_eq!(simd1, scalar1, "at m={m} on (a, b)");
            // SAFETY: same as above.
            let simd2 = unsafe { distance(&a, &text_ext) };
            let scalar2 = myers_scalar::distance(&a, &text_ext);
            assert_eq!(simd2, scalar2, "at m={m} on (a, text_ext)");
        }
    }
}
