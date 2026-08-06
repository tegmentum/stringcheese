//! The Szymkiewicz–Simpson overlap coefficient.
//!
//! Named for Szymkiewicz's 1934 statistical contribution to floristic
//! geography and Simpson's 1943 reformulation in the context of mammalian
//! biogeography; both citations are in the crate-level `References`
//! section.
//!
//! # Formula
//!
//! On sets `A`, `B`:
//!
//! ```text
//!     overlap(A, B) = |A ∩ B| / min(|A|, |B|)
//! ```
//!
//! # Class
//!
//! Overlap is **not** a metric — and, importantly, it also fails identity
//! of indiscernibles. For any strict superset relation `A ⊂ B`,
//! `overlap(A, B) = |A| / |A| = 1.0`, even though `A ≠ B`. That is the
//! trip-wire property to watch when choosing this coefficient: two inputs
//! can be "maximally similar" without being equal.
//!
//! Overlap remains a bounded, symmetric [`SimilarityMetric`] with a
//! well-defined complement `1 - overlap` in `[0.0, 1.0]`, but the
//! complement is not a metric (it fails identity-of-indiscernibles and
//! the triangle inequality both).
//!
//! # When to use it
//!
//! Overlap is the right choice when the caller wants a *containment*
//! signal: "how much of the smaller side is present in the larger?" The
//! subset-yields-one behavior is the intended feature, not a bug. When
//! the caller wants distinguishing power between subset and equality,
//! use [`crate::set_similarity::jaccard::JaccardOverSet`] instead — its `subset vs equal`
//! distinction is preserved.
//!
//! # Boundary conventions
//!
//! * `overlap(∅, ∅) = 1.0` — the crate-wide identity convention.
//! * `overlap(∅, B) = 0.0` for `B ≠ ∅` (denominator would be zero
//!   otherwise; we defer to the "one-empty is zero" convention every
//!   other similarity in this crate uses).
//!
//! [`SimilarityMetric`]: stringcheese_core::SimilarityMetric

use crate::ngram::GramSet;
use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass,
    MetricProperties, NormalizedSimilarity, Similarity, SimilarityMetric, VariantId,
};

use crate::set_similarity::shared::set_intersection_size;

/// The Szymkiewicz–Simpson overlap coefficient over [`GramSet`]s.
///
/// Named for Szymkiewicz (1934) and Simpson (1943); see the crate-level
/// `References` section for the full citations.
///
/// A zero-size unit struct implementing [`SimilarityMetric<GramSet<G>>`]
/// for any `G: Ord`. The coefficient is symmetric and bounded to
/// `[0.0, 1.0]`, but does not satisfy identity of indiscernibles (see the
/// module documentation for the subset trip-wire).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Overlap;

impl Overlap {
    /// The algorithm descriptor for the classical Szymkiewicz–Simpson
    /// coefficient. No multiset variant ships in this initial release —
    /// the multiset generalization is not universally agreed on in the
    /// literature and is deferred until a caller has a concrete need.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::OverlapCoefficient,
        VariantId("set-classic"),
        DescriptorVersion::new(0, 1, 0),
        // Szymkiewicz's 1934 formulation and Simpson's 1943 reformulation
        // are both cited here; we pin to Simpson because his paper is the
        // one most English-language references trace this coefficient to.
        DefinitionSource::Paper {
            title: "Mammals and the Nature of Continents",
            authors: "George Gaylord Simpson",
            year: 1943,
        },
    );

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes the overlap coefficient between two [`GramSet`]s,
    /// returning the range-checked [`NormalizedSimilarity`] wrapper.
    #[inline]
    #[must_use]
    pub fn similarity_normalized<G: Ord>(
        &self,
        left: &GramSet<G>,
        right: &GramSet<G>,
    ) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(overlap_set(left, right))
    }
}

impl<G: Ord> SimilarityMetric<GramSet<G>> for Overlap {
    type Output = f64;

    #[inline]
    fn similarity(&self, left: &GramSet<G>, right: &GramSet<G>) -> Similarity<Self::Output> {
        Similarity::new(overlap_set(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        // Overlap is symmetric, non-negative, and normalized, but does NOT
        // satisfy identity of indiscernibles. See the module docs for the
        // subset trip-wire.
        MetricProperties {
            symmetric: true,
            identity_of_indiscernibles: false,
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

/// The overlap-over-set kernel. Returns a raw `f64` in `[0.0, 1.0]`.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "gram-set sizes are `usize` and only lose precision above 2^53 elements; realistic n-gram set sizes are many orders of magnitude below that"
)]
pub(crate) fn overlap_set<G: Ord>(a: &GramSet<G>, b: &GramSet<G>) -> f64 {
    let la = a.len();
    let lb = b.len();
    if la == 0 && lb == 0 {
        return 1.0;
    }
    if la == 0 || lb == 0 {
        return 0.0;
    }
    let inter = set_intersection_size(a, b);
    let denom = la.min(lb);
    inter as f64 / denom as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn set_of<const N: usize>(items: [char; N]) -> GramSet<Vec<char>> {
        items.iter().map(|c| vec![*c]).collect()
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = Overlap::descriptor();
        assert_eq!(d.family, AlgorithmFamily::OverlapCoefficient);
        assert_eq!(d.variant, VariantId("set-classic"));
    }

    #[test]
    fn empty_empty_is_one_bit_exact() {
        let a: GramSet<Vec<char>> = GramSet::new();
        assert_eq!(overlap_set(&a, &a).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn subset_yields_one_bit_exact() {
        // {a, b} vs {a, b, c, d}: inter = 2, min = 2, overlap = 1.0.
        let a: GramSet<Vec<char>> = set_of(['a', 'b']);
        let b: GramSet<Vec<char>> = set_of(['a', 'b', 'c', 'd']);
        assert_eq!(overlap_set(&a, &b).to_bits(), 1.0_f64.to_bits());
        // Trip-wire: a ≠ b, yet overlap(a, b) = 1.0 — identity of
        // indiscernibles fails.
        assert_ne!(a, b);
    }

    #[test]
    fn partial_overlap_matches_hand_computed() {
        // {a, b, c} vs {b, c, d}: inter = 2, min(3, 3) = 3, overlap = 2/3.
        let a: GramSet<Vec<char>> = set_of(['a', 'b', 'c']);
        let b: GramSet<Vec<char>> = set_of(['b', 'c', 'd']);
        assert!((overlap_set(&a, &b) - 2.0_f64 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn disjoint_is_zero() {
        let a: GramSet<Vec<char>> = set_of(['a', 'b']);
        let b: GramSet<Vec<char>> = set_of(['x', 'y']);
        assert_eq!(overlap_set(&a, &b).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn properties_declares_missing_identity_of_indiscernibles() {
        let alg = Overlap;
        let p = <Overlap as SimilarityMetric<GramSet<u8>>>::properties(&alg);
        assert!(p.symmetric);
        assert!(!p.identity_of_indiscernibles);
        assert!(!p.triangle_inequality);
        assert!(p.non_negative);
        assert!(p.normalized);
    }
}
