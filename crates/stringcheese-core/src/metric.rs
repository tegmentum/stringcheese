//! Traits for comparison algorithms and descriptors for their mathematical
//! properties.
//!
//! The trait hierarchy in this module is deliberately narrow. Every trait
//! serves one purpose: to let a consumer state the *shape* of comparison it
//! wants (distance, distance-with-cutoff, similarity) and receive an
//! algorithm that satisfies it.
//!
//! The mathematical properties an algorithm claims are captured separately
//! by [`MetricProperties`] and [`MetricClass`]. Downstream infrastructure
//! (index structures, clustering, filters) inspects these to decide whether
//! an algorithm is suitable — for example, a BK-tree accepts only true
//! metrics, and clustering algorithms may require symmetry.

use crate::result::{BoundedDistance, Distance, Similarity};

/// An algorithm that computes a distance between two sequences of type `S`.
///
/// Implementations should be pure (no observable side effects) and stateless
/// with respect to the algorithm's semantics — configuration such as
/// substitution costs, cutoffs, or preprocessing state lives on `Self` and
/// does not change between calls. This makes it safe to share a single
/// algorithm value across threads.
pub trait DistanceMetric<S: ?Sized> {
    /// The numeric type of distances this algorithm produces.
    type Output;

    /// Returns the distance between `left` and `right`.
    fn distance(&self, left: &S, right: &S) -> Distance<Self::Output>;

    /// Returns the mathematical properties this algorithm claims to satisfy.
    ///
    /// These claims are exercised by StringCheese's property-based test suite;
    /// downstream infrastructure consults them to decide whether an
    /// algorithm is a suitable input.
    fn properties(&self) -> MetricProperties;

    /// Returns the algorithm's mathematical classification.
    fn class(&self) -> MetricClass;
}

/// A [`DistanceMetric`] that supports early termination when the true
/// distance exceeds a caller-supplied cutoff.
///
/// Cutoff-aware algorithms are the standard tool for large-corpus matching
/// where callers only care whether the distance is below a threshold. When
/// the true distance exceeds the cutoff, the exact value is unknown — only
/// the fact of exceedance is reported (see [`BoundedDistance`]).
pub trait BoundedDistanceMetric<S: ?Sized>: DistanceMetric<S> {
    /// Returns the distance if it is less than or equal to `cutoff`; returns
    /// [`BoundedDistance::Exceeded`] if the true distance is strictly
    /// greater than `cutoff`.
    fn distance_within(
        &self,
        left: &S,
        right: &S,
        cutoff: Self::Output,
    ) -> BoundedDistance<Self::Output>;
}

/// An algorithm that computes a similarity between two sequences of type `S`.
pub trait SimilarityMetric<S: ?Sized> {
    /// The numeric type of similarities this algorithm produces.
    type Output;

    /// Returns the similarity between `left` and `right`.
    fn similarity(&self, left: &S, right: &S) -> Similarity<Self::Output>;

    /// Returns the mathematical properties this algorithm claims to satisfy.
    fn properties(&self) -> MetricProperties;

    /// Returns the algorithm's mathematical classification.
    fn class(&self) -> MetricClass;
}

/// Mathematical properties an algorithm claims to satisfy.
///
/// These are structural claims about behavior on all valid inputs — not
/// observations on a particular corpus. StringCheese's property-based test
/// suite exercises each claimed property against randomly generated inputs.
///
/// # Field semantics
///
/// The four core axioms follow the standard definitions from metric-space
/// theory. `normalized` is a boundedness claim specific to StringCheese:
/// algorithms that produce output naturally in `[0.0, 1.0]` set it to
/// `true`. Raw distances with no natural upper bound leave it `false`; they
/// need explicit normalization (see [`crate::normalization::NormalizationPolicy`])
/// to produce a value in `[0.0, 1.0]`.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each field represents an independent metric-space axiom; collapsing them into a bitset would obscure the correspondence to the mathematical definition"
)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MetricProperties {
    /// `d(x, y) = d(y, x)` for all valid `x`, `y`.
    pub symmetric: bool,
    /// `d(x, y) = 0` if and only if `x = y` under the algorithm's chosen
    /// notion of equality on its input sequences.
    pub identity_of_indiscernibles: bool,
    /// `d(x, z) ≤ d(x, y) + d(y, z)` for all valid `x`, `y`, `z`.
    pub triangle_inequality: bool,
    /// `d(x, y) ≥ 0` for all valid `x`, `y`.
    pub non_negative: bool,
    /// The algorithm's output is naturally bounded to `[0.0, 1.0]`.
    pub normalized: bool,
}

impl MetricProperties {
    /// Properties of a true metric.
    pub const METRIC: Self = Self {
        symmetric: true,
        identity_of_indiscernibles: true,
        triangle_inequality: true,
        non_negative: true,
        normalized: false,
    };

    /// Properties of a normalized true metric (output naturally in `[0, 1]`).
    pub const NORMALIZED_METRIC: Self = Self {
        symmetric: true,
        identity_of_indiscernibles: true,
        triangle_inequality: true,
        non_negative: true,
        normalized: true,
    };

    /// Properties of a pseudometric: like a metric, but permits
    /// `d(x, y) = 0` for distinct `x`, `y`.
    pub const PSEUDOMETRIC: Self = Self {
        symmetric: true,
        identity_of_indiscernibles: false,
        triangle_inequality: true,
        non_negative: true,
        normalized: false,
    };

    /// Properties of a semimetric: symmetric and non-negative with identity
    /// of indiscernibles, but without the triangle inequality.
    pub const SEMIMETRIC: Self = Self {
        symmetric: true,
        identity_of_indiscernibles: true,
        triangle_inequality: false,
        non_negative: true,
        normalized: false,
    };

    /// Properties of a quasimetric: satisfies the metric axioms except
    /// symmetry.
    pub const QUASIMETRIC: Self = Self {
        symmetric: false,
        identity_of_indiscernibles: true,
        triangle_inequality: true,
        non_negative: true,
        normalized: false,
    };

    /// Returns `true` if the properties satisfy the definition of a metric.
    #[inline]
    #[must_use]
    pub const fn is_metric(&self) -> bool {
        self.symmetric
            && self.identity_of_indiscernibles
            && self.triangle_inequality
            && self.non_negative
    }
}

/// The mathematical classification of a comparison algorithm.
///
/// Every algorithm in StringCheese belongs to exactly one class. The class is
/// derived from the algorithm's [`MetricProperties`] together with its
/// documented interpretation (distance vs. similarity vs. raw score).
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MetricClass {
    /// A true metric: symmetric, non-negative, with identity of
    /// indiscernibles and the triangle inequality.
    Metric,
    /// A pseudometric: like a metric, but `d(x, y) = 0` may hold for
    /// distinct `x`, `y`.
    Pseudometric,
    /// A semimetric: symmetric and non-negative with identity, but without
    /// the triangle inequality.
    Semimetric,
    /// A quasimetric: satisfies the metric axioms except symmetry.
    Quasimetric,
    /// A divergence measure (typically non-symmetric, non-metric).
    Divergence,
    /// A similarity function. Higher values indicate greater similarity.
    Similarity,
    /// A positive-definite kernel function.
    Kernel,
    /// A raw alignment or scoring value whose interpretation depends on the
    /// scoring scheme.
    Score,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_predicate() {
        assert!(MetricProperties::METRIC.is_metric());
        assert!(MetricProperties::NORMALIZED_METRIC.is_metric());
        assert!(!MetricProperties::PSEUDOMETRIC.is_metric());
        assert!(!MetricProperties::SEMIMETRIC.is_metric());
        assert!(!MetricProperties::QUASIMETRIC.is_metric());
    }

    #[test]
    fn metric_properties_axioms() {
        let m = MetricProperties::METRIC;
        assert!(m.symmetric);
        assert!(m.identity_of_indiscernibles);
        assert!(m.triangle_inequality);
        assert!(m.non_negative);
        assert!(!m.normalized);

        let nm = MetricProperties::NORMALIZED_METRIC;
        assert!(nm.normalized);
        assert!(nm.is_metric());
    }
}
