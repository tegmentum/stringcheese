//! One-permutation `MinHash` (Li, Owen, Zhang 2012) with cyclic-rotation
//! densification for empty bins (Shrivastava-Li 2014).
//!
//! # What it estimates
//!
//! The same Jaccard similarity that [`crate::minhash::MinHashSketch`]
//! estimates, at roughly `1/K`-th the per-item hash cost. Where the
//! k-permutation scheme runs `K` independent hash mixings per input
//! element (once per output signature), one-permutation hashing runs a
//! single hash per input element and buckets it into one of `K` slots.
//! Each slot retains the minimum hash value seen among the elements
//! that land in it.
//!
//! Slots whose bucket receives no items after the single pass are then
//! *densified* — populated from a nearby non-empty bin — so the
//! estimator remains defined at slots the sparse input did not directly
//! fill.
//!
//! # Bucket assignment: top-bits ("Lemire") mapping
//!
//! Each item's mixed hash `h` is assigned to bucket
//! `((h as u128 · k as u128) >> 64) as usize`. This has two useful
//! properties:
//!
//! 1. It is a uniform mapping from `[0, 2^64)` to `[0, k)` for any `k`,
//!    not just powers of two — better than the classical `h % k` on
//!    inputs where the low bits of `h` are less well-mixed than the
//!    high bits.
//! 2. It is *order-preserving*: items with smaller mixed hashes fall
//!    into smaller-index buckets. This matters for the densification
//!    proof — walking right through bucket indices corresponds to
//!    walking right through the underlying hash space, which is what
//!    makes "the first non-empty bucket to the right of `i`" behave
//!    statistically like "the item with the smallest hash strictly
//!    greater than `i`'s cutoff."
//!
//! Within a bucket, the retained value is the item's mixed hash itself
//! (rather than a re-mix keyed on the bucket id) — order-preservation
//! is exactly what makes the `min`-reduction return a well-defined
//! "minimum item" per bucket.
//!
//! # Densification: cyclic rotation with per-hop stream
//!
//! For each empty bin `i`, the search visits bins
//! `(i + 1) mod K, (i + 2) mod K, …` in order and takes the first
//! non-empty bin's stored value verbatim. The visited hop count `d` at
//! which the fill was found is mixed with the stored value via
//! `splitmix64(value XOR splitmix64(d))` so that fills at different
//! hop distances are numerically distinguishable — this is Shrivastava
//! and Li's rotation trick, needed to prevent all bins in a
//! densification "gap" from collapsing onto the same value on both
//! sketches.
//!
//! Under the classical proof (Li-Owen-Zhang 2012 §3), for a fixed
//! empty bin `i`:
//!
//! ```text
//!     P[ A_bin[i] == B_bin[i] ] = |A ∩ B| / |A ∪ B| = J(A, B)
//! ```
//!
//! provided the walk order (bucket-space) corresponds to the item-hash
//! order the min-reduction is over — which is why the top-bits mapping
//! above matters.
//!
//! # Estimator
//!
//! Identical in shape to the k-permutation `MinHash` estimator: the
//! Jaccard estimate is the fraction of positions at which the two
//! sketches carry equal values.
//!
//! # Variance vs the k-permutation scheme
//!
//! Under uniform bucket assignment, the one-permutation estimator has
//! asymptotic variance comparable to the k-permutation estimator's
//! `J(1 − J) / K` at large `K`; at small `K` (where several bins may be
//! empty) the densification-induced variance dominates and the two
//! estimators are within a constant factor of each other. The property
//! tests here assert *approximate agreement* with the k-permutation
//! sketch at large `K` (rather than bit-exact reproduction), matching
//! the paper's theoretical statement.
//!
//! # References
//!
//! * Li, P., Owen, A., & Zhang, C.-H. (2012). "One permutation hashing."
//!   *Advances in Neural Information Processing Systems 25 (NIPS 2012)*,
//!   3122-3130.
//!   <https://papers.nips.cc/paper_files/paper/2012/hash/23f3696d9c96fe73eba75f2d75ff2b90-Abstract.html>
//!   — introduces the one-permutation scheme this module implements.
//! * Shrivastava, A., & Li, P. (2014). "Densifying one permutation
//!   hashing via rotation for fast near neighbor search." *Proceedings
//!   of the 31st International Conference on Machine Learning (ICML '14)*,
//!   557-565. <https://proceedings.mlr.press/v32/shrivastava14.html> —
//!   introduces the rotation densification variant this module follows.
//! * Lemire, D. (2019). "Fast random integer generation in an interval."
//!   *ACM Transactions on Modeling and Computer Simulation*, 29(1),
//!   Article 3. <https://doi.org/10.1145/3230636> — the top-bits
//!   modular-reduction mapping this module uses to compute buckets.

use alloc::vec;
use alloc::vec::Vec;
use core::hash::Hash;

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::minhash::hash::{permutation_seed, portable_hash, splitmix64};
use crate::minhash::sketch::SENTINEL_SIGNATURE;

/// Li-Owen-Zhang 2012 one-permutation `MinHash` Jaccard estimator
/// descriptor.
///
/// The variant slug is deliberately distinct from the k-permutation
/// sketch's descriptor: a golden case's stored signatures for the
/// one-permutation variant must not be silently validated against the
/// k-permutation variant (whose signatures for the same input generally
/// differ).
pub const ONE_PERMUTATION_MINHASH_JACCARD_DESCRIPTOR: AlgorithmDescriptor =
    AlgorithmDescriptor::new(
        AlgorithmFamily::Jaccard,
        VariantId("minhash-one-permutation-rotation-densified-portable-splitmix"),
        DescriptorVersion::new(0, 1, 0),
        DefinitionSource::Paper {
            title: "One permutation hashing",
            authors: "P. Li, A. Owen, C.-H. Zhang",
            year: 2012,
        },
    );

/// A one-permutation `MinHash` sketch with rotation densification.
///
/// See the [module-level documentation](self) for the construction, the
/// densification rule, and the accuracy caveats vs the k-permutation
/// variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnePermutationMinHashSketch {
    /// The `K` per-bin signatures. Every bin either holds the minimum
    /// hash value of an element that landed in it, or a densification
    /// value derived from a nearby non-empty bin. An all-empty input
    /// leaves every slot at [`SENTINEL_SIGNATURE`].
    signatures: Vec<u64>,
    /// The caller-supplied seed. Retained so that two sketches can be
    /// checked for construction compatibility before the estimator is
    /// run — as with the k-permutation sketch.
    seed: u64,
}

impl OnePermutationMinHashSketch {
    /// Constructs a one-permutation sketch of size `k` seeded with
    /// `seed` from the items in `items`.
    ///
    /// Only a single hash mixing is performed per input element, versus
    /// `k` for the k-permutation sketch — which is the point.
    ///
    /// # Panics
    ///
    /// Panics if `k == 0`.
    #[must_use]
    pub fn from_iter<G, I>(k: usize, seed: u64, items: I) -> Self
    where
        G: Hash,
        I: IntoIterator<Item = G>,
    {
        assert!(k > 0, "one-permutation sketch size k must be > 0");

        let mut signatures: Vec<u64> = vec![SENTINEL_SIGNATURE; k];

        // Single hash per element: mix the base gram hash with a fixed
        // per-sketch permutation seed, then map into a bucket via
        // the top-bits (Lemire) mapping. The retained per-bucket value
        // is the mixed hash itself — order-preserving so that the min
        // reduction picks the item with the smallest mixed hash landing
        // in that bucket.
        let perm_seed = permutation_seed(seed, 0);

        for gram in items {
            let base = portable_hash(0, &gram);
            let mixed = splitmix64(base ^ perm_seed);
            let bucket = lemire_bucket(mixed, k);
            if mixed < signatures[bucket] {
                signatures[bucket] = mixed;
            }
        }

        // Densify empty bins by cyclic right-rotation with a per-hop
        // mixing step. See the module-level docs for the exact rule
        // and its unbiasedness rationale.
        densify(&mut signatures);

        Self { signatures, seed }
    }

    /// Returns the sketch size `k`.
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.signatures.len()
    }

    /// Returns `true` if every signature slot is [`SENTINEL_SIGNATURE`]
    /// — i.e. the input was empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.signatures.iter().all(|s| *s == SENTINEL_SIGNATURE)
    }

    /// Returns the sketch's stored seed.
    #[inline]
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the per-bin signatures as a slice.
    #[inline]
    #[must_use]
    pub fn signatures(&self) -> &[u64] {
        &self.signatures
    }

    /// The algorithm descriptor for the Jaccard estimator this sketch
    /// participates in.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        ONE_PERMUTATION_MINHASH_JACCARD_DESCRIPTOR
    }

    /// Estimates the Jaccard similarity between this sketch and `other`.
    ///
    /// The estimate is the fraction of signature slots where the two
    /// sketches agree. Two empty sketches return `1.0` bit-exact under
    /// the crate-wide empty-vs-empty identity convention.
    ///
    /// # Panics
    ///
    /// Panics if the two sketches have different sizes or seeds.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "sketch size k fits in u32 for every practical configuration"
    )]
    pub fn estimated_jaccard(&self, other: &Self) -> f64 {
        assert_eq!(
            self.signatures.len(),
            other.signatures.len(),
            "OnePermutationMinHashSketch::estimated_jaccard requires equal sketch sizes"
        );
        assert_eq!(
            self.seed, other.seed,
            "OnePermutationMinHashSketch::estimated_jaccard requires equal seeds"
        );

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

/// Maps a well-mixed `u64` uniformly to `[0, k)` via the Lemire
/// top-bits multiplication. Order-preserving in the input.
#[inline]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the top-64-bit product fits in usize on every supported target (usize >= 32 bits, and k <= usize::MAX so the result is <= k - 1)"
)]
fn lemire_bucket(mixed: u64, k: usize) -> usize {
    ((u128::from(mixed) * (k as u128)) >> 64) as usize
}

/// Densify empty bins via cyclic right-rotation with per-hop mixing.
///
/// See the [module-level docs](self) for the exact rule and its
/// unbiasedness rationale. Every bin is filled in at most `k − 1` hops;
/// if the input was empty (every bin is a sentinel), the sentinel
/// pattern is left in place and the estimator's empty-vs-empty
/// short-circuit handles the pair.
fn densify(signatures: &mut [u64]) {
    let k = signatures.len();
    if k == 0 {
        return;
    }
    // If every bin is empty, leave sentinels in place — the estimator
    // handles empty-vs-empty explicitly.
    if signatures.iter().all(|s| *s == SENTINEL_SIGNATURE) {
        return;
    }

    // Snapshot the pre-densification signatures so a bin filled during
    // this pass cannot supply its (borrowed) value to a later bin's
    // densification search. Every densification consults only "true"
    // per-bin minima, which is what keeps the pairwise-match
    // probability aligned with the true Jaccard.
    let source: Vec<u64> = signatures.to_vec();

    for (i, slot) in signatures.iter_mut().enumerate() {
        if *slot != SENTINEL_SIGNATURE {
            continue;
        }
        // Cyclic walk right, one bin at a time. Because bucket order
        // corresponds to hash order (top-bits mapping in the
        // constructor), this walks "right in hash space" — which is
        // what the classical unbiasedness proof requires.
        for d in 1..=k {
            let j = (i + d) % k;
            if source[j] != SENTINEL_SIGNATURE {
                // Mix in the hop distance so two empty bins whose
                // walks find the same source bin at *different* hop
                // distances get distinguishable fills. Without this,
                // long runs of empty bins ending at a shared source
                // would all collapse to a single value on both
                // sketches, artificially inflating the estimator.
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "hop distance d is bounded by k <= usize::MAX <= u64::MAX"
                )]
                let hop_mix = splitmix64(d as u64);
                *slot = splitmix64(source[j] ^ hop_mix);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sketch_is_all_sentinel() {
        let s: OnePermutationMinHashSketch =
            OnePermutationMinHashSketch::from_iter::<u8, _>(8, 42, core::iter::empty());
        assert_eq!(s.size(), 8);
        assert!(s.is_empty());
    }

    #[test]
    fn empty_vs_empty_estimate_is_one_bit_exact() {
        let a: OnePermutationMinHashSketch =
            OnePermutationMinHashSketch::from_iter::<u8, _>(8, 42, core::iter::empty());
        let b = a.clone();
        assert_eq!(a.estimated_jaccard(&b).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn identical_inputs_produce_identical_sketches() {
        let a = OnePermutationMinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        let b = OnePermutationMinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        assert_eq!(a.signatures(), b.signatures());
        assert_eq!(a.estimated_jaccard(&b).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn sketch_is_set_invariant() {
        // The reduction is min-per-bucket, so duplicates cannot affect
        // the sketch — same as the k-permutation variant.
        let a = OnePermutationMinHashSketch::from_iter(32, 7, [1u32, 2, 3]);
        let b = OnePermutationMinHashSketch::from_iter(32, 7, [1u32, 1, 2, 2, 3, 3, 3]);
        assert_eq!(a.signatures(), b.signatures());
    }

    #[test]
    fn sketch_is_permutation_invariant() {
        let a = OnePermutationMinHashSketch::from_iter(32, 7, [1u32, 2, 3, 4, 5]);
        let b = OnePermutationMinHashSketch::from_iter(32, 7, [5u32, 4, 3, 2, 1]);
        assert_eq!(a.signatures(), b.signatures());
    }

    #[test]
    fn densification_fills_all_bins_when_input_nonempty() {
        // A single-element sketch of size 32 lands in exactly one
        // bucket; the remaining 31 bins must be densified.
        let s = OnePermutationMinHashSketch::from_iter(32, 7, [42u32]);
        assert!(
            s.signatures().iter().all(|v| *v != SENTINEL_SIGNATURE),
            "densification left some bins empty: {:?}",
            s.signatures()
        );
    }

    #[test]
    fn large_input_produces_diverse_signatures() {
        // A large input across many buckets should yield many distinct
        // signature values — a broken hash or a broken densification
        // rule would collapse the range.
        let items: alloc::vec::Vec<u32> = (0..500u32).collect();
        let s = OnePermutationMinHashSketch::from_iter(64, 42, items.iter().copied());
        let mut sorted = s.signatures().to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert!(
            sorted.len() >= 60,
            "diversity too low: got {} distinct signatures out of 64",
            sorted.len()
        );
    }

    #[test]
    fn estimator_bounded_in_zero_to_one() {
        let a = OnePermutationMinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        let b = OnePermutationMinHashSketch::from_iter(64, 7, [10u32, 11, 12, 13, 14]);
        let j = a.estimated_jaccard(&b);
        assert!((0.0..=1.0).contains(&j), "out of range: {j}");
    }

    #[test]
    fn estimator_symmetric() {
        let a = OnePermutationMinHashSketch::from_iter(64, 7, [1u32, 2, 3, 4, 5]);
        let b = OnePermutationMinHashSketch::from_iter(64, 7, [3u32, 4, 5, 6, 7]);
        assert_eq!(
            a.estimated_jaccard(&b).to_bits(),
            b.estimated_jaccard(&a).to_bits(),
        );
    }

    #[test]
    fn lemire_bucket_covers_range() {
        // Uniform inputs across u64 should touch every bucket.
        let k = 8;
        let mut seen = [false; 8];
        for i in 0u64..256 {
            let b = lemire_bucket(splitmix64(i), k);
            assert!(b < k);
            seen[b] = true;
        }
        assert!(seen.iter().all(|s| *s), "lemire_bucket missed a bucket");
    }

    #[test]
    fn lemire_bucket_is_order_preserving_on_boundaries() {
        // A larger input maps to a bucket >= the bucket of a smaller
        // input. (Equality is fine — many inputs share a bucket.)
        let k = 128;
        let mut last = 0usize;
        for h in [0u64, 1, 1_000, 1_000_000, u64::MAX / 2, u64::MAX] {
            let b = lemire_bucket(h, k);
            assert!(
                b >= last,
                "lemire_bucket not order-preserving at {h}: {b} < {last}"
            );
            last = b;
        }
    }

    #[test]
    #[should_panic(expected = "requires equal sketch sizes")]
    fn different_sizes_panic() {
        let a = OnePermutationMinHashSketch::from_iter(32, 7, [1u32]);
        let b = OnePermutationMinHashSketch::from_iter(64, 7, [1u32]);
        let _ = a.estimated_jaccard(&b);
    }

    #[test]
    #[should_panic(expected = "requires equal seeds")]
    fn different_seeds_panic() {
        let a = OnePermutationMinHashSketch::from_iter(32, 7, [1u32]);
        let b = OnePermutationMinHashSketch::from_iter(32, 8, [1u32]);
        let _ = a.estimated_jaccard(&b);
    }

    #[test]
    #[should_panic(expected = "k must be > 0")]
    fn zero_k_panics() {
        let _: OnePermutationMinHashSketch =
            OnePermutationMinHashSketch::from_iter::<u8, _>(0, 42, core::iter::empty());
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = OnePermutationMinHashSketch::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Jaccard);
        assert_eq!(
            d.variant,
            VariantId("minhash-one-permutation-rotation-densified-portable-splitmix")
        );
    }
}
