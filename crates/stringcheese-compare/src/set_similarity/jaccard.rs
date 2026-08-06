//! The Jaccard coefficient over set and multiset gram representations.
//!
//! Introduced by Paul Jaccard (1912) in a study of alpine flora
//! distribution; the full citation lives in the crate-level `References`
//! section.
//!
//! # Formula
//!
//! On sets `A`, `B` the Jaccard coefficient is
//!
//! ```text
//!     jaccard(A, B) = |A ∩ B| / |A ∪ B|
//! ```
//!
//! and always lies in `[0.0, 1.0]`. On multisets StringCheese uses the
//! *weighted* generalization (Tanimoto / Ruzicka),
//!
//! ```text
//!     weighted_jaccard(A, B) = Σ min(a_i, b_i) / Σ max(a_i, b_i)
//! ```
//!
//! which reduces to the set formula when every multiplicity is `0` or `1`.
//! The distinct-support set produces a different value for inputs whose
//! gram distributions differ, and neither answer is silently substitutable
//! for the other.
//!
//! # Class
//!
//! Jaccard is a bounded, symmetric [`SimilarityMetric`] with identity of
//! indiscernibles. The bare similarity does not satisfy the triangle
//! inequality (bounded similarities generally do not), so the
//! [`SimilarityMetric::class`] the impl advertises is
//! [`MetricClass::Similarity`]. Its **distance form**
//! `1 - jaccard` IS a true metric — a well-known result that is the
//! reason Jaccard shows up in BK-tree-style index structures. Consumers
//! wanting the metric distance should call [`JaccardOverSet::distance`]
//! (or [`JaccardOverMultiSet::distance`]), which returns a
//! [`NormalizedDistance`] and satisfies all four metric axioms on
//! non-negative-count multisets.
//!
//! # Boundary conventions
//!
//! * `jaccard(∅, ∅) = 1.0` — empty-vs-empty treated as identity, so that
//!   the "empty pair" case falls under identity of indiscernibles rather
//!   than being undefined (`0 / 0`).
//! * `jaccard(∅, B) = 0.0` for `B ≠ ∅`. Unambiguous.
//! * Distance follows: `distance(∅, ∅) = 0.0`; `distance(∅, B) = 1.0`.
//!
//! [`MetricClass::Similarity`]: stringcheese_core::MetricClass::Similarity
//! [`NormalizedDistance`]: stringcheese_core::NormalizedDistance
//! [`SimilarityMetric`]: stringcheese_core::SimilarityMetric

use crate::ngram::{GramMultiSet, GramSet};
use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass,
    MetricProperties, NormalizedDistance, NormalizedSimilarity, Similarity, SimilarityMetric,
    VariantId,
};

use crate::set_similarity::shared::{multiset_min_intersection, set_intersection_size};

/// The Jaccard coefficient over deduplicated [`GramSet`]s.
///
/// Introduced by Paul Jaccard (1912); see the crate-level `References`
/// section for the full citation.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct JaccardOverSet;

/// The Jaccard coefficient over multiplicity-preserving [`GramMultiSet`]s
/// — the "weighted Jaccard" / Ruzicka similarity.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct JaccardOverMultiSet;

const JACCARD_1912: DefinitionSource = DefinitionSource::Paper {
    title: "The Distribution of the Flora in the Alpine Zone",
    authors: "Paul Jaccard",
    year: 1912,
};

impl JaccardOverSet {
    /// The algorithm descriptor for the set-based Jaccard variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Jaccard,
        VariantId("set-classic"),
        DescriptorVersion::new(0, 1, 0),
        JACCARD_1912,
    );

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes Jaccard between two [`GramSet`]s, returning the range-checked
    /// [`NormalizedSimilarity`] wrapper.
    #[inline]
    #[must_use]
    pub fn similarity_normalized<G: Ord>(
        &self,
        left: &GramSet<G>,
        right: &GramSet<G>,
    ) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(jaccard_set(left, right))
    }

    /// Returns the Jaccard distance `1 - jaccard(left, right)`.
    ///
    /// This distance IS a true metric (well-known result), and is the
    /// canonical way to expose Jaccard to a BK-tree consumer that requires
    /// a real metric distance rather than a similarity.
    #[inline]
    #[must_use]
    pub fn distance<G: Ord>(&self, left: &GramSet<G>, right: &GramSet<G>) -> NormalizedDistance {
        NormalizedDistance::new_unchecked(1.0 - jaccard_set(left, right))
    }
}

impl JaccardOverMultiSet {
    /// The algorithm descriptor for the weighted-multiset Jaccard variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Jaccard,
        VariantId("weighted-multiset"),
        DescriptorVersion::new(0, 1, 0),
        JACCARD_1912,
    );

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes weighted Jaccard between two [`GramMultiSet`]s, returning
    /// the range-checked [`NormalizedSimilarity`] wrapper.
    #[inline]
    #[must_use]
    pub fn similarity_normalized<G: Ord>(
        &self,
        left: &GramMultiSet<G>,
        right: &GramMultiSet<G>,
    ) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(jaccard_multiset(left, right))
    }

    /// Returns the weighted-Jaccard distance
    /// `1 - weighted_jaccard(left, right)`.
    ///
    /// On non-negative multiset counts this distance is a metric — the
    /// classical extension of the Jaccard-distance metric-hood result.
    #[inline]
    #[must_use]
    pub fn distance<G: Ord>(
        &self,
        left: &GramMultiSet<G>,
        right: &GramMultiSet<G>,
    ) -> NormalizedDistance {
        NormalizedDistance::new_unchecked(1.0 - jaccard_multiset(left, right))
    }
}

impl<G: Ord> SimilarityMetric<GramSet<G>> for JaccardOverSet {
    type Output = f64;

    #[inline]
    fn similarity(&self, left: &GramSet<G>, right: &GramSet<G>) -> Similarity<Self::Output> {
        Similarity::new(jaccard_set(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        // The similarity itself is bounded and does not satisfy the
        // triangle inequality (bounded similarities generally do not). The
        // triangle inequality holds on the DISTANCE form; consumers that
        // want a metric should call `JaccardOverSet::distance`.
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

impl<G: Ord> SimilarityMetric<GramMultiSet<G>> for JaccardOverMultiSet {
    type Output = f64;

    #[inline]
    fn similarity(
        &self,
        left: &GramMultiSet<G>,
        right: &GramMultiSet<G>,
    ) -> Similarity<Self::Output> {
        Similarity::new(jaccard_multiset(left, right))
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

/// The Jaccard-over-set kernel. Returns a raw `f64` in `[0.0, 1.0]`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "gram-set sizes are `usize` and only lose precision above 2^53 elements; realistic n-gram set sizes are many orders of magnitude below that"
)]
pub(crate) fn jaccard_set<G: Ord>(a: &GramSet<G>, b: &GramSet<G>) -> f64 {
    let la = a.len();
    let lb = b.len();
    if la == 0 && lb == 0 {
        return 1.0;
    }
    let inter = set_intersection_size(a, b);
    let union_size = la + lb - inter;
    if union_size == 0 {
        // Only reachable if la = lb = 0, which the guard above already
        // handled; keep the branch as a defensive `1.0` so any accidental
        // future reshuffle does not divide by zero.
        return 1.0;
    }
    inter as f64 / union_size as f64
}

/// The weighted-Jaccard-over-multiset kernel. Returns a raw `f64` in
/// `[0.0, 1.0]`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "multiset total counts are `u64` and only lose precision above 2^53; realistic gram-count totals are many orders of magnitude below that"
)]
pub(crate) fn jaccard_multiset<G: Ord>(a: &GramMultiSet<G>, b: &GramMultiSet<G>) -> f64 {
    let ta = a.total_count();
    let tb = b.total_count();
    if ta == 0 && tb == 0 {
        return 1.0;
    }
    let inter = multiset_min_intersection(a, b);
    // Σ max = Σ a + Σ b - Σ min — an algebraic identity that lets us avoid
    // materializing a second per-gram walk.
    let union_sum = ta + tb - inter;
    if union_sum == 0 {
        return 1.0;
    }
    inter as f64 / union_sum as f64
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
        let d = JaccardOverSet::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Jaccard);
        assert_eq!(d.variant, VariantId("set-classic"));
    }

    #[test]
    fn multiset_descriptor_matches_family_and_variant() {
        let d = JaccardOverMultiSet::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Jaccard);
        assert_eq!(d.variant, VariantId("weighted-multiset"));
    }

    #[test]
    fn set_empty_empty_is_one_bit_exact() {
        let a: GramSet<Vec<char>> = GramSet::new();
        assert_eq!(jaccard_set(&a, &a).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn set_partial_overlap_matches_hand_computed() {
        // {a, b, c} vs {b, c, d}: inter = 2, union = 4, jac = 1/2.
        let a: GramSet<Vec<char>> = set_of(['a', 'b', 'c']);
        let b: GramSet<Vec<char>> = set_of(['b', 'c', 'd']);
        assert_eq!(jaccard_set(&a, &b).to_bits(), 0.5_f64.to_bits());
    }

    #[test]
    fn set_subset_yields_half() {
        // {a, b} vs {a, b, c, d}: inter = 2, union = 4, jac = 1/2.
        let a: GramSet<Vec<char>> = set_of(['a', 'b']);
        let b: GramSet<Vec<char>> = set_of(['a', 'b', 'c', 'd']);
        assert_eq!(jaccard_set(&a, &b).to_bits(), 0.5_f64.to_bits());
    }

    #[test]
    fn set_disjoint_is_zero() {
        let a: GramSet<Vec<char>> = set_of(['a', 'b']);
        let b: GramSet<Vec<char>> = set_of(['c', 'd']);
        assert_eq!(jaccard_set(&a, &b).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn set_distance_is_complement_of_similarity() {
        let a: GramSet<Vec<char>> = set_of(['a', 'b', 'c']);
        let b: GramSet<Vec<char>> = set_of(['b', 'c', 'd']);
        let alg = JaccardOverSet;
        let sim = jaccard_set(&a, &b);
        let d = alg.distance(&a, &b).into_inner();
        assert!((d - (1.0 - sim)).abs() < 1e-15);
    }

    #[test]
    fn multiset_min_max_ratio_matches_hand_computation() {
        // "aab" vs "abb": min = a=1,b=1 → sum 2; max = a=2,b=2 → sum 4.
        // weighted-jac = 2/4 = 0.5.
        let a = multiset_of(&['a', 'a', 'b']);
        let b = multiset_of(&['a', 'b', 'b']);
        assert_eq!(jaccard_multiset(&a, &b).to_bits(), 0.5_f64.to_bits());
    }

    #[test]
    fn multiset_differs_from_set() {
        // Set: {a, a, b} vs {a, b, b} → distinct sets both {a, b} → 1.0.
        // Multiset: 0.5. This is the whole point of having both variants.
        let a_set: GramSet<char> = ['a', 'b'].iter().copied().collect();
        let b_set: GramSet<char> = ['a', 'b'].iter().copied().collect();
        assert_eq!(jaccard_set(&a_set, &b_set).to_bits(), 1.0_f64.to_bits());

        let a_ms = multiset_of(&['a', 'a', 'b']);
        let b_ms = multiset_of(&['a', 'b', 'b']);
        assert_eq!(jaccard_multiset(&a_ms, &b_ms).to_bits(), 0.5_f64.to_bits());
    }
}
