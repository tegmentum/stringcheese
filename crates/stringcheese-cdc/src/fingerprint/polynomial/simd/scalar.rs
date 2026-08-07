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
//! module can be audited without cross-file jumps and so a future
//! hand-written SIMD backend can replace the reduction inline without
//! reaching back into the streaming implementation's internals.

use crate::fingerprint::polynomial::{BASE, PRIME};

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

/// `x mod PRIME` via the Mersenne trick, matching the streaming impl.
#[inline]
fn reduce_mod(x: u128) -> u64 {
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
fn mul_mod(a: u64, b: u64) -> u64 {
    reduce_mod(u128::from(a) * u128::from(b))
}

/// `(a + b) mod PRIME`, valid for `a, b < PRIME`.
#[inline]
fn add_mod(a: u64, b: u64) -> u64 {
    let sum = a + b;
    if sum >= PRIME { sum - PRIME } else { sum }
}

/// `(a - b) mod PRIME`, valid for `a, b < PRIME`.
#[inline]
fn sub_mod(a: u64, b: u64) -> u64 {
    if a >= b { a - b } else { a + PRIME - b }
}

/// `base^exp mod PRIME` by iterated squaring.
fn pow_mod(base: u64, exp: u64) -> u64 {
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
