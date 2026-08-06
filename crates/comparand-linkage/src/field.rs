//! Per-field agreement rules.
//!
//! A [`FieldComparator`] wraps an underlying similarity or distance metric
//! with the two Fellegi-Sunter parameters (`m_i` and `u_i`) and an
//! agreement threshold that turns the metric's continuous score into a
//! binary agree/disagree pattern. The crate does not itself compute the
//! per-field similarity — the caller feeds
//! [`LinkageModel::score`](crate::LinkageModel::score) a `&[f64]` of
//! precomputed scores — so the [`FieldStrategy`] enum in this module is a
//! descriptive tag that documents *which* comparator produced the score,
//! not a dispatch table.
//!
//! # Why keep the strategy tag at all?
//!
//! The choice of per-field comparator is a semantic decision that affects
//! how downstream systems interpret model output: a "surname" field
//! compared with `JaroWinklerSimilarity` is a different classifier than the
//! same field compared with `LevenshteinNormalized`, and reports, audits,
//! and reproducibility metadata should record which was used. Storing the
//! tag on the [`FieldComparator`] keeps this information alongside the
//! `m`, `u`, and threshold that were derived under the assumption of that
//! comparator, so the four fit together as a single unit downstream code
//! can inspect.

use crate::error::LinkageModelError;

/// The comparator strategy an analyst applied to produce a field's
/// similarity score.
///
/// This is metadata, not dispatch: the [`crate::LinkageModel`] never
/// invokes a comparator directly — the caller precomputes per-field
/// similarities and passes them to
/// [`crate::LinkageModel::score`]. See the module documentation for the
/// rationale.
///
/// The enum is `#[non_exhaustive]` so future comparators (n-gram, phonetic,
/// address-token) can be added without a major-version bump.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FieldStrategy {
    /// Exact byte-for-byte equality on strings; the caller's precomputed
    /// score should be `1.0` for equal values and `0.0` otherwise.
    Exact,
    /// Jaro-Winkler similarity, as implemented by the `comparand-jaro`
    /// crate. Standard choice for personal names.
    JaroWinklerSimilarity,
    /// A Levenshtein-derived normalized similarity — for example
    /// `1.0 - levenshtein(a, b) / max(|a|, |b|)`. Standard choice for
    /// short free-text fields where insertion, deletion, and substitution
    /// are all plausible edits.
    LevenshteinNormalized,
    /// An unspecified comparator; the caller supplies the score directly
    /// under whatever semantics they choose. Useful for phonetic-key
    /// equality, address-token overlap, and other field-specific
    /// comparisons the crate does not enumerate.
    Custom,
}

/// A continuous-to-binary threshold that turns a per-field similarity
/// score into a Fellegi-Sunter agree/disagree pattern.
///
/// [`FieldComparator`] carries this rule inline (as its
/// [`FieldComparator::agreement_threshold`] field) so most callers do not
/// need to reach for [`AgreementRule`] directly. The type is exposed
/// separately for callers that want to reuse a threshold outside of a
/// full [`FieldComparator`] — for example, to display an audit report
/// that shows which fields agreed for each candidate pair.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AgreementRule {
    /// Similarities `>= threshold` count as agreement; strictly less
    /// than `threshold` count as disagreement. Chosen inclusive at
    /// the threshold to match the convention that a threshold value
    /// stated in a report ("agreement at 0.85") lets a similarity of
    /// exactly `0.85` count as agreement.
    pub threshold: f64,
}

impl AgreementRule {
    /// Constructs an agreement rule.
    #[inline]
    #[must_use]
    pub const fn new(threshold: f64) -> Self {
        Self { threshold }
    }

    /// Returns `true` iff `similarity` counts as agreement under this
    /// rule.
    ///
    /// The comparison is `similarity >= threshold`. Any `NaN` similarity
    /// returns `false` because IEEE 754 comparisons against `NaN` are
    /// always false — treating a garbage input as disagreement is the
    /// safer default (it does not fabricate agreement out of missing
    /// data).
    #[inline]
    #[must_use]
    pub fn agrees(&self, similarity: f64) -> bool {
        similarity >= self.threshold
    }
}

/// A single-field comparison rule for the Fellegi-Sunter model.
///
/// [`FieldComparator`] bundles the four pieces of information a
/// [`crate::LinkageModel`] needs about each field:
///
/// * a [`name`](Self::name) for diagnostics and audit reports;
/// * a [`strategy`](Self::strategy) tag naming the comparator that
///   produced the field's similarity score;
/// * an [`agreement_threshold`](Self::agreement_threshold) that turns
///   the continuous similarity into a binary agree/disagree pattern;
/// * the two Fellegi-Sunter parameters
///   [`m_probability`](Self::m_probability) and
///   [`u_probability`](Self::u_probability), which quantify the
///   agreement pattern's discrimination power.
///
/// Fields are `pub` for direct construction of test fixtures and for
/// reflection by audit tooling. Validation happens either when the
/// [`FieldComparator`] is added to a [`crate::LinkageModel`] via
/// [`LinkageModel::new`](crate::LinkageModel::new), or explicitly via
/// [`FieldComparator::validate`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FieldComparator {
    /// The field's name, used only for diagnostics and audit reports.
    /// `&'static str` keeps the type `Copy` and lets test fixtures live
    /// in `const` context.
    pub name: &'static str,
    /// The comparator strategy the analyst applied to produce this
    /// field's similarity score.
    pub strategy: FieldStrategy,
    /// Similarities at or above this threshold count as agreement; those
    /// strictly below count as disagreement. See [`AgreementRule`] for
    /// the underlying rule and its `NaN` convention.
    pub agreement_threshold: f64,
    /// `P(agree | (A, B) is a true match)` — the probability that this
    /// field's agreement pattern is *agree* conditional on the pair
    /// being a true match. Must lie in the open interval `(0, 1)`;
    /// values of `0` or `1` collapse the log-likelihood to `±∞`.
    pub m_probability: f64,
    /// `P(agree | (A, B) is a true non-match)` — the probability that
    /// this field's agreement pattern is *agree* conditional on the
    /// pair being a true non-match. Must lie in the open interval
    /// `(0, 1)`; see the discussion on [`Self::m_probability`].
    pub u_probability: f64,
}

impl FieldComparator {
    /// Constructs a field comparator, validating its parameters.
    ///
    /// # Errors
    ///
    /// Returns [`LinkageModelError::InvalidMProbability`] if `m_probability`
    /// is not a finite value in `(0, 1)`,
    /// [`LinkageModelError::InvalidUProbability`] if `u_probability` fails
    /// the same test, or
    /// [`LinkageModelError::InvalidAgreementThreshold`] if
    /// `agreement_threshold` is non-finite.
    #[inline]
    pub fn new(
        name: &'static str,
        strategy: FieldStrategy,
        agreement_threshold: f64,
        m_probability: f64,
        u_probability: f64,
    ) -> Result<Self, LinkageModelError> {
        let candidate = Self {
            name,
            strategy,
            agreement_threshold,
            m_probability,
            u_probability,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    /// Validates the field's parameters against the Fellegi-Sunter
    /// constraints.
    ///
    /// # Errors
    ///
    /// Same as [`FieldComparator::new`] — see there for the specific
    /// variants returned.
    #[inline]
    pub fn validate(&self) -> Result<(), LinkageModelError> {
        if !self.agreement_threshold.is_finite() {
            return Err(LinkageModelError::InvalidAgreementThreshold {
                field: self.name,
                value: self.agreement_threshold,
            });
        }
        if !is_in_open_unit_interval(self.m_probability) {
            return Err(LinkageModelError::InvalidMProbability {
                field: self.name,
                value: self.m_probability,
            });
        }
        if !is_in_open_unit_interval(self.u_probability) {
            return Err(LinkageModelError::InvalidUProbability {
                field: self.name,
                value: self.u_probability,
            });
        }
        Ok(())
    }

    /// Returns the [`AgreementRule`] equivalent of this field's threshold.
    ///
    /// Convenience for callers that want to reuse the threshold logic
    /// without cloning the whole [`FieldComparator`].
    #[inline]
    #[must_use]
    pub const fn agreement_rule(&self) -> AgreementRule {
        AgreementRule::new(self.agreement_threshold)
    }

    /// Returns `true` iff a supplied similarity counts as agreement for
    /// this field. Convenience wrapper around
    /// [`AgreementRule::agrees`].
    #[inline]
    #[must_use]
    pub fn agrees(&self, similarity: f64) -> bool {
        self.agreement_rule().agrees(similarity)
    }
}

/// Returns `true` iff `x` is finite and in the open interval `(0, 1)`.
#[inline]
fn is_in_open_unit_interval(x: f64) -> bool {
    x.is_finite() && x > 0.0 && x < 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreement_rule_uses_inclusive_lower_bound() {
        let r = AgreementRule::new(0.85);
        assert!(r.agrees(0.85));
        assert!(r.agrees(0.86));
        assert!(!r.agrees(0.84));
    }

    #[test]
    fn agreement_rule_rejects_nan() {
        let r = AgreementRule::new(0.5);
        assert!(!r.agrees(f64::NAN));
    }

    #[test]
    fn validate_accepts_typical_parameters() {
        let f = FieldComparator::new(
            "surname",
            FieldStrategy::JaroWinklerSimilarity,
            0.85,
            0.9,
            0.1,
        )
        .unwrap();
        assert_eq!(f.name, "surname");
        assert!(f.agrees(0.9));
    }

    #[test]
    fn validate_rejects_m_at_zero() {
        let e = FieldComparator::new("x", FieldStrategy::Exact, 0.5, 0.0, 0.1).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidMProbability { .. }));
    }

    #[test]
    fn validate_rejects_m_at_one() {
        let e = FieldComparator::new("x", FieldStrategy::Exact, 0.5, 1.0, 0.1).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidMProbability { .. }));
    }

    #[test]
    fn validate_rejects_u_at_zero() {
        let e = FieldComparator::new("x", FieldStrategy::Exact, 0.5, 0.9, 0.0).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidUProbability { .. }));
    }

    #[test]
    fn validate_rejects_nan_m() {
        let e = FieldComparator::new("x", FieldStrategy::Exact, 0.5, f64::NAN, 0.1).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidMProbability { .. }));
    }

    #[test]
    fn validate_rejects_infinite_threshold() {
        let e =
            FieldComparator::new("x", FieldStrategy::Exact, f64::INFINITY, 0.9, 0.1).unwrap_err();
        assert!(matches!(
            e,
            LinkageModelError::InvalidAgreementThreshold { .. }
        ));
    }
}
