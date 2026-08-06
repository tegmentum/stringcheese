//! Polynomial rolling hash over the Mersenne-61 prime field.
//!
//! A textbook rolling hash. For a window `b_1 b_2 ... b_W`, the hash is
//!
//! ```text
//! H = ( sum_{i=1..W} b_i * BASE^(W - i) ) mod PRIME
//! ```
//!
//! Rolling a new byte in and the oldest byte out is a single subtraction,
//! shift, and addition — `O(1)` per byte after `BASE^W mod PRIME` is
//! precomputed at construction. This is not the same primitive as the
//! Rabin fingerprint (which operates over `GF(2)`); the two are documented
//! separately so the descriptor identifies which one produced any given
//! digest.
//!
//! # Parameter choice
//!
//! * `PRIME = 2^61 - 1` — the sixth Mersenne prime, chosen so that
//!   arithmetic modulo `PRIME` fits comfortably in 64-bit accumulators
//!   without a runtime carry check, and so that reduction can be
//!   implemented as an inline shift-and-add rather than a `%` operation.
//! * `BASE = 257` — a small prime larger than any single byte value, so
//!   that the map `byte -> byte + 0 * BASE + 0 * BASE^2 + ...` is
//!   injective on windows of length at most `⌊log_257(PRIME)⌋`.
//!
//! This is textbook material rather than a claim traceable to any single
//! paper; the [`DefinitionSource::IndependentlyDerived`] source records
//! that provenance honestly.
//!
//! # `alloc` requirement
//!
//! Like [`RabinFingerprint`][crate::fingerprint::rabin::RabinFingerprint],
//! this hash needs the leaving byte in order to roll, which requires a
//! circular buffer sized to the window. Gated on the `alloc` feature.
//!
//! [`DefinitionSource::IndependentlyDerived`]: comparand_core::DefinitionSource::IndependentlyDerived
//!
//! # References
//!
//! * Karp, R. M., & Rabin, M. O. (1987). "Efficient randomized
//!   pattern-matching algorithms." *IBM Journal of Research and
//!   Development*, 31(2), 249-260.
//!   <https://doi.org/10.1147/rd.312.0249> — the earliest formalization of
//!   the polynomial rolling-hash construction this module implements.

#![cfg(feature = "alloc")]

use alloc::{vec, vec::Vec};

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use super::RollingHash;

/// The Mersenne-61 prime, `2^61 - 1`, used as the modulus.
///
/// A Mersenne prime is chosen so that `x mod PRIME` can be implemented
/// with an inline shift-and-add — `x mod (2^61 - 1) = (x & PRIME) +
/// (x >> 61)`, followed by a subtraction if the result is still `>=
/// PRIME`. This is materially cheaper than a `%` operation on the hot
/// path.
pub const PRIME: u64 = (1u64 << 61) - 1;

/// The polynomial's base, a small prime larger than every single-byte
/// value.
pub const BASE: u64 = 257;

/// Polynomial rolling hash modulo the Mersenne-61 prime.
///
/// See the [module-level documentation][crate::fingerprint::polynomial]
/// for the parameter choice.
#[derive(Clone, Debug)]
pub struct PolynomialHash {
    /// The sliding window size in bytes.
    window: usize,
    /// Circular buffer of the bytes currently contributing to the window.
    buffer: Vec<u8>,
    /// Next slot to write in `buffer`.
    pos: usize,
    /// Total number of bytes fed since construction or `reset`.
    count: usize,
    /// The current hash value, always in the range `[0, PRIME)`.
    state: u64,
    /// Precomputed `BASE^window mod PRIME`, used to subtract the leaving
    /// byte's contribution in `O(1)`.
    base_pow_window: u64,
}

impl PolynomialHash {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::PolynomialRollingHash,
        variant: VariantId("mod-mersenne-61-base-257"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::IndependentlyDerived,
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Constructs a new polynomial hash with the given window size.
    #[must_use]
    pub fn with_window(window: usize) -> Self {
        let capacity = window.max(1);
        let buffer = vec![0u8; capacity];
        let base_pow_window = pow_mod(BASE, window as u64);
        Self {
            window,
            buffer,
            pos: 0,
            count: 0,
            state: 0,
            base_pow_window,
        }
    }
}

impl RollingHash for PolynomialHash {
    type Output = u64;

    fn new(window: usize) -> Self {
        Self::with_window(window)
    }

    fn roll(&mut self, byte: u8) {
        let leaving = self.buffer[self.pos];
        self.buffer[self.pos] = byte;
        self.pos = (self.pos + 1) % self.window.max(1);

        // Standard rolling formula:
        //   H_new = (H_old * BASE + new - leaving * BASE^window) mod PRIME.
        // Reordered to keep all intermediate values well under 2^64.

        // Add the new byte's contribution.
        let scaled = mul_mod(self.state, BASE);
        let scaled_plus_new = add_mod(scaled, u64::from(byte));

        // Subtract the leaving byte's contribution only once the window
        // has begun to overflow. Before that, no byte has fallen off.
        let updated = if self.count >= self.window && self.window > 0 {
            let leaving_contrib = mul_mod(u64::from(leaving), self.base_pow_window);
            sub_mod(scaled_plus_new, leaving_contrib)
        } else {
            scaled_plus_new
        };

        self.state = updated;
        self.count = self.count.saturating_add(1);
    }

    fn digest(&self) -> Self::Output {
        self.state
    }

    fn reset(&mut self) {
        for slot in &mut self.buffer {
            *slot = 0;
        }
        self.pos = 0;
        self.count = 0;
        self.state = 0;
    }
}

/// `x mod PRIME` via the Mersenne trick `(x & PRIME) + (x >> 61)`, with a
/// final subtraction to canonicalise into `[0, PRIME)`.
#[inline]
fn reduce_mod(x: u128) -> u64 {
    // Split into 61-bit halves; each iteration reduces the magnitude by
    // 2^61. Two iterations suffice for any `u128` that arose from a
    // multiplication of two values in `[0, PRIME)`.
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

/// Computes `base^exp mod PRIME` by iterated squaring.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_family_variant_and_parameters() {
        let d = PolynomialHash::descriptor();
        assert_eq!(d.family, AlgorithmFamily::PolynomialRollingHash);
        assert_eq!(d.variant, VariantId("mod-mersenne-61-base-257"));
    }

    #[test]
    fn empty_hash_is_zero() {
        let h = PolynomialHash::new(4);
        assert_eq!(h.digest(), 0);
    }

    #[test]
    fn single_byte_digest_is_the_byte_value() {
        let mut h = PolynomialHash::new(4);
        h.roll(0xAB);
        // With state=0: H = (0 * BASE + 0xAB) mod PRIME = 0xAB.
        assert_eq!(h.digest(), 0xAB);
    }

    #[test]
    fn reset_returns_to_empty_state() {
        let mut h = PolynomialHash::new(4);
        for &b in b"hello, world" {
            h.roll(b);
        }
        assert_ne!(h.digest(), 0);
        h.reset();
        assert_eq!(h.digest(), 0);
    }

    #[test]
    fn rolling_matches_fresh_hash_after_window_slides() {
        let total = b"the quick brown fox jumps over the lazy dog";
        let window = 8usize;

        let mut rolling = PolynomialHash::new(window);
        for &b in total {
            rolling.roll(b);
        }

        let mut fresh = PolynomialHash::new(window);
        for &b in &total[total.len() - window..] {
            fresh.roll(b);
        }

        assert_eq!(rolling.digest(), fresh.digest());
    }

    #[test]
    fn pow_mod_matches_naive() {
        // Cross-check against a simple linear implementation.
        for e in 0..12u64 {
            let mut naive: u64 = 1;
            for _ in 0..e {
                naive = mul_mod(naive, BASE);
            }
            assert_eq!(pow_mod(BASE, e), naive, "pow_mod({BASE}, {e})");
        }
    }

    #[test]
    fn add_sub_mul_stay_in_range() {
        for a in [0u64, 1, 5, PRIME - 1] {
            for b in [0u64, 1, 5, PRIME - 1] {
                assert!(add_mod(a, b) < PRIME, "add_mod({a}, {b})");
                assert!(sub_mod(a, b) < PRIME, "sub_mod({a}, {b})");
                assert!(mul_mod(a, b) < PRIME, "mul_mod({a}, {b})");
            }
        }
    }
}
