//! The public [`Hamming`] algorithm handle.
//!
//! [`Hamming`] is a zero-size unit struct that bundles the crate's kernels
//! behind the metric traits from `stringcheese-core`, plus a small pair of
//! fallible entry points ([`Hamming::try_distance`],
//! [`Hamming::try_distance_within`]) for callers who cannot statically
//! establish that both inputs are equal-length.
//!
//! # Two entry-point shapes
//!
//! * The trait implementations ([`DistanceMetric`], [`BoundedDistanceMetric`])
//!   **panic** with a clear message on unequal-length inputs. They are the
//!   correct choice when the caller has already established equal length —
//!   for example when both sides come from `&[T; N]` at the same `N`, or from
//!   a fixed-width codeword decoder.
//! * The `try_*` methods return `Result<_, LengthMismatch>` and are the
//!   correct choice everywhere else.
//!
//! The variant descriptor is [`Hamming::DESCRIPTOR`] (also available via the
//! [`Hamming::descriptor`] const function). Golden test cases reference this
//! descriptor rather than the common name.

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, BoundedDistance, BoundedDistanceMetric, DefinitionSource,
    DescriptorVersion, Distance, DistanceMetric, MetricClass, MetricProperties, VariantId,
};

use crate::hamming::error::LengthMismatch;
use crate::hamming::kernel::{hamming_distance, hamming_distance_within};

/// The equal-length Hamming distance.
///
/// The distance is the number of positions at which two sequences of the
/// same length differ under `T`'s [`Eq`] implementation. On unequal-length
/// input the [`DistanceMetric`] and [`BoundedDistanceMetric`] impls **panic**;
/// callers who cannot statically prove equal-length inputs should use the
/// fallible [`try_distance`] and [`try_distance_within`] methods instead.
///
/// Introduced by Hamming (1950) as the theoretical footing for binary
/// error-detecting and error-correcting codes; see the crate-level
/// `References` section for the full citation.
///
/// The type is a zero-sized unit struct; construct it as `Hamming` and reuse
/// the value across threads.
///
/// [`try_distance`]: Hamming::try_distance
/// [`try_distance_within`]: Hamming::try_distance_within
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hamming;

impl Hamming {
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"equal-length-generic-eq"` names the two properties
    /// that distinguish this implementation from any future sibling: equality
    /// is required over the full lengths, and comparison happens through
    /// generic `T: Eq` rather than a specialized bit-parallel byte kernel or
    /// a weighted-mismatch cost scheme.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Hamming,
        variant: VariantId("equal-length-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Error detecting and error correcting codes",
            authors: "R. W. Hamming",
            year: 1950,
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

    /// Returns the mathematical properties Hamming satisfies.
    ///
    /// Hamming is a true metric on the space of equal-length sequences: it
    /// is symmetric, non-negative, satisfies the identity of indiscernibles
    /// under `T: Eq`, and satisfies the triangle inequality. It is not
    /// naturally normalized — the maximum distance for length-`n` sequences
    /// is `n`, not `1.0`.
    ///
    /// This is a `const` accessor for use in `const` contexts where the
    /// trait method is not available.
    #[inline]
    #[must_use]
    pub const fn properties() -> MetricProperties {
        MetricProperties::METRIC
    }

    /// Returns Hamming's mathematical classification.
    ///
    /// See [`properties`](Hamming::properties) for the axioms this
    /// classification summarizes.
    #[inline]
    #[must_use]
    pub const fn class() -> MetricClass {
        MetricClass::Metric
    }

    /// Computes the Hamming distance between `left` and `right`, or returns
    /// [`LengthMismatch`] if their lengths differ.
    ///
    /// This is the fallible counterpart to the [`DistanceMetric::distance`]
    /// trait method, which panics on the same condition. Callers who cannot
    /// statically prove equal-length inputs should prefer this method.
    ///
    /// # Errors
    ///
    /// Returns [`LengthMismatch`] if `left.len() != right.len()`. The error
    /// carries both operand lengths for diagnostic output.
    #[inline]
    pub fn try_distance<T: Eq>(
        self,
        left: &[T],
        right: &[T],
    ) -> Result<Distance<u32>, LengthMismatch> {
        if left.len() != right.len() {
            return Err(LengthMismatch {
                left: left.len(),
                right: right.len(),
            });
        }
        Ok(hamming_distance(left, right))
    }

    /// Computes the Hamming distance with an early-termination cutoff, or
    /// returns [`LengthMismatch`] if the lengths differ.
    ///
    /// This is the fallible counterpart to
    /// [`BoundedDistanceMetric::distance_within`], which panics on the same
    /// condition.
    ///
    /// # Errors
    ///
    /// Returns [`LengthMismatch`] if `left.len() != right.len()`. The error
    /// carries both operand lengths for diagnostic output.
    #[inline]
    pub fn try_distance_within<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        cutoff: u32,
    ) -> Result<BoundedDistance<u32>, LengthMismatch> {
        if left.len() != right.len() {
            return Err(LengthMismatch {
                left: left.len(),
                right: right.len(),
            });
        }
        Ok(hamming_distance_within(left, right, cutoff))
    }
}

impl<T: Eq> DistanceMetric<[T]> for Hamming {
    type Output = u32;

    /// Computes the Hamming distance.
    ///
    /// # Panics
    ///
    /// Panics if `left.len() != right.len()`. Use [`Hamming::try_distance`]
    /// for a fallible variant that returns [`LengthMismatch`] instead.
    #[inline]
    fn distance(&self, left: &[T], right: &[T]) -> Distance<Self::Output> {
        hamming_distance(left, right)
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

impl<T: Eq> BoundedDistanceMetric<[T]> for Hamming {
    /// Computes the Hamming distance with an early-termination cutoff.
    ///
    /// # Panics
    ///
    /// Panics if `left.len() != right.len()`. Use
    /// [`Hamming::try_distance_within`] for a fallible variant that returns
    /// [`LengthMismatch`] instead.
    #[inline]
    fn distance_within(
        &self,
        left: &[T],
        right: &[T],
        cutoff: Self::Output,
    ) -> BoundedDistance<Self::Output> {
        hamming_distance_within(left, right, cutoff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = Hamming::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Hamming);
        assert_eq!(d.variant, VariantId("equal-length-generic-eq"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1950, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        // Available at const time, so downstream can pin it into
        // `const GOLDEN_CASE: GoldenCase<...>` records.
        const D: AlgorithmDescriptor = Hamming::DESCRIPTOR;
        assert_eq!(D.variant.0, "equal-length-generic-eq");
    }

    #[test]
    fn declares_true_metric() {
        let alg = Hamming;
        // `DistanceMetric` is generic over the sequence type, so its
        // associated methods need a concrete `S` to be resolvable — the
        // caller-facing name `alg.class()` would otherwise be ambiguous.
        assert_eq!(
            <Hamming as DistanceMetric<[u8]>>::class(&alg),
            MetricClass::Metric
        );
        assert!(<Hamming as DistanceMetric<[u8]>>::properties(&alg).is_metric());
    }

    #[test]
    fn const_accessors_agree_with_trait_methods() {
        let alg = Hamming;
        assert_eq!(
            Hamming::properties(),
            <Hamming as DistanceMetric<[u8]>>::properties(&alg),
        );
        assert_eq!(
            Hamming::class(),
            <Hamming as DistanceMetric<[u8]>>::class(&alg),
        );
    }

    #[test]
    fn distance_metric_impl_agrees_with_hand_counts() {
        let alg = Hamming;
        let pairs: &[(&[u8], &[u8], u32)] = &[
            (b"", b"", 0),
            (b"a", b"a", 0),
            (b"karolin", b"kathrin", 3),
            (b"karolin", b"kerstin", 3),
            (b"1011101", b"1001001", 2),
            (b"abc", b"xyz", 3),
        ];
        for (a, b, expected) in pairs {
            assert_eq!(
                alg.distance(a, b).into_inner(),
                *expected,
                "unexpected Hamming distance for ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn bounded_distance_metric_impl_matches_hand_counts() {
        let alg = Hamming;
        for (a, b, expected) in &[
            (b"karolin".as_ref(), b"kathrin".as_ref(), 3u32),
            (b"1011101".as_ref(), b"1001001".as_ref(), 2u32),
            (b"abc".as_ref(), b"xyz".as_ref(), 3u32),
        ] {
            for k in 0..=8 {
                let observed = alg.distance_within(a, b, k);
                if *expected <= k {
                    assert_eq!(observed, BoundedDistance::Within(Distance::new(*expected)));
                } else {
                    assert_eq!(observed, BoundedDistance::Exceeded { cutoff: k });
                }
            }
        }
    }

    #[test]
    fn try_distance_reports_length_mismatch() {
        let alg = Hamming;
        let e = alg.try_distance::<u8>(b"abc", b"abcd").unwrap_err();
        assert_eq!(e, LengthMismatch { left: 3, right: 4 });
    }

    #[test]
    fn try_distance_within_reports_length_mismatch() {
        let alg = Hamming;
        let e = alg
            .try_distance_within::<u8>(b"abcdef", b"abc", 2)
            .unwrap_err();
        assert_eq!(e, LengthMismatch { left: 6, right: 3 });
    }

    #[test]
    fn try_distance_agrees_with_trait_on_valid_input() {
        let alg = Hamming;
        let a: &[u8] = b"karolin";
        let b: &[u8] = b"kathrin";
        assert_eq!(alg.try_distance(a, b).unwrap(), alg.distance(a, b));
    }

    #[test]
    fn try_distance_within_agrees_with_trait_on_valid_input() {
        let alg = Hamming;
        let a: &[u8] = b"karolin";
        let b: &[u8] = b"kathrin";
        assert_eq!(
            alg.try_distance_within(a, b, 5).unwrap(),
            alg.distance_within(a, b, 5),
        );
    }

    #[test]
    #[should_panic(expected = "equal-length")]
    fn trait_distance_panics_on_length_mismatch() {
        let alg = Hamming;
        // Bind through a `&[u8]`-typed reference so the compiler can pick the
        // `DistanceMetric<[u8]>` impl without a turbofish.
        let left: &[u8] = b"abc";
        let right: &[u8] = b"abcd";
        let _ = alg.distance(left, right);
    }

    #[test]
    #[should_panic(expected = "equal-length")]
    fn trait_distance_within_panics_on_length_mismatch() {
        let alg = Hamming;
        let left: &[u8] = b"abc";
        let right: &[u8] = b"abcd";
        let _ = alg.distance_within(left, right, 2);
    }
}
