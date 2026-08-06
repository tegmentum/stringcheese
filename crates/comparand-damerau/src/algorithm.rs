//! The public [`Osa`] and [`Damerau`] algorithm handles.
//!
//! Both types are zero-size unit structs that bundle the crate's kernels
//! behind the metric traits from `comparand-core`. They exist as separate
//! types so that the algorithm-variant distinction is unambiguous at every
//! call site — passing an [`Osa`] to code that expects a full-Damerau
//! metric is a type error, not a silent semantic swap.
//!
//! # OSA
//!
//! [`Osa`] is the Optimal String Alignment (restricted Damerau) variant.
//! It provides both [`DistanceMetric`] and [`BoundedDistanceMetric`] impls
//! backed by the three OSA kernels; the variant descriptor is
//! [`Osa::DESCRIPTOR`].
//!
//! # Full Damerau
//!
//! [`Damerau`] is the unrestricted (full) Damerau variant. It provides a
//! [`DistanceMetric`] impl backed by the production kernel; the variant
//! descriptor is [`Damerau::DESCRIPTOR`]. `BoundedDistanceMetric` is
//! **not** implemented for full Damerau — its non-local transposition
//! branch means the classical Ukkonen-style symmetric band is not a sound
//! pruning window, and a correct banded formulation is deferred as future
//! work. Callers who need a cutoff-aware metric should use [`Osa`] instead
//! or wrap [`Damerau::distance`] with an external cutoff check.
//!
//! The Damerau trait impl requires `T: Eq + Hash` and is gated on the
//! crate's `std` feature (because the production kernel uses
//! `std::collections::HashMap`). The [`Damerau`] type itself and its
//! [`Damerau::DESCRIPTOR`] are always available, so downstream code can
//! pin the descriptor into golden cases even under alloc-only builds.

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, BoundedDistance, BoundedDistanceMetric, DefinitionSource,
    DescriptorVersion, Distance, DistanceMetric, MetricClass, MetricProperties, VariantId,
};

use crate::osa::banded::distance_banded_with_workspace;
use crate::osa::rolling_rows::distance_rolling_rows_with_workspace;
use crate::workspace::OsaWorkspace;

#[cfg(feature = "std")]
use core::hash::Hash;

#[cfg(feature = "std")]
use crate::damerau::production::distance_production_with_workspace;
#[cfg(feature = "std")]
use crate::workspace::DamerauWorkspace;

// -----------------------------------------------------------------------------
// OSA
// -----------------------------------------------------------------------------

/// The Optimal String Alignment (restricted Damerau-Levenshtein) distance.
///
/// Substitutions, insertions, deletions, and adjacent transpositions each
/// cost `1`, with the restriction that no substring of either input may be
/// edited more than once. Under that restriction OSA is a *semimetric*
/// rather than a metric: it violates the triangle inequality — see
/// [`Osa::properties`] and the crate's `property_tests` module (test-only)
/// for a hard-coded violation case that documents this as known behavior.
///
/// OSA is folkloric — the recurrence is a one-branch extension of the
/// Wagner-Fischer (1974) Levenshtein DP, and no single paper is universally
/// cited as its origin. See the crate-level `References` section for the
/// Wagner-Fischer citation.
///
/// The type is a zero-sized unit struct; construct it as `Osa` and reuse
/// the value across threads.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Osa;

impl Osa {
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"unit-cost-generic-eq"` distinguishes this from
    /// future weighted-cost or equality-refined siblings. Sourced as
    /// [`DefinitionSource::IndependentlyDerived`] because OSA is folkloric —
    /// the recurrence itself is the standard Levenshtein DP with one added
    /// branch, and no single paper is universally cited as its origin.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::OptimalStringAlignment,
        variant: VariantId("unit-cost-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::IndependentlyDerived,
    };

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

    /// Returns the mathematical properties OSA satisfies.
    ///
    /// OSA is symmetric, non-negative, and satisfies the identity of
    /// indiscernibles under `T: Eq`. It does **not** satisfy the triangle
    /// inequality (see the crate-level docs for the counterexample family)
    /// and it is not naturally normalized. This is a `const` accessor for
    /// use in `const` contexts where the trait method is not available.
    #[inline]
    #[must_use]
    pub const fn properties() -> MetricProperties {
        MetricProperties::SEMIMETRIC
    }

    /// Returns OSA's mathematical classification.
    ///
    /// See [`properties`](Osa::properties) for the axioms this
    /// classification summarizes.
    #[inline]
    #[must_use]
    pub const fn class() -> MetricClass {
        MetricClass::Semimetric
    }

    /// Computes the OSA distance between `left` and `right`, reusing `ws`
    /// as scratch across the call.
    ///
    /// This is the workspace-aware entry point for batch comparisons.
    #[inline]
    pub fn distance_with_workspace<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        ws: &mut OsaWorkspace,
    ) -> Distance<u32> {
        distance_rolling_rows_with_workspace(left, right, ws)
    }

    /// Computes the OSA distance between `left` and `right` with an
    /// early-termination cutoff, reusing `ws` as scratch across the call.
    #[inline]
    pub fn distance_within_with_workspace<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        cutoff: u32,
        ws: &mut OsaWorkspace,
    ) -> BoundedDistance<u32> {
        distance_banded_with_workspace(left, right, cutoff, ws)
    }
}

impl<T: Eq> DistanceMetric<[T]> for Osa {
    type Output = u32;

    #[inline]
    fn distance(&self, left: &[T], right: &[T]) -> Distance<Self::Output> {
        let mut ws = OsaWorkspace::new();
        self.distance_with_workspace(left, right, &mut ws)
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

impl<T: Eq> BoundedDistanceMetric<[T]> for Osa {
    #[inline]
    fn distance_within(
        &self,
        left: &[T],
        right: &[T],
        cutoff: Self::Output,
    ) -> BoundedDistance<Self::Output> {
        let mut ws = OsaWorkspace::new();
        self.distance_within_with_workspace(left, right, cutoff, &mut ws)
    }
}

// -----------------------------------------------------------------------------
// Full Damerau
// -----------------------------------------------------------------------------

/// The full (unrestricted) Damerau-Levenshtein distance.
///
/// Substitutions, insertions, deletions, and adjacent transpositions each
/// cost `1`; unlike [`Osa`], there is no per-substring edit restriction, so
/// full Damerau *is* a true metric under unit costs.
///
/// Introduced by Damerau (1964) as an information-retrieval keying scheme
/// for spelling correction; the polynomial DP formulation used here is
/// Lowrance and Wagner (1975). Full citations are in the crate-level
/// `References` section.
///
/// The type is a zero-sized unit struct; construct it as `Damerau` and
/// reuse the value across threads.
///
/// # Availability
///
/// The [`DistanceMetric`] implementation requires `T: Eq + Hash` and is
/// gated on the crate's `std` feature (because the production kernel
/// backing it uses `std::collections::HashMap`). The type itself and
/// [`Damerau::DESCRIPTOR`] are always available, so downstream code can
/// still reference the descriptor from golden cases in alloc-only builds.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Damerau;

impl Damerau {
    // NOTE (descriptor `source` choice): `DefinitionSource::Paper` holds a
    // single citation, and we deliberately point at Damerau (1964) — the
    // paper that *names* the algorithm — rather than at Lowrance & Wagner
    // (1975), which is the source of the polynomial DP formulation this
    // crate actually implements. The descriptor identifies what the
    // algorithm *is*, not how its DP is derived; readers who care about
    // the implementation lineage should consult the crate-level `References`
    // section (which lists both papers prominently) and the module-level
    // doc comment on this type (which spells out the Damerau→Lowrance-Wagner
    // provenance).
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"unrestricted-unit-cost-generic-eq"` names the
    /// three properties that distinguish this implementation from every
    /// sibling: no per-substring edit restriction (that would be OSA), unit
    /// operation costs (as opposed to weighted variants), and generic
    /// `T: Eq` symbol equality (as opposed to specialized byte kernels).
    /// Sourced as Damerau's 1964 paper — Lowrance and Wagner's 1975 paper
    /// gave the first polynomial DP formulation but the algorithm itself is
    /// Damerau's.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::DamerauLevenshtein,
        variant: VariantId("unrestricted-unit-cost-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "A technique for computer detection and correction of spelling errors",
            authors: "F. J. Damerau",
            year: 1964,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Returns the mathematical properties full Damerau satisfies.
    ///
    /// Full Damerau is a true metric under unit costs: symmetric,
    /// non-negative, satisfies the identity of indiscernibles under
    /// `T: Eq`, and satisfies the triangle inequality. It is not naturally
    /// normalized. This is a `const` accessor for use in `const` contexts
    /// where the trait method is not available.
    #[inline]
    #[must_use]
    pub const fn properties() -> MetricProperties {
        MetricProperties::METRIC
    }

    /// Returns full Damerau's mathematical classification.
    #[inline]
    #[must_use]
    pub const fn class() -> MetricClass {
        MetricClass::Metric
    }

    /// Computes the full Damerau distance between `left` and `right`,
    /// reusing `ws` as scratch across the call.
    ///
    /// This is the workspace-aware entry point for batch comparisons.
    ///
    /// The `T: Clone` bound (added in a follow-up to the initial release
    /// for the zero-allocation-on-hot-path property) is trivially satisfied
    /// by every `Copy` symbol type — `u8`, `char`, and every integer scalar
    /// — which covers essentially every real-world Damerau caller.
    /// [`DamerauWorkspace<T>`] owns the auxiliary "last position of
    /// symbol" `HashMap`; owning it in the workspace across calls is what
    /// makes the hot path allocation-free, and owning it requires owning
    /// its keys.
    #[cfg(feature = "std")]
    #[inline]
    pub fn distance_with_workspace<T: Eq + Hash + Clone>(
        self,
        left: &[T],
        right: &[T],
        ws: &mut DamerauWorkspace<T>,
    ) -> Distance<u32> {
        distance_production_with_workspace(left, right, ws)
    }
}

#[cfg(feature = "std")]
impl<T: Eq + Hash + Clone> DistanceMetric<[T]> for Damerau {
    type Output = u32;

    #[inline]
    fn distance(&self, left: &[T], right: &[T]) -> Distance<Self::Output> {
        let mut ws: DamerauWorkspace<T> = DamerauWorkspace::new();
        self.distance_with_workspace(left, right, &mut ws)
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

    // ---- OSA ----

    #[test]
    fn osa_descriptor_matches_family_and_variant() {
        let d = Osa::descriptor();
        assert_eq!(d.family, AlgorithmFamily::OptimalStringAlignment);
        assert_eq!(d.variant, VariantId("unit-cost-generic-eq"));
        assert!(matches!(d.source, DefinitionSource::IndependentlyDerived));
    }

    #[test]
    fn osa_descriptor_is_const() {
        const D: AlgorithmDescriptor = Osa::DESCRIPTOR;
        assert_eq!(D.variant.0, "unit-cost-generic-eq");
    }

    #[test]
    fn osa_declares_semimetric() {
        let alg = Osa;
        assert_eq!(
            <Osa as DistanceMetric<[u8]>>::class(&alg),
            MetricClass::Semimetric
        );
        let p = <Osa as DistanceMetric<[u8]>>::properties(&alg);
        assert!(p.symmetric);
        assert!(p.identity_of_indiscernibles);
        assert!(p.non_negative);
        assert!(
            !p.triangle_inequality,
            "OSA violates the triangle inequality"
        );
        assert!(!p.is_metric());
    }

    #[test]
    fn osa_distance_metric_impl_matches_oracle() {
        use crate::osa::full_matrix::distance_full_matrix as osa_oracle;
        let alg = Osa;
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b""),
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"totally", b"different"),
        ];
        for (a, b) in pairs {
            assert_eq!(
                alg.distance(a, b).into_inner(),
                osa_oracle(a, b),
                "OSA trait impl disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn osa_bounded_impl_matches_oracle() {
        use crate::osa::full_matrix::distance_full_matrix as osa_oracle;
        let alg = Osa;
        for (a, b) in [
            (b"kitten".as_ref(), b"sitting".as_ref()),
            (b"Saturday".as_ref(), b"Sunday".as_ref()),
            (b"ab".as_ref(), b"ba".as_ref()),
            (b"ca".as_ref(), b"abc".as_ref()),
        ] {
            let expected = osa_oracle(a, b);
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
    fn osa_workspace_reuse_matches_fresh_workspace() {
        let alg = Osa;
        let mut ws = OsaWorkspace::new();
        let a: &[u8] = b"prefix-common-tail-A";
        let b: &[u8] = b"prefix-common-tail-B";
        let with_ws = alg.distance_with_workspace(a, b, &mut ws);
        let without_ws = alg.distance(a, b);
        assert_eq!(with_ws, without_ws);
    }

    // ---- Damerau ----

    #[test]
    fn damerau_descriptor_matches_family_and_variant() {
        let d = Damerau::descriptor();
        assert_eq!(d.family, AlgorithmFamily::DamerauLevenshtein);
        assert_eq!(d.variant, VariantId("unrestricted-unit-cost-generic-eq"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1964, .. }
        ));
    }

    #[test]
    fn damerau_descriptor_is_const() {
        const D: AlgorithmDescriptor = Damerau::DESCRIPTOR;
        assert_eq!(D.variant.0, "unrestricted-unit-cost-generic-eq");
    }

    #[test]
    fn damerau_and_osa_descriptors_are_distinct() {
        // The whole reason the variant registry exists — the two "damerau"
        // implementations must not silently collide.
        assert_ne!(Damerau::DESCRIPTOR.family, Osa::DESCRIPTOR.family);
        assert_ne!(Damerau::DESCRIPTOR, Osa::DESCRIPTOR);
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_declares_metric() {
        let alg = Damerau;
        assert_eq!(
            <Damerau as DistanceMetric<[u8]>>::class(&alg),
            MetricClass::Metric
        );
        assert!(<Damerau as DistanceMetric<[u8]>>::properties(&alg).is_metric());
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_distance_metric_impl_matches_oracle() {
        use crate::damerau::full_matrix::distance_full_matrix as damerau_oracle;
        let alg = Damerau;
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b""),
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"abcd", b"badc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
        ];
        for (a, b) in pairs {
            assert_eq!(
                alg.distance(a, b).into_inner(),
                damerau_oracle(a, b),
                "Damerau trait impl disagreed with oracle on ({a:?}, {b:?})"
            );
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_workspace_reuse_matches_fresh_workspace() {
        let alg = Damerau;
        let mut ws: DamerauWorkspace<u8> = DamerauWorkspace::new();
        let a: &[u8] = b"prefix-common-tail-A";
        let b: &[u8] = b"prefix-common-tail-B";
        let with_ws = alg.distance_with_workspace(a, b, &mut ws);
        let without_ws = alg.distance(a, b);
        assert_eq!(with_ws, without_ws);
    }

    /// Zero-allocation-on-hot-path invariant: after the workspace has been
    /// warmed up on one comparison, a second comparison of the same shape
    /// must give the same answer as a fresh workspace *and* the same answer
    /// as any subsequent call — the reused `HashMap` and DP matrix must not
    /// corrupt the result. This is the correctness property the
    /// Item-1-motivated `HashMap` reuse in `DamerauWorkspace` protects.
    #[cfg(feature = "std")]
    #[test]
    fn damerau_workspace_reuse_matches_per_call_workspace_across_shapes() {
        let alg = Damerau;
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b""),
            (b"", b"z"),
            (b"ab", b"ba"),
            (b"ca", b"abc"),
            (b"abcd", b"badc"),
            (b"kitten", b"sitting"),
            (b"Saturday", b"Sunday"),
            (b"aaaaa", b"aaaaa"),
            (b"prefix-common-tail-A", b"prefix-common-tail-B"),
            (b"", b"long-tail-after-a-short-lhs"),
        ];
        let mut hot_ws: DamerauWorkspace<u8> = DamerauWorkspace::new();
        for (a, b) in pairs {
            // Fresh workspace each iteration — the baseline.
            let mut cold_ws: DamerauWorkspace<u8> = DamerauWorkspace::new();
            let cold = alg.distance_with_workspace(a, b, &mut cold_ws).into_inner();
            // Reused workspace — must match, even after previous calls left
            // arbitrary residue in the HashMap and DP-matrix cells.
            let hot = alg.distance_with_workspace(a, b, &mut hot_ws).into_inner();
            assert_eq!(
                cold, hot,
                "reused-workspace disagreed with per-call workspace on ({a:?}, {b:?})"
            );
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_and_osa_disagree_on_ca_abc() {
        // The distinguishing example the whole crate exists to distinguish.
        let osa = Osa;
        let dam = Damerau;
        assert_eq!(
            osa.distance(b"ca".as_ref(), b"abc".as_ref()).into_inner(),
            3
        );
        assert_eq!(
            dam.distance(b"ca".as_ref(), b"abc".as_ref()).into_inner(),
            2
        );
    }
}
