//! SSE2-gated Jaro kernel for `x86_64`.
//!
//! This module compiles only on `x86_64` targets. It is the SSE2 fallback
//! selected by the dispatcher when AVX2 is unavailable — every `x86_64`
//! CPU has SSE2 as part of the baseline ABI, so this branch is always a
//! valid target.
//!
//! # Algorithm
//!
//! The kernel is the classical Jaro similarity — Jaro (1989) — with the
//! matching-window scan lifted into a SIMD vector-compare + first-set-bit
//! reduction. Concretely:
//!
//! * For each `a[i]`, we broadcast the byte into a `__m128i` with
//!   `_mm_set1_epi8` and walk `b[start..end]` in 16-byte blocks.
//! * Each block is loaded with `_mm_loadu_si128`, compared with
//!   `_mm_cmpeq_epi8`, and reduced to a 16-bit mask by
//!   `_mm_movemask_epi8`.
//! * The corresponding 16 bits of the packed "already-matched" bitmap
//!   are extracted from a flat `Vec<u64>` via a straddling shift-OR.
//! * The candidate set is `eq_mask & !bitmap & valid_lanes_mask`; the
//!   first set bit (via `trailing_zeros`) gives the absolute position of
//!   the earliest unclaimed match in this block.
//!
//! Transposition counting and the final score compute stay scalar because
//! they walk sparse matched positions and don't vectorize.
//!
//! # Safety
//!
//! [`similarity`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature must
//! be present at run time. On `x86_64`, SSE2 is guaranteed by the ABI;
//! the dispatcher checks it anyway for consistency with the other arch
//! branches.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::cast_possible_wrap,
    reason = "`_mm_set1_epi8` takes `i8`; the byte-broadcast is a pure bit-transmute, not a numeric conversion"
)]
#![allow(
    clippy::cast_ptr_alignment,
    reason = "every pointer cast in this module feeds an *unaligned* SSE2 load (`_mm_loadu_si128`), which by contract accepts any-alignment `*const __m128i`; the clippy lint doesn't know the intrinsic tolerates under-alignment"
)]
#![allow(
    clippy::needless_range_loop,
    reason = "the outer index `i` is used to compute the matching-window bounds `[i-w, i+w+1)` in `b` and to index the `a_matched` bitmap; converting to `a.iter().enumerate()` would obscure that the loop drives the window offset, not just the read of `a[i]`"
)]
#![allow(
    clippy::cast_possible_truncation,
    reason = "`read_bits(off, valid_len)` is called with `valid_len <= 16`, so the returned `u64` always fits in the target `u16`; the cast is exact by construction"
)]

use core::arch::x86_64::{
    __m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_movemask_epi8, _mm_set1_epi8,
};
use core::cmp::{max, min};

use super::common::{Bitmap, scalar_fallback};

/// SSE2 block width in bytes — one `__m128i` per iteration of the inner
/// window scan.
const BLOCK: usize = 16;

/// SSE2-gated Jaro similarity for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` this is
/// guaranteed by the ABI, but the dispatcher still checks
/// `is_x86_feature_detected!("sse2")` to keep every dispatch branch
/// uniform.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn similarity(a: &[u8], b: &[u8]) -> f64 {
    // Very-short inputs stay on the scalar path — the block-copy and
    // bitmap-allocation overheads of the SIMD scan don't pay off below
    // the SIMD block width on either side.
    if a.len() < BLOCK && b.len() < BLOCK {
        return scalar_fallback(a, b);
    }
    // SAFETY: SSE2 target-feature context established by this function's
    // `#[target_feature(enable = "sse2")]` upholds the SSE2 precondition
    // of `similarity_impl`.
    unsafe { similarity_impl(a, b) }
}

/// Full Jaro similarity computation under an SSE2 target-feature context.
///
/// # Safety
///
/// The caller must ensure SSE2 is available.
#[target_feature(enable = "sse2")]
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
        // SAFETY: SSE2 target-feature context in place; `find_match_in_window`
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

/// SSE2-lifted `find_match_in_window`. Walks `b[start..end]` in 16-byte
/// blocks, comparing each block against a broadcast of `needle` and
/// masking against the packed `b_matched` bitmap.
///
/// # Safety
///
/// SSE2 must be available.
#[target_feature(enable = "sse2")]
unsafe fn find_match_in_window(
    needle: u8,
    b: &[u8],
    start: usize,
    end: usize,
    b_matched: &Bitmap,
) -> Option<usize> {
    // SAFETY: SSE2 target-feature context established by this function's
    // `#[target_feature(enable = "sse2")]` — every intrinsic call below
    // is SSE2.
    unsafe {
        let needle_bcast = _mm_set1_epi8(needle as i8);
        let mut off = start;
        while off < end {
            let valid_len = BLOCK.min(end - off);
            // Load 16 bytes. If we can read a full block without going
            // past the end of `b`, do the direct unaligned load — the
            // `valid_lanes_mask` below filters out any spurious matches
            // beyond the window. Otherwise pad the tail into a stack
            // buffer.
            let block = if off + BLOCK <= b.len() {
                _mm_loadu_si128(b.as_ptr().add(off).cast::<__m128i>())
            } else {
                let mut buf = [0u8; BLOCK];
                buf[..valid_len].copy_from_slice(&b[off..off + valid_len]);
                _mm_loadu_si128(buf.as_ptr().cast::<__m128i>())
            };
            let cmp = _mm_cmpeq_epi8(block, needle_bcast);
            let eq_mask = _mm_movemask_epi8(cmp).cast_unsigned() as u16;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jaro::jaro::jaro_similarity;

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { similarity(a, b) };
            let generic = jaro_similarity(a, b);
            assert_eq!(
                simd.to_bits(),
                generic.to_bits(),
                "sse2 disagreed with generic on ({a:?}, {b:?})"
            );
        }
    }

    /// Every pattern length across the SIMD-relevant range must agree
    /// with the generic Jaro kernel bit-for-bit. Boundaries covered:
    /// 15/16/17 (single block edge) and 63/64/65 (bitmap-word straddle).
    #[test]
    fn differential_across_lengths() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for m in 1..=80usize {
            let a: alloc::vec::Vec<u8> = (0..m)
                .map(|i| u8::try_from(i & 0xff).unwrap().wrapping_mul(31))
                .collect();
            let mut b = a.clone();
            if !b.is_empty() {
                b[m / 2] ^= 0x5A;
            }
            // SAFETY: is_x86_feature_detected!("sse2") returned true.
            let simd = unsafe { similarity(&a, &b) };
            let generic = jaro_similarity(&a, &b);
            assert_eq!(simd.to_bits(), generic.to_bits(), "at m={m} on (a, b)");
        }
    }

    /// Boundary check at exactly one full SSE2 block on the shorter side.
    #[test]
    fn boundary_length_16_matches_generic() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..16u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[15] ^= 0x02;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Boundary check just past one block — hits the tail-padding path.
    #[test]
    fn boundary_length_17_matches_generic() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..17u8).collect();
        let mut b = a.clone();
        b[16] ^= 0x04;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Longer window — multiple SSE2 blocks per outer iteration.
    #[test]
    fn boundary_length_128_matches_generic() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[63] ^= 0x02;
        b[64] ^= 0x04;
        b[127] ^= 0x08;
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Extreme asymmetric case — very short `b`, long `a` (window can
    /// overshoot end of `b`).
    #[test]
    fn asymmetric_lengths_match_generic() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        let a = alloc::vec![0u8; 83];
        let b = alloc::vec![0u8; 1];
        // SAFETY: is_x86_feature_detected!("sse2") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }
}
