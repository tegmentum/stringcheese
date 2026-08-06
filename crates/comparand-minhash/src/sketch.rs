//! The [`MinHashSketch`] and its Jaccard estimator.
//!
//! # Construction
//!
//! For a set `A` of grams, a sketch of size `k` and seed `s` computes:
//!
//! ```text
//!     for each permutation i in 0..k:
//!         seed_i           = permutation_seed(s, i)
//!         signatures[i]    = min over g in A of permuted_hash(portable_hash(0, g), seed_i)
//! ```
//!
//! Empty inputs receive the [`SENTINEL_SIGNATURE`] value ([`u64::MAX`])
//! for every permutation, so [`MinHashSketch::estimated_jaccard`] on two
//! all-sentinel sketches returns exactly `1.0`. See the crate-level
//! *Empty-vs-empty convention* section.
//!
//! # Estimator
//!
//! The Broder estimator counts positions where the two sketches agree
//! and divides by `k`. Its expected value on a fixed pair `(A, B)` is
//! exactly `jaccard(A, B)`, and its variance is bounded by
//! `jaccard(A, B) · (1 - jaccard(A, B)) / k`. Callers pick `k` to trade
//! off memory against variance: `k = 128` gives standard error `~0.04` at
//! `J = 0.5`; `k = 1024` gives `~0.015`.
//!
//! # Base-hash seed
//!
//! The base hash uses seed `0` deliberately: the *per-permutation* seed
//! carries all the entropy the sketch needs, and fixing the base seed
//! keeps the invariant that "two sketches with the same k and seed
//! consumed the same items" implies "two sketches with equal signatures."
//! A caller-controllable base seed would risk two sketches built from
//! the same items disagreeing on their signatures, which would make
//! [`MinHashSketch::estimated_jaccard`] useless as a cross-corpus
//! comparator.

use alloc::vec;
use alloc::vec::Vec;
use core::hash::Hash;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::hash::{permutation_seed, permuted_hash, portable_hash};

/// The sentinel value assigned to an empty-set signature slot.
///
/// Chosen as [`u64::MAX`] so that `min(sentinel, real_hash)` always
/// prefers a real hash, even when the sketch is being updated
/// incrementally. The probability that a real gram produces this exact
/// value under the [`crate::hash`] construction is `~2^-64` per
/// permutation, so treating it as a sentinel produces no observable
/// false-collision behavior at any realistic corpus size.
pub const SENTINEL_SIGNATURE: u64 = u64::MAX;

/// Broder 1997 `MinHash` Jaccard estimator descriptor.
///
/// This is the *comparison* descriptor. The sketch type itself is
/// representation-layer infrastructure and does not carry its own
/// descriptor (mirroring `comparand-index`'s rule that BK-trees and
/// VP-trees do not carry descriptors either — they wrap descriptor-
/// carrying comparisons rather than being one).
pub const MINHASH_JACCARD_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
    AlgorithmFamily::Jaccard,
    VariantId("minhash-k-permutation-portable-splitmix"),
    DescriptorVersion::new(0, 1, 0),
    DefinitionSource::Paper {
        title: "On the resemblance and containment of documents",
        authors: "A. Z. Broder",
        year: 1997,
    },
);

/// A k-permutation `MinHash` sketch of a set of items.
///
/// See the [crate-level docs](crate) for the construction and the
/// [module-level docs](self) for the estimator's statistical properties.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinHashSketch {
    /// The `k` minimum-hash signatures, one per permutation. Empty
    /// permutation slots hold [`SENTINEL_SIGNATURE`].
    signatures: Vec<u64>,
    /// The caller-supplied seed. Retained so that two sketches can be
    /// compared for construction compatibility (same `k`, same `seed`)
    /// before the estimator is run.
    seed: u64,
}

impl MinHashSketch {
    /// Constructs a sketch of size `k` seeded with `seed`, consuming
    /// `items` once.
    ///
    /// The sketch treats the input as a *set*: duplicate grams do not
    /// affect the resulting signatures. Iteration order does not affect
    /// the signatures either — the reduction is commutative (`min`) and
    /// associative.
    ///
    /// # Panics
    ///
    /// Panics if `k == 0`; a zero-size sketch cannot estimate Jaccard.
    #[must_use]
    pub fn from_iter<G, I>(k: usize, seed: u64, items: I) -> Self
    where
        G: Hash,
        I: IntoIterator<Item = G>,
    {
        assert!(k > 0, "MinHash sketch size k must be > 0");

        // Pre-materialize the per-permutation seeds so the inner loop is
        // a straight-line reduction — this is the hot path for large
        // corpora.
        let mut perm_seeds: Vec<u64> = Vec::with_capacity(k);
        for i in 0..k {
            perm_seeds.push(permutation_seed(seed, i));
        }

        let mut signatures: Vec<u64> = vec![SENTINEL_SIGNATURE; k];

        for gram in items {
            // Base-hash the gram once; every permutation reuses this.
            let base = portable_hash(0, &gram);
            for (i, sig) in signatures.iter_mut().enumerate() {
                let h = permuted_hash(base, perm_seeds[i]);
                if h < *sig {
                    *sig = h;
                }
            }
        }

        Self { signatures, seed }
    }

    /// Returns the sketch size `k`.
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.signatures.len()
    }

    /// Returns `true` if every signature slot holds [`SENTINEL_SIGNATURE`],
    /// i.e. the sketch was constructed from an empty input.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.signatures.iter().all(|s| *s == SENTINEL_SIGNATURE)
    }

    /// Returns the sketch's stored seed. Two sketches with different
    /// seeds are *not* comparable — their permutations are drawn from
    /// different distributions.
    #[inline]
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the sketch's signatures as a raw slice.
    #[inline]
    #[must_use]
    pub fn signatures(&self) -> &[u64] {
        &self.signatures
    }

    /// The algorithm descriptor for the Jaccard estimator this sketch
    /// participates in. Present for parity with the other Comparand
    /// similarity kernels, and referenced by golden cases.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        MINHASH_JACCARD_DESCRIPTOR
    }

    /// Estimates the Jaccard similarity between this sketch and `other`.
    ///
    /// The estimate is the fraction of signature slots where the two
    /// sketches agree.
    ///
    /// # Empty-vs-empty
    ///
    /// Two empty sketches (both all-sentinel) return `1.0` bit-exactly,
    /// matching `comparand-set-similarity`'s crate-wide convention.
    ///
    /// # Panics
    ///
    /// Panics if the two sketches have different sizes or different
    /// seeds — the estimator is undefined in either case.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "sketch size k fits in u32 for every practical configuration; a hypothetical k > 2^53 could lose precision, but is impractical"
    )]
    pub fn estimated_jaccard(&self, other: &Self) -> f64 {
        assert_eq!(
            self.signatures.len(),
            other.signatures.len(),
            "MinHashSketch::estimated_jaccard requires equal sketch sizes"
        );
        assert_eq!(
            self.seed, other.seed,
            "MinHashSketch::estimated_jaccard requires equal seeds"
        );

        // Empty-vs-empty short-circuit: both all-sentinel sketches
        // trivially match, which yields `1.0` via the general path too,
        // but this branch documents the convention and avoids a loop
        // whose result is fixed by construction.
        if self.is_empty() && other.is_empty() {
            return 1.0;
        }

        let mut matches: usize = 0;
        for (a, b) in self.signatures.iter().zip(other.signatures.iter()) {
            if a == b {
                matches += 1;
            }
        }
        matches as f64 / self.signatures.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sketch_is_all_sentinel() {
        let s: MinHashSketch = MinHashSketch::from_iter::<u8, _>(8, 42, core::iter::empty());
        assert_eq!(s.size(), 8);
        assert!(s.is_empty());
        assert!(s.signatures().iter().all(|v| *v == SENTINEL_SIGNATURE));
    }

    #[test]
    fn empty_vs_empty_estimate_is_one_bit_exact() {
        let a: MinHashSketch = MinHashSketch::from_iter::<u8, _>(8, 42, core::iter::empty());
        let b = a.clone();
        assert_eq!(a.estimated_jaccard(&b).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn identical_inputs_produce_identical_sketches() {
        let a = MinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        let b = MinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        assert_eq!(a.signatures(), b.signatures());
        assert_eq!(a.estimated_jaccard(&b).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn sketch_is_set_invariant() {
        // Duplicates do not affect the sketch because the reduction is
        // `min` and hashing the same gram twice yields the same value.
        let a = MinHashSketch::from_iter(32, 7, [1u32, 2, 3]);
        let b = MinHashSketch::from_iter(32, 7, [1u32, 1, 2, 2, 3, 3, 3]);
        assert_eq!(a.signatures(), b.signatures());
    }

    #[test]
    fn sketch_is_permutation_invariant() {
        let a = MinHashSketch::from_iter(32, 7, [1u32, 2, 3, 4, 5]);
        let b = MinHashSketch::from_iter(32, 7, [5u32, 4, 3, 2, 1]);
        assert_eq!(a.signatures(), b.signatures());
    }

    #[test]
    fn estimator_is_symmetric() {
        let a = MinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        let b = MinHashSketch::from_iter(64, 7, [3u32, 4, 5, 6, 7]);
        assert_eq!(
            a.estimated_jaccard(&b).to_bits(),
            b.estimated_jaccard(&a).to_bits(),
        );
    }

    #[test]
    fn estimator_bounded_in_zero_to_one() {
        let a = MinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        let b = MinHashSketch::from_iter(64, 7, [10u32, 11, 12, 13, 14]);
        let j = a.estimated_jaccard(&b);
        assert!((0.0..=1.0).contains(&j), "out of range: {j}");
    }

    #[test]
    #[should_panic(expected = "requires equal sketch sizes")]
    fn different_sizes_panic() {
        let a = MinHashSketch::from_iter(32, 7, [1u32]);
        let b = MinHashSketch::from_iter(64, 7, [1u32]);
        let _ = a.estimated_jaccard(&b);
    }

    #[test]
    #[should_panic(expected = "requires equal seeds")]
    fn different_seeds_panic() {
        let a = MinHashSketch::from_iter(32, 7, [1u32]);
        let b = MinHashSketch::from_iter(32, 8, [1u32]);
        let _ = a.estimated_jaccard(&b);
    }

    #[test]
    #[should_panic(expected = "k must be > 0")]
    fn zero_k_panics() {
        let _: MinHashSketch = MinHashSketch::from_iter::<u8, _>(0, 42, core::iter::empty());
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = MinHashSketch::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Jaccard);
        assert_eq!(
            d.variant,
            VariantId("minhash-k-permutation-portable-splitmix")
        );
    }
}
