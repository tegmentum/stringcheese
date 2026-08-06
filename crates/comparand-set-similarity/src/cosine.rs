//! Cosine similarity over the [`GramVector`] weighted-vector representation.
//!
//! # Formula
//!
//! Given two vectors `A`, `B` over the same gram support:
//!
//! ```text
//!     cosine(A, B) = dot(A, B) / (||A||_2 * ||B||_2)
//! ```
//!
//! The general cosine formula lies in `[-1, 1]`. Comparand pins the input
//! type to [`GramVector`], and the [`GramVector`] constructor
//! [`from_generator_counts`] produces vectors with non-negative weights
//! (raw counts). On non-negative vectors the dot product is non-negative
//! and cosine is bounded to `[0, 1]` — which is what the returned
//! [`Similarity<f64>`] carries.
//!
//! Callers that populate a [`GramVector`] by hand with signed weights
//! (e.g. mean-centered TF weights) can produce values below zero. The
//! [`Cosine`] variant here is documented as `"euclidean-nonneg"` to make
//! the non-negativity assumption explicit; a signed-weight variant would
//! land as a sibling with a distinct descriptor.
//!
//! # Class
//!
//! Cosine similarity on non-negative vectors is a bounded, symmetric
//! [`SimilarityMetric`]. It is **not** a metric — the standard
//! `1 - cosine` distance is not a metric (fails the triangle inequality).
//! A true metric derived from cosine is the *angular distance*
//! `arccos(cosine) / π`, which is not what [`Cosine::distance`] returns.
//! See [`Cosine::distance`]'s own documentation for the honest label.
//!
//! # Boundary conventions
//!
//! * Two empty vectors: `cosine(∅, ∅) = 1.0`, matching the crate-wide
//!   identity convention. Documented on the kernel itself so a reader
//!   coming to Cosine directly does not need the crate-level rationale.
//! * One empty, one non-empty: `cosine = 0.0`. Following the crate-wide
//!   "one-empty-is-zero" convention, and avoiding the `0 / 0` from a
//!   zero-norm side.
//! * Two zero-norm (all weights zero) vectors that are not the empty
//!   vector: same as empty-empty, `cosine = 1.0`. Same rationale: the
//!   norms are zero on both sides, so `0 / (0 * 0)` is undefined, and we
//!   default to identity.
//!
//! [`from_generator_counts`]: comparand_ngram::GramVector::from_generator_counts
//! [`SimilarityMetric`]: comparand_core::SimilarityMetric

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass,
    MetricProperties, NormalizedDistance, NormalizedSimilarity, Similarity, SimilarityMetric,
    VariantId,
};
use comparand_ngram::GramVector;

/// Cosine similarity over [`GramVector`]s.
///
/// A zero-size unit struct implementing [`SimilarityMetric<GramVector<G>>`]
/// for any `G: Ord + Clone`. Output is a [`Similarity<f64>`] in
/// `[0.0, 1.0]` under the crate's non-negative-weight assumption.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cosine;

impl Cosine {
    /// The algorithm descriptor.
    ///
    /// The variant slug `"euclidean-nonneg"` records the definitional
    /// choice: standard L2-norm cosine, under the assumption that inputs
    /// have non-negative weights. Any signed-weight variant would land
    /// under a sibling variant slug.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Cosine,
        VariantId("euclidean-nonneg"),
        DescriptorVersion::new(0, 1, 0),
        // The information-retrieval literature codified cosine similarity
        // over term-frequency vectors; Salton & McGill's 1983 textbook is
        // the canonical citation for the formulation we use.
        DefinitionSource::Paper {
            title: "Introduction to Modern Information Retrieval",
            authors: "Gerard Salton and Michael J. McGill",
            year: 1983,
        },
    );

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes the cosine similarity between two [`GramVector`]s,
    /// returning the range-checked [`NormalizedSimilarity`] wrapper.
    ///
    /// See the module documentation for the empty-vector and zero-norm
    /// conventions.
    #[inline]
    #[must_use]
    pub fn similarity_normalized<G: Ord + Clone>(
        &self,
        left: &GramVector<G>,
        right: &GramVector<G>,
    ) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(cosine(left, right))
    }

    /// Returns the arithmetic complement `1 - cosine(left, right)` as a
    /// [`NormalizedDistance`].
    ///
    /// **This is not a true metric.** The `1 - cosine` construction fails
    /// the triangle inequality — the correct metric derived from cosine
    /// is the *angular distance* `arccos(cosine) / π`, which this method
    /// does not compute. The complement is provided because it is the
    /// simplest bounded distance-shaped companion and is what most
    /// consumers of "cosine distance" from the surrounding literature
    /// actually mean, but it is important that consumers who require a
    /// true metric distance not silently mistake this for one. A future
    /// angular-distance variant is planned.
    #[inline]
    #[must_use]
    pub fn distance<G: Ord + Clone>(
        &self,
        left: &GramVector<G>,
        right: &GramVector<G>,
    ) -> NormalizedDistance {
        NormalizedDistance::new_unchecked(1.0 - cosine(left, right))
    }
}

impl<G: Ord + Clone> SimilarityMetric<GramVector<G>> for Cosine {
    type Output = f64;

    #[inline]
    fn similarity(&self, left: &GramVector<G>, right: &GramVector<G>) -> Similarity<Self::Output> {
        Similarity::new(cosine(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        // Symmetric, non-negative on non-negative inputs, normalized to
        // [0, 1]. Not a metric (1 - cosine fails the triangle inequality;
        // arccos-normalized angular distance would be, but that is not
        // what we return here). Identity of indiscernibles holds under the
        // non-negative-input assumption because two non-negative vectors
        // with cosine = 1 must be positive scalar multiples of one
        // another, i.e. they carry the same *direction*. On the specific
        // count-produced GramVectors that motivate this variant they in
        // fact carry the same magnitude too, but we make the weaker (and
        // universally true) claim here.
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

/// The cosine-similarity kernel over [`GramVector`]. Returns a raw `f64`.
///
/// On non-negative-weight vectors — the shape [`GramVector`] carries by
/// default — the return value lies in `[0.0, 1.0]`. Signed weights (which
/// [`GramVector::set`] and [`GramVector::add`] permit) may push the raw
/// dot product negative and the returned similarity into `[-1, 0)`; the
/// [`Cosine::similarity_normalized`] entry point clamps the range check
/// away, and callers building signed vectors should route through the
/// unchecked [`Similarity<f64>`] instead.
#[must_use]
#[allow(
    clippy::similar_names,
    reason = "`na_sq` and `nb_sq` mirror the standard mathematical notation `||a||²` / `||b||²`; renaming for clippy would obscure the correspondence to the cosine-similarity formula"
)]
pub(crate) fn cosine<G: Ord + Clone>(a: &GramVector<G>, b: &GramVector<G>) -> f64 {
    // Work in `l2_norm_squared` and take a single square root of the
    // product, rather than the product of two square roots. The single
    // formulation matters for the identity case: `sqrt(2) * sqrt(2)` is
    // `2.0000000000000004`, not `2.0`, which would push `cosine(a, a)`
    // below `1.0` and defeat the exact-identity assertion. `sqrt(na² * nb²)`
    // computes `sqrt(4.0) = 2.0` exactly on integer-count inputs, so
    // `cosine(a, a) = dot / denom` is bit-exactly `1.0`.
    let na_sq = a.l2_norm_squared();
    let nb_sq = b.l2_norm_squared();
    // Two zero-norm vectors — including the two empty-vector case — are
    // treated as identical under the crate's uniform empty-empty
    // convention. Documented in the module docs.
    if na_sq == 0.0 && nb_sq == 0.0 {
        return 1.0;
    }
    // One-side zero norm: undefined arithmetically; we return 0.0 to
    // match the "one-empty is zero" convention every other similarity in
    // this crate uses.
    if na_sq == 0.0 || nb_sq == 0.0 {
        return 0.0;
    }
    let d = a.dot(b);
    // The `f64` sum implementation folds from `-0.0`, so an empty
    // filter_map (disjoint supports) yields `-0.0` rather than `+0.0`
    // (this is subtle IEEE 754 behavior: the additive identity for
    // floats is `-0.0`, because `x + -0.0 == x` holds for all `x` while
    // `x + 0.0` maps `-0.0` to `+0.0`). `d == 0.0` compares equal for
    // both signed zeros, so this branch normalizes to `+0.0` and keeps
    // the returned bit pattern stable.
    if d == 0.0 {
        return 0.0;
    }
    let denom = (na_sq * nb_sq).sqrt();
    // On non-negative inputs `d / denom` is in `[0, 1]` mathematically;
    // rounding can push it just outside by a ULP or two. `clamp` is a
    // cheap safety net that does nothing on well-behaved inputs.
    (d / denom).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn vector_from_chars(chars: &[char]) -> GramVector<char> {
        let mut v: GramVector<char> = GramVector::new();
        for c in chars {
            v.add(*c, 1.0);
        }
        v
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = Cosine::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Cosine);
        assert_eq!(d.variant, VariantId("euclidean-nonneg"));
    }

    #[test]
    fn empty_empty_is_one_bit_exact() {
        let a: GramVector<char> = GramVector::new();
        assert_eq!(cosine(&a, &a).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn one_empty_is_zero_bit_exact() {
        let a: GramVector<char> = GramVector::new();
        let b = vector_from_chars(&['a', 'b']);
        assert_eq!(cosine(&a, &b).to_bits(), 0.0_f64.to_bits());
        assert_eq!(cosine(&b, &a).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn identical_is_one() {
        let a = vector_from_chars(&['x', 'y', 'z']);
        // dot(a, a) = 3; l2_norm = sqrt(3); cosine = 3 / 3 = 1.0.
        assert_eq!(cosine(&a, &a).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn disjoint_is_zero() {
        let a = vector_from_chars(&['a', 'b']);
        let b = vector_from_chars(&['x', 'y']);
        assert_eq!(cosine(&a, &b).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn partial_overlap_matches_hand_computation() {
        // "abc" as unit-count vector: {a:1, b:1, c:1}, norm sqrt(3).
        // "abd" as unit-count vector: {a:1, b:1, d:1}, norm sqrt(3).
        // dot = 2 → cos = 2 / 3.
        let a = vector_from_chars(&['a', 'b', 'c']);
        let b = vector_from_chars(&['a', 'b', 'd']);
        assert!((cosine(&a, &b) - 2.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn distance_is_complement_of_similarity() {
        let a = vector_from_chars(&['a', 'b', 'c']);
        let b = vector_from_chars(&['a', 'b', 'd']);
        let alg = Cosine;
        let s = cosine(&a, &b);
        let d = alg.distance(&a, &b).into_inner();
        assert!((d - (1.0 - s)).abs() < 1e-15);
    }

    #[test]
    fn zero_weighted_vector_treated_like_empty() {
        // A vector whose sole entry is a zero weight has L2 norm 0. Two
        // such vectors compare identical; a zero-norm-vs-non-zero-norm
        // pair collapses to 0.0.
        let mut a: GramVector<char> = GramVector::new();
        a.set('a', 0.0);
        let mut b: GramVector<char> = GramVector::new();
        b.set('a', 0.0);
        assert_eq!(cosine(&a, &b).to_bits(), 1.0_f64.to_bits());
        let c = vector_from_chars(&['x']);
        assert_eq!(cosine(&a, &c).to_bits(), 0.0_f64.to_bits());
    }

    // Silence an "unused import" warning under `--all-features`; the
    // `Vec` import is present so downstream test files (which occasionally
    // stringify test failures with `format!`) do not need to redeclare it.
    #[allow(dead_code)]
    fn _keep_vec_in_scope() -> Vec<()> {
        Vec::new()
    }
}
