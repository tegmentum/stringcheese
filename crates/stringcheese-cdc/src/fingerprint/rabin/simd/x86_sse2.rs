//! SSE2-gated Rabin-fingerprint slice-batch backend for `x86_64`.
//!
//! Compiled only on `x86_64`. This module ships two internal kernels
//! selected by a runtime CPU-feature check:
//!
//! * When `pclmulqdq` is available it dispatches to the
//!   `digest_of_slice_pclmul` kernel — a real vectorized kernel built
//!   on 8-byte block folding through `_mm_clmulepi64_si128`.
//! * Otherwise it falls back to the portable scalar core under the
//!   `sse2` target-feature context.
//!
//! # Kernel shape — 8-byte block folding via `pclmulqdq`
//!
//! Rabin's per-byte rolling recurrence
//!
//! ```text
//! state = (state << 8) ^ byte ^ SHIFT_TABLE[state >> 56]
//! ```
//!
//! unrolls, over any k-byte run starting from `state_0`, into the closed
//! form
//!
//! ```text
//! state_k  =  (state_0 * x^{8k}  +  Σ_{i=0..k}  b_i * x^{8(k-1-i)})  mod P
//! ```
//!
//! where `P(x) = x^64 + LOW_P` with `LOW_P = 0x1B`. Unlike Gear the
//! leading `state_0 * x^{8k}` term does *not* wipe to zero at `k = 8`
//! (the reduction leaves a non-trivial contribution modulo `P`), but the
//! full-window contract lets us truncate: for `bytes.len() > window`
//! the streaming rolling hash's digest depends only on the last `window`
//! bytes, and can be reproduced by feeding just that tail into a fresh
//! `state = 0`. Once the effective slice is chosen this way, the
//! zero-initialised block form gives exact per-8-byte folding:
//!
//! ```text
//! state_{k+8} = state_k * x^64  +  u64_be(bytes[k..k+8])  (mod P)
//! ```
//!
//! and the reduction `state_k * x^64 mod P` collapses cleanly to a
//! single `pclmulqdq`:
//!
//! * `clmul(state_k, LOW_P)` produces a 128-bit result whose low 64 bits
//!   are `state * LOW_P` modulo `x^64` and whose high bits are the
//!   at-most-4-bit polynomial `(state * LOW_P) >> 64`.
//! * A second reduction of those 4 high bits is exactly the byte-indexed
//!   `SHIFT_TABLE[high] = h * LOW_P` — the same table the streaming
//!   scalar path already builds, reused here without allocation because
//!   `high` always fits in eight bits (in fact in four).
//!
//! Below one 8-byte block the reformulation offers no lift, and any
//! `effective_len % 8` tail after the last full block is consumed by
//! the same scalar recurrence — both paths share the byte-identical
//! contract with the streaming reference.
//!
//! # Safety
//!
//! Every `unsafe fn` here is `#[target_feature(...)]`-declared. The
//! `sse2`-only entry point is safe once the dispatcher confirms SSE2
//! (guaranteed by the `x86_64` ABI); the `pclmulqdq` inner kernel adds a
//! `#[target_feature(enable = "sse2,pclmulqdq")]` and is only called
//! after `is_x86_feature_detected!("pclmulqdq")` returns true. See the
//! [module docs][super] for the shared `unsafe` policy.

#![allow(
    unsafe_code,
    reason = "`#[target_feature]` functions are unsafe by declaration; this module is one of the four documented SIMD exceptions listed in the crate root."
)]

use core::arch::x86_64::{
    _mm_clmulepi64_si128, _mm_cvtsi64_si128, _mm_cvtsi128_si64, _mm_extract_epi64,
};

use super::scalar;

/// Block size for the block-folding kernel — 8 bytes is the natural
/// unit for a `u64`-wide state (an 8-byte block-hash polynomial has
/// degree 63 and fits exactly).
const BLOCK: usize = 8;

/// SSE2-gated Rabin-fingerprint digest of a byte slice.
///
/// Runtime-detects `pclmulqdq` on the first line: when present, routes
/// to the real vectorized 8-byte-block folding kernel; otherwise falls
/// back to the portable scalar core inside this `#[target_feature]`
/// context, letting the compiler generate SSE2-flavoured code.
///
/// # Safety
///
/// The caller must ensure SSE2 is available. On `x86_64` SSE2 is
/// guaranteed by the ABI; the dispatcher still checks
/// `is_x86_feature_detected!("sse2")` for consistency with the other
/// arch branches.
#[target_feature(enable = "sse2")]
#[must_use]
pub unsafe fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    if is_x86_feature_detected!("pclmulqdq") {
        // SAFETY: is_x86_feature_detected!("pclmulqdq") just returned
        // true, so the `pclmulqdq` target-feature precondition of
        // `digest_of_slice_pclmul` is upheld. SSE2 is upheld by this
        // function's own target-feature context.
        return unsafe { digest_of_slice_pclmul(window, bytes) };
    }
    scalar::digest_of_slice(window, bytes)
}

/// PCLMULQDQ-gated Rabin-fingerprint digest of a byte slice.
///
/// Implements the 8-byte block-folding kernel documented at the module
/// level. The public [`digest_of_slice`] entry point already runtime-
/// detects `pclmulqdq` and forwards here; the kernel is also exposed
/// as a `pub(super)` symbol so the [AVX2 sibling][super::x86_avx2] can
/// delegate to the same fold when its own VPCLMULQDQ path is not
/// available.
///
/// # Safety
///
/// * SSE2 and PCLMULQDQ must both be available at run time. Both are
///   enabled via `#[target_feature]` on this function; the caller must
///   have confirmed `pclmulqdq` availability before invocation.
#[target_feature(enable = "sse2,pclmulqdq")]
#[must_use]
pub(super) unsafe fn digest_of_slice_pclmul(window: usize, bytes: &[u8]) -> u64 {
    // Determine the effective slice: only the last `window` bytes
    // affect the streaming Rabin digest (see `RabinFingerprint`'s
    // roll-out cancellation). Window `0` is degenerate — no eviction
    // ever fires, so every byte contributes; keep the full slice.
    let effective_start = if window == 0 {
        0
    } else {
        bytes.len().saturating_sub(window)
    };
    let effective = &bytes[effective_start..];
    let len = effective.len();

    if len < BLOCK {
        // No block-folding is possible; fall through to the scalar
        // recurrence directly (still inside the target-feature context
        // for consistent codegen).
        return scalar_from_zero(effective);
    }

    let full_blocks = len / BLOCK;
    let mut state: u64 = 0;

    // SAFETY: this function is `#[target_feature(enable = "sse2,pclmulqdq")]`,
    // so every intrinsic invoked below has its ISA precondition upheld
    // by the enclosing call context. Each `chunk_ptr.cast::<[u8; 8]>().read_unaligned()`
    // reads a 8-byte block whose start offset satisfies
    // `b * BLOCK + BLOCK <= full_blocks * BLOCK <= len`, so the read
    // stays within `effective`.
    unsafe {
        #[allow(
            clippy::cast_possible_wrap,
            reason = "`_mm_cvtsi64_si128` takes an i64 by intrinsic signature; the reinterpretation is a bit-pattern change with no arithmetic meaning — the polynomial coefficients are unsigned in `GF(2)`"
        )]
        let low_p_v = _mm_cvtsi64_si128(super::super::LOW_P as i64);
        let base_ptr = effective.as_ptr();
        for b in 0..full_blocks {
            let chunk_ptr = base_ptr.add(b * BLOCK).cast::<[u8; 8]>();
            let chunk = u64::from_be_bytes(chunk_ptr.read_unaligned());

            // `state * x^64 mod P`: since `x^64 ≡ LOW_P (mod P)`,
            // this is `state * LOW_P mod P`. The 128-bit clmul result
            // splits into a low 64-bit polynomial and a high sub-4-bit
            // polynomial; a second `high * LOW_P` fold (via the same
            // SHIFT_TABLE the streaming path already builds) collapses
            // the high bits back into 64.
            #[allow(
                clippy::cast_possible_wrap,
                reason = "`_mm_cvtsi64_si128` takes an i64 by intrinsic signature; the reinterpretation is a bit-pattern change with no arithmetic meaning — the polynomial coefficients are unsigned in `GF(2)`"
            )]
            let state_v = _mm_cvtsi64_si128(state as i64);
            let product = _mm_clmulepi64_si128::<0x00>(state_v, low_p_v);

            #[allow(
                clippy::cast_sign_loss,
                reason = "`_mm_cvtsi128_si64` returns i64 by intrinsic signature; the polynomial coefficients are a bit-pattern where signed/unsigned is a re-interpretation, not a value change"
            )]
            let low64 = _mm_cvtsi128_si64(product) as u64;
            #[allow(
                clippy::cast_sign_loss,
                reason = "`_mm_extract_epi64` returns i64 by intrinsic signature; the polynomial coefficients are a bit-pattern where signed/unsigned is a re-interpretation, not a value change"
            )]
            let high64 = _mm_extract_epi64::<1>(product) as u64;

            // high64 has degree at most 3 (state * LOW_P has degree at
            // most 67, so bits 64..67 populate high64); `SHIFT_TABLE`
            // is indexed by a byte and encodes `h * LOW_P` in GF(2)
            // for `h` up to eight bits — exactly the second-fold table
            // we need. `usize::try_from` cannot fail on the 64-bit
            // targets this backend compiles for (high64 is bounded by
            // 0..=15); use it anyway to satisfy the conservative
            // clippy `cast_possible_truncation` lint on hypothetical
            // 32-bit builds.
            let idx = usize::try_from(high64).expect("high64 fits in 4 bits");
            state = low64 ^ super::super::SHIFT_TABLE[idx] ^ chunk;
        }
    }

    // Scalar tail: length in `[0, BLOCK)` by construction. Reuse the
    // streaming recurrence — same table, same reduction contract.
    let tail_start = full_blocks * BLOCK;
    for &b in &effective[tail_start..] {
        let high = (state >> 56) as usize;
        state = (state << 8) ^ u64::from(b) ^ super::super::SHIFT_TABLE[high];
    }

    state
}

/// Runs the scalar Rabin recurrence over `bytes` from `state = 0`, no
/// window bookkeeping. Used by [`digest_of_slice_pclmul`] on inputs
/// shorter than one block, where the block-folding kernel has nothing
/// to fold.
///
/// This does not go through `RabinFingerprint` because the effective-
/// slice truncation upstream has already collapsed the streaming
/// eviction into a fresh state; the roll-out table is not needed.
#[inline]
fn scalar_from_zero(bytes: &[u8]) -> u64 {
    let mut state: u64 = 0;
    for &b in bytes {
        let high = (state >> 56) as usize;
        state = (state << 8) ^ u64::from(b) ^ super::super::SHIFT_TABLE[high];
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::rabin::RabinFingerprint;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = RabinFingerprint::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn matches_scalar_reference_on_diverse_inputs() {
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for &window in &[1usize, 8, 32, 64, 100] {
            let cases: &[&[u8]] = &[
                b"",
                b"a",
                b"the quick brown fox jumps over the lazy dog",
                &[0u8; 128],
                &[0xFFu8; 200],
            ];
            for &input in cases {
                // SAFETY: is_x86_feature_detected!("sse2") returned true.
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
    fn matches_scalar_reference_at_block_boundaries() {
        // Explicitly test the boundary between the pclmul block path
        // and the scalar tail — the whole point of the reformulation.
        // 7 is below the block threshold (fully scalar). 8 is exactly
        // one block, no tail. 9 is one block plus a 1-byte tail. 63 is
        // seven blocks plus a 7-byte tail. 64 is eight blocks, no tail.
        // 65 is eight blocks plus a 1-byte tail.
        if !is_x86_feature_detected!("sse2") {
            return;
        }
        for &window in &[8usize, 64, 128, 512] {
            for &size in &[7usize, 8, 9, 15, 16, 17, 63, 64, 65, 127, 128, 129] {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "deterministic pseudo-random byte via low-bits truncation of a mixed u32"
                )]
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(1) >> 16) as u8)
                    .collect();
                // SAFETY: is_x86_feature_detected!("sse2") returned true.
                let simd = unsafe { digest_of_slice(window, &input) };
                assert_eq!(
                    simd,
                    reference(window, &input),
                    "at boundary size={size} window={window}"
                );
            }
        }
    }

    #[test]
    fn pclmul_kernel_matches_scalar_when_available() {
        // Directly hit the inner pclmul kernel — the entry-point test
        // above dispatches on runtime detection and may route to
        // scalar on hosts without pclmulqdq; this test asserts the
        // kernel itself is bit-identical on every host that can run it.
        if !is_x86_feature_detected!("pclmulqdq") {
            return;
        }
        for &window in &[1usize, 8, 32, 64, 100, 1024] {
            for &size in &[0usize, 1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 4096] {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "deterministic pseudo-random byte via low-bits truncation of a mixed u64"
                )]
                let input: alloc::vec::Vec<u8> = (0..size)
                    .map(|i| ((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 24) as u8)
                    .collect();
                // SAFETY: is_x86_feature_detected!("pclmulqdq") returned true.
                let simd = unsafe { digest_of_slice_pclmul(window, &input) };
                assert_eq!(
                    simd,
                    reference(window, &input),
                    "pclmul kernel diverged at size={size} window={window}"
                );
            }
        }
    }
}
