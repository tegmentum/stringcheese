//! Error types for [`LinkageModel`](crate::LinkageModel) construction.
//!
//! Model construction is the only step in the crate that can reject its
//! input on domain grounds — the scoring and classification paths themselves
//! are total functions once the model has been built. This module therefore
//! holds a single [`LinkageModelError`] enum that summarizes every
//! constructor-time rejection reason.
//!
//! Estimator-time errors live in [`crate::estimation::EstimationError`]
//! rather than here — they arise from a different stage of the pipeline
//! (learning parameters from data) and their remediation is different
//! (supplying more data, choosing a different smoothing constant), so they
//! carry their own type.

use core::fmt;

/// Reason [`LinkageModel::new`](crate::LinkageModel::new) rejected a
/// candidate model configuration.
///
/// Every variant is a domain-level rejection: the type system already
/// prevents the mechanical mistakes ([`f64`] on both probabilities and
/// thresholds, a non-empty [`Vec`] of fields), so the checks here catch
/// the semantic constraints of the Fellegi-Sunter model.
///
/// The enum is `#[non_exhaustive]` so a future substrate release can add
/// new rejection reasons without a major-version bump.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LinkageModelError {
    /// The match and non-match thresholds do not satisfy
    /// `non_match_threshold < match_threshold`.
    ///
    /// Fellegi-Sunter requires a strictly separated middle region for the
    /// possible-match outcome; two equal thresholds would collapse it to a
    /// zero-width set that no numeric weight can land inside, and a swapped
    /// pair would produce nonsensical (empty) match and non-match regions.
    /// Non-finite thresholds are also rejected here.
    InvalidThresholds {
        /// The match (upper) threshold that was supplied.
        match_threshold: f64,
        /// The non-match (lower) threshold that was supplied.
        non_match_threshold: f64,
    },
    /// A field's `m_i` probability is not in the open interval `(0, 1)`
    /// or is non-finite.
    ///
    /// A value of `0` or `1` degenerates the log-likelihood
    /// `log2(m_i / u_i)` or `log2((1 - m_i) / (1 - u_i))` into ±infinity;
    /// no field can be treated as a certain discriminator, and no
    /// well-formed Fellegi-Sunter model has zero-or-one probabilities.
    InvalidMProbability {
        /// The field the invalid probability was declared on.
        field: &'static str,
        /// The offending probability value.
        value: f64,
    },
    /// A field's `u_i` probability is not in the open interval `(0, 1)`
    /// or is non-finite.
    ///
    /// The same reasoning as [`Self::InvalidMProbability`] applies: a `u_i`
    /// of `0` implies the field never agrees among non-matches, which
    /// would let a single agreement carry infinite weight, and a `u_i`
    /// of `1` implies non-matches always agree, which is not a
    /// meaningful field for record linkage.
    InvalidUProbability {
        /// The field the invalid probability was declared on.
        field: &'static str,
        /// The offending probability value.
        value: f64,
    },
    /// A field's agreement threshold is non-finite.
    ///
    /// The threshold is compared directly against the per-field similarity
    /// score supplied at scoring time; a `NaN` or infinite value would make
    /// the comparison meaningless.
    InvalidAgreementThreshold {
        /// The field the invalid threshold was declared on.
        field: &'static str,
        /// The offending threshold value.
        value: f64,
    },
    /// The model was constructed with an empty field list.
    ///
    /// A Fellegi-Sunter model with no fields is not a meaningful classifier
    /// — every pair receives weight `0`, and the two-threshold decision
    /// collapses to a constant answer determined solely by whether `0`
    /// falls in the middle region.
    NoFields,
}

impl fmt::Display for LinkageModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidThresholds {
                match_threshold,
                non_match_threshold,
            } => write!(
                f,
                "invalid Fellegi-Sunter thresholds: non_match_threshold ({non_match_threshold}) must be strictly less than match_threshold ({match_threshold}), and both must be finite"
            ),
            Self::InvalidMProbability { field, value } => write!(
                f,
                "field {field:?} has m_probability {value} outside the open interval (0, 1)"
            ),
            Self::InvalidUProbability { field, value } => write!(
                f,
                "field {field:?} has u_probability {value} outside the open interval (0, 1)"
            ),
            Self::InvalidAgreementThreshold { field, value } => write!(
                f,
                "field {field:?} has agreement_threshold {value}; must be finite"
            ),
            Self::NoFields => f.write_str(
                "a Fellegi-Sunter linkage model must declare at least one field comparator",
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LinkageModelError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_the_offending_field() {
        let e = LinkageModelError::InvalidMProbability {
            field: "surname",
            value: 1.5,
        };
        let msg = format!("{e}");
        assert!(msg.contains("surname"), "message missing field: {msg}");
        assert!(msg.contains("1.5"), "message missing value: {msg}");
    }

    #[test]
    fn display_names_both_thresholds() {
        let e = LinkageModelError::InvalidThresholds {
            match_threshold: 1.0,
            non_match_threshold: 2.0,
        };
        let msg = format!("{e}");
        assert!(msg.contains('1'), "message missing match_threshold: {msg}");
        assert!(
            msg.contains('2'),
            "message missing non_match_threshold: {msg}"
        );
    }

    #[test]
    fn implements_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}
        assert_error(&LinkageModelError::NoFields);
    }
}
