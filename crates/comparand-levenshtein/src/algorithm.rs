//! The public [`Levenshtein`] algorithm handle.
//!
//! [`Levenshtein`] is a zero-size unit struct that bundles the crate's
//! kernels behind the metric traits from `comparand-core`. Two entry-point
//! shapes are provided:
//!
//! * The trait implementations ([`DistanceMetric`] and [`BoundedDistanceMetric`])
//!   allocate a fresh [`LevenshteinWorkspace`] on every call. These are the
//!   convenient one-shot APIs.
//! * The `*_with_workspace` methods take a caller-owned workspace, so batch
//!   callers can reuse a single allocation across many comparisons.
//!
//! The variant descriptor is [`Levenshtein::DESCRIPTOR`] (also available via
//! the [`Levenshtein::descriptor`] const function). Golden test cases
//! reference this descriptor rather than the common name, so a "Levenshtein"
//! case cannot silently be validated against the wrong variant.

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, BoundedDistance, BoundedDistanceMetric,
    DefinitionSource, DescriptorVersion, Distance, DistanceMetric, MetricClass,
    MetricProperties, VariantId,
};

use crate::banded::distance_banded_with_workspace;
use crate::rolling_rows::distance_rolling_rows_with_workspace;
use crate::workspace::LevenshteinWorkspace;

/// The unit-cost Levenshtein edit distance.
///
/// Substitutions, insertions, and deletions each cost `1`; there is no
/// separate transposition operation (a swap of two adjacent symbols costs
/// `2` under this metric — Damerau–Levenshtein is a distinct algorithm and
/// lives in its own crate).
///
/// The type is a zero-sized unit struct; construct it as `Levenshtein` and
/// reuse the value across threads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Levenshtein;

impl Levenshtein {
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"unit-cost-generic-eq"` distinguishes this from
    /// weighted-cost, transposition-aware, and equality-refined siblings.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Levenshtein,
        variant: VariantId("unit-cost-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Binary codes capable of correcting deletions, insertions, and reversals",
            authors: "V. I. Levenshtein",
            year: 1966,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    ///
    /// A `const` accessor is provided so descriptors can be pinned in `const`
    /// context — for example, as the `descriptor` field of a `GoldenCase`.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes the distance between `left` and `right`, reusing `ws` as
    /// scratch across the call.
    ///
    /// This is the workspace-aware entry point for batch comparisons: a
    /// single [`LevenshteinWorkspace`] is grown to fit the largest
    /// comparison in the batch and reused for every subsequent call, so
    /// the allocation cost is amortized across the batch.
    #[inline]
    pub fn distance_with_workspace<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        ws: &mut LevenshteinWorkspace,
    ) -> Distance<u32> {
        distance_rolling_rows_with_workspace(left, right, ws)
    }

    /// Computes the distance between `left` and `right` with an
    /// early-termination cutoff, reusing `ws` as scratch across the call.
    #[inline]
    pub fn distance_within_with_workspace<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        cutoff: u32,
        ws: &mut LevenshteinWorkspace,
    ) -> BoundedDistance<u32> {
        distance_banded_with_workspace(left, right, cutoff, ws)
    }
}

impl<T: Eq> DistanceMetric<[T]> for Levenshtein {
    type Output = u32;

    #[inline]
    fn distance(&self, left: &[T], right: &[T]) -> Distance<Self::Output> {
        let mut ws = LevenshteinWorkspace::new();
        self.distance_with_workspace(left, right, &mut ws)
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        MetricProperties::METRIC
    }

    #[inline]
    fn class(&self) -> MetricClass {
        MetricClass::Metric
    }
}

impl<T: Eq> BoundedDistanceMetric<[T]> for Levenshtein {
    #[inline]
    fn distance_within(
        &self,
        left: &[T],
        right: &[T],
        cutoff: Self::Output,
    ) -> BoundedDistance<Self::Output> {
        let mut ws = LevenshteinWorkspace::new();
        self.distance_within_with_workspace(left, right, cutoff, &mut ws)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::full_matrix::distance_full_matrix;

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = Levenshtein::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Levenshtein);
        assert_eq!(d.variant, VariantId("unit-cost-generic-eq"));
        assert!(matches!(d.source, DefinitionSource::Paper { year: 1966, .. }));
    }

    #[test]
    fn descriptor_is_const() {
        // Available at const time, so downstream can pin it into
        // `const GOLDEN_CASE: GoldenCase<...>` records.
        const D: AlgorithmDescriptor = Levenshtein::DESCRIPTOR;
        assert_eq!(D.variant.0, "unit-cost-generic-eq");
    }

    #[test]
    fn declares_true_metric() {
        let alg = Levenshtein;
        // `DistanceMetric` is generic over the sequence type, so its
        // associated methods need a concrete `S` to be resolvable — the
        // caller-facing name `alg.class()` would otherwise be ambiguous.
        assert_eq!(
            <Levenshtein as DistanceMetric<[u8]>>::class(&alg),
            MetricClass::Metric
        );
        assert!(<Levenshtein as DistanceMetric<[u8]>>::properties(&alg).is_metric());
    }

    #[test]
    fn distance_metric_impl_matches_oracle() {
        let alg = Levenshtein;
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b""),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"totally", b"different"),
        ];
        for (a, b) in pairs {
            assert_eq!(
                alg.distance(a, b).into_inner(),
                distance_full_matrix(a, b),
                "trait DistanceMetric impl disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn bounded_distance_metric_impl_matches_oracle() {
        let alg = Levenshtein;
        for (a, b) in [
            (b"kitten".as_ref(), b"sitting".as_ref()),
            (b"Saturday".as_ref(), b"Sunday".as_ref()),
            (b"cat".as_ref(), b"dog".as_ref()),
        ] {
            let expected = distance_full_matrix(a, b);
            for k in 0..=8 {
                let observed = alg.distance_within(a, b, k);
                if expected <= k {
                    assert_eq!(observed, BoundedDistance::Within(Distance::new(expected)));
                } else {
                    assert_eq!(observed, BoundedDistance::Exceeded { cutoff: k });
                }
            }
        }
    }

    #[test]
    fn workspace_reuse_matches_fresh_workspace() {
        let alg = Levenshtein;
        let mut ws = LevenshteinWorkspace::new();
        let a = b"prefix-common-tail-A";
        let b = b"prefix-common-tail-B";
        let with_ws = alg.distance_with_workspace(a, b, &mut ws);
        let without_ws = alg.distance(a, b);
        assert_eq!(with_ws, without_ws);
    }
}
