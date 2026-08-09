//! Portable scalar polynomial-hash slice-batch reference.
//!
//! Always compiled — the byte-identical correctness anchor every
//! arch-specific backend is differentially tested against. See the
//! [module docs][super] for the SIMD-tree contract.
//!
//! The Mersenne-61 modular arithmetic used here is bit-identical to
//! that in the streaming [`PolynomialHash`][crate::fingerprint::polynomial::PolynomialHash]
//! implementation — the small helpers (`reduce_mod`, `mul_mod`,
//! `add_mod`, `sub_mod`, `pow_mod`) are duplicated verbatim so this
//! module can be audited without cross-file jumps and so the
//! hand-written SIMD backends can share the reduction primitive
//! without reaching back into the streaming implementation's
//! internals.
//!
//! # Block form shared with the SIMD backends
//!
//! The arch-specific backends collapse the per-byte recurrence into a
//! `BLOCK_LEN`-byte block form:
//!
//! ```text
//! state_{k+BLOCK} = state_k * BASE^BLOCK
//!                 + Σ_{i=0..BLOCK}  bytes[k+i] * BASE^(BLOCK-1-i)  (mod PRIME)
//! ```
//!
//! The trailing sum is a small polynomial evaluation over one block —
//! independent of the running state and therefore vectorizable in a
//! per-lane multiply-accumulate. Each coefficient `pk[j] = BASE^j mod
//! PRIME` fits in the 61-bit Mersenne field; splitting it into
//! `pk_hi = pk[j] >> 32` (≤ 29 bits) and `pk_lo = pk[j] & 0xFFFF_FFFF`
//! (32 bits) lets a byte × coefficient product be expressed as two
//! independent 32×32 → 64 multiplies, both fitting in a u64 lane.
//! Accumulating those partial products across a `BLOCK_LEN = 16`
//! block leaves `hi_sum ≤ ~2^41` and `lo_sum ≤ ~2^44`, well under the
//! 64-bit lane bound and combined into a `u128` for the final
//! Mersenne reduction. The precomputed `COEFF_HI`, `COEFF_LO`, and
//! `PK_BLOCK` tables here carry that block-form constants for every
//! backend — they are `const`-evaluated so no runtime pow-mod runs on
//! the hot path.

// The block-form helpers below (BLOCK_LEN, scalar_from_zero, PK,
// PK_BLOCK, COEFF_HI, COEFF_LO, and the const-fn arithmetic they use)
// are consumed only by the arch-specific SIMD backends. On targets
// where no SIMD backend is compiled (wasm32 without simd128), they
// become dead code. Silencing the lint at module scope avoids
// scattering the same `#[cfg]` guard across eight items.
#![allow(
    dead_code,
    reason = "SIMD block-form helpers used only when an arch backend is compiled"
)]

use crate::fingerprint::polynomial::{BASE, PRIME};

/// Block size for the block-form kernel — 16 bytes per block. Picked
/// so that a 4-lane SIMD kernel (AVX2) issues four widening 32×32→64
/// multiply-accumulate rounds per block and a 2-lane kernel
/// (SSE2/NEON/wasm SIMD128) issues eight rounds, matching the natural
/// unroll factor of each ISA.
pub(super) const BLOCK_LEN: usize = 16;

/// Portable single-`u64` polynomial-hash digest of a byte slice.
///
/// Byte-for-byte identical to
///
/// ```ignore
/// let mut h = PolynomialHash::new(window);
/// for &b in bytes { h.roll(b); }
/// h.digest()
/// ```
///
/// The eviction byte at position `i` (once the window has begun to
/// overflow) is read from `bytes[i - window]` directly, matching the
/// scalar implementation's circular-buffer semantics without allocating
/// one here.
#[inline]
#[must_use]
pub fn digest_of_slice(window: usize, bytes: &[u8]) -> u64 {
    // Precompute `BASE^window mod PRIME` once; the streaming impl does
    // the same in its constructor. `pow_mod` is `O(log window)`, hoisted
    // out of the inner loop.
    let base_pow_window = pow_mod(BASE, window as u64);

    let mut state: u64 = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        let scaled = mul_mod(state, BASE);
        let scaled_plus_new = add_mod(scaled, u64::from(byte));
        state = if i >= window && window > 0 {
            let leaving = bytes[i - window];
            let leaving_contrib = mul_mod(u64::from(leaving), base_pow_window);
            sub_mod(scaled_plus_new, leaving_contrib)
        } else {
            scaled_plus_new
        };
    }
    state
}

/// Polynomial-hash of a byte slice starting from `state = 0`, with no
/// window bookkeeping. Used by the arch SIMD backends after the
/// effective-slice truncation collapses the streaming eviction into a
/// fresh state, and by the scalar tail after the block-form kernel.
#[inline]
pub(super) fn scalar_from_zero(bytes: &[u8]) -> u64 {
    let mut state: u64 = 0;
    for &b in bytes {
        state = add_mod(mul_mod(state, BASE), u64::from(b));
    }
    state
}

/// `x mod PRIME` via the Mersenne trick `(x & PRIME) + (x >> 61)`, with
/// a final subtraction to canonicalise into `[0, PRIME)`.
#[inline]
pub(super) fn reduce_mod(x: u128) -> u64 {
    let mut y = (x & u128::from(PRIME)) + (x >> 61);
    y = (y & u128::from(PRIME)) + (y >> 61);
    let mut y64 = u64::try_from(y).expect("two-step Mersenne reduction fits in u64");
    if y64 >= PRIME {
        y64 -= PRIME;
    }
    y64
}

/// `(a * b) mod PRIME`.
#[inline]
pub(super) fn mul_mod(a: u64, b: u64) -> u64 {
    reduce_mod(u128::from(a) * u128::from(b))
}

/// `(a + b) mod PRIME`, valid for `a, b < PRIME`.
#[inline]
pub(super) fn add_mod(a: u64, b: u64) -> u64 {
    let sum = a + b;
    if sum >= PRIME { sum - PRIME } else { sum }
}

/// `(a - b) mod PRIME`, valid for `a, b < PRIME`.
#[inline]
pub(super) fn sub_mod(a: u64, b: u64) -> u64 {
    if a >= b { a - b } else { a + PRIME - b }
}

/// `base^exp mod PRIME` by iterated squaring.
pub(super) fn pow_mod(base: u64, exp: u64) -> u64 {
    let mut result: u64 = 1;
    let mut base = base % PRIME;
    let mut exp = exp;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, base);
        }
        base = mul_mod(base, base);
        exp >>= 1;
    }
    result
}

// ---------------------------------------------------------------------
// Block-form constants — const-evaluated so the SIMD backends carry no
// runtime pow-mod on the hot path.
// ---------------------------------------------------------------------

/// `const fn` Mersenne reduction, bit-identical to [`reduce_mod`].
///
/// Duplicated because [`reduce_mod`] uses `u64::try_from` (not `const
/// fn` on stable) and the trait bounds needed for a `const fn` version
/// would obscure the arithmetic.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the two-step Mersenne reduction bounds `y` in [0, 2 * PRIME), which fits in u64 — the truncation is arithmetically defined"
)]
const fn reduce_mod_const(x: u128) -> u64 {
    let mut y = (x & PRIME as u128) + (x >> 61);
    y = (y & PRIME as u128) + (y >> 61);
    let mut y64 = y as u64;
    if y64 >= PRIME {
        y64 -= PRIME;
    }
    y64
}

/// `const fn` `(a * b) mod PRIME`, bit-identical to [`mul_mod`].
const fn mul_mod_const(a: u64, b: u64) -> u64 {
    reduce_mod_const((a as u128) * (b as u128))
}

/// Powers of `BASE` mod `PRIME`, indexed by exponent in `0..=BLOCK_LEN`.
/// `PK[k] = BASE^k mod PRIME`.
const PK: [u64; BLOCK_LEN + 1] = {
    let mut arr = [0u64; BLOCK_LEN + 1];
    arr[0] = 1;
    let mut i = 1;
    while i <= BLOCK_LEN {
        arr[i] = mul_mod_const(arr[i - 1], BASE);
        i += 1;
    }
    arr
};

/// `BASE^BLOCK_LEN mod PRIME`, the per-block scale applied to the
/// running state each time a full block folds in.
pub(super) const PK_BLOCK: u64 = PK[BLOCK_LEN];

/// High 32 bits of `pk[BLOCK_LEN - 1 - i]` for `i` in `0..BLOCK_LEN`.
///
/// `COEFF_HI[i]` is the high 32 bits of the coefficient that multiplies
/// byte `i` of a block. Values are ≤ `2^29 - 1` because
/// `PK[j] < PRIME = 2^61 - 1` — so `PK[j] >> 32 < 2^29`. Storing as
/// `u64` (with the high 32 bits zero) lets the backends drop them
/// straight into 64-bit lanes for the 32×32 → 64 SIMD multiplies.
pub(super) const COEFF_HI: [u64; BLOCK_LEN] = {
    let mut arr = [0u64; BLOCK_LEN];
    let mut i = 0;
    while i < BLOCK_LEN {
        arr[i] = PK[BLOCK_LEN - 1 - i] >> 32;
        i += 1;
    }
    arr
};

/// Low 32 bits of `pk[BLOCK_LEN - 1 - i]` for `i` in `0..BLOCK_LEN`.
///
/// See [`COEFF_HI`]; the two tables split the coefficient into the
/// halves the SIMD kernels multiply independently, then recombine as
/// `hi_sum * 2^32 + lo_sum` before the Mersenne reduction.
pub(super) const COEFF_LO: [u64; BLOCK_LEN] = {
    let mut arr = [0u64; BLOCK_LEN];
    let mut i = 0;
    while i < BLOCK_LEN {
        arr[i] = PK[BLOCK_LEN - 1 - i] & 0xFFFF_FFFF;
        i += 1;
    }
    arr
};

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test inputs use `(i as u8)` patterns to derive deterministic pseudo-random bytes from small counters; sizes are bounded well below `u32::MAX`, so truncation cannot occur"
)]
mod tests {
    use super::*;
    use crate::fingerprint::RollingHash;
    use crate::fingerprint::polynomial::PolynomialHash;

    fn reference(window: usize, bytes: &[u8]) -> u64 {
        let mut h = PolynomialHash::new(window);
        for &b in bytes {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn block_form_constants_are_consistent() {
        // Sanity-check the const-evaluated tables against runtime
        // `pow_mod` — a divergence would silently corrupt every SIMD
        // backend's block sum.
        for (k, &pk_k) in PK.iter().enumerate() {
            assert_eq!(pk_k, pow_mod(BASE, k as u64), "PK[{k}]");
        }
        for (i, (&hi, &lo)) in COEFF_HI.iter().zip(COEFF_LO.iter()).enumerate() {
            let pk = PK[BLOCK_LEN - 1 - i];
            assert_eq!(hi, pk >> 32, "COEFF_HI[{i}]");
            assert_eq!(lo, pk & 0xFFFF_FFFF, "COEFF_LO[{i}]");
        }
        assert_eq!(PK_BLOCK, pow_mod(BASE, BLOCK_LEN as u64));
    }

    #[test]
    fn scalar_from_zero_matches_reference_when_input_is_the_window() {
        // The effective-slice truncation the SIMD backends perform
        // reduces to `scalar_from_zero(last window bytes)`. Anchor that
        // reduction against the streaming reference directly.
        for &window in &[1usize, 4, 16, 32, 64] {
            let total: alloc::vec::Vec<u8> = (0..(window * 3))
                .map(|i| (i as u8).wrapping_mul(17))
                .collect();
            let last_window = &total[total.len() - window..];
            assert_eq!(
                scalar_from_zero(last_window),
                reference(window, &total),
                "window={window}"
            );
        }
    }
}
