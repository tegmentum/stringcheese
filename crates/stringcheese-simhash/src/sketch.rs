//! SimHash sketch builder + Hamming-similarity comparator.
//!
//! The algorithm:
//!
//! 1. For each feature, hash it to a 64-bit (or 128-bit) integer.
//! 2. For each bit position `i` in the hash, add `+weight` to
//!    accumulator `i` if the bit is 1, else `-weight`.
//! 3. After every feature is accumulated, the final fingerprint
//!    is `sign(accumulator[i])` per position.
//!
//! Two similar bags produce similar bit patterns because most
//! individual feature hashes contribute the same sign at most
//! positions. Random / unrelated bags average out to ~half agreement.
//!
//! Weight defaults to 1 for the unweighted [`Sketcher::add`] path;
//! use [`Sketcher::add_weighted`] for tf-idf-style feature
//! weighting.

use alloc::vec::Vec;
use core::hash::Hash;

use ahash::RandomState;

/// A 64-bit SimHash fingerprint.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Sketch64 {
    bits: u64,
    seed: u64,
}

impl Sketch64 {
    /// Raw bit pattern. Exposed for LSH banding in [`crate::lsh`]
    /// and for external serialisation.
    #[must_use]
    pub fn bits(&self) -> u64 {
        self.bits
    }

    /// The seed the fingerprint was built with. Two fingerprints
    /// must share a seed to be compared — otherwise the bit
    /// positions carry different implicit random projections and
    /// the similarity number is meaningless.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Number of bit positions where the two fingerprints disagree.
    ///
    /// # Panics
    ///
    /// Panics when the two fingerprints were built with different
    /// seeds (their random projections are drawn from different
    /// hash families and the distance is meaningless).
    #[must_use]
    pub fn hamming_distance(&self, other: &Sketch64) -> u32 {
        assert_eq!(
            self.seed, other.seed,
            "fingerprints must share a seed to be compared",
        );
        (self.bits ^ other.bits).count_ones()
    }

    /// Cosine-similar signal in `[0.0, 1.0]`. `1.0` means the
    /// fingerprints are identical; `0.0` means every bit disagrees.
    /// Computed as `1 - hamming / 64`.
    ///
    /// # Panics
    ///
    /// Panics when the two fingerprints were built with different
    /// seeds — see [`Self::hamming_distance`].
    #[must_use]
    pub fn similarity(&self, other: &Sketch64) -> f64 {
        1.0 - f64::from(self.hamming_distance(other)) / 64.0
    }
}

/// A 128-bit SimHash fingerprint. Higher precision than [`Sketch64`]
/// at the cost of a wider sketch — a caller who wants stronger
/// separation between near-duplicates picks this width; 64-bit is
/// plenty for coarse candidate generation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Sketch128 {
    // Stored as [hi, lo] halves; combined into a full 128-bit
    // hamming count in `hamming_distance`.
    hi: u64,
    lo: u64,
    seed: u64,
}

impl Sketch128 {
    /// Raw `(hi, lo)` bit pattern.
    #[must_use]
    pub fn bits(&self) -> (u64, u64) {
        (self.hi, self.lo)
    }

    /// The seed the fingerprint was built with.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Number of bit positions where the two fingerprints disagree.
    ///
    /// # Panics
    ///
    /// Panics when the two fingerprints were built with different
    /// seeds.
    #[must_use]
    pub fn hamming_distance(&self, other: &Sketch128) -> u32 {
        assert_eq!(
            self.seed, other.seed,
            "fingerprints must share a seed to be compared",
        );
        (self.hi ^ other.hi).count_ones() + (self.lo ^ other.lo).count_ones()
    }

    /// Cosine-similar signal in `[0.0, 1.0]`.
    ///
    /// # Panics
    ///
    /// Panics when the two fingerprints were built with different
    /// seeds — see [`Self::hamming_distance`].
    #[must_use]
    pub fn similarity(&self, other: &Sketch128) -> f64 {
        1.0 - f64::from(self.hamming_distance(other)) / 128.0
    }
}

/// Builder for [`Sketch64`] / [`Sketch128`].
///
/// Accumulate features (weighted or unweighted) via [`Self::add`]
/// / [`Self::add_weighted`], then call [`Self::finalize_64`] or
/// [`Self::finalize_128`] to materialise the fingerprint.
#[derive(Clone, Debug)]
pub struct Sketcher {
    // 128 per-bit accumulators. `finalize_64` uses the low 64;
    // `finalize_128` uses all 128. i32 keeps the accumulator
    // compact and is plenty for corpora — a single feature moves
    // an accumulator by at most `weight`; adversarial inputs
    // would need billions of features to overflow.
    accum: Vec<i64>,
    seed: u64,
}

impl Default for Sketcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Sketcher {
    /// Fixed default seed used when the caller doesn't specify one.
    /// Named constant so downstream determinism guarantees survive
    /// refactors. Distinct from stringcheese-minhash's default so
    /// cross-family seed reuse can't accidentally silently work.
    pub const DEFAULT_SEED: u64 = 0x5A5A_A5A5_5A5A_A5A5;

    /// Construct a fresh sketcher using [`Self::DEFAULT_SEED`].
    #[must_use]
    pub fn new() -> Self {
        Self::seeded(Self::DEFAULT_SEED)
    }

    /// Construct a sketcher with a specific seed. Two fingerprints
    /// with different seeds are not comparable.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self {
            accum: alloc::vec![0i64; 128],
            seed,
        }
    }

    /// Add one feature with weight 1. Chainable.
    // `clippy::should_implement_trait` fires because the name
    // collides with `std::ops::Add::add`. The overlap is
    // superficial — this is a builder-style feed, not arithmetic
    // — and `feature` / `push` / `insert` all read worse in the
    // fluent chain than `add`.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn add<T: Hash>(mut self, feature: T) -> Self {
        self.mix(&feature, 1);
        self
    }

    /// Add one feature with a caller-supplied weight. Chainable.
    /// Zero-weighted features contribute nothing (useful for
    /// idf-weighted-with-cutoff flows).
    #[must_use]
    pub fn add_weighted<T: Hash>(mut self, feature: T, weight: i64) -> Self {
        self.mix(&feature, weight);
        self
    }

    /// Add every feature from `features` with weight 1. Chainable.
    #[must_use]
    pub fn add_all<T: Hash, I: IntoIterator<Item = T>>(mut self, features: I) -> Self {
        for f in features {
            self.mix(&f, 1);
        }
        self
    }

    fn mix<T: Hash>(&mut self, feature: &T, weight: i64) {
        if weight == 0 {
            return;
        }
        // Hash to 128 bits by mixing the seed differently for the
        // high and low halves — one call per half, both cheap.
        let hi = RandomState::with_seeds(self.seed, 1, 2, 3).hash_one(feature);
        let lo = RandomState::with_seeds(self.seed, 4, 5, 6).hash_one(feature);
        // Update each of the 128 accumulators: +weight if bit is
        // 1, -weight if bit is 0.
        for i in 0..64 {
            self.accum[i] += if (hi >> i) & 1 == 1 { weight } else { -weight };
        }
        for i in 0..64 {
            self.accum[64 + i] += if (lo >> i) & 1 == 1 { weight } else { -weight };
        }
    }

    /// Finalise into a 64-bit fingerprint. Uses the first 64
    /// accumulator positions.
    #[must_use]
    pub fn finalize_64(self) -> Sketch64 {
        let mut bits = 0u64;
        for (i, &a) in self.accum.iter().take(64).enumerate() {
            // Signed-zero accumulator (a == 0 means "equally many
            // features pulled this bit each way") flips the bit to
            // 0 by convention — matches the reference formulation.
            if a > 0 {
                bits |= 1 << i;
            }
        }
        Sketch64 {
            bits,
            seed: self.seed,
        }
    }

    /// Finalise into a 128-bit fingerprint. Uses all 128
    /// accumulator positions.
    #[must_use]
    pub fn finalize_128(self) -> Sketch128 {
        let mut hi = 0u64;
        let mut lo = 0u64;
        for (i, &a) in self.accum.iter().take(64).enumerate() {
            if a > 0 {
                hi |= 1 << i;
            }
        }
        for (i, &a) in self.accum.iter().skip(64).take(64).enumerate() {
            if a > 0 {
                lo |= 1 << i;
            }
        }
        Sketch128 {
            hi,
            lo,
            seed: self.seed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_bags_produce_identical_fingerprints() {
        let s1 = Sketcher::new().add_all(["foo", "bar", "baz"]).finalize_64();
        let s2 = Sketcher::new().add_all(["foo", "bar", "baz"]).finalize_64();
        assert_eq!(s1, s2);
        assert_eq!(s1.similarity(&s2), 1.0);
        assert_eq!(s1.hamming_distance(&s2), 0);
    }

    #[test]
    fn very_similar_bags_produce_similar_fingerprints() {
        // 4 shared features + one differing → high overlap.
        let s1 = Sketcher::new()
            .add_all(["a", "b", "c", "d", "e"])
            .finalize_64();
        let s2 = Sketcher::new()
            .add_all(["a", "b", "c", "d", "f"])
            .finalize_64();
        let sim = s1.similarity(&s2);
        assert!(sim > 0.7, "expected > 0.7, got {sim}");
    }

    #[test]
    fn disjoint_bags_diverge() {
        // Two 16-element disjoint bags. Random projection gives
        // expected ~50% agreement (32/64 bits match by chance);
        // 3σ tolerance on width 64 ≈ 0.19, so require < 0.75.
        let a: alloc::vec::Vec<alloc::string::String> =
            (0..16).map(|i| alloc::format!("aaa-{i}")).collect();
        let b: alloc::vec::Vec<alloc::string::String> =
            (0..16).map(|i| alloc::format!("bbb-{i}")).collect();
        let s1 = Sketcher::new()
            .add_all(a.iter().map(alloc::string::String::as_str))
            .finalize_64();
        let s2 = Sketcher::new()
            .add_all(b.iter().map(alloc::string::String::as_str))
            .finalize_64();
        let sim = s1.similarity(&s2);
        assert!(sim < 0.75, "expected < 0.75, got {sim}");
    }

    #[test]
    fn weighted_features_dominate_when_heavier() {
        // Two bags where one shared feature is heavily weighted
        // in both — should pull the fingerprints close together
        // even though the other features differ.
        let s1 = Sketcher::new()
            .add_weighted("shared", 100)
            .add_all(["a", "b", "c"])
            .finalize_64();
        let s2 = Sketcher::new()
            .add_weighted("shared", 100)
            .add_all(["x", "y", "z"])
            .finalize_64();
        let sim = s1.similarity(&s2);
        // The heavyweight shared feature swamps the light ones.
        assert!(sim > 0.85, "expected > 0.85, got {sim}");
    }

    #[test]
    fn zero_weight_features_dont_move_fingerprint() {
        let s1 = Sketcher::new().add_all(["a", "b", "c"]).finalize_64();
        let s2 = Sketcher::new()
            .add_all(["a", "b", "c"])
            .add_weighted("noise", 0)
            .finalize_64();
        assert_eq!(s1, s2);
    }

    #[test]
    fn empty_sketcher_produces_all_zero_fingerprint() {
        let s = Sketcher::new().finalize_64();
        assert_eq!(s.bits(), 0);
    }

    #[test]
    fn sketch128_wider_signal_for_finer_grained_dedup() {
        let s1 = Sketcher::new().add_all(["a", "b", "c"]).finalize_128();
        let s2 = Sketcher::new().add_all(["a", "b", "c"]).finalize_128();
        assert_eq!(s1, s2);
        assert_eq!(s1.similarity(&s2), 1.0);
        assert_eq!(s1.hamming_distance(&s2), 0);
    }

    #[test]
    fn seeded_fingerprints_are_deterministic() {
        let s1 = Sketcher::seeded(42).add_all(["a", "b", "c"]).finalize_64();
        let s2 = Sketcher::seeded(42).add_all(["a", "b", "c"]).finalize_64();
        assert_eq!(s1.bits(), s2.bits());
    }

    #[test]
    #[should_panic(expected = "must share a seed")]
    fn mismatched_seed_panics() {
        let s1 = Sketcher::seeded(1).add("a").finalize_64();
        let s2 = Sketcher::seeded(2).add("a").finalize_64();
        let _ = s1.hamming_distance(&s2);
    }

    #[test]
    #[should_panic(expected = "must share a seed")]
    fn mismatched_seed_panics_128() {
        let s1 = Sketcher::seeded(1).add("a").finalize_128();
        let s2 = Sketcher::seeded(2).add("a").finalize_128();
        let _ = s1.hamming_distance(&s2);
    }
}
