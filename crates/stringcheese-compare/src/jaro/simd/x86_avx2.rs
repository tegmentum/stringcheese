//! AVX2-gated Jaro kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. AVX2 doubles the
//! matching-window scan width from SSE2's 16 bytes per iteration to 32
//! bytes per iteration, so the AVX2 backend is meaningfully faster than
//! the SSE2 sibling whenever the shorter side of the input pair is long
//! enough to warrant the wider registers (roughly, `min(|a|, |b|) >
//! 2 · BLOCK`).
//!
//! # Algorithm
//!
//! The kernel is the classical Jaro similarity — Jaro (1989) — with the
//! matching-window scan lifted into a SIMD vector-compare + first-set-bit
//! reduction. Concretely:
//!
//! * For each `a[i]`, we broadcast the byte into a `__m256i` with
//!   `_mm256_set1_epi8` and walk `b[start..end]` in 32-byte blocks.
//! * Each block is loaded with `_mm256_loadu_si256`, compared with
//!   `_mm256_cmpeq_epi8`, and reduced to a 32-bit mask by
//!   `_mm256_movemask_epi8`.
//! * The corresponding 32 bits of the packed "already-matched" bitmap
//!   are extracted from a flat `Vec<u64>`; a straddling shift-OR yields
//!   the 32-bit slice at any absolute bit position.
//! * The candidate set is `eq_mask & !bitmap & valid_lanes_mask`; the
//!   first set bit (via `trailing_zeros`) gives the absolute position of
//!   the earliest unclaimed match in this block.
//!
//! # Safety
//!
//! [`similarity`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher in [`crate::jaro::simd`]
//! gates every call on `is_x86_feature_detected!("avx2")`, so the
//! precondition is met by construction; call sites outside the dispatcher
//! must uphold the same contract.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_possible_wrap,
    reason = "`_mm256_set1_epi8` takes `i8`; the byte-broadcast is a pure bit-transmute, not a numeric conversion"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "every pointer cast in this module feeds an *unaligned* AVX2 load (`_mm256_loadu_si256`), which by contract accepts any-alignment `*const __m256i`; the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]
#![allow(
    clippy::needless_range_loop,
    reason = "the outer index `i` is used to compute the matching-window bounds `[i-w, i+w+1)` in `b` and to index the `a_matched` bitmap; converting to `a.iter().enumerate()` would obscure that the loop drives the window offset, not just the read of `a[i]`"
)]
#![allow(
    clippy::cast_possible_truncation,
    reason = "`read_bits(off, valid_len)` is called with `valid_len <= 32`, so the returned `u64` always fits in the target `u32`; the cast is exact by construction"
)]

use core::arch::x86_64::{
    __m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
};
use core::cmp::{max, min};

use super::common::{Bitmap, scalar_fallback};

/// AVX2 block width in bytes — one `__m256i` per iteration of the inner
/// window scan.
const BLOCK: usize = 32;

/// AVX2-gated Jaro similarity for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure AVX2 is available on the running CPU. The
/// dispatcher in the parent [`super`] module guarantees this via
/// `is_x86_feature_detected!("avx2")`.
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn similarity(a: &[u8], b: &[u8]) -> f64 {
    // Very-short inputs stay on the scalar path — the block-copy and
    // bitmap-allocation overheads of the AVX2 scan don't pay off below
    // one full 32-byte block on either side.
    if a.len() < BLOCK && b.len() < BLOCK {
        return scalar_fallback(a, b);
    }
    // SAFETY: AVX2 target-feature context established by this function's
    // `#[target_feature(enable = "avx2")]` upholds the AVX2 precondition
    // of `similarity_impl`.
    unsafe { similarity_impl(a, b) }
}

/// Full Jaro similarity computation under an AVX2 target-feature context.
///
/// # Safety
///
/// The caller must ensure AVX2 is available.
#[target_feature(enable = "avx2")]
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
        // SAFETY: AVX2 target-feature context in place; `find_match_in_window`
        // upholds its own preconditions via this outer context.
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

/// AVX2-lifted `find_match_in_window`. Walks `b[start..end]` in 32-byte
/// blocks; each block is a single `pcmpeqb`+`pmovmskb` pair.
///
/// # Safety
///
/// AVX2 must be available.
#[target_feature(enable = "avx2")]
unsafe fn find_match_in_window(
    needle: u8,
    b: &[u8],
    start: usize,
    end: usize,
    b_matched: &Bitmap,
) -> Option<usize> {
    // SAFETY: AVX2 target-feature context established by this function's
    // `#[target_feature(enable = "avx2")]` — every intrinsic call below
    // is AVX2.
    unsafe {
        let needle_bcast = _mm256_set1_epi8(needle as i8);
        let mut off = start;
        while off < end {
            let valid_len = BLOCK.min(end - off);
            let block = if off + BLOCK <= b.len() {
                _mm256_loadu_si256(b.as_ptr().add(off).cast::<__m256i>())
            } else {
                let mut buf = [0u8; BLOCK];
                buf[..valid_len].copy_from_slice(&b[off..off + valid_len]);
                _mm256_loadu_si256(buf.as_ptr().cast::<__m256i>())
            };
            let cmp = _mm256_cmpeq_epi8(block, needle_bcast);
            let eq_mask = _mm256_movemask_epi8(cmp).cast_unsigned();
            let bm = b_matched.read_bits(off, valid_len) as u32;
            let valid_lanes_mask: u32 = if valid_len == BLOCK {
                u32::MAX
            } else {
                (1u32 << valid_len) - 1
            };
            let candidate = eq_mask & !bm & valid_lanes_mask;
            if candidate != 0 {
                return Some(off + candidate.trailing_zeros() as usize);
            }
            off += valid_len;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jaro::jaro::jaro_similarity;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { similarity(a, b) };
            let generic = jaro_similarity(a, b);
            assert_eq!(
                simd.to_bits(),
                generic.to_bits(),
                "avx2 disagreed with generic on ({a:?}, {b:?})"
            );
        }
    }

    /// Every pattern length across the SIMD-relevant range must agree
    /// with the generic Jaro kernel bit-for-bit. Boundaries covered:
    /// 31/32/33 (single block edge), 63/64/65 (bitmap-word straddle).
    #[test]
    fn differential_across_lengths() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for m in 1..=100usize {
            let a: alloc::vec::Vec<u8> = (0..m)
                .map(|i| u8::try_from(i & 0xff).unwrap().wrapping_mul(31))
                .collect();
            let mut b = a.clone();
            if !b.is_empty() {
                b[m / 2] ^= 0x5A;
            }
            // SAFETY: is_x86_feature_detected!("avx2") returned true.
            let simd = unsafe { similarity(&a, &b) };
            let generic = jaro_similarity(&a, &b);
            assert_eq!(simd.to_bits(), generic.to_bits(), "at m={m} on (a, b)");
        }
    }

    /// Boundary at exactly one full AVX2 block on the shorter side.
    #[test]
    fn boundary_length_32_matches_generic() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..32u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[31] ^= 0x02;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Boundary just past one block — hits the tail-padding path.
    #[test]
    fn boundary_length_33_matches_generic() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..33u8).collect();
        let mut b = a.clone();
        b[32] ^= 0x04;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Larger inputs — multi-block window per outer iteration.
    #[test]
    fn boundary_length_256_matches_generic() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..=255u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[127] ^= 0x02;
        b[128] ^= 0x04;
        b[255] ^= 0x08;
        // SAFETY: is_x86_feature_detected!("avx2") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }
}
