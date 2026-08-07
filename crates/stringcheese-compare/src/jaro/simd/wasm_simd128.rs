//! wasm SIMD128-gated Jaro kernel for `wasm32`.
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
//! The kernel is the classical Jaro similarity — Jaro (1989) — with the
//! matching-window scan lifted into a SIMD vector-compare + first-set-bit
//! reduction, same shape as the SSE2 sibling. Concretely:
//!
//! * For each `a[i]`, we broadcast the byte into a `v128` with
//!   `u8x16_splat` and walk `b[start..end]` in 16-byte blocks.
//! * Each block is loaded with `v128_load` (unaligned-tolerant by spec),
//!   compared with `u8x16_eq`, and reduced to a 16-bit mask by
//!   `u8x16_bitmask` — the wasm SIMD analogue of SSE2's
//!   `_mm_movemask_epi8`.
//! * The corresponding 16 bits of the packed "already-matched" bitmap
//!   are extracted from a flat `Vec<u64>` via a straddling shift-OR
//!   (shared with the sibling arch backends via [`super::common`]).
//! * The candidate set is `eq_mask & !bitmap & valid_lanes_mask`; the
//!   first set bit (via `trailing_zeros`) gives the absolute position of
//!   the earliest unclaimed match in this block.
//!
//! Transposition counting and the final score compute stay scalar because
//! they walk sparse matched positions and don't vectorize.
//!
//! # Safety
//!
//! [`similarity`] is `unsafe fn` for parity with the sibling SSE2/NEON
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
#![allow(
    clippy::needless_range_loop,
    reason = "the outer index `i` is used to compute the matching-window bounds `[i-w, i+w+1)` in `b` and to index the `a_matched` bitmap; converting to `a.iter().enumerate()` would obscure that the loop drives the window offset, not just the read of `a[i]`"
)]
#![allow(
    clippy::cast_possible_truncation,
    reason = "`read_bits(off, valid_len)` is called with `valid_len <= 16`, so the returned `u64` always fits in the target `u16`; the cast is exact by construction"
)]

use core::arch::wasm32::{u8x16_bitmask, u8x16_eq, u8x16_splat, v128, v128_load};
use core::cmp::{max, min};

use super::common::{Bitmap, scalar_fallback};

/// wasm SIMD128 block width in bytes — one `v128` per iteration of the
/// inner window scan.
const BLOCK: usize = 16;

/// wasm SIMD128-gated Jaro similarity for byte-slice inputs.
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
#[must_use]
pub unsafe fn similarity(a: &[u8], b: &[u8]) -> f64 {
    // Very-short inputs stay on the scalar path — the block-copy and
    // bitmap-allocation overheads of the SIMD scan don't pay off below
    // the SIMD block width on either side.
    if a.len() < BLOCK && b.len() < BLOCK {
        return scalar_fallback(a, b);
    }
    // SAFETY: the containing `unsafe fn` matches the shape of the
    // sibling SSE2/NEON backends; `similarity_impl` uses only wasm SIMD
    // intrinsics that are legal whenever this module is compiled (the
    // `target_feature = "simd128"` cfg-gate at the dispatch site).
    unsafe { similarity_impl(a, b) }
}

/// Full Jaro similarity computation. The wasm SIMD intrinsics used
/// inside are safe to call once the module is compiled with
/// `target_feature = "simd128"`.
///
/// # Safety
///
/// See [`similarity`].
#[allow(
    clippy::many_single_char_names,
    reason = "the Jaro paper's canonical notation is `a`, `b`, `m`, `t`, `w`; renaming would obscure the direct correspondence with the published definition"
)]
#[allow(
    clippy::cast_precision_loss,
    reason = "inputs approaching 2^53 symbols exceed every practical Jaro use; the cast is exact for anything smaller"
)]
unsafe fn similarity_impl(a: &[u8], b: &[u8]) -> f64 {
    let len_a = a.len();
    let len_b = b.len();

    if len_a == 0 && len_b == 0 {
        return 1.0;
    }
    if len_a == 0 || len_b == 0 {
        return 0.0;
    }

    let max_len = max(len_a, len_b);
    let window = (max_len / 2).saturating_sub(1);

    let mut a_matched = Bitmap::new(len_a);
    let mut b_matched = Bitmap::new(len_b);

    let mut matches: usize = 0;
    for i in 0..len_a {
        let start = i.saturating_sub(window).min(len_b);
        let end = min(len_b, i + window + 1);
        if start >= end {
            continue;
        }
        // SAFETY: `find_match_in_window` uses only wasm SIMD intrinsics
        // that are legal whenever this module is compiled.
        if let Some(j) = unsafe { find_match_in_window(a[i], b, start, end, &b_matched) } {
            a_matched.set(i);
            b_matched.set(j);
            matches += 1;
        }
    }

    if matches == 0 {
        return 0.0;
    }

    let mut disagreements: usize = 0;
    let mut k: usize = 0;
    for i in 0..len_a {
        if !a_matched.get(i) {
            continue;
        }
        while !b_matched.get(k) {
            k += 1;
        }
        if a[i] != b[k] {
            disagreements += 1;
        }
        k += 1;
    }
    let transpositions = disagreements / 2;

    let m = matches as f64;
    let a_len_f = len_a as f64;
    let b_len_f = len_b as f64;
    let t = transpositions as f64;
    (m / a_len_f + m / b_len_f + (m - t) / m) / 3.0
}

/// wasm SIMD128-lifted `find_match_in_window`. Walks `b[start..end]` in
/// 16-byte blocks, comparing each block against a broadcast of `needle`
/// and masking against the packed `b_matched` bitmap.
///
/// # Safety
///
/// Uses `v128_load` which is `unsafe fn` on the pointer it dereferences;
/// each pointer either points inside `b` for a full-block read or into
/// a stack-allocated 16-byte buffer for the tail path. Both are valid
/// for a 16-byte read whenever the load fires.
unsafe fn find_match_in_window(
    needle: u8,
    b: &[u8],
    start: usize,
    end: usize,
    b_matched: &Bitmap,
) -> Option<usize> {
    let needle_bcast = u8x16_splat(needle);
    let mut off = start;
    while off < end {
        let valid_len = BLOCK.min(end - off);
        // Load 16 bytes. If we can read a full block without going
        // past the end of `b`, do the direct unaligned load — the
        // `valid_lanes_mask` below filters out any spurious matches
        // beyond the window. Otherwise pad the tail into a stack
        // buffer.
        //
        // SAFETY: the full-block branch reads exactly 16 bytes from
        // `b.as_ptr().add(off)`, guarded by `off + BLOCK <= b.len()`;
        // the tail branch reads from a 16-byte stack buffer that has
        // its `valid_len` prefix populated and the rest zero-initialised.
        let block = unsafe {
            if off + BLOCK <= b.len() {
                v128_load(b.as_ptr().add(off).cast::<v128>())
            } else {
                let mut buf = [0u8; BLOCK];
                buf[..valid_len].copy_from_slice(&b[off..off + valid_len]);
                v128_load(buf.as_ptr().cast::<v128>())
            }
        };
        let cmp = u8x16_eq(block, needle_bcast);
        // `u8x16_bitmask` on wasm32 returns a `u16` whose bits are the
        // per-lane MSB of `cmp` — exactly the shape SSE2's
        // `_mm_movemask_epi8` produces after truncation.
        let eq_mask = u8x16_bitmask(cmp);
        let bm = b_matched.read_bits(off, valid_len) as u16;
        let valid_lanes_mask: u16 = if valid_len == BLOCK {
            u16::MAX
        } else {
            (1u16 << valid_len) - 1
        };
        let candidate = eq_mask & !bm & valid_lanes_mask;
        if candidate != 0 {
            return Some(off + candidate.trailing_zeros() as usize);
        }
        off += valid_len;
    }
    None
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128", test))]
mod tests {
    use super::*;
    use crate::jaro::jaro::jaro_similarity;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: this test module is cfg-gated on
            // `target_feature = "simd128"`, so the wasm SIMD intrinsics
            // used inside `similarity` are guaranteed to be legal for
            // the engine executing this test binary.
            let simd = unsafe { similarity(a, b) };
            let generic = jaro_similarity(a, b);
            assert_eq!(
                simd.to_bits(),
                generic.to_bits(),
                "wasm simd128 disagreed with generic on ({a:?}, {b:?})"
            );
        }
    }

    /// Every pattern length across the SIMD-relevant range must agree
    /// with the generic Jaro kernel bit-for-bit. Boundaries covered:
    /// 1, 15/16/17 (single block edge) and 63/64/65 (bitmap-word straddle).
    #[test]
    fn differential_across_lengths() {
        for m in 1..=80usize {
            let a: alloc::vec::Vec<u8> = (0..m)
                .map(|i| u8::try_from(i & 0xff).unwrap().wrapping_mul(31))
                .collect();
            let mut b = a.clone();
            if !b.is_empty() {
                b[m / 2] ^= 0x5A;
            }
            // SAFETY: see `matches_scalar_on_canonical_pairs`.
            let simd = unsafe { similarity(&a, &b) };
            let generic = jaro_similarity(&a, &b);
            assert_eq!(simd.to_bits(), generic.to_bits(), "at m={m} on (a, b)");
        }
    }

    /// Boundary at `m = 1` — the shortest non-empty input.
    #[test]
    fn boundary_length_1_matches_generic() {
        let a = b"a";
        let b = b"a";
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(a, b) };
        let generic = jaro_similarity(a, b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Boundary at `m = 15` — one bit short of a full block.
    #[test]
    fn boundary_length_15_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..15u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[14] ^= 0x02;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Boundary check at exactly one full wasm SIMD block on the shorter
    /// side.
    #[test]
    fn boundary_length_16_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..16u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[15] ^= 0x02;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Boundary check just past one block — hits the tail-padding path.
    #[test]
    fn boundary_length_17_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..17u8).collect();
        let mut b = a.clone();
        b[16] ^= 0x04;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Bitmap-word straddle boundary — the packed `b_matched` bitmap is a
    /// flat `Vec<u64>`, so windows crossing the 64-bit boundary must
    /// still read the correct bits.
    #[test]
    fn boundary_length_63_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..63u8).collect();
        let mut b = a.clone();
        b[62] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    #[test]
    fn boundary_length_64_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..64u8).collect();
        let mut b = a.clone();
        b[63] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    #[test]
    fn boundary_length_65_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..65u8).collect();
        let mut b = a.clone();
        b[64] ^= 0x01;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Longer window — multiple wasm SIMD blocks per outer iteration.
    #[test]
    fn boundary_length_127_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..127u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[63] ^= 0x02;
        b[64] ^= 0x04;
        b[126] ^= 0x08;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    #[test]
    fn boundary_length_128_matches_generic() {
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[63] ^= 0x02;
        b[64] ^= 0x04;
        b[127] ^= 0x08;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Extreme asymmetric case — very short `b`, long `a` (window can
    /// overshoot end of `b`).
    #[test]
    fn asymmetric_lengths_match_generic() {
        let a = alloc::vec![0u8; 83];
        let b = alloc::vec![0u8; 1];
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Deterministic pseudo-random inputs — a lightweight property-style
    /// pass. `proptest` isn't available under the wasm build (its
    /// `wait-timeout` transitive dep is unix/windows-only), so we roll
    /// a small xorshift and cover the SIMD-relevant length band.
    #[test]
    fn pseudo_random_matches_generic() {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..64 {
            let m = 16 + usize::try_from(next() % 128).unwrap();
            let n = 16 + usize::try_from(next() % 128).unwrap();
            let a: alloc::vec::Vec<u8> = (0..m)
                .map(|_| u8::try_from(next() & 0x0f).unwrap())
                .collect();
            let b: alloc::vec::Vec<u8> = (0..n)
                .map(|_| u8::try_from(next() & 0x0f).unwrap())
                .collect();
            // SAFETY: see `matches_scalar_on_canonical_pairs`.
            let simd = unsafe { similarity(&a, &b) };
            let generic = jaro_similarity(&a, &b);
            assert_eq!(
                simd.to_bits(),
                generic.to_bits(),
                "wasm simd128 Jaro disagreed with generic on random inputs (m={m}, n={n})"
            );
        }
    }
}
