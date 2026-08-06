//! Public [`Lcs`] and [`LcsDistance`] algorithm handles.
//!
//! `Lcs` and `LcsDistance` are zero-size unit structs. `Lcs` exposes the
//! LCS *length* as a plain [`Score<u32>`](stringcheese_core::Score) — a
//! quantity that is neither a distance nor a similarity in the canonical
//! sense. `LcsDistance` derives the metric distance
//! `|a| + |b| - 2 · lcs(a, b)` and implements the
//! [`DistanceMetric`] trait as a true metric.
//!
//! Two entry-point shapes are provided on each type:
//!
//! * The convenient one-shot methods ([`Lcs::length`],
//!   [`LcsDistance::distance`], and the [`DistanceMetric`] impl) allocate a
//!   fresh [`LcsWorkspace`] on every call.
//! * The `*_with_workspace` methods take a caller-owned workspace, so batch
//!   callers can reuse a single allocation across many comparisons.
//!
//! Golden test cases reference these algorithms by their descriptors
//! ([`Lcs::DESCRIPTOR`] and [`LcsDistance::DESCRIPTOR`]) rather than by
//! common name, so an LCS-length case cannot silently be validated against
//! an LCS-distance implementation.

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, Distance,
    DistanceMetric, MetricClass, MetricProperties, Score, VariantId,
};

use crate::lcs::rolling_rows::{
    lcs_distance_rolling_rows_with_workspace, lcs_length_rolling_rows_with_workspace,
};
use crate::lcs::workspace::LcsWorkspace;

/// The Longest Common Subsequence *length* algorithm.
///
/// Returns the length of the longest sequence that is a (not necessarily
/// contiguous) subsequence of both inputs, wrapped as a
/// [`Score<u32>`](stringcheese_core::Score). The value has no natural upper
/// bound other than `min(|a|, |b|)`; if a normalized similarity is desired,
/// the caller can divide by `max(|a|, |b|)` at the call site.
///
/// The DP recurrence backing every kernel here is the "no substitution"
/// specialization of the Wagner-Fischer (1974) string-to-string correction
/// DP; Hirschberg (1975) is the standard reference for the linear-space
/// variant. See the crate-level `References` section for full citations.
///
/// LCS length is not itself a metric (it does not satisfy identity of
/// indiscernibles: `lcs(x, x) = |x|`, not `0`). The related
/// [`LcsDistance`] type provides the corresponding true metric.
///
/// The type is a zero-sized unit struct; construct it as `Lcs` and reuse
/// the value across threads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lcs;

impl Lcs {
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"length-generic-eq"` names the two properties
    /// that distinguish this implementation from any future sibling: the
    /// output is the *length* of the LCS (as opposed to the derived
    /// distance, or a reconstructed alignment), and comparison happens
    /// through generic `T: Eq` rather than a specialized bit-parallel
    /// byte kernel.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::LongestCommonSubsequence,
        VariantId("length-generic-eq"),
        DescriptorVersion::new(0, 1, 0),
        DefinitionSource::IndependentlyDerived,
    );

    /// Returns the algorithm descriptor for this variant.
    ///
    /// A `const` accessor is provided so descriptors can be pinned in
    /// `const` context — for example, as the `descriptor` field of a
    /// `GoldenCase`.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Returns the length of the longest common subsequence of `left` and
    /// `right`.
    ///
    /// This is a one-shot convenience method that allocates a fresh
    /// [`LcsWorkspace`] on each call. For batch use, prefer
    /// [`length_with_workspace`](Lcs::length_with_workspace).
    #[inline]
    pub fn length<T: Eq>(self, left: &[T], right: &[T]) -> Score<u32> {
        let mut ws = LcsWorkspace::new();
        self.length_with_workspace(left, right, &mut ws)
    }

    /// Returns the length of the longest common subsequence of `left` and
    /// `right`, reusing `ws` as scratch across the call.
    ///
    /// This is the workspace-aware entry point for batch comparisons: a
    /// single [`LcsWorkspace`] is grown to fit the largest comparison in
    /// the batch and reused for every subsequent call, so the allocation
    /// cost is amortized across the batch.
    #[inline]
    pub fn length_with_workspace<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        ws: &mut LcsWorkspace,
    ) -> Score<u32> {
        lcs_length_rolling_rows_with_workspace(left, right, ws)
    }
}

/// The Longest Common Subsequence *distance* metric.
///
/// Defined as `|a| + |b| - 2 · lcs(a, b)`, this counts the minimum number
/// of single-symbol insertions plus deletions needed to transform `a` into
/// `b`. **Substitutions are not permitted under this metric** — that is
/// the defining distinction from Levenshtein, which allows substitutions
/// at unit cost. For example, `LcsDistance` reports `2` for the pair
/// `("abcd", "abed")` (delete `c`, insert `e`), whereas Levenshtein
/// reports `1` for the same pair.
///
/// LCS distance is a **true metric**: symmetric, non-negative, satisfies
/// identity of indiscernibles under `T: Eq`, and satisfies the triangle
/// inequality. See Bergroth, Hakonen, and Raita (2000) for the standard
/// treatment; the full citation is in the crate-level `References` section.
///
/// The type is a zero-sized unit struct; construct it as `LcsDistance`
/// and reuse the value across threads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LcsDistance;

impl LcsDistance {
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"distance-generic-eq"` names the two properties
    /// that distinguish this implementation from any future sibling: the
    /// output is the LCS *distance* (as opposed to the raw length, or a
    /// reconstructed alignment script), and comparison happens through
    /// generic `T: Eq` rather than a specialized bit-parallel byte
    /// kernel.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::LongestCommonSubsequence,
        VariantId("distance-generic-eq"),
        DescriptorVersion::new(0, 1, 0),
        DefinitionSource::Paper {
            title: "A survey of longest common subsequence algorithms",
            authors: "L. Bergroth, H. Hakonen, T. Raita",
            year: 2000,
        },
    );

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Returns the mathematical properties LCS distance satisfies.
    ///
    /// LCS distance is a true metric: symmetric, non-negative, satisfies
    /// identity of indiscernibles under `T: Eq`, and satisfies the
    /// triangle inequality. It is not naturally normalized — the maximum
    /// value for length-`n` inputs is `2n`, not `1.0`.
    #[inline]
    #[must_use]
    pub const fn properties() -> MetricProperties {
        MetricProperties::METRIC
    }

    /// Returns LCS distance's mathematical classification.
    #[inline]
    #[must_use]
    pub const fn class() -> MetricClass {
        MetricClass::Metric
    }

    /// Returns the LCS distance between `left` and `right`.
    ///
    /// This is a one-shot convenience method that allocates a fresh
    /// [`LcsWorkspace`] on each call. For batch use, prefer
    /// [`distance_with_workspace`](LcsDistance::distance_with_workspace).
    #[inline]
    pub fn distance<T: Eq>(self, left: &[T], right: &[T]) -> Distance<u32> {
        let mut ws = LcsWorkspace::new();
        self.distance_with_workspace(left, right, &mut ws)
    }

    /// Returns the LCS distance between `left` and `right`, reusing `ws`
    /// as scratch across the call.
    #[inline]
    pub fn distance_with_workspace<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        ws: &mut LcsWorkspace,
    ) -> Distance<u32> {
        lcs_distance_rolling_rows_with_workspace(left, right, ws)
    }
}

impl<T: Eq> DistanceMetric<[T]> for LcsDistance {
    type Output = u32;

    #[inline]
    fn distance(&self, left: &[T], right: &[T]) -> Distance<Self::Output> {
        LcsDistance::distance(*self, left, right)
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        Self::properties()
    }

    #[inline]
    fn class(&self) -> MetricClass {
        Self::class()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcs::full_matrix::{lcs_distance_full_matrix, lcs_length_full_matrix};

    #[test]
    fn lcs_descriptor_matches_family_and_variant() {
        let d = Lcs::descriptor();
        assert_eq!(d.family, AlgorithmFamily::LongestCommonSubsequence);
        assert_eq!(d.variant, VariantId("length-generic-eq"));
        assert!(matches!(d.source, DefinitionSource::IndependentlyDerived));
    }

    #[test]
    fn lcs_descriptor_is_const() {
        const D: AlgorithmDescriptor = Lcs::DESCRIPTOR;
        assert_eq!(D.variant.0, "length-generic-eq");
    }

    #[test]
    fn lcs_distance_descriptor_matches_family_and_variant() {
        let d = LcsDistance::descriptor();
        assert_eq!(d.family, AlgorithmFamily::LongestCommonSubsequence);
        assert_eq!(d.variant, VariantId("distance-generic-eq"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 2000, .. }
        ));
    }

    #[test]
    fn lcs_distance_descriptor_is_const() {
        const D: AlgorithmDescriptor = LcsDistance::DESCRIPTOR;
        assert_eq!(D.variant.0, "distance-generic-eq");
    }

    #[test]
    fn lcs_length_matches_oracle_on_canonical_pairs() {
        let alg = Lcs;
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b""),
            (b"kitten", b"sitting"),
            (b"ABCBDAB", b"BDCAB"),
            (b"AGCAT", b"GAC"),
        ];
        for (a, b) in pairs {
            assert_eq!(
                alg.length(a, b).into_inner(),
                lcs_length_full_matrix(a, b).into_inner(),
                "Lcs::length disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn lcs_distance_declares_true_metric() {
        let alg = LcsDistance;
        assert_eq!(
            <LcsDistance as DistanceMetric<[u8]>>::class(&alg),
            MetricClass::Metric
        );
        assert!(<LcsDistance as DistanceMetric<[u8]>>::properties(&alg).is_metric());
    }

    #[test]
    fn lcs_distance_const_accessors_agree_with_trait_methods() {
        let alg = LcsDistance;
        assert_eq!(
            LcsDistance::properties(),
            <LcsDistance as DistanceMetric<[u8]>>::properties(&alg),
        );
        assert_eq!(
            LcsDistance::class(),
            <LcsDistance as DistanceMetric<[u8]>>::class(&alg),
        );
    }

    #[test]
    fn lcs_distance_impl_matches_oracle() {
        let alg = LcsDistance;
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b""),
            (b"kitten", b"sitting"),
            (b"abcd", b"abed"),
            (b"AGCAT", b"GAC"),
        ];
        for (a, b) in pairs {
            assert_eq!(
                alg.distance(a, b).into_inner(),
                lcs_distance_full_matrix(a, b).into_inner(),
                "LcsDistance trait impl disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn workspace_reuse_matches_fresh_workspace_for_length() {
        let alg = Lcs;
        let mut ws = LcsWorkspace::new();
        let a = b"prefix-common-tail-A";
        let b = b"prefix-common-tail-B";
        let with_ws = alg.length_with_workspace(a, b, &mut ws);
        let without_ws = alg.length(a, b);
        assert_eq!(with_ws, without_ws);
    }

    #[test]
    fn workspace_reuse_matches_fresh_workspace_for_distance() {
        let alg = LcsDistance;
        let mut ws = LcsWorkspace::new();
        let a = b"prefix-common-tail-A";
        let b = b"prefix-common-tail-B";
        let with_ws = alg.distance_with_workspace(a, b, &mut ws);
        let without_ws = alg.distance(a, b);
        assert_eq!(with_ws, without_ws);
    }

    #[test]
    fn lcs_and_lcs_distance_share_family_but_differ_in_variant() {
        // The two algorithms belong to the same family; if they didn't,
        // downstream infrastructure that dispatches on `AlgorithmFamily`
        // (index selection, adapter tables) would treat them as
        // unrelated. But their variant slugs must differ so a golden case
        // for one cannot silently be validated by the other.
        assert_eq!(Lcs::DESCRIPTOR.family, LcsDistance::DESCRIPTOR.family);
        assert_ne!(Lcs::DESCRIPTOR.variant, LcsDistance::DESCRIPTOR.variant);
    }
}
