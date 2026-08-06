//! The [`JaroWinkler`] family of similarities.
//!
//! Winkler (1990) proposed a *boost* on top of the base Jaro similarity that
//! favors pairs sharing a common prefix — an empirical observation from
//! record-linkage work that surname disagreements are much more likely at
//! the end of a string than the beginning. See the crate-level `References`
//! section for the full citation. The boost is
//!
//! ```text
//!     jw = jaro + p * s * (1 - jaro)
//! ```
//!
//! where `p` is the length of the common prefix, capped at a prefix limit
//! (typically 4), `s` is a scaling factor (typically 0.1), and the boost is
//! applied only when the base Jaro score meets or exceeds a threshold
//! (typically 0.0 in the original 1990 formulation and 0.7 in Winkler's
//! later modification).
//!
//! # A variant family, not a single algorithm
//!
//! The three parameters — prefix limit, scaling, boost threshold —
//! are exactly the kind of configuration the design document identifies as
//! belonging to the [algorithm-variant registry](../../docs/DESIGN.md).
//! Two named constructors pin the canonical historical variants ([`classic`]
//! and [`with_threshold`]), each returning an instance whose
//! [`descriptor`] returns a distinct [`AlgorithmDescriptor`] so golden cases
//! for one cannot silently validate against the other. Arbitrary
//! configurations are available through [`new`], and are validated against
//! the invariant that keeps the boosted output bounded to `[0.0, 1.0]`.
//!
//! [`classic`]: JaroWinkler::classic
//! [`with_threshold`]: JaroWinkler::with_threshold
//! [`descriptor`]: JaroWinkler::descriptor
//! [`new`]: JaroWinkler::new

use core::fmt;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass,
    MetricProperties, NormalizedSimilarity, Similarity, SimilarityMetric, VariantId,
};

use crate::jaro::{jaro_similarity, jaro_similarity_with_workspace};
use crate::workspace::JaroWorkspace;

/// The Jaro-Winkler similarity family.
///
/// The prefix-boost variant introduced by Winkler (1990) on top of Jaro's
/// (1989) base similarity; see the crate-level `References` section for
/// full citations.
///
/// Configured by three parameters:
///
/// * `prefix_limit` — the maximum length of the common prefix that
///   contributes to the boost. Winkler's original paper caps this at 4.
/// * `scaling` — the weight applied to each prefix character. Winkler's
///   value is 0.1; must satisfy `scaling * prefix_limit <= 1.0` so the
///   boosted output cannot exceed `1.0`.
/// * `boost_threshold` — the minimum Jaro score at which the boost is
///   applied. Winkler's 1990 paper sets this to `0.0` (always boost); his
///   later modification sets it to `0.7`, on the grounds that boosting
///   already-low similarities inflates false positives.
///
/// Construct via [`JaroWinkler::classic`] or [`JaroWinkler::with_threshold`]
/// for the historically canonical variants, or [`JaroWinkler::new`] for a
/// custom configuration.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct JaroWinkler {
    /// Cap on the common-prefix length that contributes to the boost.
    pub prefix_limit: u8,
    /// Per-character scaling weight applied to the common prefix.
    pub scaling: f64,
    /// Minimum base Jaro score at which the boost is applied. Below this
    /// threshold the algorithm returns the Jaro score unmodified.
    pub boost_threshold: f64,
}

impl JaroWinkler {
    /// Descriptor for the classic Winkler-1990 variant returned by
    /// [`JaroWinkler::classic`].
    pub const CLASSIC_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::JaroWinkler,
        variant: VariantId("winkler-1990-limit-4-scaling-0.1"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "String Comparator Metrics and Enhanced Decision Rules in the Fellegi-Sunter Model of Record Linkage",
            authors: "William E. Winkler",
            year: 1990,
        },
    };

    // NOTE (descriptor `source` choice): `DefinitionSource::Paper` holds a
    // single citation, so we cannot list both Winkler (1990) — the base
    // Jaro-Winkler paper — and Winkler (1999) — the paper introducing the
    // 0.7-threshold gating this variant implements. We cite 1999 because
    // this descriptor identifies the *specific variant* (threshold-gated),
    // not the Jaro-Winkler family in general; the family paper lives on
    // `CLASSIC_DESCRIPTOR` and the crate-level `References` section lists
    // both papers for readers tracing the historical relationship.
    /// Descriptor for the threshold-gated variant returned by
    /// [`JaroWinkler::with_threshold`].
    pub const WITH_THRESHOLD_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::JaroWinkler,
        variant: VariantId("winkler-limit-4-scaling-0.1-threshold-0.7"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "The State of Record Linkage and Current Research Problems",
            authors: "William E. Winkler",
            year: 1999,
        },
    };

    /// Descriptor for arbitrary configurations produced by
    /// [`JaroWinkler::new`].
    ///
    /// The descriptor records only the family and a `"configured"` slug;
    /// the specific parameter values are not encoded in the [`VariantId`]
    /// because slugs are `&'static str` and floating-point parameters do
    /// not have canonical decimal representations. Consumers that need to
    /// distinguish specific parameterizations should record the parameter
    /// values alongside the descriptor.
    pub const CONFIGURED_DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::JaroWinkler,
        variant: VariantId("configured"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::IndependentlyDerived,
    };

    /// Returns the Winkler-1990 canonical configuration: prefix limit 4,
    /// scaling 0.1, boost threshold 0.0 (always apply the boost).
    #[inline]
    #[must_use]
    pub const fn classic() -> Self {
        Self {
            prefix_limit: 4,
            scaling: 0.1,
            boost_threshold: 0.0,
        }
    }

    /// Returns Winkler's threshold-gated configuration: prefix limit 4,
    /// scaling 0.1, boost threshold 0.7. The boost is applied only when the
    /// base Jaro score is at least 0.7; below the threshold the Jaro score
    /// is returned unmodified.
    #[inline]
    #[must_use]
    pub const fn with_threshold() -> Self {
        Self {
            prefix_limit: 4,
            scaling: 0.1,
            boost_threshold: 0.7,
        }
    }

    /// Constructs an arbitrary Jaro-Winkler configuration.
    ///
    /// # Errors
    ///
    /// Returns [`JaroWinklerError`] if the parameters would allow the
    /// boosted output to exceed `1.0` — specifically, if `scaling` is
    /// negative or non-finite, if `boost_threshold` is outside `[0.0, 1.0]`
    /// or non-finite, or if `scaling * prefix_limit > 1.0`.
    ///
    /// # Design choice: `Result`, not panic
    ///
    /// This constructor returns a `Result` rather than panicking on invalid
    /// input. The check is a domain constraint (like a range check on a
    /// port number), not a debug-only sanity check; a panic here would
    /// abort a program that could otherwise recover from a user-supplied
    /// bad configuration.
    #[inline]
    pub fn new(
        prefix_limit: u8,
        scaling: f64,
        boost_threshold: f64,
    ) -> Result<Self, JaroWinklerError> {
        if !scaling.is_finite() || scaling < 0.0 {
            return Err(JaroWinklerError::InvalidScaling { scaling });
        }
        if !boost_threshold.is_finite() || !(0.0..=1.0).contains(&boost_threshold) {
            return Err(JaroWinklerError::InvalidBoostThreshold { boost_threshold });
        }
        // `scaling * prefix_limit <= 1.0` is the invariant that keeps the
        // boost from pushing an already-high Jaro score above 1.0.
        // Multiplying in f64 is exact here for the small integer prefix
        // limits Winkler considers; we do the check in f64 for uniform
        // treatment of arbitrary future scaling values.
        if scaling * f64::from(prefix_limit) > 1.0 {
            return Err(JaroWinklerError::PrefixScalingExceedsUnity {
                prefix_limit,
                scaling,
                product: scaling * f64::from(prefix_limit),
            });
        }
        Ok(Self {
            prefix_limit,
            scaling,
            boost_threshold,
        })
    }

    /// Returns the algorithm descriptor for this configuration.
    ///
    /// Configurations produced by the named constructors [`classic`] and
    /// [`with_threshold`] return their respective canonical descriptors;
    /// configurations produced by [`new`] return
    /// [`CONFIGURED_DESCRIPTOR`].
    ///
    /// [`classic`]: JaroWinkler::classic
    /// [`with_threshold`]: JaroWinkler::with_threshold
    /// [`new`]: JaroWinkler::new
    /// [`CONFIGURED_DESCRIPTOR`]: JaroWinkler::CONFIGURED_DESCRIPTOR
    #[inline]
    #[must_use]
    pub fn descriptor(&self) -> AlgorithmDescriptor {
        // Compare against the canonical parameter tuples by exact bit
        // pattern; the two named constructors both write literal
        // `0.1`, `0.7`, and `0.0` which round to fixed IEEE 754
        // representations, so bit equality is exactly what we want.
        if self.prefix_limit == 4
            && self.scaling.to_bits() == 0.1_f64.to_bits()
            && self.boost_threshold.to_bits() == 0.0_f64.to_bits()
        {
            Self::CLASSIC_DESCRIPTOR
        } else if self.prefix_limit == 4
            && self.scaling.to_bits() == 0.1_f64.to_bits()
            && self.boost_threshold.to_bits() == 0.7_f64.to_bits()
        {
            Self::WITH_THRESHOLD_DESCRIPTOR
        } else {
            Self::CONFIGURED_DESCRIPTOR
        }
    }

    /// Computes the Jaro-Winkler similarity between `left` and `right`,
    /// returning the range-checked [`NormalizedSimilarity`] wrapper.
    ///
    /// This is the preferred API for consumers that carry the result across
    /// module boundaries; see [`Jaro::similarity_normalized`] for the
    /// rationale.
    ///
    /// [`Jaro::similarity_normalized`]: crate::jaro::Jaro::similarity_normalized
    #[inline]
    #[must_use]
    pub fn similarity_normalized<T: Eq>(&self, left: &[T], right: &[T]) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(self.raw_similarity(left, right))
    }

    /// The raw floating-point Jaro-Winkler score. Internal helper used by
    /// [`similarity_normalized`] and the [`SimilarityMetric`] impl.
    ///
    /// Allocates a throw-away Jaro workspace internally; batch callers
    /// should prefer
    /// [`similarity_with_workspace`](Self::similarity_with_workspace)
    /// instead.
    ///
    /// [`similarity_normalized`]: JaroWinkler::similarity_normalized
    #[inline]
    fn raw_similarity<T: Eq>(&self, a: &[T], b: &[T]) -> f64 {
        self.apply_boost(a, b, jaro_similarity(a, b))
    }

    /// Workspace-aware Jaro-Winkler similarity: computes the base Jaro
    /// score reusing the bitmaps in `ws`, then applies the prefix boost.
    ///
    /// The workspace holds the same two `Vec<bool>` buffers the base Jaro
    /// kernel uses; there is no separate Jaro-Winkler workspace, because
    /// the boost itself is `O(prefix_limit)` and takes no scratch.
    #[inline]
    #[must_use]
    pub fn similarity_with_workspace<T: Eq>(
        &self,
        left: &[T],
        right: &[T],
        ws: &mut JaroWorkspace,
    ) -> Similarity<f64> {
        let base = jaro_similarity_with_workspace(left, right, ws);
        Similarity::new(self.apply_boost(left, right, base))
    }

    /// Workspace-aware Jaro-Winkler similarity in the range-checked
    /// [`NormalizedSimilarity`] wrapper.
    #[inline]
    #[must_use]
    pub fn similarity_with_workspace_normalized<T: Eq>(
        &self,
        left: &[T],
        right: &[T],
        ws: &mut JaroWorkspace,
    ) -> NormalizedSimilarity {
        let base = jaro_similarity_with_workspace(left, right, ws);
        NormalizedSimilarity::new_unchecked(self.apply_boost(left, right, base))
    }

    /// Applies the prefix boost to an already-computed base Jaro score.
    /// Shared by the allocating and workspace-aware entry points.
    #[inline]
    fn apply_boost<T: Eq>(&self, a: &[T], b: &[T], base: f64) -> f64 {
        if base < self.boost_threshold {
            // Below the threshold, no boost is applied. This produces
            // bit-exact equality with the base Jaro score — the threshold
            // variant's `jw = jaro` property relies on that.
            return base;
        }
        // Common prefix, capped at `prefix_limit`. Iterating both slices in
        // lockstep is O(min(|a|, |b|, prefix_limit)) and takes no scratch.
        let cap = usize::from(self.prefix_limit);
        let mut prefix: usize = 0;
        for (x, y) in a.iter().zip(b.iter()).take(cap) {
            if x != y {
                break;
            }
            prefix += 1;
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "prefix is bounded by prefix_limit (u8), so it always fits exactly in an f64"
        )]
        let prefix_f = prefix as f64;
        base + prefix_f * self.scaling * (1.0 - base)
    }
}

impl<T: Eq> SimilarityMetric<[T]> for JaroWinkler {
    type Output = f64;

    #[inline]
    fn similarity(&self, left: &[T], right: &[T]) -> Similarity<Self::Output> {
        Similarity::new(self.raw_similarity(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        // Same as Jaro: symmetric, identity of indiscernibles (`sim(x, x) = 1`
        // since the boost multiplies by `(1 - 1) = 0`), non-negative,
        // normalized, no triangle inequality.
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

/// Error returned by [`JaroWinkler::new`] when the parameters would allow
/// the boosted output to escape the `[0.0, 1.0]` range.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum JaroWinklerError {
    /// `scaling` was negative, `NaN`, or infinite.
    InvalidScaling {
        /// The offending scaling value.
        scaling: f64,
    },
    /// `boost_threshold` was outside `[0.0, 1.0]`, `NaN`, or infinite.
    InvalidBoostThreshold {
        /// The offending threshold value.
        boost_threshold: f64,
    },
    /// `scaling * prefix_limit > 1.0`; the boost could push the output
    /// above `1.0`, violating the [`NormalizedSimilarity`] invariant.
    PrefixScalingExceedsUnity {
        /// The prefix limit passed in.
        prefix_limit: u8,
        /// The scaling factor passed in.
        scaling: f64,
        /// The product `scaling * prefix_limit`.
        product: f64,
    },
}

impl fmt::Display for JaroWinklerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScaling { scaling } => {
                write!(f, "scaling must be finite and non-negative, got {scaling}")
            }
            Self::InvalidBoostThreshold { boost_threshold } => {
                write!(
                    f,
                    "boost_threshold must be finite and in [0.0, 1.0], got {boost_threshold}"
                )
            }
            Self::PrefixScalingExceedsUnity {
                prefix_limit,
                scaling,
                product,
            } => {
                write!(
                    f,
                    "scaling * prefix_limit must not exceed 1.0 (got {scaling} * {prefix_limit} = {product})"
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for JaroWinklerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_descriptor_slug() {
        let alg = JaroWinkler::classic();
        assert_eq!(
            alg.descriptor().variant,
            VariantId("winkler-1990-limit-4-scaling-0.1")
        );
    }

    #[test]
    fn with_threshold_descriptor_slug() {
        let alg = JaroWinkler::with_threshold();
        assert_eq!(
            alg.descriptor().variant,
            VariantId("winkler-limit-4-scaling-0.1-threshold-0.7")
        );
    }

    #[test]
    fn arbitrary_configuration_is_marked_configured() {
        let alg = JaroWinkler::new(3, 0.05, 0.5).unwrap();
        assert_eq!(alg.descriptor().variant, VariantId("configured"));
    }

    #[test]
    fn new_rejects_scaling_prefix_product_over_one() {
        // 5 * 0.25 = 1.25, would allow boosts above 1.0.
        let err = JaroWinkler::new(5, 0.25, 0.7).unwrap_err();
        assert!(matches!(
            err,
            JaroWinklerError::PrefixScalingExceedsUnity { .. }
        ));
    }

    #[test]
    fn new_rejects_bad_scaling() {
        assert!(matches!(
            JaroWinkler::new(4, -0.1, 0.7).unwrap_err(),
            JaroWinklerError::InvalidScaling { .. }
        ));
        assert!(matches!(
            JaroWinkler::new(4, f64::NAN, 0.7).unwrap_err(),
            JaroWinklerError::InvalidScaling { .. }
        ));
    }

    #[test]
    fn new_rejects_bad_threshold() {
        assert!(matches!(
            JaroWinkler::new(4, 0.1, -0.1).unwrap_err(),
            JaroWinklerError::InvalidBoostThreshold { .. }
        ));
        assert!(matches!(
            JaroWinkler::new(4, 0.1, 1.5).unwrap_err(),
            JaroWinklerError::InvalidBoostThreshold { .. }
        ));
    }

    #[test]
    fn class_and_properties_declare_bounded_similarity() {
        let alg = JaroWinkler::classic();
        assert_eq!(
            <JaroWinkler as SimilarityMetric<[u8]>>::class(&alg),
            MetricClass::Similarity
        );
        let p = <JaroWinkler as SimilarityMetric<[u8]>>::properties(&alg);
        assert!(p.symmetric);
        assert!(p.identity_of_indiscernibles);
        assert!(!p.triangle_inequality);
        assert!(p.non_negative);
        assert!(p.normalized);
    }

    #[test]
    fn classic_martha_marhta_matches_published_value() {
        let alg = JaroWinkler::classic();
        let s = alg.similarity(b"MARTHA", b"MARHTA").into_inner();
        // Jaro = 17/18; prefix = 3 (MAR); boost = 3 * 0.1 * (1 - 17/18) = 1/60.
        let expected = 17.0_f64 / 18.0_f64 + 3.0 * 0.1 * (1.0 - 17.0_f64 / 18.0_f64);
        assert_eq!(s.to_bits(), expected.to_bits());
    }

    #[test]
    fn with_threshold_leaves_low_scores_alone() {
        let alg = JaroWinkler::with_threshold();
        // "abc" vs "xyz" — Jaro = 0 (< 0.7 threshold), so the boost is not
        // applied and jw equals jaro exactly.
        let jw = alg.similarity(b"abc", b"xyz").into_inner();
        let j = jaro_similarity(b"abc", b"xyz");
        assert_eq!(jw.to_bits(), j.to_bits());
    }

    #[test]
    fn identical_still_yields_one() {
        let alg = JaroWinkler::classic();
        let s = alg.similarity(b"identity", b"identity").into_inner();
        assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn error_display_formats_readably() {
        let e = JaroWinkler::new(5, 0.25, 0.7).unwrap_err();
        let msg = alloc::format!("{e}");
        assert!(msg.contains("scaling * prefix_limit"));
    }
}
