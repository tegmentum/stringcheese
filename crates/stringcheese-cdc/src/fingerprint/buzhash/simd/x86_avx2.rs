//! AVX2-gated Buzhash slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. This is the widest x86 backend
//! [the dispatcher][super::digest_of_slice] can select; it falls back to
//! the [SSE2 sibling][super::x86_sse2] on hosts without AVX2.
//!
//! # Kernel shape — block reformulation
//!
//! Buzhash's per-byte recurrence
//! `state_{n+1} = state_n.rotate_left(1) ^ contrib_n`, where
//! `contrib_n = T[bytes[n]] ^ (n >= window ? T[bytes[n-window]].rotate_left(window mod 64) : 0)`,
//! unrolls over any `k`-byte run into the closed form
//!
//! ```text
//! state_k = state_0.rotate_left(k)
//!         ^ Σ_{i=0..k}  contrib_i.rotate_left(k-1-i)
//! ```
//!
//! At `k = 64` the leading `state_0.rotate_left(64)` reduces to `state_0`
//! unchanged (a `u64` rotate cycles at 64 bits), so a 64-byte block
//! folds into the running state as a simple XOR:
//!
//! ```text
//! state_after = state_before ^ blockhash
//! blockhash   = Σ_{i=0..64} contrib_i.rotate_left(63 - i)
//! ```
//!
//! The block sum has a natural 4-lane SIMD shape when written as a
//! Horner recurrence over 16 groups of 4 bytes:
//!
//! ```text
//! acc = acc.rotate_left(4) ^ [ ROL(c[4k+0], 3), ROL(c[4k+1], 2),
//!                              ROL(c[4k+2], 1), ROL(c[4k+3], 0) ]
//! ```
//!
//! After 16 iterations, XOR-reducing the four `u64` lanes reproduces
//! the block sum bit-for-bit. Below 64 bytes the kernel defers to the
//! scalar loop, and any `len % 64` tail after the last full block is
//! consumed by the same scalar recurrence — both share the reference
//! `state = state.rotate_left(1) ^ contrib_i` core that anchors the
//! differential contract.
//!
//! # Implementation
//!
//! Contributions are computed scalar-side into a 64-entry stack buffer
//! — each `contrib_i` is one 256-entry table lookup for the incoming
//! byte plus, once past the window, a second lookup and a per-lane
//! rotate for the leaving byte. AVX2 has no per-byte gather narrower
//! than 32-bit indices, and the leaving-byte rotate uses a per-block
//! constant count anyway, so the scalar-side pack costs less than a
//! partially-emulated SIMD gather+rotate would.
//!
//! The SIMD kernel then reads the 64-entry buffer as sixteen unaligned
//! 4×`u64` loads via `_mm256_loadu_si256`, applies the per-lane
//! `[3, 2, 1, 0]` pre-rotate via `_mm256_sllv_epi64`,
//! `_mm256_srlv_epi64`, and `_mm256_or_si256`, advances the accumulator
//! with a 4-bit rotate via `_mm256_slli_epi64::<4>`,
//! `_mm256_srli_epi64::<60>`, and `_mm256_or_si256`, and folds with
//! `_mm256_xor_si256`. Horizontal XOR of the four lanes at the end
//! reduces to a single `u64` digest with `_mm256_extracti128_si256`
//! plus one `_mm_xor_si128` and two `_mm_extract_epi64` extractions.
//!
//! Using shift-count `0` on the identity lanes is deliberate: the
//! per-lane right-shift vector is `[61, 62, 63, 0]` (lane 3 rotates by
//! zero), which keeps every count in `_mm256_srlv_epi64`'s in-range
//! `0..=63` band and preserves the ROL identity by OR-ing lane 3's
//! left-shift-by-0 with its right-shift-by-0 (both equal to the input).
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time. The dispatcher checks
//! `is_x86_feature_detected!("avx2")` before every call.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::x86_64::{
    __m128i, __m256i, _mm_extract_epi64, _mm_xor_si128, _mm256_castsi256_si128,
    _mm256_extracti128_si256, _mm256_loadu_si256, _mm256_or_si256, _mm256_setr_epi64x,
    _mm256_setzero_si256, _mm256_slli_epi64, _mm256_sllv_epi64, _mm256_srli_epi64,
    _mm256_srlv_epi64, _mm256_xor_si256,
};

use super::scalar;
use crate::fingerprint::buzhash::BUZ_TABLE;

/// Buzhash block width used by the block-reformulation kernel — 64
/// bytes, the point at which the `state.rotate_left(k)` decay term
/// cycles back to `state` for a `u64` state.
const BLOCK_LEN: usize = 64;

/// AVX2-gated Buzhash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure AVX2 is available (see the module-level
/// safety note).
#[target_feature(enable = "avx2")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    if window == 0 {
        return 0;
    }

    let len = bytes.len();
    if len < BLOCK_LEN {
        // Below one block the reformulation gives no lift — the
        // `state.rotate_left(k)` term does not cycle back yet.
        return scalar::digest_of_slice(window, bytes);
    }

    // Rotating a `u64` by `k` bits is equivalent to rotating by
    // `k mod 64`; folding once at the top of the function keeps the
    // eviction rotate cheap even when `window > 64`. `window % 64` is
    // always in `0..64` and fits in a `u32`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "`window % 64` is in 0..64 and always fits in a u32"
    )]
    let window_rot = (window % 64) as u32;

    let mut state: u64 = 0;
    let full_blocks = len / BLOCK_LEN;

    for b in 0..full_blocks {
        let start = b * BLOCK_LEN;
        let contribs = compute_contribs(window, window_rot, bytes, start);
        // SAFETY: `contribs` is a valid stack-allocated `[u64; 64]`
        // whose 512 bytes are fully initialized above; AVX2 is upheld
        // by this function's target-feature context.
        let block_hash = unsafe { block_hash_avx2(&contribs) };
        state ^= block_hash;
    }

    // Scalar tail — length in `[0, BLOCK_LEN)` by construction. Uses
    // the same per-byte recurrence the block form was derived from, so
    // any partial trailing block continues bit-for-bit against the
    // scalar reference.
    let tail_start = full_blocks * BLOCK_LEN;
    for i in tail_start..len {
        let new_contrib = BUZ_TABLE[bytes[i] as usize];
        if i >= window {
            let leaving = bytes[i - window];
            let leaving_contrib = BUZ_TABLE[leaving as usize].rotate_left(window_rot);
            state = state.rotate_left(1) ^ leaving_contrib ^ new_contrib;
        } else {
            state = state.rotate_left(1) ^ new_contrib;
        }
    }

    state
}

/// Precomputes the per-index contribution values for one 64-byte block.
///
/// `contribs[i] = T[bytes[start+i]] ^ (start+i >= window ? T[bytes[start+i-window]].rotate_left(window_rot) : 0)`.
#[inline]
fn compute_contribs(
    window: usize,
    window_rot: u32,
    bytes: &[u8],
    start: usize,
) -> [u64; BLOCK_LEN] {
    let mut contribs = [0u64; BLOCK_LEN];
    for (i, slot) in contribs.iter_mut().enumerate() {
        let pos = start + i;
        let new_contrib = BUZ_TABLE[bytes[pos] as usize];
        if pos >= window {
            let leaving = bytes[pos - window];
            let leaving_contrib = BUZ_TABLE[leaving as usize].rotate_left(window_rot);
            *slot = new_contrib ^ leaving_contrib;
        } else {
            *slot = new_contrib;
        }
    }
    contribs
}

/// AVX2 kernel: compute the block hash of exactly 64 contributions.
///
/// Returns `Σ_{i=0..64} contribs[i].rotate_left(63 - i)` under XOR — the
/// closed form the Buzhash recurrence collapses to over a 64-byte
/// window when the leading `state_0.rotate_left(64)` term cycles back
/// to `state_0` (see the module docs).
///
/// # Safety
///
/// * AVX2 must be available at run time.
#[target_feature(enable = "avx2")]
unsafe fn block_hash_avx2(contribs: &[u64; BLOCK_LEN]) -> u64 {
    // SAFETY: this function is `#[target_feature(enable = "avx2")]`,
    // so every AVX2 intrinsic invoked below has its ISA precondition
    // upheld by the enclosing call context. The pointer arithmetic
    // `base.add(k)` reads 32 bytes starting at `contribs[4 * k]`;
    // `k * 4 + 4 <= 64` bounds the read within the 512-byte stack
    // buffer. `_mm256_loadu_si256` accepts any-alignment pointers by
    // contract.
    unsafe {
        // Per-lane pre-rotate: lane 0 by 3, lane 1 by 2, lane 2 by 1,
        // lane 3 by 0. `_mm256_setr_epi64x` packs its arguments in
        // ascending-lane order, so lane 0 receives the first argument.
        // The right-shift counterpart uses `[61, 62, 63, 0]` — lane 3's
        // rotate-by-zero is realised as `(x << 0) | (x >> 0) == x` so
        // no lane ever sees a shift count outside `_mm256_srlv_epi64`'s
        // in-range `0..=63` band. (`_mm256_srlv_epi64`'s spec makes
        // shift counts `>= 64` zero the lane, which is also correct,
        // but keeping every count in-range avoids the corner case.)
        let sl = _mm256_setr_epi64x(3, 2, 1, 0);
        let sr = _mm256_setr_epi64x(61, 62, 63, 0);

        let mut acc = _mm256_setzero_si256();
        // Every load below uses `_mm256_loadu_si256`, whose contract
        // accepts any alignment — the `*const __m256i` type is a
        // vocabulary cast, not a promise of 32-byte alignment. Clippy's
        // `cast_ptr_alignment` sees only the type and flags conservatively.
        #[allow(
            clippy::cast_ptr_alignment,
            reason = "loadu accepts any alignment; the __m256i cast is a vocabulary cast for the intrinsic signature"
        )]
        let base = contribs.as_ptr().cast::<__m256i>();

        // 16 iterations × 4 contribs = 64. Manual unrolling yields no
        // measurable benefit over the compiler's loop unrolling at
        // `-O2`/`-O3` and keeps the source auditable against the
        // derivation.
        for k in 0..16 {
            let v = _mm256_loadu_si256(base.add(k));

            // Per-lane rotate: (v << [3, 2, 1, 0]) | (v >> [61, 62, 63, 0]).
            let vl = _mm256_sllv_epi64(v, sl);
            let vr = _mm256_srlv_epi64(v, sr);
            let rotated = _mm256_or_si256(vl, vr);

            // Uniform 4-bit rotate for the Horner advance.
            let al = _mm256_slli_epi64::<4>(acc);
            let ar = _mm256_srli_epi64::<60>(acc);
            let advanced = _mm256_or_si256(al, ar);

            acc = _mm256_xor_si256(advanced, rotated);
        }

        // Horizontal XOR of the four `u64` lanes.
        let hi: __m128i = _mm256_extracti128_si256::<1>(acc);
        let lo: __m128i = _mm256_castsi256_si128(acc);
        let pair = _mm_xor_si128(lo, hi);

        // `_mm_extract_epi64` yields `i64`; reinterpret as `u64` — the
        // Buzhash state is a bit pattern where signed/unsigned is a
        // reinterpretation, not a value change.
        #[allow(
            clippy::cast_sign_loss,
            reason = "`_mm_extract_epi64` returns i64 by intrinsic signature; Buzhash state is a bit pattern where signed/unsigned is a re-interpretation, not a value change"
        )]
        let lane0 = _mm_extract_epi64::<0>(pair) as u64;
        #[allow(
            clippy::cast_sign_loss,
            reason = "`_mm_extract_epi64` returns i64 by intrinsic signature; Buzhash state is a bit pattern where signed/unsigned is a re-interpretation, not a value change"
        )]
        let lane1 = _mm_extract_epi64::<1>(pair) as u64;
        lane0 ^ lane1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::buzhash::Buzhash;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = Buzhash::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for &window in &[0usize, 1, 8, 32, 64, 100] {
            let cases: &[&[u8]] = &[
                b"",
                b"a",
                b"the quick brown fox jumps over the lazy dog",
                &[0u8; 128],
                &[0xFFu8; 200],
            ];
            for &input in cases {
                // SAFETY: is_x86_feature_detected!("avx2") returned true.
                let simd = unsafe { digest_of_slice(window, input) };
                assert_eq!(
                    simd,
                    reference(window, input),
                    "on {input:?} window {window}"
                );
            }
        }
    }

    #[test]
    fn matches_scalar_reference_at_word_bits_boundary() {
        // Explicitly test the block boundary — the whole point of the
        // reformulation. 63 is below one block (fully scalar). 64 is
        // exactly one block. 65 is one block plus a 1-byte tail. 127 /
        // 128 / 129 straddle the second-block boundary. Every window
        // that could put a full block into the windowed phase, and one
        // that keeps every block in the pre-window phase.
        if !is_x86_feature_detected!("avx2") {
            return;
        }
        for &window in &[1usize, 4, 8, 32, 64, 100, 200] {
            for &size in &[63usize, 64, 65, 127, 128, 129] {
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| {
                        #[allow(
                            clippy::cast_possible_truncation,
                            reason = "deterministic pseudo-random byte via low-bits truncation of a mixed u32"
                        )]
                        let m =
                            ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(1) >> 16) as u8;
                        m
                    })
                    .collect();
                // SAFETY: is_x86_feature_detected!("avx2") returned true.
                let simd = unsafe { digest_of_slice(window, &input) };
                assert_eq!(
                    simd,
                    reference(window, &input),
                    "at boundary size={size} window={window}"
                );
            }
        }
    }
}
