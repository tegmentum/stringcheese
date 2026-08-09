//! MinHash sketch builder + Jaccard estimator.
//!
//! Implementation is the classic "K independent hash functions"
//! formulation: for each of the `k` permutations, keep the minimum
//! hash of any input element. Two sketches' Jaccard similarity is
//! estimated as `matches / k`.
//!
//! The K permutations are synthesised from one seed by hashing
//! each element together with the permutation index — a cheap and
//! well-behaved substitute for K independent hash families that
//! avoids the ~megabyte-sized universal-hashing tables classical
//! MinHash formulations require.

use alloc::vec;
use alloc::vec::Vec;
use core::hash::Hash;

use ahash::RandomState;

/// A fixed-permutation MinHash sketch.
///
/// Two sketches of the same width can be compared via
/// [`Self::jaccard`] in O(width). Sketches of different widths
/// panic on comparison — the estimator is only defined for
/// same-width pairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sketch {
    // Per-permutation minimum hashes. `u64::MAX` marks "no
    // element seen yet under this permutation" during build; a
    // finalized sketch may still carry `u64::MAX` for an empty
    // input, and [`Self::jaccard`] handles that as 1.0 (two empty
    // sets are trivially equal).
    mins: Vec<u64>,
    seed: u64,
}

impl Sketch {
    /// Number of permutations (`k`) this sketch carries. Two
    /// sketches must have the same width to be compared.
    #[must_use]
    pub fn width(&self) -> usize {
        self.mins.len()
    }

    /// The seed the sketch was built with. Two sketches must share
    /// a seed to be comparable — cross-seed comparisons produce
    /// meaningless numbers because the K permutations are seed-
    /// derived.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Raw access to the per-permutation min values. Exposed for
    /// LSH banding in [`crate::lsh`] and for external
    /// serialisation.
    #[must_use]
    pub fn as_slice(&self) -> &[u64] {
        &self.mins
    }

    /// Jaccard-similarity estimate against another sketch. Returns
    /// `matches / k` — the fraction of positions where both
    /// sketches recorded the same minimum hash.
    ///
    /// # Panics
    ///
    /// Panics when the two sketches have different widths or seeds
    /// (they aren't drawn from the same permutation family).
    #[must_use]
    pub fn jaccard(&self, other: &Sketch) -> f64 {
        assert_eq!(
            self.width(),
            other.width(),
            "sketches must have equal width",
        );
        assert_eq!(
            self.seed, other.seed,
            "sketches must share a seed to be compared",
        );
        if self.mins.is_empty() {
            return 1.0;
        }
        let matches = self
            .mins
            .iter()
            .zip(other.mins.iter())
            .filter(|(a, b)| a == b)
            .count();
        matches as f64 / self.width() as f64
    }
}

/// Builder for [`Sketch`] — configure width and seed once, then
/// call [`Self::sketch`] on an iterator of gram values.
#[derive(Clone, Debug)]
pub struct Sketcher {
    width: usize,
    seed: u64,
}

impl Sketcher {
    /// Fixed default seed used when the caller doesn't specify one.
    /// A named constant (rather than 0 or a random value) so that
    /// downstream determinism guarantees survive refactors.
    pub const DEFAULT_SEED: u64 = 0xA5A5_5A5A_A5A5_5A5A;

    /// Construct a sketcher of `width` permutations using
    /// [`Self::DEFAULT_SEED`].
    ///
    /// # Panics
    ///
    /// Panics on `width == 0` — a zero-width sketch carries no
    /// information.
    #[must_use]
    pub fn new(width: usize) -> Self {
        assert!(width > 0, "MinHash width must be > 0");
        Self {
            width,
            seed: Self::DEFAULT_SEED,
        }
    }

    /// Construct a sketcher with a specific seed. Two sketches
    /// with different seeds are not comparable.
    ///
    /// # Panics
    ///
    /// Panics on `width == 0`.
    #[must_use]
    pub fn seeded(width: usize, seed: u64) -> Self {
        assert!(width > 0, "MinHash width must be > 0");
        Self { width, seed }
    }

    /// Build a [`Sketch`] from an iterator of gram values.
    ///
    /// `grams` accepts any iterator whose items are `Hash` — the
    /// gram types emitted by the `stringcheese-ngram` crate's
    /// producers (`&str` for char/token/grapheme grams, `&[u8]`
    /// for byte grams) all satisfy this out of the box.
    #[must_use]
    pub fn sketch<T, I>(&self, grams: I) -> Sketch
    where
        T: Hash,
        I: IntoIterator<Item = T>,
    {
        // Per-permutation minimums. `u64::MAX` sentinel means "no
        // element seen under this permutation yet"; every real
        // hash falls strictly below and displaces it.
        let mut mins = vec![u64::MAX; self.width];
        for gram in grams {
            for (k, min) in mins.iter_mut().enumerate() {
                let h = hash_with_permutation(&gram, k as u64, self.seed);
                if h < *min {
                    *min = h;
                }
            }
        }
        Sketch {
            mins,
            seed: self.seed,
        }
    }
}

/// Hash `gram` under the `k`-th permutation for a given `seed`.
///
/// K independent hash functions are synthesised by mixing the
/// permutation index into a single `ahash::RandomState` — cheap
/// and empirically well-distributed enough for MinHash without
/// dragging in the universal-hashing tables.
fn hash_with_permutation<T: Hash>(gram: &T, k: u64, seed: u64) -> u64 {
    // Derive a per-permutation ahash state by mixing k into the
    // seed. `RandomState::with_seeds` takes four 64-bit values —
    // varying two of them per permutation gives K distinct hash
    // families from one caller-supplied seed.
    RandomState::with_seeds(seed ^ k, seed.wrapping_add(k), k, !seed).hash_one(gram)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_sets_have_jaccard_one() {
        let s1 = Sketcher::new(64).sketch(["foo", "bar", "baz"]);
        let s2 = Sketcher::new(64).sketch(["foo", "bar", "baz"]);
        assert_eq!(s1.jaccard(&s2), 1.0);
    }

    #[test]
    fn disjoint_sets_have_low_jaccard() {
        // With 128 permutations the estimate for two disjoint
        // 4-element sets is expected 0.0 but has some noise; the
        // upper 3σ bound at width 128 is well under 0.2.
        let s1 = Sketcher::new(128).sketch(["a", "b", "c", "d"]);
        let s2 = Sketcher::new(128).sketch(["w", "x", "y", "z"]);
        let j = s1.jaccard(&s2);
        assert!(j < 0.2, "expected < 0.2, got {j}");
    }

    #[test]
    fn empty_sets_are_trivially_equal() {
        let s1 = Sketcher::new(64).sketch::<&str, _>([]);
        let s2 = Sketcher::new(64).sketch::<&str, _>([]);
        // Both min-vectors are all-MAX; every position matches.
        assert_eq!(s1.jaccard(&s2), 1.0);
    }

    #[test]
    fn estimate_tracks_true_jaccard() {
        // Set A: 100 unique strings. Set B: 50 shared + 50 unique.
        // True Jaccard = 50 / 150 = 0.333.
        let a: Vec<String> = (0..100).map(|i| alloc::format!("item-{i}")).collect();
        let b: Vec<String> = (0..50)
            .map(|i| alloc::format!("item-{i}"))
            .chain((100..150).map(|i| alloc::format!("item-{i}")))
            .collect();
        // Width 256 → σ ≈ √(0.333·0.667/256) ≈ 0.029.
        let s1 = Sketcher::new(256).sketch(a.iter().map(String::as_str));
        let s2 = Sketcher::new(256).sketch(b.iter().map(String::as_str));
        let j = s1.jaccard(&s2);
        // 3σ tolerance around the true 0.333.
        assert!((j - 0.333_f64).abs() < 0.1, "expected ~0.333, got {j}");
    }

    #[test]
    fn seeded_sketches_are_deterministic() {
        let s1 = Sketcher::seeded(32, 42).sketch(["a", "b", "c"]);
        let s2 = Sketcher::seeded(32, 42).sketch(["a", "b", "c"]);
        assert_eq!(s1.as_slice(), s2.as_slice());
    }

    #[test]
    #[should_panic(expected = "MinHash width must be > 0")]
    fn zero_width_panics() {
        let _ = Sketcher::new(0);
    }

    #[test]
    #[should_panic(expected = "must have equal width")]
    fn mismatched_width_panics() {
        let s1 = Sketcher::new(32).sketch(["a"]);
        let s2 = Sketcher::new(64).sketch(["a"]);
        let _ = s1.jaccard(&s2);
    }

    #[test]
    #[should_panic(expected = "must share a seed")]
    fn mismatched_seed_panics() {
        let s1 = Sketcher::seeded(32, 1).sketch(["a"]);
        let s2 = Sketcher::seeded(32, 2).sketch(["a"]);
        let _ = s1.jaccard(&s2);
    }
}
