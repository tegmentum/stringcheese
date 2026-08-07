//! NEON-gated Jaro kernel for `aarch64`.
//!
//! This module compiles only on `aarch64` targets. NEON is part of the
//! aarch64 baseline, so the dispatcher's `is_aarch64_feature_detected!`
//! check is defensive rather than gating.
//!
//! # Algorithm
//!
//! The kernel is the classical Jaro similarity — Jaro (1989) — with the
//! matching-window scan lifted into a SIMD vector-compare + first-set-bit
//! reduction. Concretely:
//!
//! * For each `a[i]`, we broadcast the byte into a `uint8x16_t` with
//!   `vdupq_n_u8` and walk `b[start..end]` in 16-byte blocks.
//! * Each block is loaded with `vld1q_u8`, compared with `vceqq_u8`, and
//!   reduced to a 64-bit "compressed" mask via the standard
//!   `vshrn_n_u16::<4>` idiom — each source byte's compare result
//!   (0xff or 0x00) becomes a 4-bit nibble (0xf or 0x0) in the compressed
//!   mask. Trailing-zero divided by 4 recovers the first-set-lane index.
//! * The corresponding 16 bits of the packed "already-matched" bitmap
//!   are expanded to a matching 64-bit "nibble-per-lane" mask so the
//!   AND-and-check step happens uniformly on the compressed representation.
//!
//! Transposition counting and the final score compute stay scalar because
//! they walk sparse matched positions and don't vectorize.
//!
//! # Safety
//!
//! [`similarity`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. On `aarch64`, NEON is guaranteed by the
//! standard ABI; the dispatcher checks it anyway for uniformity across
//! architectures.

#![allow(
    unsafe_code,
    reason = "SIMD intrinsics and #[target_feature] functions are unsafe by declaration; this module is the single-file exception documented at the SIMD module root"
)]
#![allow(
    clippy::needless_range_loop,
    reason = "the outer index `i` is used to compute the matching-window bounds `[i-w, i+w+1)` in `b` and to index the `a_matched` bitmap; converting to `a.iter().enumerate()` would obscure that the loop drives the window offset, not just the read of `a[i]`"
)]
#![allow(
    clippy::cast_possible_truncation,
    reason = "`read_bits(off, valid_len)` is called with `valid_len <= 16`, so the returned `u64` always fits in the target `u16`; the cast is exact by construction"
)]

use core::arch::aarch64::{
    vceqq_u8, vdupq_n_u8, vget_lane_u64, vld1q_u8, vreinterpret_u64_u8, vreinterpretq_u16_u8,
    vshrn_n_u16,
};
use core::cmp::{max, min};

use super::common::{Bitmap, scalar_fallback};

/// NEON block width in bytes — one `uint8x16_t` per iteration of the
/// inner window scan.
const BLOCK: usize = 16;

/// NEON-gated Jaro similarity for byte-slice inputs.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` this is
/// guaranteed by the standard ABI, but the dispatcher still checks
/// `std::arch::is_aarch64_feature_detected!("neon")` for uniformity.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn similarity(a: &[u8], b: &[u8]) -> f64 {
    if a.len() < BLOCK && b.len() < BLOCK {
        return scalar_fallback(a, b);
    }
    // SAFETY: NEON target-feature context established by this function's
    // `#[target_feature(enable = "neon")]` upholds the NEON precondition
    // of `similarity_impl`.
    unsafe { similarity_impl(a, b) }
}

/// Full Jaro similarity computation under a NEON target-feature context.
///
/// # Safety
///
/// The caller must ensure NEON is available.
#[target_feature(enable = "neon")]
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
        // SAFETY: NEON target-feature context in place; `find_match_in_window`
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

/// Expand a 16-bit bitmap slice (one bit per lane) into a 64-bit
/// "nibble-per-lane" mask (4 bits per lane, either 0xf or 0x0), matching
/// the shape produced by the `vshrn_n_u16::<4>` movemask idiom below.
///
/// This is used to combine the "already-matched" bitmap with the compare
/// result on the compressed representation, without needing to leave the
/// GPR side of the pipeline.
#[inline]
fn expand_bits_to_nibbles(bits: u16) -> u64 {
    // The idea: each set bit i in `bits` should produce `0xf << (i * 4)`
    // in the nibble mask. There's no NEON pdep-style intrinsic; the
    // fastest portable form is a small lookup on nibbles.
    //
    // With `bits` having only 16 bits, we split into two u8 nibbles and
    // build the mask via a per-bit sequence. The compiler unrolls this
    // to a straight-line dependency chain.
    // Walk in u32 (not u16) so the `bits >>= tz + 1` at `tz = 15` — a
    // 16-bit shift of a 16-bit value — doesn't overflow the shift amount
    // when the top bit was the last one set.
    let mut out: u64 = 0;
    let mut bits = u32::from(bits);
    let mut i: u32 = 0;
    while bits != 0 {
        let tz = bits.trailing_zeros();
        out |= 0xfu64 << ((i + tz) * 4);
        i += tz + 1;
        bits >>= tz;
        bits >>= 1;
    }
    out
}

/// NEON-lifted `find_match_in_window`. Walks `b[start..end]` in 16-byte
/// blocks, comparing each block against a broadcast of `needle`, and
/// reducing to a 64-bit nibble-per-lane mask for the first-set-lane
/// lookup.
///
/// # Safety
///
/// NEON must be available.
#[target_feature(enable = "neon")]
unsafe fn find_match_in_window(
    needle: u8,
    b: &[u8],
    start: usize,
    end: usize,
    b_matched: &Bitmap,
) -> Option<usize> {
    // SAFETY: NEON target-feature context established by this function's
    // `#[target_feature(enable = "neon")]` — every intrinsic call below
    // is NEON.
    unsafe {
        let needle_bcast = vdupq_n_u8(needle);
        let mut off = start;
        while off < end {
            let valid_len = BLOCK.min(end - off);
            let block = if off + BLOCK <= b.len() {
                vld1q_u8(b.as_ptr().add(off))
            } else {
                let mut buf = [0u8; BLOCK];
                buf[..valid_len].copy_from_slice(&b[off..off + valid_len]);
                vld1q_u8(buf.as_ptr())
            };
            let cmp = vceqq_u8(block, needle_bcast);
            // Standard NEON movemask idiom: reinterpret as u16x8, then
            // narrow with a >>4 to get u8x8 where each source u16 lane
            // becomes an 8-bit "OR" of its two byte halves shifted into
            // the low nibble. For lanes where both source bytes were
            // 0xff, the result nibble is 0xf; where both were 0x00, the
            // result nibble is 0x0. The full u64 gets 4 bits per source
            // byte.
            let narrow = vshrn_n_u16::<4>(vreinterpretq_u16_u8(cmp));
            let eq_nibbles: u64 = vget_lane_u64::<0>(vreinterpret_u64_u8(narrow));

            let bm = b_matched.read_bits(off, valid_len) as u16;
            let bm_nibbles = expand_bits_to_nibbles(bm);

            let valid_lanes_nibbles: u64 = if valid_len == BLOCK {
                u64::MAX
            } else {
                // valid_len nibbles set to 0xf.
                (1u64 << (valid_len * 4)) - 1
            };

            let candidate = eq_nibbles & !bm_nibbles & valid_lanes_nibbles;
            if candidate != 0 {
                // 4 bits per lane — divide trailing_zeros by 4 to get
                // the byte-lane index of the first match.
                let lane = (candidate.trailing_zeros() / 4) as usize;
                return Some(off + lane);
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
    fn expand_bits_to_nibbles_smoke() {
        assert_eq!(expand_bits_to_nibbles(0), 0);
        assert_eq!(expand_bits_to_nibbles(0b1), 0xf);
        assert_eq!(expand_bits_to_nibbles(0b10), 0xf0);
        assert_eq!(expand_bits_to_nibbles(0b101), 0xf0f);
        assert_eq!(expand_bits_to_nibbles(u16::MAX), u64::MAX);
    }

    #[test]
    fn matches_scalar_on_canonical_pairs() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        for (a, b) in [
            (b"".as_ref(), b"".as_ref()),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
        ] {
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { similarity(a, b) };
            let generic = jaro_similarity(a, b);
            assert_eq!(
                simd.to_bits(),
                generic.to_bits(),
                "neon disagreed with generic on ({a:?}, {b:?})"
            );
        }
    }

    /// Every pattern length across the SIMD-relevant range must agree
    /// with the generic Jaro kernel bit-for-bit. Boundaries covered:
    /// 15/16/17 (single block edge) and 63/64/65 (bitmap-word straddle).
    #[test]
    fn differential_across_lengths() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
            // SAFETY: is_aarch64_feature_detected!("neon") returned true.
            let simd = unsafe { similarity(&a, &b) };
            let generic = jaro_similarity(&a, &b);
            assert_eq!(simd.to_bits(), generic.to_bits(), "at m={m} on (a, b)");
        }
    }

    /// Boundary at exactly one full NEON block on the shorter side.
    #[test]
    fn boundary_length_16_matches_generic() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..16u8).collect();
        let mut b = a.clone();
        b[3] ^= 0x01;
        b[15] ^= 0x02;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Boundary just past one block — hits the tail-padding path.
    #[test]
    fn boundary_length_17_matches_generic() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..17u8).collect();
        let mut b = a.clone();
        b[16] ^= 0x04;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }

    /// Longer window — multiple NEON blocks per outer iteration.
    #[test]
    fn boundary_length_128_matches_generic() {
        if !std::arch::is_aarch64_feature_detected!("neon") {
            return;
        }
        let a: alloc::vec::Vec<u8> = (0..128u8).collect();
        let mut b = a.clone();
        b[10] ^= 0x01;
        b[63] ^= 0x02;
        b[64] ^= 0x04;
        b[127] ^= 0x08;
        // SAFETY: is_aarch64_feature_detected!("neon") returned true.
        let simd = unsafe { similarity(&a, &b) };
        let generic = jaro_similarity(&a, &b);
        assert_eq!(simd.to_bits(), generic.to_bits());
    }
}
