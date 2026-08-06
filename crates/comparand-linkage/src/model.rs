//! The [`LinkageModel`] classifier.
//!
//! [`LinkageModel`] is the crate's top-level type: a Fellegi-Sunter (1969)
//! classifier that combines a fixed sequence of per-field
//! [`FieldComparator`]s into a scoring and decision pipeline.
//!
//! # Construction and precomputation
//!
//! [`LinkageModel::new`] validates the model's parameters — the two
//! thresholds must be finite and strictly ordered, every per-field
//! probability must lie in `(0, 1)` — and precomputes the two
//! [`crate::weight::agree_weight`] / [`crate::weight::disagree_weight`]
//! values for each field. The precomputed table is stored on the model
//! so that per-pair scoring is a plain floating-point summation with no
//! `log2` in the inner loop. For a model with `K` fields, scoring a
//! candidate pair is `O(K)` additions plus `K` comparisons.
//!
//! # Scoring and classification
//!
//! [`LinkageModel::score`] takes a slice of per-field similarity scores
//! (paired positionally with the fields declared at construction) and
//! returns the record-pair weight. [`LinkageModel::classify`] returns
//! the [`LinkageDecision`]; [`LinkageModel::classify_weight`] short-
//! circuits the arithmetic and takes a precomputed weight directly, which
//! is useful in tests and in explanation reports that show the weight
//! and the decision side by side.

use alloc::vec::Vec;

use crate::classifier::LinkageDecision;
use crate::error::LinkageModelError;
use crate::field::FieldComparator;
use crate::weight::{agree_weight, disagree_weight};

/// A Fellegi-Sunter (1969) probabilistic record-linkage classifier.
///
/// Combines a fixed sequence of per-field [`FieldComparator`]s into a
/// two-threshold classifier that maps candidate record pairs to one of
/// the three [`LinkageDecision`] outcomes.
///
/// The model is immutable once constructed: the field list, thresholds,
/// and precomputed per-field weights cannot be changed. Callers that
/// need a different configuration should build a new model.
#[derive(Clone, Debug)]
pub struct LinkageModel {
    /// The per-field comparators, in declaration order. The order is
    /// meaningful — [`LinkageModel::score`] pairs each entry positionally
    /// with a supplied per-field similarity score.
    fields: Vec<FieldComparator>,
    /// Precomputed `(agree_weight, disagree_weight)` for each field,
    /// stored in the same order as `fields`. Keeping this here (rather
    /// than recomputing inside [`agree_weight`] / [`disagree_weight`]
    /// on every score call) turns the per-pair scoring path into a
    /// straight-line floating-point summation with no transcendental
    /// calls in the inner loop.
    precomputed_weights: Vec<(f64, f64)>,
    /// The upper threshold `T_μ`. A weight at or above this value is
    /// classified as [`LinkageDecision::Match`].
    match_threshold: f64,
    /// The lower threshold `T_λ`. A weight at or below this value is
    /// classified as [`LinkageDecision::NonMatch`].
    non_match_threshold: f64,
}

impl LinkageModel {
    /// Constructs a linkage model.
    ///
    /// # Errors
    ///
    /// Returns [`LinkageModelError::NoFields`] if `fields` is empty;
    /// [`LinkageModelError::InvalidThresholds`] if the two thresholds are
    /// non-finite or not strictly ordered
    /// (`non_match_threshold < match_threshold`); or one of
    /// [`LinkageModelError::InvalidMProbability`],
    /// [`LinkageModelError::InvalidUProbability`],
    /// [`LinkageModelError::InvalidAgreementThreshold`] if any field's
    /// parameters are outside their required ranges.
    pub fn new(
        fields: Vec<FieldComparator>,
        match_threshold: f64,
        non_match_threshold: f64,
    ) -> Result<Self, LinkageModelError> {
        if fields.is_empty() {
            return Err(LinkageModelError::NoFields);
        }
        if !match_threshold.is_finite()
            || !non_match_threshold.is_finite()
            || non_match_threshold >= match_threshold
        {
            return Err(LinkageModelError::InvalidThresholds {
                match_threshold,
                non_match_threshold,
            });
        }
        for f in &fields {
            f.validate()?;
        }
        let precomputed_weights = fields
            .iter()
            .map(|f| {
                (
                    agree_weight(f.m_probability, f.u_probability),
                    disagree_weight(f.m_probability, f.u_probability),
                )
            })
            .collect();
        Ok(Self {
            fields,
            precomputed_weights,
            match_threshold,
            non_match_threshold,
        })
    }

    /// Returns the per-field comparators, in declaration order.
    #[inline]
    #[must_use]
    pub fn fields(&self) -> &[FieldComparator] {
        &self.fields
    }

    /// Returns the upper threshold `T_μ`. Weights at or above this
    /// value classify as [`LinkageDecision::Match`].
    #[inline]
    #[must_use]
    pub const fn match_threshold(&self) -> f64 {
        self.match_threshold
    }

    /// Returns the lower threshold `T_λ`. Weights at or below this
    /// value classify as [`LinkageDecision::NonMatch`].
    #[inline]
    #[must_use]
    pub const fn non_match_threshold(&self) -> f64 {
        self.non_match_threshold
    }

    /// Returns the number of fields declared on the model.
    ///
    /// This is the required length of the `field_similarities` slice
    /// passed to [`LinkageModel::score`] and [`LinkageModel::classify`].
    #[inline]
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Returns the precomputed `(agree_weight, disagree_weight)` for
    /// each field, in declaration order. Primarily useful for audit
    /// reports and property tests; typical callers should go through
    /// [`LinkageModel::score`] instead.
    #[inline]
    #[must_use]
    pub fn precomputed_weights(&self) -> &[(f64, f64)] {
        &self.precomputed_weights
    }

    /// Computes the Fellegi-Sunter log-likelihood weight for a pair of
    /// records, given a per-field similarity for every declared field.
    ///
    /// The caller is responsible for pairing fields with scores in the
    /// same order the model was constructed with. Similarities `>=`
    /// their field's threshold contribute the precomputed
    /// `agree_weight`; those strictly below contribute
    /// `disagree_weight`.
    ///
    /// # Panics
    ///
    /// Panics if `field_similarities.len() != self.field_count()`. This
    /// is a fail-loud precondition: a length mismatch is a bug at the
    /// call site (the analyst declared `K` fields but supplied a
    /// different number of scores), not a recoverable runtime state. A
    /// fallible counterpart [`LinkageModel::try_score`] returns a
    /// [`LinkageScoreError`] instead of panicking, for callers that
    /// cannot statically prove the lengths agree.
    #[must_use]
    pub fn score(&self, field_similarities: &[f64]) -> f64 {
        assert!(
            field_similarities.len() == self.fields.len(),
            "LinkageModel::score: expected {} field similarities, got {}",
            self.fields.len(),
            field_similarities.len(),
        );
        self.weight_unchecked(field_similarities)
    }

    /// Fallible counterpart to [`LinkageModel::score`].
    ///
    /// # Errors
    ///
    /// Returns [`LinkageScoreError::LengthMismatch`] if the caller
    /// supplied a different number of similarities than the model
    /// declared fields for.
    #[inline]
    pub fn try_score(&self, field_similarities: &[f64]) -> Result<f64, LinkageScoreError> {
        if field_similarities.len() != self.fields.len() {
            return Err(LinkageScoreError::LengthMismatch {
                expected: self.fields.len(),
                actual: field_similarities.len(),
            });
        }
        Ok(self.weight_unchecked(field_similarities))
    }

    /// Classifies a candidate pair based on its per-field similarities.
    ///
    /// # Panics
    ///
    /// Panics under the same condition as [`LinkageModel::score`]. Use
    /// [`LinkageModel::try_classify`] for the fallible counterpart.
    #[must_use]
    pub fn classify(&self, field_similarities: &[f64]) -> LinkageDecision {
        self.classify_weight(self.score(field_similarities))
    }

    /// Fallible counterpart to [`LinkageModel::classify`].
    ///
    /// # Errors
    ///
    /// Returns [`LinkageScoreError::LengthMismatch`] on a length
    /// mismatch (same condition as [`LinkageModel::try_score`]).
    #[inline]
    pub fn try_classify(
        &self,
        field_similarities: &[f64],
    ) -> Result<LinkageDecision, LinkageScoreError> {
        Ok(self.classify_weight(self.try_score(field_similarities)?))
    }

    /// Classifies a candidate pair based on a precomputed weight.
    ///
    /// Useful in audit reports that display the weight alongside the
    /// decision, and in property tests that want to exercise the
    /// two-threshold logic without also exercising the summation.
    #[inline]
    #[must_use]
    pub fn classify_weight(&self, weight: f64) -> LinkageDecision {
        // NaN never satisfies `>=` or `<=`, so a NaN weight falls
        // through to the middle region and is classified as
        // PossibleMatch. That is the safer default: an unclassifiable
        // weight becomes a clerical-review case, not a silent Match.
        if weight >= self.match_threshold {
            LinkageDecision::Match
        } else if weight <= self.non_match_threshold {
            LinkageDecision::NonMatch
        } else {
            LinkageDecision::PossibleMatch
        }
    }

    /// The unchecked scoring path — assumes the length precondition has
    /// already been verified by the caller.
    #[inline]
    fn weight_unchecked(&self, field_similarities: &[f64]) -> f64 {
        let mut total = 0.0_f64;
        for (i, sim) in field_similarities.iter().enumerate() {
            let (agree, disagree) = self.precomputed_weights[i];
            let contribution = if self.fields[i].agrees(*sim) {
                agree
            } else {
                disagree
            };
            total += contribution;
        }
        total
    }
}

/// Error returned by the fallible scoring entry points on a length
/// mismatch between the model's field count and the supplied
/// per-field similarity slice.
///
/// The `#[non_exhaustive]` marker keeps future error variants (e.g.
/// non-finite similarity handling under a stricter mode) additive.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LinkageScoreError {
    /// The caller supplied `actual` similarities but the model was
    /// constructed with `expected` fields. Fellegi-Sunter scoring is
    /// positional (see [`LinkageModel::score`]) so the two must match
    /// exactly.
    LengthMismatch {
        /// The number of fields the model was constructed with.
        expected: usize,
        /// The number of similarities the caller supplied.
        actual: usize,
    },
}

impl core::fmt::Display for LinkageScoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LengthMismatch { expected, actual } => write!(
                f,
                "LinkageModel scoring expects {expected} field similarities, got {actual}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LinkageScoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldStrategy;
    use alloc::vec;

    /// Standard one-field discriminator used in several tests. `m=0.9`,
    /// `u=0.1` gives `agree_weight = log2(9) ≈ 3.17` and
    /// `disagree_weight = log2(1/9) ≈ -3.17`.
    fn one_field_model() -> LinkageModel {
        let f = FieldComparator::new(
            "surname",
            FieldStrategy::JaroWinklerSimilarity,
            0.85,
            0.9,
            0.1,
        )
        .unwrap();
        LinkageModel::new(vec![f], 1.0, -1.0).unwrap()
    }

    #[test]
    fn new_rejects_empty_field_list() {
        let e = LinkageModel::new(vec![], 1.0, -1.0).unwrap_err();
        assert!(matches!(e, LinkageModelError::NoFields));
    }

    #[test]
    fn new_rejects_swapped_thresholds() {
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, 0.9, 0.1).unwrap();
        let e = LinkageModel::new(vec![f], -1.0, 1.0).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidThresholds { .. }));
    }

    #[test]
    fn new_rejects_equal_thresholds() {
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, 0.9, 0.1).unwrap();
        let e = LinkageModel::new(vec![f], 0.0, 0.0).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidThresholds { .. }));
    }

    #[test]
    fn new_rejects_non_finite_thresholds() {
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, 0.9, 0.1).unwrap();
        let e = LinkageModel::new(vec![f], f64::INFINITY, 0.0).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidThresholds { .. }));
        let e = LinkageModel::new(vec![f], f64::NAN, 0.0).unwrap_err();
        assert!(matches!(e, LinkageModelError::InvalidThresholds { .. }));
    }

    #[test]
    fn score_agree_matches_hand_computation() {
        let m = one_field_model();
        let w = m.score(&[1.0]);
        let expected = 9.0_f64.log2();
        assert!((w - expected).abs() < 1e-12);
    }

    #[test]
    fn score_disagree_matches_hand_computation() {
        let m = one_field_model();
        let w = m.score(&[0.0]);
        let expected = -9.0_f64.log2();
        assert!((w - expected).abs() < 1e-12);
    }

    #[test]
    fn classify_matches_at_upper_threshold() {
        let m = one_field_model();
        // score ≈ 3.17 > 1.0 -> Match
        assert_eq!(m.classify(&[1.0]), LinkageDecision::Match);
    }

    #[test]
    fn classify_non_matches_below_lower_threshold() {
        let m = one_field_model();
        // score ≈ -3.17 < -1.0 -> NonMatch
        assert_eq!(m.classify(&[0.0]), LinkageDecision::NonMatch);
    }

    #[test]
    fn classify_possible_match_between_thresholds() {
        // Wide bounds so the score falls strictly between them.
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, 0.9, 0.1).unwrap();
        let m = LinkageModel::new(vec![f], 10.0, -10.0).unwrap();
        assert_eq!(m.classify(&[1.0]), LinkageDecision::PossibleMatch);
    }

    #[test]
    fn classify_weight_boundary_at_upper_is_match() {
        let m = one_field_model();
        assert_eq!(
            m.classify_weight(m.match_threshold()),
            LinkageDecision::Match
        );
    }

    #[test]
    fn classify_weight_boundary_at_lower_is_non_match() {
        let m = one_field_model();
        assert_eq!(
            m.classify_weight(m.non_match_threshold()),
            LinkageDecision::NonMatch
        );
    }

    #[test]
    fn classify_weight_nan_is_possible_match() {
        // NaN falls through both `>=` and `<=` checks and lands on
        // PossibleMatch — the safer default.
        let m = one_field_model();
        assert_eq!(m.classify_weight(f64::NAN), LinkageDecision::PossibleMatch);
    }

    #[test]
    #[should_panic(expected = "expected 1 field similarities, got 2")]
    fn score_panics_on_length_mismatch() {
        let m = one_field_model();
        let _ = m.score(&[1.0, 0.5]);
    }

    #[test]
    fn try_score_reports_length_mismatch() {
        let m = one_field_model();
        let e = m.try_score(&[1.0, 0.5]).unwrap_err();
        assert_eq!(
            e,
            LinkageScoreError::LengthMismatch {
                expected: 1,
                actual: 2
            }
        );
    }

    #[test]
    fn try_classify_reports_length_mismatch() {
        let m = one_field_model();
        let e = m.try_classify(&[]).unwrap_err();
        assert_eq!(
            e,
            LinkageScoreError::LengthMismatch {
                expected: 1,
                actual: 0
            }
        );
    }

    #[test]
    fn multi_field_score_sums_contributions() {
        // Three fields with distinct m/u; verify the total weight equals
        // the hand-summed per-field contributions.
        let fields = vec![
            FieldComparator::new("f0", FieldStrategy::JaroWinklerSimilarity, 0.85, 0.9, 0.1)
                .unwrap(),
            FieldComparator::new("f1", FieldStrategy::LevenshteinNormalized, 0.8, 0.85, 0.15)
                .unwrap(),
            FieldComparator::new("f2", FieldStrategy::Exact, 0.5, 0.99, 0.01).unwrap(),
        ];
        let m = LinkageModel::new(fields, 5.0, -5.0).unwrap();
        let w = m.score(&[1.0, 1.0, 1.0]);
        let expected = (0.9_f64 / 0.1).log2() + (0.85_f64 / 0.15).log2() + (0.99_f64 / 0.01).log2();
        assert!((w - expected).abs() < 1e-12);
    }
}
