//! wasm SIMD128-gated Myers Levenshtein kernel for `wasm32`.
//!
//! This module compiles only on `wasm32` targets and only when the
//! `simd128` target-feature is enabled at compile time. Unlike `x86_64`
//! and `aarch64`, wasm has no runtime CPU-feature detection: whether the SIMD
//! opcodes are legal is a property of the wasm engine executing the
//! module, and the module either uses them or it does not. Callers control
//! the choice via `RUSTFLAGS=-C target-feature=+simd128` at build time,
//! and the dispatcher in [`super`] compiles this path in or out with a
//! matching `#[cfg(target_feature = "simd128")]` gate.
//!
//! # Algorithm
//!
//! The kernel implements Myers's bit-parallel edit-distance algorithm
//! (Myers, JACM 1999, §4) extended to a 128-bit column state via Hyyrö's
//! wide-block reformulation (Hyyrö, 2003) — same shape as the SSE2 and
//! NEON siblings. Concretely:
//!
//! * For patterns of length `m ≤ 64` the SIMD register width is wasted;
//!   this path delegates straight to [`super::myers_scalar`], which uses
//!   a single `u64` and pays the smaller Peq-table build cost.
//! * For `64 < m ≤ 128` we pack `Pv`, `Mv`, and each `Peq[c]` entry into
//!   `v128` values and run the same six-op inner loop with a 128-bit
//!   integer add and a 128-bit shift-left-by-one supplying the cross-lane
//!   carries.
//! * For `m > 128` the pattern no longer fits in one 128-bit register;
//!   this path delegates back to [`super::myers_scalar`] which in turn
//!   falls back to a rolling-rows DP. wasm SIMD256 (relaxed-simd) is not
//!   yet stable enough to rely on here.
//!
//! # wasm-SIMD-specific carry mechanics
//!
//! `u64x2_add` is per-lane and does **not** surface a cross-lane carry.
//! The full-width 128-bit add is done by extracting the two `u64` halves
//! with `u64x2_extract_lane`, chaining a scalar `overflowing_add`, and
//! reassembling with `u64x2_replace_lane`. The full-width shift-left-by-1
//! is done by a per-lane `u64x2_shl(v, 1)` plus a scalar-side lane
//! extract/insert that pulls the low lane's bit-63 (via a plain
//! `u64 >> 63`) and ORs it into the shifted high lane's bit-0. (wasm
//! SIMD has no `_mm_slli_si128`-style whole-register byte shift on the
//! u64 lane dimension, so the cross-lane carry is moved through the
//! extract/insert pair rather than a single opcode.)
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` for parity with the sibling SSE2/NEON
//! backends' `#[target_feature]`-gated signature, even though on wasm the
//! target feature is a compile-time property rather than a runtime
//! precondition. On wasm32 with `target_feature = "simd128"` this
//! function is unconditionally safe to call.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics are unsafe by declaration (v128_load); this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "the `.cast::<v128>()` in the inner loop feeds `v128_load`, which by spec accepts any-alignment pointers (the wasm SIMD load is unaligned-tolerant, same as SSE2's `_mm_loadu_si128`); the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]

use core::arch::wasm32::{
    u64x2_extract_lane, u64x2_replace_lane, u64x2_shl, u64x2_splat, v128, v128_and, v128_load,
    v128_or, v128_xor,
};

use super::myers_scalar;

/// Machine-word width used by the scalar single-word path. Any pattern of
/// length at most this many symbols is faster on the scalar kernel; the
/// wasm SIMD wide-block path only wins from `m = W_SCALAR + 1` upward.
const W_SCALAR: usize = 64;

/// Widest pattern length the wasm SIMD wide-block path can handle. Equal
/// to the SIMD128 register width in bits — one `Pv`/`Mv` bit per pattern
/// position.
const W_SIMD128: usize = 128;

/// wasm SIMD128-gated Levenshtein distance for byte-slice inputs.
///
/// # Safety
///
/// wasm-SIMD target features are compile-time gates: this function is
/// only compiled when `target_feature = "simd128"` is set, so calling it
/// on a wasm engine without SIMD support is a load-time module rejection
/// rather than an in-function precondition. The `unsafe fn` signature
/// mirrors the SSE2/NEON siblings for uniformity of the dispatcher's
/// call sites and to preserve the `v128_load` intrinsic's own
/// `unsafe fn` contract.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols — matches
/// the panic contract of the crate's other DP kernels.
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

    if m <= W_SCALAR || m > W_SIMD128 {
        // Below the wide-block break-even the scalar single-word path
        // wins; above the register width there aren't enough bits for
        // the wide-block state, so fall back to the scalar module which
        // handles both the m ≤ 64 fast path and the m > 64 rolling-rows
        // fallback.
        return myers_scalar::distance(a, b);
    }

    // SAFETY: the containing `unsafe fn` matches the shape of the
    // sibling SSE2/NEON backends; `wide_block_128` uses only wasm SIMD
    // intrinsics that are legal whenever this module is compiled (the
    // `target_feature = "simd128"` cfg-gate at the dispatch site).
    unsafe { wide_block_128(pattern, text, m) }
}

/// Wide-block Myers for `64 < m ≤ 128`. Every state variable lives in a
/// `v128`; carry propagation between the two 64-bit lanes is explicit.
///
/// # Safety
///
/// Uses `v128_load` which is `unsafe fn` on the pointer it dereferences;
/// the caller keeps the `peq` `Vec` alive across the loop, and each load
/// pointer is bounds-checked at `Vec::as_ptr().add(idx)` — every `idx`
/// used here is `< 512` by construction and `peq` has length 512.
unsafe fn wide_block_128(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SCALAR && m <= W_SIMD128);

    // Peq is stored as an interleaved `[u64; 256 × 2]`, low lane then
    // high lane per byte value. This layout lets the inner loop pull
    // each `Peq[c]` with a single 128-bit load.
    let mut peq: alloc::vec::Vec<u64> = alloc::vec![0u64; 512];
    for (i, &c) in pattern.iter().enumerate() {
        let idx = (c as usize) * 2 + i / W_SCALAR;
        let bit = i % W_SCALAR;
        peq[idx] |= 1u64 << bit;
    }

    // Initial Pv = 1^m (bits 0..m set). Low lane is always full (m > 64);
    // high lane fills the remaining `m - 64` bits.
    let hi_mask: u64 = if m == W_SIMD128 {
        u64::MAX
    } else {
        (1u64 << (m - W_SCALAR)) - 1
    };
    let msb_bit = m - 1;
    let mut score: u32 = u32::try_from(m).expect("pattern length exceeds u32::MAX");

    // Initial state.
    // Lane 0 (low) is the low 64 bits, lane 1 (high) is the top 64 bits.
    let mut pv = u64x2_replace_lane::<1>(u64x2_splat(u64::MAX), hi_mask);
    let mut mv = u64x2_splat(0);
    let all_ones = u64x2_splat(u64::MAX);
    // Constant vector `[1, 0]` used to inject a `1` into bit 0 of the
    // shifted `Ph`.
    let one_lo = u64x2_replace_lane::<0>(u64x2_splat(0), 1);

    for &c in text {
        // `Peq[c]` — 128-bit load out of the interleaved table. Note
        // that the two `u64` entries per byte value are stored
        // contiguously, so a single `v128_load` grabs both lanes at
        // once. wasm SIMD's `v128_load` is unaligned-tolerant, so the
        // `u64`-aligned Vec backing store is fine.
        //
        // SAFETY: `(c as usize) * 2 + 1 < 512` because `c: u8` and
        // `peq.len() == 512`; the pointer is valid for a 16-byte read
        // and stays within the Vec's allocation.
        let eq = unsafe {
            let eq_ptr = peq.as_ptr().add((c as usize) * 2).cast::<v128>();
            v128_load(eq_ptr)
        };

        // Myers 1999 §4 inner loop, verbatim, in 128-bit form.
        let xv = v128_or(eq, mv);

        let eq_and_pv = v128_and(eq, pv);
        let sum = add128(eq_and_pv, pv);
        let xh = v128_or(v128_xor(sum, pv), eq);

        let ph = v128_or(mv, v128_xor(v128_or(xh, pv), all_ones));
        let mh = v128_and(pv, xh);

        if bit_at(ph, msb_bit) {
            score += 1;
        }
        if bit_at(mh, msb_bit) {
            score -= 1;
        }

        // Ph = (Ph << 1) | 1, Mh <<= 1
        let ph_shifted = v128_or(shl1(ph), one_lo);
        let mh_shifted = shl1(mh);

        pv = v128_or(mh_shifted, v128_xor(v128_or(xv, ph_shifted), all_ones));
        mv = v128_and(ph_shifted, xv);
    }

    score
}

/// 128-bit big-integer add, wrapping at 2^128.
///
/// wasm SIMD's `u64x2_add` is per-lane with no cross-lane carry, so we
/// extract, chain a scalar `overflowing_add`, and reassemble.
#[inline]
fn add128(a: v128, b: v128) -> v128 {
    let a_lo = u64x2_extract_lane::<0>(a);
    let a_hi = u64x2_extract_lane::<1>(a);
    let b_lo = u64x2_extract_lane::<0>(b);
    let b_hi = u64x2_extract_lane::<1>(b);
    let (s_lo, carry) = a_lo.overflowing_add(b_lo);
    let s_hi = a_hi.wrapping_add(b_hi).wrapping_add(u64::from(carry));
    let out = u64x2_replace_lane::<0>(u64x2_splat(0), s_lo);
    u64x2_replace_lane::<1>(out, s_hi)
}

/// Shift a full 128-bit value left by one bit; the outgoing bit 127 is
/// dropped.
///
/// wasm SIMD has no whole-register byte-shift-left on the u64 lane
/// dimension (SSE2's `_mm_slli_si128` and NEON's `vextq_u64` have no
/// direct equivalent that works at the byte level and still gives the
/// same result cheaply here). The cross-lane carry is moved via a
/// lane extract/replace pair: pull the low lane's bit-63, then insert
/// it into the high lane's bit-0 of the per-lane-shifted vector.
#[inline]
fn shl1(v: v128) -> v128 {
    // Per-lane 64-bit shift-left by 1.
    let shifted = u64x2_shl(v, 1);
    // Extract bit 63 of the low lane — this is the carry that must
    // move into bit 0 of the high lane.
    let carry_from_lo = u64x2_extract_lane::<0>(v) >> 63;
    // OR the carry into the high lane's bit-0. `u64x2_extract_lane::<1>`
    // gives us the current high lane's shifted value; ORing `carry_from_lo`
    // (which is 0 or 1) into it and writing it back places the carry at
    // bit-0 of the high lane, exactly where a full-width 128-bit shift
    // would have put it.
    let hi_shifted = u64x2_extract_lane::<1>(shifted);
    u64x2_replace_lane::<1>(shifted, hi_shifted | carry_from_lo)
}

/// Extract bit `i` (0..128) from a 128-bit vector.
#[inline]
fn bit_at(v: v128, i: usize) -> bool {
    debug_assert!(i < W_SIMD128);
    if i < W_SCALAR {
        (u64x2_extract_lane::<0>(v) >> i) & 1 != 0
    } else {
        (u64x2_extract_lane::<1>(v) >> (i - W_SCALAR)) & 1 != 0
    }
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128", test))]
mod tests {
    use super::*;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: this test module is cfg-gated on
            // `target_feature = "simd128"`, so the wasm SIMD intrinsics
            // used inside `distance` are guaranteed to be legal for
            // the engine executing this test binary.
            let simd = unsafe { distance(a, b) };
            let scalar = myers_scalar::distance(a, b);
            assert_eq!(
                simd, scalar,
                "wasm simd128 disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }

    /// Boundary at `m = 63` — last pattern length inside the scalar
    /// single-word range; the wasm SIMD path must delegate.
    #[test]
    fn wide_block_delegates_below_m_64() {
        let a: alloc::vec::Vec<u8> = (0..63u8).collect();
        let mut b = a.clone();
        b[62] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Boundary at `m = 64` — exact scalar single-word width. Also the
    /// wasm SIMD path delegates (scalar handles `m ≤ 64` faster).
    #[test]
    fn wide_block_delegates_at_m_64() {
        let a: alloc::vec::Vec<u8> = (0..64u8).collect();
        let mut b = a.clone();
        b[63] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Boundary at `m = 65` — first pattern length past the scalar
    /// single-word cutoff. Exercises the wide-block code path.
    #[test]
    fn wide_block_matches_scalar_at_m_65() {
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b[64] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
        assert_eq!(simd, 1);
    }

    /// Boundary at `m = 127` — one bit short of the register width.
    #[test]
    fn wide_block_matches_scalar_at_m_127() {
        let a: alloc::vec::Vec<u8> = (0..127u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x40;
        b[126] ^= 0x40;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Boundary at `m = 128` — exact register width. Exercises the
    /// `m == W_SIMD128` special-case for the initial `Pv`.
    #[test]
    fn wide_block_matches_scalar_at_m_128() {
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b[0] ^= 0x80;
        b[127] ^= 0x80;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Boundary at `m = 129` — first pattern length past the wasm SIMD
    /// wide-block range. Delegates to the scalar rolling-rows fallback.
    #[test]
    fn wide_block_delegates_past_m_128() {
        let a: alloc::vec::Vec<u8> = (0..129u8).collect();
        let mut b = a.clone();
        b[128] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let scalar = myers_scalar::distance(&a, &b);
        assert_eq!(simd, scalar);
    }

    /// Differential across the full m ∈ [1, 200] range against
    /// scalar Myers. Every pattern length must agree bit-for-bit.
    #[test]
    fn differential_across_lengths() {
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
            // SAFETY: see `matches_scalar_on_canonical_pairs`.
            let simd1 = unsafe { distance(&a, &b) };
            let scalar1 = myers_scalar::distance(&a, &b);
            assert_eq!(simd1, scalar1, "at m={m} on (a, b)");
            // SAFETY: same as above.
            let simd2 = unsafe { distance(&a, &text_ext) };
            let scalar2 = myers_scalar::distance(&a, &text_ext);
            assert_eq!(simd2, scalar2, "at m={m} on (a, text_ext)");
        }
    }

    /// Deterministic pseudo-random inputs — a lightweight property-style
    /// pass. `proptest` isn't available under the wasm build (its
    /// `wait-timeout` transitive dep is unix/windows-only), so we roll
    /// a small xorshift and cover the full m ∈ [64, 130] band that the
    /// wide-block path exercises.
    #[test]
    fn pseudo_random_matches_scalar() {
        // xorshift64 — fine for a differential coverage sweep.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..64 {
            // Truncation is intentional: pseudo-random 64-bit output reduced
            // into small ranges via modulo / low-byte masking; `next() & 0xff`
            // yields a `u64` in `0..=255` which is safe to narrow to `u8` via
            // `u8::try_from`.
            let m = 60 + usize::try_from(next() % 80).unwrap(); // 60..140
            let n = m + usize::try_from(next() % 200).unwrap();
            let a: alloc::vec::Vec<u8> = (0..m)
                .map(|_| u8::try_from(next() & 0xff).unwrap())
                .collect();
            let b: alloc::vec::Vec<u8> = (0..n)
                .map(|_| u8::try_from(next() & 0xff).unwrap())
                .collect();
            // SAFETY: see `matches_scalar_on_canonical_pairs`.
            let simd = unsafe { distance(&a, &b) };
            let scalar = myers_scalar::distance(&a, &b);
            assert_eq!(
                simd, scalar,
                "wasm simd128 disagreed with scalar on random inputs (m={m}, n={n})"
            );
        }
    }
}
