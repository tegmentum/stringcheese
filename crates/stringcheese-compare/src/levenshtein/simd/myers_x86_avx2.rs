//! AVX2-gated Myers Levenshtein kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. AVX2 doubles the
//! bit-parallel Myers register width from SSE2's 128 to 256 bits, so
//! patterns up to `m = 256` bytes now stay on the bit-parallel path.
//!
//! # Algorithm
//!
//! The kernel implements Myers's bit-parallel edit-distance algorithm
//! (Myers, JACM 1999, §4) extended to a 256-bit column state via Hyyrö's
//! wide-block reformulation (Hyyrö, 2003). Concretely:
//!
//! * For `m ≤ 64` the AVX2 register width is wasted; this path delegates
//!   to [`super::myers_scalar`], which uses a single `u64` and pays the
//!   smaller Peq-table build cost.
//! * For `64 < m ≤ 128` we use the SSE2 sibling backend
//!   ([`super::myers_x86_sse2`]) — SSE2 is a strict subset of AVX2, so
//!   calling it from an AVX2-gated function is safe. The 128-bit state
//!   is a better fit than a 256-bit state that would waste half the
//!   register.
//! * For `128 < m ≤ 256` we pack `Pv`, `Mv`, and each `Peq[c]` entry
//!   into `__m256i` values (four `u64` lanes) and run the same six-op
//!   inner loop with a 256-bit integer add and a 256-bit
//!   shift-left-by-one supplying the cross-lane carries.
//! * For `m > 256` the pattern no longer fits in one AVX2 register; this
//!   path delegates back to [`super::myers_scalar`] which in turn falls
//!   back to a rolling-rows DP.
//!
//! # AVX2-specific carry mechanics
//!
//! AVX2 has `_mm256_add_epi64` (per-lane 64-bit add, **no** cross-lane
//! carry) and `_mm256_slli_si256` (byte-granular whole-*lane*-half
//! shift — it does not cross the 128-bit half boundary). The full 256-bit
//! add is done by storing the vectors to `[u64; 4]`, chaining an
//! `overflowing_add` across the four lanes, and reloading. The full
//! shift-left-by-1 uses `_mm256_slli_epi64(v, 1)` for the per-lane part;
//! the carry lane comes from `_mm256_srli_epi64(v, 63)` shifted one
//! 64-bit lane to the "left" via `_mm256_permute4x64_epi64` (which,
//! unlike `_mm256_slli_si256`, is a true whole-register permute), then
//! masked to zero out the wrap-around lane.
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher in
//! [`crate::levenshtein::simd`] gates every call on
//! `is_x86_feature_detected!("avx2")`, so the precondition is met by
//! construction; call sites outside the dispatcher must uphold the same
//! contract.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "several AVX2 intrinsics (`_mm256_setr_epi64x`, `_mm256_set1_epi64x`) take `i64`; the wide-block Myers state is unsigned by construction, so `as i64` / `as u64` at those boundaries is a pure bit-transmute, not a numeric conversion"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "every pointer cast in this module feeds an *unaligned* AVX2 load/store (`_mm256_loadu_si256`, `_mm256_storeu_si256`), which by contract accepts any-alignment pointers; the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]

use core::arch::x86_64::{
    __m256i, _mm256_and_si256, _mm256_loadu_si256, _mm256_or_si256, _mm256_permute4x64_epi64,
    _mm256_set1_epi64x, _mm256_setr_epi64x, _mm256_setzero_si256, _mm256_slli_epi64,
    _mm256_srli_epi64, _mm256_storeu_si256, _mm256_xor_si256,
};

use super::{myers_scalar, myers_x86_sse2};

/// Machine-word width used by the scalar single-word path.
const W_SCALAR: usize = 64;

/// Register width in bits for the SSE2 sibling path.
const W_SSE2: usize = 128;

/// Widest pattern length the AVX2 wide-block path can handle.
const W_AVX2: usize = 256;

/// AVX2-gated Levenshtein distance for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure AVX2 is available on the running CPU. The
/// dispatcher in the parent [`super`] module guarantees this via
/// `is_x86_feature_detected!("avx2")`.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols — matches
/// the panic contract of the crate's other DP kernels.
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
        // The scalar single-word Myers wins on setup cost for small m.
        return myers_scalar::distance(a, b);
    }

    if m <= W_SSE2 {
        // SAFETY: AVX2 is a strict superset of SSE2 — every CPU that
        // provides AVX2 also provides SSE2 — so the SSE2 backend's
        // `#[target_feature(enable = "sse2")]` precondition is upheld by
        // this function's AVX2 target-feature context.
        return unsafe { myers_x86_sse2::distance(a, b) };
    }

    if m > W_AVX2 {
        // Above the register width, the pattern doesn't fit in one AVX2
        // block; delegate to the scalar module which falls back to a
        // rolling-rows DP for m > 64.
        return myers_scalar::distance(a, b);
    }

    // SAFETY: the AVX2 target-feature attribute on this function upholds
    // the AVX2 precondition of `wide_block_256`.
    unsafe { wide_block_256(pattern, text, m) }
}

/// Wide-block Myers for `128 < m ≤ 256`. Every state variable lives in a
/// `__m256i` (four `u64` lanes); carry propagation between lanes is
/// explicit.
///
/// # Safety
///
/// The caller must ensure AVX2 is available.
#[target_feature(enable = "avx2")]
unsafe fn wide_block_256(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SSE2 && m <= W_AVX2);

    // Peq stored as `[u64; 256 × 4]`, four u64s per byte value. Loading
    // each `Peq[c]` is one unaligned 256-bit load.
    let mut peq: alloc::vec::Vec<u64> = alloc::vec![0u64; 1024];
    for (i, &c) in pattern.iter().enumerate() {
        let idx = (c as usize) * 4 + i / W_SCALAR;
        let bit = i % W_SCALAR;
        peq[idx] |= 1u64 << bit;
    }

    // Initial Pv = 1^m across four lanes. Lanes below the "partial"
    // lane are all-ones; the partial lane has `m % 64` low bits set;
    // lanes above (if any) are zero.
    let full_lanes = m / W_SCALAR;
    let rem_bits = m % W_SCALAR;
    let mut init_pv = [0u64; 4];
    for lane in init_pv.iter_mut().take(full_lanes) {
        *lane = u64::MAX;
    }
    if rem_bits > 0 && full_lanes < 4 {
        init_pv[full_lanes] = (1u64 << rem_bits) - 1;
    }

    let msb_bit = m - 1;
    let msb_lane = msb_bit / W_SCALAR;
    let msb_lane_bit = msb_bit % W_SCALAR;
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    // SAFETY: AVX2 target-feature context established by the containing
    // `#[target_feature(enable = "avx2")]` — every intrinsic below is
    // AVX2.
    unsafe {
        let mut pv = _mm256_loadu_si256(init_pv.as_ptr().cast::<__m256i>());
        let mut mv = _mm256_setzero_si256();
        let all_ones = _mm256_set1_epi64x(-1);
        // Constant vector `[1, 0, 0, 0]` (lane 0 low): injects `1` into
        // bit 0 of shifted Ph.
        let one_lo = _mm256_setr_epi64x(1, 0, 0, 0);
        // Mask that zeroes lane 0 while preserving lanes 1..4 — used to
        // discard the wrap-around carry in `shl1_256`.
        let hi_lanes_mask = _mm256_setr_epi64x(0, -1i64, -1i64, -1i64);

        // Reused scratch buffers for the extract/reload steps.
        let mut scratch_a = [0u64; 4];
        let mut scratch_b = [0u64; 4];

        for &c in text {
            let eq_ptr = peq.as_ptr().add((c as usize) * 4).cast::<__m256i>();
            let eq = _mm256_loadu_si256(eq_ptr);

            let xv = _mm256_or_si256(eq, mv);

            let eq_and_pv = _mm256_and_si256(eq, pv);
            let sum = add256(eq_and_pv, pv, &mut scratch_a, &mut scratch_b);
            let xh = _mm256_or_si256(_mm256_xor_si256(sum, pv), eq);

            let ph = _mm256_or_si256(mv, _mm256_xor_si256(_mm256_or_si256(xh, pv), all_ones));
            let mh = _mm256_and_si256(pv, xh);

            if bit_at(ph, msb_lane, msb_lane_bit, &mut scratch_a) {
                score += 1;
            }
            if bit_at(mh, msb_lane, msb_lane_bit, &mut scratch_a) {
                score -= 1;
            }

            let ph_shifted = _mm256_or_si256(shl1_256(ph, hi_lanes_mask), one_lo);
            let mh_shifted = shl1_256(mh, hi_lanes_mask);

            pv = _mm256_or_si256(
                mh_shifted,
                _mm256_xor_si256(_mm256_or_si256(xv, ph_shifted), all_ones),
            );
            mv = _mm256_and_si256(ph_shifted, xv);
        }

        score
    }
}

/// 256-bit big-integer add, wrapping at 2^256. The caller supplies two
/// `[u64; 4]` scratch buffers to avoid re-allocating them each call.
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
    // SAFETY: AVX2 target-feature attribute in place; scratch buffers
    // are 32-byte-sized `[u64; 4]`, which is exactly the width of the
    // AVX2 store.
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
/// dropped. `hi_lanes_mask` is the constant `[0, -1, -1, -1]` used to
/// zero the wrap-around lane after the cross-lane permute.
///
/// # Safety
///
/// AVX2 must be available.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn shl1_256(v: __m256i, hi_lanes_mask: __m256i) -> __m256i {
    // Register-only AVX2 intrinsics — no memory access — stably safe on
    // x86_64. The outer `unsafe fn` covers only the `#[target_feature]`
    // precondition, which the caller upholds.
    //
    // Per-lane 64-bit shift-left by 1.
    let shifted = _mm256_slli_epi64(v, 1);
    // Extract bit 63 of each lane into bit 0 of that lane.
    let top_bits = _mm256_srli_epi64(v, 63);
    // Rotate the 64-bit lanes by one to the "left" (toward higher
    // lane indices). `_mm256_permute4x64_epi64` takes an 8-bit
    // immediate where bits [1:0], [3:2], [5:4], [7:6] pick the source
    // lane for destination lanes 0, 1, 2, 3 respectively. To rotate
    // dest[i] = src[(i - 1) mod 4], we pick src[3], src[0], src[1],
    // src[2] for dest lanes 0..3 respectively — imm8 =
    // (2 << 6) | (1 << 4) | (0 << 2) | 3 = 0b_10_01_00_11.
    let rotated = _mm256_permute4x64_epi64::<0b_10_01_00_11>(top_bits);
    // Mask out lane 0 to discard the wrap-around carry (bit 63 of
    // lane 3, which is the bit shifted off the top of the 256-bit
    // register).
    let carry = _mm256_and_si256(rotated, hi_lanes_mask);
    _mm256_or_si256(shifted, carry)
}

/// Extract bit `(lane * 64 + lane_bit)` from a 256-bit vector. The
/// caller supplies a `[u64; 4]` scratch buffer.
///
/// # Safety
///
/// AVX2 must be available.
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn bit_at(v: __m256i, lane: usize, lane_bit: usize, scratch: &mut [u64; 4]) -> bool {
    debug_assert!(lane < 4 && lane_bit < W_SCALAR);
    // SAFETY: AVX2 target-feature attribute in place.
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
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { distance(a, b) };
            let scalar = myers_scalar::distance(a, b);
            assert_eq!(simd, scalar, "avx2 disagreed with scalar on ({a:?}, {b:?})");
        }
    }

    /// Boundary at `m = 129` — first pattern length past the SSE2 wide-
    /// block cutoff. Exercises the 256-bit wide-block code path.
    #[test]
    fn wide_block_matches_scalar_at_m_129() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..129u8).collect();
        let mut b = a.clone();
        b[128] ^= 0x01;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
        assert_eq!(simd, 1);
    }

    /// Boundary at `m = 256` — exact register width. Exercises the
    /// full-register initial `Pv`.
    #[test]
    fn wide_block_matches_scalar_at_m_256() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..=255u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x80;
        b[255] ^= 0x80;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
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
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Differential across the full m ∈ [1, 300] range against
    /// scalar Myers. Every pattern length must agree bit-for-bit.
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
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
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
