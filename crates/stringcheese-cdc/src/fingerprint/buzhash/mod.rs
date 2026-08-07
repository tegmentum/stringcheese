//! Buzhash — cyclic-polynomial rolling hash (Uzgalis 1983).
//!
//! Buzhash represents each byte by a fixed 64-bit value `T[byte]` drawn
//! from a 256-entry substitution table, then combines the entries for the
//! bytes currently in the window with a bitwise rotate-left. For a window
//! `b_0, b_1, ..., b_{n-1}` the hash is
//!
//! ```text
//! h = ROL(T[b_0], n-1) ^ ROL(T[b_1], n-2) ^ ... ^ ROL(T[b_{n-2}], 1) ^ T[b_{n-1}]
//! ```
//!
//! where `ROL` is a 64-bit rotate-left. Sliding the window from
//! `[b_0, ..., b_{n-1}]` to `[b_1, ..., b_n]` is a single-step update:
//!
//! ```text
//! h' = ROL(h, 1) ^ ROL(T[b_0], n) ^ T[b_n]
//! ```
//!
//! The `ROL(T[b_0], n)` term evicts the outgoing byte's contribution.
//! `b_0` was rotated `n - 1` times while it was in the window; the first
//! `ROL(h, 1)` on the right rotates it once more, to `n` — exactly the
//! precomputed roll-out value the XOR cancels.
//!
//! # The table
//!
//! Uzgalis's original paper specifies a random permutation of `u64` values
//! but does not pin any single canonical set — the widely-cited reference
//! implementations (`rdedup`, `restic`) each carry their own 256-entry
//! table drawn from the same underlying construction. This implementation
//! generates its table deterministically at compile time from a fixed
//! `SplitMix64` seed, encoded in the variant slug as
//! `"splitmix64-seed-buzz"` so a golden case tied to this table cannot
//! silently be re-run against a differently-parameterized implementation.
//!
//! Deterministic generation matters for the same reason it does for
//! `GearHash`: the design commits to identical hash output across native
//! and Wasm targets and across debug and release builds. An
//! ASLR-influenced or wall-clock-seeded table would violate that
//! invariant.
//!
//! # `alloc` requirement
//!
//! The rolling formulation needs to know the byte being evicted, which in
//! turn requires a circular buffer sized to the window. On a no-alloc
//! build there is no way to size that buffer at run time, so — matching
//! [`RabinFingerprint`][super::RabinFingerprint] and
//! [`PolynomialHash`][super::PolynomialHash] — the type is available only
//! with the `alloc` feature enabled.
//!
//! # References
//!
//! * Uzgalis, R. C. (1983). "Hashing concepts and the Java programming
//!   language." — introduces the cyclic-polynomial rolling hash this
//!   module implements.
//! * Cohen, J. D. (1997). "Recursive hashing functions for n-grams."
//!   *ACM Transactions on Information Systems*, 15(3), 291-320.
//!   <https://doi.org/10.1145/256163.256168> — formal treatment of the
//!   cyclic-polynomial family Buzhash belongs to, including a proof of
//!   uniformity for random tables.

#![cfg(feature = "alloc")]

use alloc::{vec, vec::Vec};

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use super::RollingHash;

#[cfg(feature = "simd")]
pub mod simd;

/// The 256-entry Buzhash substitution table.
///
/// Generated at compile time via `SplitMix64` seeded with a fixed constant.
/// See the [module-level documentation][crate::fingerprint::buzhash] for
/// the seed and the reason it is exposed as part of the variant slug
/// rather than baked in silently.
pub const BUZ_TABLE: [u64; 256] = build_buz_table();

/// Buzhash cyclic-polynomial rolling hash (Uzgalis 1983).
///
/// Maintains a running `u64` state and a circular buffer of the bytes
/// currently in the window; each roll performs one 64-bit rotate, two
/// table lookups (one plus one rotated eviction), and two XORs.
///
/// See the [module-level documentation][crate::fingerprint::buzhash] for
/// the update formula and the table's provenance.
#[derive(Clone, Debug)]
pub struct Buzhash {
    /// The sliding window size in bytes.
    window: usize,
    /// `window mod 64`, cached as a `u32` so the hot-path rotate does not
    /// re-perform the modulus. Rotating a `u64` by `window` bits is
    /// equivalent to rotating by `window mod 64` bits, so folding here
    /// keeps the eviction rotate cheap even when the window exceeds 64.
    window_rot: u32,
    /// Circular buffer of the bytes currently contributing to the window.
    /// Only used to reconstruct the leaving byte on eviction.
    buffer: Vec<u8>,
    /// Next slot to write in `buffer`; also the slot that will be
    /// overwritten on the next `roll` once the window is full.
    pos: usize,
    /// Total number of bytes fed since construction or `reset`. Used to
    /// decide when the window has begun to overflow.
    count: usize,
    /// Current hash state.
    state: u64,
}

impl Buzhash {
    /// The algorithm descriptor for this Buzhash variant.
    ///
    /// The variant slug pins the specific substitution table in use so a
    /// golden case for `"splitmix64-seed-buzz"` cannot silently be run
    /// against a differently-parameterized Buzhash implementation.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Buzhash,
        variant: VariantId("splitmix64-seed-buzz"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Hashing concepts and the Java programming language",
            authors: "R. C. Uzgalis",
            year: 1983,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Constructs a new Buzhash with the given window size.
    ///
    /// A `window` of zero is legal but degenerate — the hash never
    /// accumulates any bytes and always reports the identity digest of
    /// zero. Every subsequent `roll` is a no-op.
    #[must_use]
    pub fn with_window(window: usize) -> Self {
        // Match `RabinFingerprint`'s convention: give the buffer a
        // capacity of at least one so `buffer[pos]` and the modulus on
        // `pos` are always well-defined, even when `window == 0`. The
        // `roll` hot path early-returns on `window == 0` so the buffer is
        // never read or written in that degenerate case.
        let capacity = window.max(1);
        let buffer = vec![0u8; capacity];

        // Rotating a `u64` by `k` bits is equivalent to rotating by
        // `k mod 64`; folding once at construction time keeps the
        // eviction rotate cheap even when `window > 64`. The rotate-left
        // API on `u64` takes a `u32`; `window % 64` is always in
        // `0..64` and fits.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "`window % 64` is in 0..64 and always fits in a u32"
        )]
        let window_rot = (window % 64) as u32;

        Self {
            window,
            window_rot,
            buffer,
            pos: 0,
            count: 0,
            state: 0,
        }
    }
}

impl RollingHash for Buzhash {
    type Output = u64;

    fn new(window: usize) -> Self {
        Self::with_window(window)
    }

    fn roll(&mut self, byte: u8) {
        // Degenerate zero-length window: nothing accumulates, the digest
        // is always zero, and no buffer accounting is ever needed.
        if self.window == 0 {
            return;
        }

        let new_contrib = BUZ_TABLE[byte as usize];

        // Read the byte about to be evicted, but only apply the eviction
        // XOR once the window has begun to overflow (`count >= window`).
        // Before that, the buffer slot at `pos` holds a never-fed
        // sentinel `0` and we omit the roll-out term.
        if self.count >= self.window {
            let leaving = self.buffer[self.pos];
            let leaving_contrib = BUZ_TABLE[leaving as usize].rotate_left(self.window_rot);
            self.state = self.state.rotate_left(1) ^ leaving_contrib ^ new_contrib;
        } else {
            self.state = self.state.rotate_left(1) ^ new_contrib;
        }

        self.buffer[self.pos] = byte;
        self.pos = (self.pos + 1) % self.window;
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

/// A single `SplitMix64` step.
///
/// The classical `SplitMix64` output function due to Steele, Lea, and Flood
/// (2014), used verbatim as documented in the reference. Constants are
/// intentionally kept as-published so a reader can cross-check against
/// the reference. This is a private duplicate of the same routine in
/// [`crate::fingerprint::gear`]; keeping the two copies textually
/// identical lets each fingerprint's compile-time table be audited
/// without cross-module coupling.
#[allow(
    clippy::unreadable_literal,
    reason = "these are the published `SplitMix64` mixing constants — separating with underscores obscures the fact that they match the reference"
)]
const fn splitmix64_next(state: u64) -> (u64, u64) {
    let next_state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = next_state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D49BB133111EB1);
    z ^= z >> 31;
    (next_state, z)
}

/// The fixed `SplitMix64` seed that produces `BUZ_TABLE`.
///
/// A repeating-`0xBB` byte pattern; the variant slug
/// `"splitmix64-seed-buzz"` names it mnemonically. Distinct from
/// [`crate::fingerprint::gear`]'s `GEAR_SEED` so the two fingerprints
/// draw uncorrelated table entries.
const BUZ_SEED: u64 = 0x00BB_BBBB_BBBB_BB00;

/// Builds `BUZ_TABLE` at compile time.
const fn build_buz_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut state = BUZ_SEED;
    let mut i = 0;
    while i < 256 {
        let (next_state, z) = splitmix64_next(state);
        state = next_state;
        table[i] = z;
        i += 1;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_family_variant_and_year() {
        let d = Buzhash::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Buzhash);
        assert_eq!(d.variant, VariantId("splitmix64-seed-buzz"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1983, .. }
        ));
    }

    #[test]
    fn empty_hash_is_zero() {
        let h = Buzhash::new(8);
        assert_eq!(h.digest(), 0);
    }

    #[test]
    fn zero_window_is_degenerate_but_valid() {
        // With a zero-length window every `roll` is a no-op; the digest
        // is always the identity zero and no buffer indexing is ever
        // performed.
        let mut h = Buzhash::new(0);
        h.roll(0xAB);
        h.roll(0xCD);
        assert_eq!(h.digest(), 0);
    }

    #[test]
    fn single_byte_matches_table_entry() {
        // Feeding a single byte into a fresh hasher (state = 0) yields
        // `ROL(0, 1) ^ T[byte] = T[byte]`.
        for b in [0u8, 1, 42, 0xFF] {
            let mut h = Buzhash::new(8);
            h.roll(b);
            assert_eq!(h.digest(), BUZ_TABLE[b as usize]);
        }
    }

    #[test]
    fn two_bytes_match_formula() {
        // `h = ROL(T[b0], 1) ^ T[b1]` for any window >= 2.
        let mut h = Buzhash::new(8);
        h.roll(b'A');
        h.roll(b'B');
        let expected = BUZ_TABLE[b'A' as usize].rotate_left(1) ^ BUZ_TABLE[b'B' as usize];
        assert_eq!(h.digest(), expected);
    }

    #[test]
    fn reset_returns_to_empty_state() {
        let mut h = Buzhash::new(4);
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

        let mut rolling = Buzhash::new(window);
        for &b in total {
            rolling.roll(b);
        }

        let mut fresh = Buzhash::new(window);
        for &b in &total[total.len() - window..] {
            fresh.roll(b);
        }

        assert_eq!(rolling.digest(), fresh.digest());
    }

    #[test]
    fn window_larger_than_64_still_rolls_correctly() {
        // The rotate folds modulo 64, but the eviction accounting must
        // still track the actual window size. Windows > 64 exercise that.
        let window = 100usize;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the truncation is the intended byte-derivation step"
        )]
        let total: Vec<u8> = (0..300u32).map(|i| (i.wrapping_mul(37)) as u8).collect();

        let mut rolling = Buzhash::new(window);
        for &b in &total {
            rolling.roll(b);
        }

        let mut fresh = Buzhash::new(window);
        for &b in &total[total.len() - window..] {
            fresh.roll(b);
        }

        assert_eq!(rolling.digest(), fresh.digest());
    }

    #[test]
    fn table_is_diverse() {
        // Sanity: `SplitMix64` should not produce a degenerate table.
        // For 256 draws from a 64-bit space, uniqueness is overwhelmingly
        // likely and easy to check.
        assert_ne!(BUZ_TABLE[0], BUZ_TABLE[1]);
        assert_ne!(BUZ_TABLE[0], 0);
        for (i, &a) in BUZ_TABLE.iter().enumerate() {
            for &b in &BUZ_TABLE[i + 1..] {
                assert_ne!(a, b, "table collision detected at index {i}");
            }
        }
    }

    #[test]
    fn table_is_distinct_from_gear_table() {
        // Buzhash and Gear draw from distinct SplitMix64 seeds; their
        // 256-entry tables must not accidentally coincide.
        assert_ne!(
            &BUZ_TABLE[..],
            &crate::fingerprint::gear::GEAR_TABLE[..],
            "Buzhash and Gear tables must not coincide"
        );
    }

    #[test]
    fn deterministic_across_calls() {
        // Two Buzhash instances fed the same bytes must produce
        // identical digests, always.
        let mut h1 = Buzhash::new(16);
        let mut h2 = Buzhash::new(16);
        for &b in b"reproducibility matters here" {
            h1.roll(b);
            h2.roll(b);
        }
        assert_eq!(h1.digest(), h2.digest());
    }

    #[test]
    fn low_bits_are_approximately_uniform() {
        // A crude chi-square sanity check — not statistical rigor. Feed
        // a pseudo-random byte stream and bucket the low four bits of
        // the digest at each window position. With a good rolling hash
        // the buckets should be roughly balanced.
        //
        // The threshold below is deliberately loose; we are checking
        // that the low bits are not stuck at a constant value or
        // degenerate to a single bucket.
        let window = 32usize;
        let n_samples = 4096usize;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "n_samples + window is bounded well below u32::MAX; the byte cast on the mixed value is the intended byte-derivation step"
        )]
        let bytes: Vec<u8> = (0..(n_samples + window) as u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 8) as u8)
            .collect();

        let mut h = Buzhash::new(window);
        let mut buckets = [0u64; 16];
        for (i, &b) in bytes.iter().enumerate() {
            h.roll(b);
            if i + 1 >= window {
                let idx = (h.digest() & 0xF) as usize;
                buckets[idx] += 1;
            }
        }

        // Chi-square against a uniform expectation.
        #[allow(
            clippy::cast_precision_loss,
            reason = "sample sizes stay well within f64 precision"
        )]
        let expected = n_samples as f64 / 16.0;
        #[allow(
            clippy::cast_precision_loss,
            reason = "bucket counts stay well within f64 precision"
        )]
        let chi_sq: f64 = buckets
            .iter()
            .map(|&c| {
                let d = c as f64 - expected;
                d * d / expected
            })
            .sum();

        // A truly uniform bucketing has chi-square around 15 (dof = 15).
        // A degenerate hash (every digest lands in one bucket) has
        // chi-square around n_samples * 15. We allow a very generous
        // upper bound to keep the test from flapping on unusual PRNG
        // streams; the point is to catch gross degeneracy.
        assert!(
            chi_sq < 100.0,
            "chi-square {chi_sq} suggests degenerate low-bit distribution; buckets = {buckets:?}"
        );
    }
}
