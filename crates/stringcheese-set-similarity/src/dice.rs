//! The Sørensen–Dice coefficient over set and multiset gram representations.
//!
//! Introduced by Dice (1945) and independently by Sørensen (1948) in the
//! context of ecological association between species; see the crate-level
//! `References` section for full citations.
//!
//! # Formula
//!
//! On sets `A`, `B` the Dice coefficient is
//!
//! ```text
//!     dice(A, B) = 2 * |A ∩ B| / (|A| + |B|)
//! ```
//!
//! and always lies in the closed interval `[0.0, 1.0]`. On multisets the
//! same formula applies with `|A|` and `|B|` interpreted as *total counts*
//! (sum of per-gram multiplicities) and `|A ∩ B|` as the per-gram minimum
//! count summed across grams — the classical multiset generalization.
//!
//! # Class
//!
//! Dice is a bounded, symmetric [`SimilarityMetric`] with identity of
//! indiscernibles. It is **not** a metric: the "distance" `1 - dice` is a
//! semimetric (fails the triangle inequality). Consumers that need a
//! metric distance from a set-similarity family should reach for
//! [`crate::jaccard::JaccardOverSet`] instead — its `1 - similarity`
//! distance IS a metric.
//!
//! # Boundary conventions
//!
//! * `dice(∅, ∅) = 1.0` — empty-vs-empty is treated as identity, matching
//!   the crate-wide convention and the behavior of every other similarity
//!   in this crate.
//! * `dice(∅, B) = 0.0` for `B ≠ ∅`, and symmetrically. Unambiguous in
//!   every published formulation.
//!
//! [`SimilarityMetric`]: stringcheese_core::SimilarityMetric

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass,
    MetricProperties, NormalizedSimilarity, Similarity, SimilarityMetric, VariantId,
};
use stringcheese_ngram::{GramMultiSet, GramSet};

use crate::shared::{multiset_min_intersection, set_intersection_size};

/// The Dice coefficient over deduplicated [`GramSet`]s.
///
/// Introduced by Dice (1945); the crate-level `References` section carries
/// the full citation, along with the parallel Sørensen (1948) reference for
/// the Sørensen-Dice pairing.
///
/// A zero-size unit struct implementing [`SimilarityMetric<GramSet<G>>`]
/// for any `G: Ord`. Construct it as `DiceOverSet` and share across
/// threads; no per-call state is held.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DiceOverSet;

/// The Dice coefficient over multiplicity-preserving [`GramMultiSet`]s.
///
/// The multiset formulation uses total counts and per-gram min-counts —
/// the distinct-gram set produces a different value for inputs whose gram
/// distributions differ, and neither answer is silently substitutable for
/// the other.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DiceOverMultiSet;

// The two variants share a source-of-truth citation: Dice's 1945 paper.
// A `const` here keeps the string interned into a single static slice.
const DICE_1945: DefinitionSource = DefinitionSource::Paper {
    title: "Measures of the amount of ecologic association between species",
    authors: "Lee R. Dice",
    year: 1945,
};

impl DiceOverSet {
    /// The algorithm descriptor for the set-based Dice variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Dice,
        VariantId("set-classic"),
        DescriptorVersion::new(0, 1, 0),
        DICE_1945,
    );

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes Dice between two [`GramSet`]s, returning the range-checked
    /// [`NormalizedSimilarity`] wrapper.
    ///
    /// See the module documentation for the boundary conventions.
    #[inline]
    #[must_use]
    pub fn similarity_normalized<G: Ord>(
        &self,
        left: &GramSet<G>,
        right: &GramSet<G>,
    ) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(dice_set(left, right))
    }
}

impl DiceOverMultiSet {
    /// The algorithm descriptor for the multiset-based Dice variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Dice,
        VariantId("multiset"),
        DescriptorVersion::new(0, 1, 0),
        DICE_1945,
    );

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes Dice between two [`GramMultiSet`]s, returning the
    /// range-checked [`NormalizedSimilarity`] wrapper.
    #[inline]
    #[must_use]
    pub fn similarity_normalized<G: Ord>(
        &self,
        left: &GramMultiSet<G>,
        right: &GramMultiSet<G>,
    ) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(dice_multiset(left, right))
    }
}

impl<G: Ord> SimilarityMetric<GramSet<G>> for DiceOverSet {
    type Output = f64;

    #[inline]
    fn similarity(&self, left: &GramSet<G>, right: &GramSet<G>) -> Similarity<Self::Output> {
        Similarity::new(dice_set(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        // Bounded similarity with identity of indiscernibles. NOT a metric:
        // the triangle inequality on `1 - dice` fails; we spell out the
        // axioms individually rather than picking a preset that would
        // misdescribe the algorithm.
        MetricProperties {
            symmetric: true,
            identity_of_indiscernibles: true,
            triangle_inequality: false,
            non_negative: true,
            normalized: true,
        }
    }

    #[inline]
    fn class(&self) -> MetricClass {
        MetricClass::Similarity
    }
}

impl<G: Ord> SimilarityMetric<GramMultiSet<G>> for DiceOverMultiSet {
    type Output = f64;

    #[inline]
    fn similarity(
        &self,
        left: &GramMultiSet<G>,
        right: &GramMultiSet<G>,
    ) -> Similarity<Self::Output> {
        Similarity::new(dice_multiset(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        MetricProperties {
            symmetric: true,
            identity_of_indiscernibles: true,
            triangle_inequality: false,
            non_negative: true,
            normalized: true,
        }
    }

    #[inline]
    fn class(&self) -> MetricClass {
        MetricClass::Similarity
    }
}

/// The Dice-over-set kernel. Returns a raw `f64` in `[0.0, 1.0]`.
///
/// Split out from the trait implementation so the kernel can be re-called
/// from property tests without going through the `Similarity<f64>`
/// wrapper.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "gram-set sizes are `usize` and only lose precision above 2^53 elements; realistic n-gram set sizes are many orders of magnitude below that"
)]
pub(crate) fn dice_set<G: Ord>(a: &GramSet<G>, b: &GramSet<G>) -> f64 {
    let la = a.len();
    let lb = b.len();
    if la == 0 && lb == 0 {
        return 1.0;
    }
    if la == 0 || lb == 0 {
        return 0.0;
    }
    let inter = set_intersection_size(a, b);
    2.0 * inter as f64 / (la + lb) as f64
}

/// The Dice-over-multiset kernel. Returns a raw `f64` in `[0.0, 1.0]`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "multiset total counts are `u64` and only lose precision above 2^53; realistic gram-count totals are many orders of magnitude below that"
)]
pub(crate) fn dice_multiset<G: Ord>(a: &GramMultiSet<G>, b: &GramMultiSet<G>) -> f64 {
    let ta = a.total_count();
    let tb = b.total_count();
    if ta == 0 && tb == 0 {
        return 1.0;
    }
    if ta == 0 || tb == 0 {
        return 0.0;
    }
    let inter = multiset_min_intersection(a, b);
    2.0 * inter as f64 / (ta + tb) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn set_of<const N: usize>(items: [char; N]) -> GramSet<Vec<char>> {
        items.iter().map(|c| vec![*c]).collect()
    }

    fn multiset_of(items: &[char]) -> GramMultiSet<char> {
        let mut ms = GramMultiSet::new();
        for c in items {
            ms.add(*c);
        }
        ms
    }

    #[test]
    fn set_descriptor_matches_family_and_variant() {
        let d = DiceOverSet::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Dice);
        assert_eq!(d.variant, VariantId("set-classic"));
    }

    #[test]
    fn multiset_descriptor_matches_family_and_variant() {
        let d = DiceOverMultiSet::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Dice);
        assert_eq!(d.variant, VariantId("multiset"));
    }

    #[test]
    fn set_empty_empty_is_one_bit_exact() {
        let a: GramSet<Vec<char>> = GramSet::new();
        let s = dice_set(&a, &a);
        assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn set_one_empty_is_zero_bit_exact() {
        let a: GramSet<Vec<char>> = set_of(['a', 'b']);
        let b: GramSet<Vec<char>> = GramSet::new();
        assert_eq!(dice_set(&a, &b).to_bits(), 0.0_f64.to_bits());
        assert_eq!(dice_set(&b, &a).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn set_identical_is_one_bit_exact() {
        let a: GramSet<Vec<char>> = set_of(['a', 'b', 'c']);
        assert_eq!(dice_set(&a, &a).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn set_disjoint_is_zero_bit_exact() {
        let a: GramSet<Vec<char>> = set_of(['a', 'b', 'c']);
        let b: GramSet<Vec<char>> = set_of(['x', 'y', 'z']);
        assert_eq!(dice_set(&a, &b).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn set_partial_overlap_matches_hand_computed() {
        // {a, b, c} vs {b, c, d}: intersection = 2, denom = 6, dice = 4/6 = 2/3.
        let a: GramSet<Vec<char>> = set_of(['a', 'b', 'c']);
        let b: GramSet<Vec<char>> = set_of(['b', 'c', 'd']);
        assert!((dice_set(&a, &b) - 2.0_f64 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn multiset_repeats_lift_similarity() {
        // "aab" vs "abb": total counts 3 and 3; min-counts a=1, b=1 → inter = 2.
        // dice = 2*2 / (3+3) = 2/3.
        let a = multiset_of(&['a', 'a', 'b']);
        let b = multiset_of(&['a', 'b', 'b']);
        assert!((dice_multiset(&a, &b) - 2.0_f64 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn multiset_identical_repeats_is_one() {
        let a = multiset_of(&['x', 'x', 'y', 'y', 'y']);
        assert_eq!(dice_multiset(&a, &a).to_bits(), 1.0_f64.to_bits());
    }
}
