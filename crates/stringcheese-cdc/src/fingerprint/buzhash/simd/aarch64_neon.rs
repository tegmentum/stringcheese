//! NEON-gated Buzhash slice-batch backend for `aarch64`.
//!
//! Compiled only on `aarch64`. NEON is baseline for `aarch64`, so
//! this branch is always a valid target when the crate is built for
//! that architecture; the dispatcher still checks
//! `is_aarch64_feature_detected!("neon")` for uniformity with the x86
//! branches.
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
//! The block sum has a natural 2-lane SIMD shape when written as a
//! Horner recurrence over 32 groups of 2 bytes:
//!
//! ```text
//! acc = acc.rotate_left(2) ^ [ ROL(c[2k+0], 1), ROL(c[2k+1], 0) ]
//! ```
//!
//! After 32 iterations, XOR-reducing the two `u64` lanes reproduces
//! the block sum bit-for-bit. Below 64 bytes the kernel defers to the
//! scalar loop, and any `len % 64` tail after the last full block is
//! consumed by the same scalar recurrence — both share the reference
//! `state = state.rotate_left(1) ^ contrib_i` core that anchors the
//! differential contract.
//!
//! # Implementation
//!
//! Contributions are computed scalar-side into a 64-entry stack
//! buffer — NEON has no `u64` gather instruction and the per-block
//! leaving-byte rotate uses a constant count anyway, so the
//! scalar-side pack costs less than a partially-emulated SIMD gather
//! would. The SIMD kernel then reads the buffer as thirty-two
//! unaligned 2×`u64` loads (`vld1q_u64`), applies the per-lane
//! `[1, 0]` pre-rotate via `vshlq_u64` with signed count vectors
//! `[1, 0]` (left) and `[-63, 0]` (right, negative = logical right on
//! `vshlq_u64`), OR-combines them (`vorrq_u64`), advances the
//! accumulator with a constant 2-bit rotate (`vshlq_n_u64::<2>` +
//! `vshrq_n_u64::<62>` + `vorrq_u64`), and folds with `veorq_u64`.
//! Horizontal XOR of the two lanes at the end reduces to a single
//! `u64` digest with two `vgetq_lane_u64` extractions and a scalar
//! `^`.
//!
//! Keeping every right-shift count in `[0, 63]` avoids the
//! implementation-ambiguous `shift by ±64` corner of `vshlq_u64`: lane
//! 1's rotate-by-zero is realised as `(x << 0) | (x >> 0) == x`,
//! matching the identity rotate exactly.
//!
//! # Safety
//!
//! [`digest_of_slice`] is `unsafe fn` because `#[target_feature(enable = ...)]`
//! functions have a documented precondition — the enabled ISA feature
//! must be present at run time.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::aarch64::{
    int64x2_t, uint64x2_t, vdupq_n_s64, vdupq_n_u64, veorq_u64, vgetq_lane_u64, vld1q_u64,
    vorrq_u64, vsetq_lane_s64, vshlq_n_u64, vshlq_u64, vshrq_n_u64,
};

use super::scalar;
use crate::fingerprint::buzhash::BUZ_TABLE;

/// Buzhash block width used by the block-reformulation kernel — 64
/// bytes, the point at which the `state.rotate_left(k)` decay term
/// cycles back to `state` for a `u64` state.
const BLOCK_LEN: usize = 64;

/// NEON-gated Buzhash digest of a byte slice.
///
/// # Safety
///
/// The caller must ensure NEON is available. On `aarch64` NEON is
/// guaranteed by the ABI, but the dispatcher still checks
/// `is_aarch64_feature_detected!("neon")` to keep every dispatch
/// branch uniform.
#[target_feature(enable = "neon")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    if window == 0 {
        return 0;
    }

    let len = bytes.len();
    if len < BLOCK_LEN {
        return scalar::digest_of_slice(window, bytes);
    }

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
        // whose 512 bytes are fully initialized above; NEON is upheld
        // by this function's target-feature context.
        let block_hash = unsafe { block_hash_neon(&contribs) };
        state ^= block_hash;
    }

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

/// NEON kernel: compute the block hash of exactly 64 contributions.
///
/// Returns `Σ_{i=0..64} contribs[i].rotate_left(63 - i)` under XOR —
/// the closed form the Buzhash recurrence collapses to over a 64-byte
/// window when the leading `state_0.rotate_left(64)` term cycles back
/// to `state_0` (see the module docs).
///
/// # Safety
///
/// * NEON must be available at run time.
#[target_feature(enable = "neon")]
unsafe fn block_hash_neon(contribs: &[u64; BLOCK_LEN]) -> u64 {
    // SAFETY: this function is `#[target_feature(enable = "neon")]`,
    // so every NEON intrinsic invoked below has its ISA precondition
    // upheld by the enclosing call context. The pointer arithmetic
    // `base.add(k * 2)` reads 16 bytes starting at `contribs[k * 2]`;
    // `k * 2 + 2 <= 64` bounds the read within the 512-byte stack
    // buffer. `vld1q_u64` accepts any-alignment pointers by contract.
    unsafe {
        // Per-lane pre-rotate: lane 0 by 1, lane 1 by 0. `vshlq_u64`
        // takes a signed count per lane where positive means left,
        // negative means logical-right. The right-shift counterpart
        // uses `[-63, 0]` — lane 1's rotate-by-zero is realised as
        // `(x << 0) | (x >> 0) == x`, which keeps every count in
        // `vshlq_u64`'s unambiguous `[-63, 63]` band.
        let sl: int64x2_t = vsetq_lane_s64::<0>(1, vdupq_n_s64(0));
        let sr: int64x2_t = vsetq_lane_s64::<0>(-63, vdupq_n_s64(0));

        let mut acc: uint64x2_t = vdupq_n_u64(0);
        let base = contribs.as_ptr();

        // 32 iterations × 2 contribs = 64. Manual unrolling yields no
        // measurable benefit over the compiler's loop unrolling at
        // `-O2`/`-O3` and keeps the source auditable against the
        // derivation.
        for k in 0..32 {
            let pair: uint64x2_t = vld1q_u64(base.add(k * 2));

            // Per-lane rotate: (pair << [1, 0]) | (pair >> [63, 0]).
            let vl = vshlq_u64(pair, sl);
            let vr = vshlq_u64(pair, sr);
            let rotated = vorrq_u64(vl, vr);

            // Uniform 2-bit rotate for the Horner advance.
            let al = vshlq_n_u64::<2>(acc);
            let ar = vshrq_n_u64::<62>(acc);
            let advanced = vorrq_u64(al, ar);

            acc = veorq_u64(advanced, rotated);
        }

        // Horizontal XOR. `veorvq_u64` does not exist as a single
        // NEON reduce; two lane extractions plus a scalar `^` compile
        // to the same one-cycle sequence at `-O2` and keep the source
        // uniform with the wasm and x86 backends.
        let lane0 = vgetq_lane_u64::<0>(acc);
        let lane1 = vgetq_lane_u64::<1>(acc);
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
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
                // SAFETY: is_aarch64_feature_detected!("neon") returned true.
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
        if !std::arch::is_aarch64_feature_detected!("neon") {
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
                // SAFETY: is_aarch64_feature_detected!("neon") returned true.
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
