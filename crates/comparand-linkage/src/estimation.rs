//! Estimating the Fellegi-Sunter `m` and `u` probabilities.
//!
//! Three broadly recognized approaches exist for choosing the per-field
//! `m_i` and `u_i` parameters that drive a [`crate::LinkageModel`]:
//!
//! 1. **Prior knowledge.** The analyst sets `m` and `u` from domain
//!    expertise — census demographics, existing linkage benchmarks, or
//!    field-level plausibility arguments. See [`PriorProbabilities`].
//! 2. **Maximum-likelihood estimation from labeled pairs.** Given a
//!    corpus of `(record_a, record_b, is_match)` triples with the
//!    per-field agreement patterns already computed, the analyst can
//!    estimate each parameter as a smoothed frequency. See
//!    [`LabeledPairsEstimator`].
//! 3. **The Expectation-Maximization (EM) algorithm.** Given only the
//!    agreement patterns and no labels, EM alternates between assigning
//!    latent match/non-match probabilities and re-estimating `m` and
//!    `u`. This is the classic Fellegi-Sunter estimation strategy and
//!    is powerful precisely because it does not require ground truth.
//!    See [`EmEstimator`] for the reserved API surface and the deferral
//!    rationale.
//!
//! # Deferral of EM
//!
//! The EM loop is a substantial standalone implementation — its
//! initialization choices, its convergence criteria, its handling of
//! degenerate agreement patterns, and its interaction with weighted
//! blocking all deserve their own dedicated test coverage. It is
//! deliberately deferred to a follow-up release; the [`EmEstimator`]
//! type in this module exists to reserve the API surface and to keep
//! future callers from having to change import paths.

use alloc::vec::Vec;
use core::fmt;

use crate::error::LinkageModelError;
use crate::field::FieldComparator;

/// Analyst-supplied or estimator-produced `m_i` and `u_i` probabilities.
///
/// This struct is both the return type of the estimators in this module
/// and the natural container for analyst-supplied prior parameters —
/// callers who set `m` and `u` from domain expertise can construct one
/// directly. The two vectors must be the same length (one entry per
/// field) and each entry must lie in the open interval `(0, 1)`. The
/// [`PriorProbabilities::validate`] method enforces both invariants.
///
/// Field names are stored alongside the probabilities so downstream
/// [`FieldComparator`] construction can pair the two without a separate
/// bookkeeping step. When a caller uses
/// [`PriorProbabilities::into_field_comparators`], the field names
/// become the [`FieldComparator::name`] of each constructed comparator.
#[derive(Clone, Debug, PartialEq)]
pub struct PriorProbabilities {
    /// Field names, in declaration order. Used to construct
    /// [`FieldComparator`]s and to emit diagnostics.
    pub field_names: Vec<&'static str>,
    /// `m_i` values, in the same order as `field_names`. Each must lie
    /// in `(0, 1)`.
    pub m_probabilities: Vec<f64>,
    /// `u_i` values, in the same order as `field_names`. Each must lie
    /// in `(0, 1)`.
    pub u_probabilities: Vec<f64>,
}

impl PriorProbabilities {
    /// Returns the number of fields the estimate covers.
    #[inline]
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.field_names.len()
    }

    /// Validates the shape and value ranges.
    ///
    /// # Errors
    ///
    /// Returns [`EstimationError::MismatchedVectorLengths`] if the three
    /// vectors do not have identical lengths;
    /// [`EstimationError::NoFields`] if all three are empty;
    /// [`EstimationError::InvalidProbability`] if any probability is
    /// outside `(0, 1)` or non-finite.
    pub fn validate(&self) -> Result<(), EstimationError> {
        let k = self.field_names.len();
        if k == 0 {
            return Err(EstimationError::NoFields);
        }
        if self.m_probabilities.len() != k || self.u_probabilities.len() != k {
            return Err(EstimationError::MismatchedVectorLengths {
                field_names: k,
                m_probabilities: self.m_probabilities.len(),
                u_probabilities: self.u_probabilities.len(),
            });
        }
        for (i, &m) in self.m_probabilities.iter().enumerate() {
            if !is_in_open_unit_interval(m) {
                return Err(EstimationError::InvalidProbability {
                    field: self.field_names[i],
                    kind: ProbabilityKind::MProbability,
                    value: m,
                });
            }
        }
        for (i, &u) in self.u_probabilities.iter().enumerate() {
            if !is_in_open_unit_interval(u) {
                return Err(EstimationError::InvalidProbability {
                    field: self.field_names[i],
                    kind: ProbabilityKind::UProbability,
                    value: u,
                });
            }
        }
        Ok(())
    }

    /// Converts the estimate into a `Vec<FieldComparator>`, ready to
    /// hand to [`crate::LinkageModel::new`].
    ///
    /// The `strategies` and `agreement_thresholds` slices must be the
    /// same length as `field_names`. The caller supplies these
    /// alongside the estimated probabilities because neither is a
    /// property of the estimation itself — the strategy is
    /// metadata about how the similarity was computed, and the
    /// threshold is the analyst's choice of where to cut the
    /// continuous similarity into a binary pattern.
    ///
    /// # Errors
    ///
    /// Returns [`EstimationError::MismatchedVectorLengths`] if the
    /// strategy or threshold slices are the wrong length, or any of
    /// the [`LinkageModelError`] variants propagated from
    /// [`FieldComparator::new`] on invalid values.
    pub fn into_field_comparators(
        self,
        strategies: &[crate::field::FieldStrategy],
        agreement_thresholds: &[f64],
    ) -> Result<Vec<FieldComparator>, EstimationError> {
        self.validate()?;
        let k = self.field_names.len();
        if strategies.len() != k || agreement_thresholds.len() != k {
            return Err(EstimationError::MismatchedVectorLengths {
                field_names: k,
                m_probabilities: strategies.len(),
                u_probabilities: agreement_thresholds.len(),
            });
        }
        let mut out = Vec::with_capacity(k);
        for i in 0..k {
            let f = FieldComparator::new(
                self.field_names[i],
                strategies[i],
                agreement_thresholds[i],
                self.m_probabilities[i],
                self.u_probabilities[i],
            )
            .map_err(EstimationError::FieldRejected)?;
            out.push(f);
        }
        Ok(out)
    }
}

/// A single labeled record pair, as consumed by [`LabeledPairsEstimator`].
///
/// The per-field *agreement pattern* — a boolean per field — is what the
/// estimator observes; the caller is responsible for computing the
/// binary agree/disagree pattern from the raw record pair, typically by
/// applying each field's [`crate::AgreementRule`].
#[derive(Copy, Clone, Debug)]
pub struct LabeledPair<'a> {
    /// The per-field boolean agreement pattern, in the same field
    /// order the eventual [`crate::LinkageModel`] will declare.
    pub field_agreements: &'a [bool],
    /// Ground-truth label: `true` if this pair is a genuine match.
    pub is_match: bool,
}

/// A maximum-likelihood estimator for the Fellegi-Sunter `m` and `u`
/// probabilities from labeled record pairs.
///
/// # Algorithm
///
/// For each field `i`, the estimator maintains four running counters:
///
/// * agreements observed among matches
/// * total matches observed
/// * agreements observed among non-matches
/// * total non-matches observed
///
/// The MLE with Jeffreys smoothing is
///
/// ```text
///     m_i = (agree_matches_i + α) / (matches + 2α)
///     u_i = (agree_non_matches_i + α) / (non_matches + 2α)
/// ```
///
/// with `α = 0.5` by default. The smoothing serves two purposes: it
/// pushes the estimate away from the boundary values `0` and `1` (which
/// would collapse the log-likelihood to `±∞`) and it produces the
/// non-informative Jeffreys prior when the data are silent.
///
/// # Design note
///
/// The estimator observes agreement patterns rather than raw similarity
/// scores; the caller applies each field's [`crate::AgreementRule`]
/// before feeding a pair to [`LabeledPairsEstimator::observe`]. This
/// separation matters because the same labeled corpus can be re-used
/// with different threshold choices without regenerating the pairs.
#[derive(Clone, Debug)]
pub struct LabeledPairsEstimator {
    field_names: Vec<&'static str>,
    match_agreements: Vec<u64>,
    match_total: u64,
    non_match_agreements: Vec<u64>,
    non_match_total: u64,
    smoothing: f64,
}

impl LabeledPairsEstimator {
    /// The default Jeffreys smoothing constant, `α = 0.5`. Used by
    /// [`LabeledPairsEstimator::new`]; callers wanting a different
    /// value should use [`LabeledPairsEstimator::with_smoothing`].
    pub const DEFAULT_SMOOTHING: f64 = 0.5;

    /// Constructs an estimator that will observe agreement patterns
    /// over the given fields. The `field_names` are copied into the
    /// estimator and used to label the eventual estimate; their order
    /// determines the positional convention for
    /// [`LabeledPair::field_agreements`].
    ///
    /// The default smoothing constant is [`Self::DEFAULT_SMOOTHING`].
    ///
    /// # Errors
    ///
    /// Returns [`EstimationError::NoFields`] if `field_names` is empty.
    pub fn new(field_names: Vec<&'static str>) -> Result<Self, EstimationError> {
        Self::with_smoothing(field_names, Self::DEFAULT_SMOOTHING)
    }

    /// Same as [`Self::new`] but with an explicit Jeffreys smoothing
    /// constant. Values of `α = 0` disable smoothing (which will
    /// produce `Err(EstimationError::DegenerateCategory)` from
    /// [`Self::estimate`] whenever an all-agree or all-disagree
    /// category is observed).
    ///
    /// # Errors
    ///
    /// Returns [`EstimationError::NoFields`] if `field_names` is
    /// empty, or [`EstimationError::InvalidSmoothing`] if the
    /// smoothing constant is negative or non-finite.
    pub fn with_smoothing(
        field_names: Vec<&'static str>,
        smoothing: f64,
    ) -> Result<Self, EstimationError> {
        if field_names.is_empty() {
            return Err(EstimationError::NoFields);
        }
        if !smoothing.is_finite() || smoothing < 0.0 {
            return Err(EstimationError::InvalidSmoothing { value: smoothing });
        }
        let k = field_names.len();
        Ok(Self {
            field_names,
            match_agreements: alloc::vec![0; k],
            match_total: 0,
            non_match_agreements: alloc::vec![0; k],
            non_match_total: 0,
            smoothing,
        })
    }

    /// Observes a single labeled pair.
    ///
    /// # Errors
    ///
    /// Returns [`EstimationError::MismatchedVectorLengths`] if the
    /// pair's agreement pattern has a different length than the
    /// estimator's declared field count.
    pub fn observe(&mut self, pair: LabeledPair<'_>) -> Result<(), EstimationError> {
        let k = self.field_names.len();
        if pair.field_agreements.len() != k {
            return Err(EstimationError::MismatchedVectorLengths {
                field_names: k,
                m_probabilities: pair.field_agreements.len(),
                u_probabilities: pair.field_agreements.len(),
            });
        }
        if pair.is_match {
            self.match_total += 1;
            for (i, &agrees) in pair.field_agreements.iter().enumerate() {
                if agrees {
                    self.match_agreements[i] += 1;
                }
            }
        } else {
            self.non_match_total += 1;
            for (i, &agrees) in pair.field_agreements.iter().enumerate() {
                if agrees {
                    self.non_match_agreements[i] += 1;
                }
            }
        }
        Ok(())
    }

    /// Convenience wrapper: observes an iterator of labeled pairs in
    /// bulk. Short-circuits on the first error.
    ///
    /// # Errors
    ///
    /// Returns the first error emitted by [`Self::observe`] and stops
    /// iterating. Pairs that were successfully observed before the
    /// error remain in the estimator's running counters.
    pub fn observe_all<'a, I>(&mut self, pairs: I) -> Result<(), EstimationError>
    where
        I: IntoIterator<Item = LabeledPair<'a>>,
    {
        for p in pairs {
            self.observe(p)?;
        }
        Ok(())
    }

    /// Returns the number of matching pairs observed so far.
    #[inline]
    #[must_use]
    pub const fn match_total(&self) -> u64 {
        self.match_total
    }

    /// Returns the number of non-matching pairs observed so far.
    #[inline]
    #[must_use]
    pub const fn non_match_total(&self) -> u64 {
        self.non_match_total
    }

    /// Produces the maximum-likelihood estimate.
    ///
    /// # Errors
    ///
    /// Returns [`EstimationError::NoMatches`] or
    /// [`EstimationError::NoNonMatches`] if either class is empty;
    /// [`EstimationError::DegenerateCategory`] if smoothing is zero and
    /// a per-field agreement count is `0` or equal to its class total.
    pub fn estimate(&self) -> Result<PriorProbabilities, EstimationError> {
        if self.match_total == 0 {
            return Err(EstimationError::NoMatches);
        }
        if self.non_match_total == 0 {
            return Err(EstimationError::NoNonMatches);
        }
        let alpha = self.smoothing;
        let two_alpha = 2.0 * alpha;
        // f64 is precise enough for the u64 counts a labeled-pair
        // estimator would ever accumulate. If the analyst is somehow
        // feeding 2^53 pairs into a single estimator, floating-point
        // rounding is not their most pressing problem.
        #[allow(
            clippy::cast_precision_loss,
            reason = "u64 -> f64 is exact for the counts a labeled-pair estimator would realistically accumulate"
        )]
        let match_total_f = self.match_total as f64;
        #[allow(clippy::cast_precision_loss, reason = "see above")]
        let non_match_total_f = self.non_match_total as f64;
        let mut m_probabilities = Vec::with_capacity(self.field_names.len());
        let mut u_probabilities = Vec::with_capacity(self.field_names.len());
        for i in 0..self.field_names.len() {
            #[allow(clippy::cast_precision_loss, reason = "see above")]
            let match_agrees_f = self.match_agreements[i] as f64;
            #[allow(clippy::cast_precision_loss, reason = "see above")]
            let non_match_agrees_f = self.non_match_agreements[i] as f64;
            let m = (match_agrees_f + alpha) / (match_total_f + two_alpha);
            let u = (non_match_agrees_f + alpha) / (non_match_total_f + two_alpha);
            // A zero-smoothing estimator can still emit degenerate
            // (0 or 1) probabilities; report them rather than silently
            // returning ±∞ from the downstream weight computation.
            if !is_in_open_unit_interval(m) {
                return Err(EstimationError::DegenerateCategory {
                    field: self.field_names[i],
                    kind: ProbabilityKind::MProbability,
                    value: m,
                });
            }
            if !is_in_open_unit_interval(u) {
                return Err(EstimationError::DegenerateCategory {
                    field: self.field_names[i],
                    kind: ProbabilityKind::UProbability,
                    value: u,
                });
            }
            m_probabilities.push(m);
            u_probabilities.push(u);
        }
        Ok(PriorProbabilities {
            field_names: self.field_names.clone(),
            m_probabilities,
            u_probabilities,
        })
    }
}

/// Unsupervised EM-based estimator for the Fellegi-Sunter parameters.
///
/// # Status: reserved API surface
///
/// The EM loop is deliberately not implemented in this release. The
/// full algorithm — its initialization choices, convergence criteria,
/// degenerate-agreement handling, and interaction with weighted
/// blocking — is a substantial standalone commitment that deserves
/// its own coverage. [`EmEstimator::estimate`] always returns
/// [`EstimationError::NotYetImplemented`]; the type exists to reserve
/// the import path so a subsequent release can fill in the algorithm
/// without breaking callers that currently reference the type.
#[derive(Clone, Debug)]
pub struct EmEstimator {
    field_names: Vec<&'static str>,
}

impl EmEstimator {
    /// Constructs a reserved EM estimator over the given fields. The
    /// field names have the same positional convention as in
    /// [`LabeledPairsEstimator`].
    ///
    /// # Errors
    ///
    /// Returns [`EstimationError::NoFields`] if `field_names` is empty.
    pub fn new(field_names: Vec<&'static str>) -> Result<Self, EstimationError> {
        if field_names.is_empty() {
            return Err(EstimationError::NoFields);
        }
        Ok(Self { field_names })
    }

    /// Returns the number of fields the estimator was constructed for.
    #[inline]
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.field_names.len()
    }

    /// **Stub.** Always returns
    /// [`EstimationError::NotYetImplemented`]. See the type-level
    /// documentation for the deferral rationale.
    ///
    /// # Errors
    ///
    /// Always returns [`EstimationError::NotYetImplemented`].
    pub fn estimate(
        &self,
        _agreement_patterns: &[&[bool]],
    ) -> Result<PriorProbabilities, EstimationError> {
        Err(EstimationError::NotYetImplemented)
    }
}

/// Discriminator for which of the two Fellegi-Sunter probabilities an
/// error variant refers to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProbabilityKind {
    /// The match-conditional probability `m_i`.
    MProbability,
    /// The non-match-conditional probability `u_i`.
    UProbability,
}

impl fmt::Display for ProbabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MProbability => f.write_str("m_probability"),
            Self::UProbability => f.write_str("u_probability"),
        }
    }
}

/// Errors returned by the estimation module.
///
/// The `#[non_exhaustive]` marker allows a future release (e.g. the
/// full EM implementation) to add new variants without a major-version
/// bump. Existing callers should always match on this enum with a
/// catch-all arm.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum EstimationError {
    /// The estimator was constructed with an empty field list, or the
    /// [`PriorProbabilities`] passed to [`PriorProbabilities::validate`]
    /// had none.
    NoFields,
    /// [`LabeledPairsEstimator::estimate`] was called before any
    /// matching pair was observed.
    NoMatches,
    /// [`LabeledPairsEstimator::estimate`] was called before any
    /// non-matching pair was observed.
    NoNonMatches,
    /// The three vectors of a [`PriorProbabilities`], or the pair's
    /// agreement pattern vs the estimator's field count, had
    /// mismatched lengths.
    MismatchedVectorLengths {
        /// Length of the `field_names` vector, or the estimator's
        /// declared field count when this variant comes from an
        /// `observe` call.
        field_names: usize,
        /// Length of the `m_probabilities` vector, or the observed
        /// agreement-pattern length when this variant comes from an
        /// `observe` call.
        m_probabilities: usize,
        /// Length of the `u_probabilities` vector, or the same
        /// observed agreement-pattern length as above.
        u_probabilities: usize,
    },
    /// A [`PriorProbabilities`] value was outside the open interval
    /// `(0, 1)` or non-finite.
    InvalidProbability {
        /// The offending field.
        field: &'static str,
        /// Whether the offending value was an `m_i` or a `u_i`.
        kind: ProbabilityKind,
        /// The offending value.
        value: f64,
    },
    /// The Jeffreys smoothing constant was negative or non-finite.
    InvalidSmoothing {
        /// The offending smoothing value.
        value: f64,
    },
    /// A zero-smoothing MLE produced a `0` or `1` probability for a
    /// field, which would collapse the downstream weight to `±∞`.
    DegenerateCategory {
        /// The offending field.
        field: &'static str,
        /// Whether the offending value was an `m_i` or a `u_i`.
        kind: ProbabilityKind,
        /// The offending value.
        value: f64,
    },
    /// A [`FieldComparator`] constructed from the estimate was rejected
    /// by [`FieldComparator::new`].
    FieldRejected(LinkageModelError),
    /// The EM estimator was called; the loop is not yet implemented.
    /// See [`EmEstimator`] for the deferral rationale.
    NotYetImplemented,
}

impl fmt::Display for EstimationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFields => f.write_str("no fields declared"),
            Self::NoMatches => f.write_str(
                "cannot estimate m_probabilities without at least one observed match",
            ),
            Self::NoNonMatches => f.write_str(
                "cannot estimate u_probabilities without at least one observed non-match",
            ),
            Self::MismatchedVectorLengths {
                field_names,
                m_probabilities,
                u_probabilities,
            } => write!(
                f,
                "mismatched vector lengths: field_names={field_names}, m_probabilities={m_probabilities}, u_probabilities={u_probabilities}"
            ),
            Self::InvalidProbability { field, kind, value } => write!(
                f,
                "field {field:?} has {kind} {value} outside the open interval (0, 1)"
            ),
            Self::InvalidSmoothing { value } => write!(
                f,
                "smoothing constant must be finite and non-negative, got {value}"
            ),
            Self::DegenerateCategory { field, kind, value } => write!(
                f,
                "zero-smoothing MLE produced degenerate {kind}={value} for field {field:?}; consider increasing the smoothing constant"
            ),
            Self::FieldRejected(inner) => write!(f, "field comparator rejected: {inner}"),
            Self::NotYetImplemented => f.write_str(
                "the EM estimator is a reserved API surface; the algorithm has not yet been implemented",
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EstimationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::FieldRejected(inner) = self {
            Some(inner)
        } else {
            None
        }
    }
}

#[inline]
fn is_in_open_unit_interval(x: f64) -> bool {
    x.is_finite() && x > 0.0 && x < 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::FieldStrategy;
    use alloc::vec;

    #[test]
    fn prior_probabilities_validate_accepts_typical_values() {
        let p = PriorProbabilities {
            field_names: vec!["surname", "given"],
            m_probabilities: vec![0.9, 0.8],
            u_probabilities: vec![0.1, 0.2],
        };
        p.validate().unwrap();
    }

    #[test]
    fn prior_probabilities_validate_rejects_length_mismatch() {
        let p = PriorProbabilities {
            field_names: vec!["surname"],
            m_probabilities: vec![0.9, 0.8],
            u_probabilities: vec![0.1],
        };
        let e = p.validate().unwrap_err();
        assert!(matches!(e, EstimationError::MismatchedVectorLengths { .. }));
    }

    #[test]
    fn prior_probabilities_validate_rejects_out_of_range() {
        let p = PriorProbabilities {
            field_names: vec!["surname"],
            m_probabilities: vec![1.0],
            u_probabilities: vec![0.1],
        };
        assert!(matches!(
            p.validate().unwrap_err(),
            EstimationError::InvalidProbability { .. }
        ));
    }

    #[test]
    fn into_field_comparators_pairs_names_with_strategies() {
        let p = PriorProbabilities {
            field_names: vec!["surname", "given"],
            m_probabilities: vec![0.9, 0.85],
            u_probabilities: vec![0.1, 0.2],
        };
        let fields = p
            .into_field_comparators(
                &[
                    FieldStrategy::JaroWinklerSimilarity,
                    FieldStrategy::LevenshteinNormalized,
                ],
                &[0.85, 0.8],
            )
            .unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "surname");
        assert_eq!(fields[0].strategy, FieldStrategy::JaroWinklerSimilarity);
        assert_eq!(fields[1].strategy, FieldStrategy::LevenshteinNormalized);
    }

    #[test]
    fn labeled_pairs_estimator_matches_hand_computation() {
        // 100 matches: 90 agree on field 0, 80 on field 1.
        // 100 non-matches: 10 agree on field 0, 20 on field 1.
        // With Jeffreys smoothing α=0.5:
        // m_0 = (90 + 0.5) / (100 + 1) = 90.5 / 101 ≈ 0.8960
        // u_0 = (10 + 0.5) / (100 + 1) = 10.5 / 101 ≈ 0.1040
        let mut est = LabeledPairsEstimator::new(vec!["f0", "f1"]).unwrap();
        for i in 0..100 {
            let f0 = i < 90;
            let f1 = i < 80;
            est.observe(LabeledPair {
                field_agreements: &[f0, f1],
                is_match: true,
            })
            .unwrap();
        }
        for i in 0..100 {
            let f0 = i < 10;
            let f1 = i < 20;
            est.observe(LabeledPair {
                field_agreements: &[f0, f1],
                is_match: false,
            })
            .unwrap();
        }
        let p = est.estimate().unwrap();
        assert!((p.m_probabilities[0] - 90.5 / 101.0).abs() < 1e-12);
        assert!((p.u_probabilities[0] - 10.5 / 101.0).abs() < 1e-12);
        assert!((p.m_probabilities[1] - 80.5 / 101.0).abs() < 1e-12);
        assert!((p.u_probabilities[1] - 20.5 / 101.0).abs() < 1e-12);
    }

    #[test]
    fn labeled_pairs_estimator_rejects_no_matches() {
        let mut est = LabeledPairsEstimator::new(vec!["f0"]).unwrap();
        est.observe(LabeledPair {
            field_agreements: &[false],
            is_match: false,
        })
        .unwrap();
        assert!(matches!(
            est.estimate().unwrap_err(),
            EstimationError::NoMatches
        ));
    }

    #[test]
    fn labeled_pairs_estimator_rejects_no_non_matches() {
        let mut est = LabeledPairsEstimator::new(vec!["f0"]).unwrap();
        est.observe(LabeledPair {
            field_agreements: &[true],
            is_match: true,
        })
        .unwrap();
        assert!(matches!(
            est.estimate().unwrap_err(),
            EstimationError::NoNonMatches
        ));
    }

    #[test]
    fn labeled_pairs_estimator_rejects_pair_of_wrong_length() {
        let mut est = LabeledPairsEstimator::new(vec!["f0", "f1"]).unwrap();
        let e = est
            .observe(LabeledPair {
                field_agreements: &[true],
                is_match: true,
            })
            .unwrap_err();
        assert!(matches!(e, EstimationError::MismatchedVectorLengths { .. }));
    }

    #[test]
    fn zero_smoothing_reports_degenerate_categories() {
        // No smoothing + all-agree observations -> m_0 would be 1.0.
        let mut est = LabeledPairsEstimator::with_smoothing(vec!["f0"], 0.0).unwrap();
        est.observe(LabeledPair {
            field_agreements: &[true],
            is_match: true,
        })
        .unwrap();
        est.observe(LabeledPair {
            field_agreements: &[false],
            is_match: false,
        })
        .unwrap();
        let e = est.estimate().unwrap_err();
        assert!(matches!(e, EstimationError::DegenerateCategory { .. }));
    }

    #[test]
    fn em_estimator_returns_not_yet_implemented() {
        let em = EmEstimator::new(vec!["f0"]).unwrap();
        assert!(matches!(
            em.estimate(&[]).unwrap_err(),
            EstimationError::NotYetImplemented
        ));
    }

    #[test]
    fn em_estimator_rejects_empty_field_list() {
        assert!(matches!(
            EmEstimator::new(vec![]).unwrap_err(),
            EstimationError::NoFields
        ));
    }

    #[test]
    fn labeled_estimator_rejects_negative_smoothing() {
        assert!(matches!(
            LabeledPairsEstimator::with_smoothing(vec!["f0"], -0.1).unwrap_err(),
            EstimationError::InvalidSmoothing { .. }
        ));
    }

    #[test]
    fn observe_all_short_circuits_on_error() {
        let mut est = LabeledPairsEstimator::new(vec!["f0"]).unwrap();
        let pairs = vec![
            LabeledPair {
                field_agreements: &[true],
                is_match: true,
            },
            LabeledPair {
                field_agreements: &[true, false],
                is_match: false,
            },
        ];
        assert!(matches!(
            est.observe_all(pairs).unwrap_err(),
            EstimationError::MismatchedVectorLengths { .. }
        ));
    }
}
