//! wasm SIMD128-gated OSA (restricted Damerau-Levenshtein) kernel for
//! `wasm32`.
//!
//! This module compiles only on `wasm32` targets and only when the
//! `simd128` target-feature is enabled at compile time. Unlike `x86_64`
//! and `aarch64`, wasm has no runtime CPU-feature detection: whether the
//! SIMD opcodes are legal is a property of the wasm engine executing the
//! module, and the module either uses them or it does not. Callers
//! control the choice via `RUSTFLAGS=-C target-feature=+simd128` at
//! build time, and the dispatcher in [`super`] compiles this path in or
//! out with a matching `#[cfg(target_feature = "simd128")]` gate.
//!
//! # Algorithm
//!
//! The kernel implements Hyyrö's bit-parallel OSA algorithm — Myers's
//! word-parallel Levenshtein extended with an extra bit-vector `Pm_old`
//! (the previous column's `Peq[text[j-1]]`) and a diagonal-zero vector
//! `D0`. See Hyyrö (2003), "Bit-parallel approximate string matching
//! algorithms with transposition" (SPIRE 2003) for the derivation, and
//! [`super::scalar`] for the rolling-rows reference. This is the same
//! recurrence as the SSE2 and NEON OSA backends, just expressed with
//! wasm SIMD intrinsics.
//!
//! Concretely:
//!
//! * For patterns of length `m ≤ 64` the SIMD register width is wasted;
//!   this path delegates to [`super::scalar`] which uses the rolling-rows
//!   DP for correctness anchor.
//! * For `64 < m ≤ 128` we pack `Pv`, `Mv`, `D0`, `Pm_old`, and each
//!   `Peq[c]` entry into `v128` values and run the Hyyrö inner loop with
//!   a 128-bit integer add and a 128-bit shift-left-by-one supplying the
//!   cross-lane carries.
//! * For `m > 128` the pattern no longer fits in one 128-bit register;
//!   this path delegates back to [`super::scalar`] which handles the
//!   longer-pattern rolling-rows fallback. A block-form Hyyrö-OSA for
//!   `m > 128` (matching the SSE2/NEON siblings' scope) is documented
//!   follow-up work.
//!
//! # wasm-SIMD-specific carry mechanics
//!
//! Identical to the wasm SIMD Levenshtein backend
//! (see `crate::levenshtein::simd::wasm_simd128`): `u64x2_add` is
//! per-lane and does **not** surface a cross-lane carry. The full-width
//! 128-bit add is done by extracting the two `u64` halves with
//! `u64x2_extract_lane`, chaining a scalar `overflowing_add`, and
//! reassembling with `u64x2_replace_lane`. The full-width
//! shift-left-by-1 is done by a per-lane `u64x2_shl(v, 1)` plus a
//! scalar-side lane extract/insert that pulls the low lane's bit-63
//! (via a plain `u64 >> 63`) and ORs it into the shifted high lane's
//! bit-0. (wasm SIMD has no `_mm_slli_si128`-style whole-register byte
//! shift on the u64 lane dimension, so the cross-lane carry is moved
//! through the extract/insert pair rather than a single opcode.)
//!
//! # Safety
//!
//! [`distance`] is `unsafe fn` for parity with the sibling SSE2/NEON
//! backends' `#[target_feature]`-gated signature, even though on wasm
//! the target feature is a compile-time property rather than a runtime
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
#![allow(
    clippy::similar_names,
    reason = "the Hyyrö recurrence has structurally similar names — `hp`/`hn`, `hp_shifted`/`hn_shifted`, `d0_or_hp`/`d0_or_vp` — that map 1:1 to the paper's variables; renaming them to be more distinct would put a translation layer between the code and the derivation"
)]

use core::arch::wasm32::{
    u64x2_extract_lane, u64x2_replace_lane, u64x2_shl, u64x2_splat, v128, v128_and, v128_andnot,
    v128_load, v128_or, v128_xor,
};

use super::scalar;

/// Machine-word width used by the scalar single-word path.
const W_SCALAR: usize = 64;

/// Widest pattern length the wasm SIMD wide-block path handles.
const W_SIMD128: usize = 128;

/// wasm SIMD128-gated OSA distance for byte-slice inputs.
///
/// # Safety
///
/// wasm-SIMD target features are compile-time gates: this function is
/// only compiled when `target_feature = "simd128"` is set, so calling
/// it on a wasm engine without SIMD support is a load-time module
/// rejection rather than an in-function precondition. The `unsafe fn`
/// signature mirrors the SSE2/NEON siblings for uniformity of the
/// dispatcher's call sites and to preserve the `v128_load` intrinsic's
/// own `unsafe fn` contract.
///
/// # Panics
///
/// Panics if either input is longer than `u32::MAX` symbols — matches
/// the panic contract of the crate's other DP kernels.
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    let (pattern, text) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let m = pattern.len();
    let n = text.len();

    if m == 0 {
        return u32::try_from(n).expect("input length exceeds u32::MAX");
    }

    if m <= W_SCALAR || m > W_SIMD128 {
        // Below the wide-block break-even the scalar single-word path
        // wins; above the register width the pattern doesn't fit in
        // one 128-bit block. Fall back to the SIMD-shaped scalar OSA
        // which uses rolling-rows DP.
        return scalar::distance(a, b);
    }

    // SAFETY: the containing `unsafe fn` matches the shape of the
    // sibling SSE2/NEON backends; `wide_block_128` uses only wasm SIMD
    // intrinsics that are legal whenever this module is compiled (the
    // `target_feature = "simd128"` cfg-gate at the dispatch site).
    unsafe { wide_block_128(pattern, text, m) }
}

/// Wide-block Hyyrö-OSA for `64 < m ≤ 128`. Every bit-vector variable
/// lives in a `v128`; carry propagation between the two 64-bit lanes is
/// explicit.
///
/// # Safety
///
/// Uses `v128_load` which is `unsafe fn` on the pointer it dereferences;
/// the caller keeps the `peq` `Vec` alive across the loop, and each load
/// pointer is bounds-checked at `Vec::as_ptr().add(idx)` — every `idx`
/// used here is `< 512` by construction and `peq` has length 512.
unsafe fn wide_block_128(pattern: &[u8], text: &[u8], m: usize) -> u32 {
    debug_assert!(m > W_SCALAR && m <= W_SIMD128);

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

    // Hyyrö's OSA uses `Vp` initialized to all-ones (see the
    // rapidfuzz-rs `hyrroe2003` reference). High bits above the
    // pattern length are canceled by the mask on the score-update
    // step below.
    let mut pv = u64x2_splat(u64::MAX);
    let mut mv = u64x2_splat(0);
    let mut d0 = u64x2_splat(0);
    let mut pm_old = u64x2_splat(0);
    let all_ones = u64x2_splat(u64::MAX);
    let one_lo = u64x2_replace_lane::<0>(u64x2_splat(0), 1);

    for &c in text {
        // `Peq[c]` — 128-bit load out of the interleaved table.
        //
        // SAFETY: `(c as usize) * 2 + 1 < 512` because `c: u8` and
        // `peq.len() == 512`; the pointer is valid for a 16-byte read
        // and stays within the Vec's allocation.
        let pm_j = unsafe {
            let pm_ptr = peq.as_ptr().add((c as usize) * 2).cast::<v128>();
            v128_load(pm_ptr)
        };

        // Transposition contribution:
        //   tr = shl1((~D0_prev) & pm_j) & pm_old
        let not_d0_and_pm = v128_andnot(pm_j, d0);
        let tr_shifted = shl1(not_d0_and_pm);
        let tr = v128_and(tr_shifted, pm_old);

        // Myers D0 with `| vn | tr`:
        //   d0 = (((pm_j & vp) + vp) ^ vp) | pm_j | vn | tr
        let pm_and_vp = v128_and(pm_j, pv);
        let sum = add128(pm_and_vp, pv);
        let xor = v128_xor(sum, pv);
        let d0_new = v128_or(v128_or(xor, pm_j), v128_or(mv, tr));

        // Hp = vn | ~(d0 | vp); Hn = d0 & vp
        let d0_or_vp = v128_or(d0_new, pv);
        let hp = v128_or(mv, v128_xor(d0_or_vp, all_ones));
        let hn = v128_and(d0_new, pv);

        // Score update via MSB check.
        if bit_at(hp, msb_bit) {
            score += 1;
        }
        if bit_at(hn, msb_bit) {
            score -= 1;
        }

        // hp = (hp << 1) | 1; hn <<= 1
        let hp_shifted = v128_or(shl1(hp), one_lo);
        let hn_shifted = shl1(hn);

        // vp = hn_shifted | ~(d0 | hp_shifted); vn = hp_shifted & d0
        let d0_or_hp = v128_or(d0_new, hp_shifted);
        let pv_new = v128_or(hn_shifted, v128_xor(d0_or_hp, all_ones));
        let mv_new = v128_and(hp_shifted, d0_new);

        d0 = d0_new;
        pm_old = pm_j;
        pv = pv_new;
        mv = mv_new;
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
/// same result cheaply here). The cross-lane carry is moved via a lane
/// extract/replace pair: pull the low lane's bit-63, then insert it
/// into the high lane's bit-0 of the per-lane-shifted vector.
#[inline]
fn shl1(v: v128) -> v128 {
    let shifted = u64x2_shl(v, 1);
    let carry_from_lo = u64x2_extract_lane::<0>(v) >> 63;
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
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: this test module is cfg-gated on
            // `target_feature = "simd128"`, so the wasm SIMD intrinsics
            // used inside `distance` are guaranteed to be legal for
            // the engine executing this test binary.
            let simd = unsafe { distance(a, b) };
            let sc = scalar::distance(a, b);
            assert_eq!(
                simd, sc,
                "wasm simd128 disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }

    /// Boundary at `m = 1` — the shortest non-empty input. Delegates to
    /// the scalar single-word path.
    #[test]
    fn wide_block_delegates_at_m_1() {
        let a = b"a";
        let b = b"b";
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(a, b) };
        let sc = scalar::distance(a, b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 15` — one bit short of a single wide-block.
    #[test]
    fn wide_block_delegates_at_m_15() {
        let a: alloc::vec::Vec<u8> = (0..15u8).collect();
        let mut b = a.clone();
        b.swap(3, 4);
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 16` — one full SIMD block on either side.
    #[test]
    fn wide_block_delegates_at_m_16() {
        let a: alloc::vec::Vec<u8> = (0..16u8).collect();
        let mut b = a.clone();
        b.swap(7, 8);
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 17` — just past a single block.
    #[test]
    fn wide_block_delegates_at_m_17() {
        let a: alloc::vec::Vec<u8> = (0..17u8).collect();
        let mut b = a.clone();
        b.swap(15, 16);
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 63` — last pattern length inside the scalar
    /// single-word range; the wasm SIMD path must delegate.
    #[test]
    fn wide_block_delegates_below_m_64() {
        let a: alloc::vec::Vec<u8> = (0..63u8).collect();
        let mut b = a.clone();
        b.swap(30, 31);
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 64` — exact scalar single-word width. Also the
    /// wasm SIMD path delegates.
    #[test]
    fn wide_block_delegates_at_m_64() {
        let a: alloc::vec::Vec<u8> = (0..64u8).collect();
        let mut b = a.clone();
        b.swap(31, 32);
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 65` — first pattern length past the scalar
    /// single-word cutoff. Exercises the wide-block code path.
    #[test]
    fn wide_block_matches_scalar_at_m_65() {
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b.swap(30, 31);
        b[64] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 127` — one bit short of the register width.
    #[test]
    fn wide_block_matches_scalar_at_m_127() {
        let a: alloc::vec::Vec<u8> = (0..127u8).collect();
        let mut b = a.clone();
        b.swap(0, 1);
        b.swap(63, 64);
        b[126] ^= 0x40;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Boundary at `m = 128` — exact register width. Exercises the
    /// full-register initial `Pv`.
    #[test]
    fn wide_block_matches_scalar_at_m_128() {
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b.swap(0, 1);
        b[63] ^= 0x02;
        b.swap(64, 65);
        b[127] ^= 0x08;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance(&a, &b) };
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
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
        let sc = scalar::distance(&a, &b);
        assert_eq!(simd, sc);
    }

    /// Differential across the full m ∈ [1, 200] range against
    /// scalar OSA. Every pattern length must agree bit-for-bit.
    #[test]
    fn differential_across_lengths() {
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
            // SAFETY: see `matches_scalar_on_canonical_pairs`.
            let simd1 = unsafe { distance(&a, &b) };
            let sc1 = scalar::distance(&a, &b);
            assert_eq!(simd1, sc1, "at m={m} on (a, b)");
            // SAFETY: same as above.
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
            // SAFETY: see `matches_scalar_on_canonical_pairs`.
            let simd = unsafe { distance(&a, &b) };
            let sc = scalar::distance(&a, &b);
            assert_eq!(simd, sc, "seed={seed}");
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
            let sc = scalar::distance(&a, &b);
            assert_eq!(
                simd, sc,
                "wasm simd128 OSA disagreed with scalar on random inputs (m={m}, n={n})"
            );
        }
    }
}
