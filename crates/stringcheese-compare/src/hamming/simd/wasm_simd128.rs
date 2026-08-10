//! wasm SIMD128-gated Hamming kernel for `wasm32`.
//!
//! This module compiles only on `wasm32` targets and only when the
//! `simd128` target-feature is enabled at compile time. Unlike `x86_64`
//! and `aarch64`, wasm has no runtime CPU-feature detection: whether the
//! SIMD opcodes are legal is a property of the wasm engine executing the
//! module, and the module either uses them or it does not.
//!
//! # Algorithm
//!
//! Same shape as the SSE2 sibling with a 16-byte block width:
//!
//! * Load a 16-byte block from each side with `v128_load`.
//! * Compare with `u8x16_eq` (0xff where equal, 0x00 where different).
//! * Reduce to a 16-bit mask with `u8x16_bitmask` — the wasm SIMD
//!   analogue of SSE2's `_mm_movemask_epi8`.
//! * Block mismatch count = `BLOCK - matches.count_ones()`.
//! * Tail (fewer than 16 bytes) runs as a scalar byte loop.
//!
//! # Safety
//!
//! [`distance`] and [`distance_within`] are `unsafe fn` for parity with
//! the sibling SSE2/NEON backends' `#[target_feature]`-gated signature,
//! even though on wasm the target feature is a compile-time property
//! rather than a runtime precondition. On wasm32 with
//! `target_feature = "simd128"` these functions are unconditionally safe
//! to call, but `v128_load` is itself `unsafe fn` on its pointer
//! argument.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics are unsafe by declaration (v128_load); this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "the `.cast::<v128>()` in the inner loop feeds `v128_load`, which by spec accepts any-alignment pointers (the wasm SIMD load is unaligned-tolerant, same as SSE2's `_mm_loadu_si128`); the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]

use core::arch::wasm32::{u8x16_bitmask, u8x16_eq, v128, v128_load};

/// wasm SIMD128 block width in bytes — one `v128` per iteration of the
/// inner loop.
const BLOCK: usize = 16;

/// wasm SIMD128 block width as `u32` — used inside the hot loop for the
/// `BLOCK - matches` mismatch count.
const BLOCK_U32: u32 = 16;

/// wasm SIMD128-gated Hamming distance for equal-length byte slices.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// See the module-level `Safety` note.
#[must_use]
pub unsafe fn distance(a: &[u8], b: &[u8]) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::wasm_simd128::distance requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: `off + BLOCK <= len` guarantees both 16-byte reads
        // stay inside their respective slices; wasm SIMD is enabled by
        // this module's compile-time cfg-gate.
        let va = unsafe { v128_load(a.as_ptr().add(off).cast::<v128>()) };
        let vb = unsafe { v128_load(b.as_ptr().add(off).cast::<v128>()) };
        let eq = u8x16_eq(va, vb);
        let match_mask = u32::from(u8x16_bitmask(eq));
        let matches = match_mask.count_ones();
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

/// wasm SIMD128-gated Hamming distance with an early-termination cutoff.
///
/// # Panics
///
/// Panics if `a.len() != b.len()`.
///
/// # Safety
///
/// See the module-level `Safety` note.
#[must_use]
pub unsafe fn distance_within(a: &[u8], b: &[u8], cutoff: u32) -> u32 {
    assert_eq!(
        a.len(),
        b.len(),
        "hamming::simd::wasm_simd128::distance_within requires equal-length inputs (got {} and {})",
        a.len(),
        b.len(),
    );
    let len = a.len();
    let mut mismatches: u32 = 0;
    let mut off = 0usize;
    while off + BLOCK <= len {
        // SAFETY: see `distance`.
        let va = unsafe { v128_load(a.as_ptr().add(off).cast::<v128>()) };
        let vb = unsafe { v128_load(b.as_ptr().add(off).cast::<v128>()) };
        let eq = u8x16_eq(va, vb);
        let match_mask = u32::from(u8x16_bitmask(eq));
        let matches = match_mask.count_ones();
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

#[cfg(all(target_arch = "wasm32", target_feature = "simd128", test))]
mod tests {
    use super::*;
    use crate::hamming::simd::scalar;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b"a"),
            (b"karolin", b"kathrin"),
            (b"1011101", b"1001001"),
            (b"abc", b"xyz"),
        ];
        for (a, b) in cases {
            // SAFETY: this test module is cfg-gated on
            // `target_feature = "simd128"`, so the wasm SIMD intrinsics
            // used inside `distance` are guaranteed to be legal for the
            // engine executing this test binary.
            let simd = unsafe { distance(a, b) };
            let scalar_ref = scalar::distance(a, b);
            assert_eq!(
                simd, scalar_ref,
                "wasm simd128 disagreed with scalar on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn differential_across_lengths() {
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
            // SAFETY: see `matches_scalar_on_canonical_pairs`.
            let simd = unsafe { distance(&a, &b) };
            let scalar_ref = scalar::distance(&a, &b);
            assert_eq!(simd, scalar_ref, "at len={len}");
        }
    }

    #[test]
    fn distance_within_matches_scalar_below_cutoff() {
        let a: alloc::vec::Vec<u8> = (0..100u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[50] ^= 0x02;
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance_within(&a, &b, 10) };
        assert_eq!(simd, 2);
    }

    #[test]
    fn distance_within_reports_exceeded_above_cutoff() {
        let a = alloc::vec![0u8; 100];
        let b = alloc::vec![0xffu8; 100];
        // SAFETY: see `matches_scalar_on_canonical_pairs`.
        let simd = unsafe { distance_within(&a, &b, 5) };
        assert!(simd > 5);
    }
}
