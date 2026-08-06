//! Rabin polynomial fingerprint over `GF(2)`.
//!
//! The classical Rabin fingerprint, introduced in Rabin's 1981 technical
//! report *Fingerprinting by random polynomials*. An input byte stream is
//! interpreted as a polynomial over the field `GF(2)`; the fingerprint is
//! that polynomial reduced modulo a fixed irreducible polynomial `P(x)` of
//! degree 64. Two windows collide only when their reductions agree — a
//! property that, for random inputs and a randomly chosen `P`, gives
//! provably small collision probability.
//!
//! # The polynomial
//!
//! This implementation uses the degree-64 primitive polynomial
//!
//! ```text
//! P(x) = x^64 + x^4 + x^3 + x + 1
//! ```
//!
//! chosen from Koopman's polynomial database (variant slug
//! `"gf2-poly-koopman-64-1b"`). Its low 64 coefficients are `0x1B`; the
//! degree-64 term is implicit in the reduction. This polynomial has two
//! practical virtues:
//!
//! * Every reduction table entry fits in a `u64` without a second-level
//!   reduction — the low bits of `P` have degree 4, so `(byte * x^64) mod P`
//!   has degree at most 11.
//! * It is documented in a stable, third-party reference so a golden case
//!   tied to this variant cannot silently be re-run against a differently
//!   parametrized Rabin implementation.
//!
//! # Rolling
//!
//! The rolling update is `O(1)` per byte, backed by two precomputed
//! 256-entry tables:
//!
//! * `SHIFT_TABLE[h] = (h * x^64) mod P` — corrects for the 8 high bits
//!   that fall off `state << 8`.
//! * `roll_out_table[b] = (b * x^(8*W)) mod P` — the contribution of a
//!   byte `b` that has been in the window for exactly `W` steps and is now
//!   being evicted. This table depends on the window size and is
//!   constructed lazily when the fingerprint is instantiated.
//!
//! A byte's contribution to the state grows as it is rolled through the
//! window; when the window overflows, the leaving byte's contribution is
//! removed with a single XOR against `roll_out_table`.
//!
//! # `alloc` requirement
//!
//! The rolling formulation needs to know the byte that is being evicted,
//! which in turn requires a circular buffer sized to the window. On a
//! no-alloc build there is no way to size that buffer at run time, so the
//! type is available only with the `alloc` feature enabled.

#![cfg(feature = "alloc")]

use alloc::{vec, vec::Vec};

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use super::RollingHash;

/// The low 64 coefficients of the fixed reduction polynomial
/// `P(x) = x^64 + x^4 + x^3 + x + 1`.
///
/// Because `x^64 ≡ LOW_P (mod P)`, this is precisely the reduction that
/// must be `XOR`ed into the state whenever the state's degree would exceed
/// 63.
const LOW_P: u64 = 0x1B;

/// Precomputed `SHIFT_TABLE[h] = (h * x^64) mod P` for the fixed polynomial
/// `P(x) = x^64 + x^4 + x^3 + x + 1`.
///
/// Because `x^64 ≡ 0x1B (mod P)` and `h` fits in eight bits, the product
/// `h * 0x1B` is at most 12 bits and never requires a second-level
/// reduction. The table entries below are simply `h * 0x1B` in the
/// `GF(2)` (i.e. carry-free) sense.
const SHIFT_TABLE: [u64; 256] = build_shift_table();

/// Rabin polynomial fingerprint over the fixed polynomial
/// `P(x) = x^64 + x^4 + x^3 + x + 1`.
///
/// The fingerprint of a window of bytes `b_1 b_2 ... b_W` is the reduction
/// modulo `P(x)` of the polynomial `sum_i b_i * x^(8 * (W - i))`, where
/// each byte is interpreted as an 8-bit polynomial.
///
/// See the [module-level documentation][crate::fingerprint::rabin] for the
/// choice of polynomial and the rolling scheme.
#[derive(Clone, Debug)]
pub struct RabinFingerprint {
    /// Size of the sliding window in bytes.
    window: usize,
    /// Circular buffer of the bytes currently contributing to the window.
    /// Only used to reconstruct the leaving byte on eviction.
    buffer: Vec<u8>,
    /// Next slot to write in `buffer`; also the slot that will be
    /// overwritten on the next `roll` once the window is full.
    pos: usize,
    /// Total number of bytes fed since construction or `reset`. Used to
    /// decide when the window has begun to overflow.
    count: usize,
    /// Current polynomial state, reduced modulo `P(x)`.
    state: u64,
    /// `roll_out_table[b] = (b * x^(8 * window)) mod P` — the contribution
    /// of a byte that has been in the window for exactly `window` steps
    /// and is now being evicted.
    roll_out_table: Vec<u64>,
}

impl RabinFingerprint {
    /// The algorithm descriptor for this Rabin variant.
    ///
    /// The variant slug pins the specific polynomial in use, so a golden
    /// case for `"gf2-poly-koopman-64-1b"` cannot silently be run against
    /// a differently-parametrized Rabin implementation.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::RabinFingerprint,
        variant: VariantId("gf2-poly-koopman-64-1b"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Fingerprinting by random polynomials",
            authors: "M. O. Rabin",
            year: 1981,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Constructs a new Rabin fingerprint with the given window size.
    ///
    /// The precomputation of the roll-out table is `O(window)`; subsequent
    /// `roll` calls are `O(1)`.
    #[must_use]
    pub fn with_window(window: usize) -> Self {
        // The circular buffer must have capacity at least 1 to avoid a
        // divide-by-zero on `pos = (pos + 1) % window` even when `window`
        // is nominally zero. A zero window is a legal degenerate config —
        // in that case the digest is always the identity (0), and no
        // buffer accounting is ever performed.
        let capacity = window.max(1);
        // `vec![0u8; capacity]` zeroes every slot; the explicit
        // `pos = 0` below starts rolling from index zero.
        let buffer = vec![0u8; capacity];

        let roll_out_table = build_roll_out_table(window);

        Self {
            window,
            buffer,
            pos: 0,
            count: 0,
            state: 0,
            roll_out_table,
        }
    }
}

impl RollingHash for RabinFingerprint {
    type Output = u64;

    fn new(window: usize) -> Self {
        Self::with_window(window)
    }

    fn roll(&mut self, byte: u8) {
        // Read the byte about to be evicted, if the window is already
        // full. When `count < window`, the buffer slot at `pos` holds a
        // never-fed sentinel `0` — we simply do not perform the eviction
        // XOR below.
        let leaving = self.buffer[self.pos];
        self.buffer[self.pos] = byte;
        // `window.max(1)` matches the buffer capacity so the modulus never
        // reads past the end and never divides by zero.
        self.pos = (self.pos + 1) % self.window.max(1);

        // Multiply state by x^8 and add the reduction for the 8 high bits
        // that fall off. `state << 8` clears the low byte; ORing in the
        // new byte places it as coefficients of x^0..x^7.
        let high = usize::try_from(self.state >> 56).expect("high byte fits in usize");
        self.state = (self.state << 8) ^ u64::from(byte) ^ SHIFT_TABLE[high];

        // Remove the leaving byte's contribution once the window has
        // begun to overflow.
        if self.count >= self.window && self.window > 0 {
            self.state ^= self.roll_out_table[leaving as usize];
        }

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

/// Builds the fixed `SHIFT_TABLE` at compile time.
const fn build_shift_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut h = 0usize;
    while h < 256 {
        // `(h * x^64) mod P` with `P = x^64 + x^4 + x^3 + x + 1` reduces
        // to the carry-free product of `h` and `LOW_P`. Because both
        // operands have low degree, the result fits in a `u64` without
        // any further reduction.
        table[h] = gf2_carryless_mul_small(h as u64, LOW_P);
        h += 1;
    }
    table
}

/// Builds `roll_out_table` such that `table[b] = (b * x^(8 * window)) mod P`.
#[cfg(feature = "alloc")]
fn build_roll_out_table(window: usize) -> Vec<u64> {
    if window == 0 {
        return vec![0u64; 256];
    }

    // Compute `x^(8 * window) mod P` by repeated multiplication by `x^8`
    // with reduction after each step. Starting value: `x^0 = 1`.
    let mut x_to_8w: u64 = 1;
    for _ in 0..window {
        x_to_8w = gf2_mul_x8_mod_p(x_to_8w);
    }

    let mut table = vec![0u64; 256];
    for (b, slot) in table.iter_mut().enumerate() {
        // (b * x^(8*W)) mod P — safe because both fit in 64 bits and
        // multiplication by a byte reduces further via the same table.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "b is 0..256 and always fits in u64"
        )]
        let b_u64 = b as u64;
        *slot = gf2_mul_mod_p(b_u64, x_to_8w);
    }
    table
}

/// Multiplies `state` by `x^8` in `GF(2)` and reduces modulo `P`.
///
/// Equivalent to: shift `state` left by 8 bits and XOR in the reduction
/// contributed by the 8 bits that would fall off the top.
#[cfg(feature = "alloc")]
fn gf2_mul_x8_mod_p(state: u64) -> u64 {
    let high = usize::try_from(state >> 56).expect("high byte fits in usize");
    (state << 8) ^ SHIFT_TABLE[high]
}

/// Full `GF(2)` multiplication modulo `P` of two 64-bit polynomials.
///
/// Used at construction time to build the roll-out table; not on the hot
/// path.
#[cfg(feature = "alloc")]
fn gf2_mul_mod_p(a: u64, b: u64) -> u64 {
    let mut result: u64 = 0;
    let mut a_shifted = a;
    // Iterate the bits of `b`; each set bit contributes a shifted copy
    // of `a`. Each shift-left is followed by a reduction if the top bit
    // was set (since `x^64 ≡ LOW_P (mod P)`).
    for i in 0..64 {
        if (b >> i) & 1 == 1 {
            result ^= a_shifted;
        }
        let overflow = a_shifted >> 63;
        a_shifted <<= 1;
        if overflow != 0 {
            a_shifted ^= LOW_P;
        }
    }
    result
}

/// `GF(2)` carry-free multiplication for small operands that cannot
/// overflow a `u64`.
///
/// Used at compile time only, so a plain `while` loop is fine.
const fn gf2_carryless_mul_small(a: u64, b: u64) -> u64 {
    let mut result: u64 = 0;
    let mut i = 0;
    while i < 64 {
        if (b >> i) & 1 == 1 {
            result ^= a << i;
        }
        i += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_family_variant_and_year() {
        let d = RabinFingerprint::descriptor();
        assert_eq!(d.family, AlgorithmFamily::RabinFingerprint);
        assert_eq!(d.variant, VariantId("gf2-poly-koopman-64-1b"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1981, .. }
        ));
    }

    #[test]
    fn zero_window_is_degenerate_but_valid() {
        let mut h = RabinFingerprint::new(0);
        h.roll(0xAB);
        h.roll(0xCD);
        // With a zero-length window, no bytes are ever considered part
        // of the reduction — every fed byte is immediately evicted on
        // the next roll, so the state evolves but reflects no window.
        // We only assert that no panic occurs and digest is a valid
        // value.
        let _ = h.digest();
    }

    #[test]
    fn single_byte_digest_matches_manual_reduction() {
        let mut h = RabinFingerprint::new(8);
        h.roll(0xAB);
        // After one roll, state = (0 << 8) ^ 0xAB ^ SHIFT_TABLE[0]
        //                       = 0xAB.
        assert_eq!(h.digest(), 0xAB);
    }

    #[test]
    fn reset_returns_to_empty_state() {
        let mut h = RabinFingerprint::new(4);
        for &b in b"hello, world" {
            h.roll(b);
        }
        assert_ne!(h.digest(), 0);
        h.reset();
        assert_eq!(h.digest(), 0);
    }

    #[test]
    fn rolling_matches_fresh_hash_after_window_slides() {
        // Feed `total` bytes into a rolling hash; feed only the trailing
        // `window` bytes into a fresh hash. The two digests must agree.
        let total = b"the quick brown fox jumps over the lazy dog";
        let window = 8usize;

        let mut rolling = RabinFingerprint::new(window);
        for &b in total {
            rolling.roll(b);
        }

        let mut fresh = RabinFingerprint::new(window);
        for &b in &total[total.len() - window..] {
            fresh.roll(b);
        }

        assert_eq!(rolling.digest(), fresh.digest());
    }

    #[test]
    fn shift_table_matches_manual_carryless_multiply() {
        // Spot-check a few entries against a hand-computed value.
        // gf2_mul(1, 0x1B) = 0x1B.
        assert_eq!(SHIFT_TABLE[1], 0x1B);
        // gf2_mul(2, 0x1B) = 0x36 (0x1B shifted left by 1 in GF(2)).
        assert_eq!(SHIFT_TABLE[2], 0x36);
        // gf2_mul(3, 0x1B) = 0x1B ^ 0x36 = 0x2D.
        assert_eq!(SHIFT_TABLE[3], 0x1B ^ 0x36);
    }
}
