//! The per-field log-likelihood weight computation.
//!
//! The two functions in this module encode the single load-bearing formula
//! of the Fellegi-Sunter model. Given a field's `m_i` and `u_i`
//! probabilities:
//!
//! * [`agree_weight`] returns `log2(m_i / u_i)`, the contribution to the
//!   record-pair weight when the field's agreement pattern is *agree*.
//! * [`disagree_weight`] returns `log2((1 - m_i) / (1 - u_i))`, the
//!   contribution when the pattern is *disagree*.
//!
//! Both functions are exposed at the crate root so callers can compute
//! weights outside of a full [`crate::LinkageModel`] — for example, in an
//! auditing pipeline that already has a table of `(m_i, u_i)` pairs and
//! wants to reproduce the classifier's arithmetic without re-materializing
//! the model. [`crate::LinkageModel`] itself precomputes both values once
//! per field at construction, so its per-pair scoring cost is a plain
//! floating-point summation with no `log2` in the inner loop.
//!
//! # Preconditions
//!
//! The functions accept any `f64` and do not themselves validate their
//! arguments — validation is the [`crate::LinkageModel`] constructor's
//! job, and doing it a second time in the arithmetic-hot path would be
//! wasted work. If the caller passes `m` or `u` outside `(0, 1)` the
//! result may be `±∞` or `NaN`; that is the caller's contract to uphold.
//!
//! # Numerical stability
//!
//! The formulas are evaluated as a single `log2` of a ratio, matching the
//! traditional Fellegi-Sunter presentation. For probabilities in the
//! open interval `(0, 1)` — the only inputs a validated [`crate::LinkageModel`]
//! ever passes to these functions — the intermediate ratios are finite
//! and the results are well-defined finite doubles.

/// The Fellegi-Sunter "agree" weight contribution for a field with
/// probabilities `m` and `u`.
///
/// Returns `log2(m / u)`. For a discriminating field (`m > u`), the return
/// value is positive; for a field with no discrimination (`m == u`), it is
/// zero; for a field that agrees *more* under non-matches than under
/// matches (which is unusual but not disallowed), it is negative.
///
/// # Arguments
///
/// * `m` — the probability of agreement conditional on a true match,
///   `P(γ = agree | true match)`. Expected to lie in `(0, 1)`.
/// * `u` — the probability of agreement conditional on a true non-match,
///   `P(γ = agree | true non-match)`. Expected to lie in `(0, 1)`.
///
/// # Preconditions
///
/// Validation of the input range is the caller's job — see the module
/// documentation for the rationale. Passing `u == 0.0` produces `+∞`;
/// passing `m == 0.0` produces `-∞`.
#[inline]
#[must_use]
pub fn agree_weight(m: f64, u: f64) -> f64 {
    (m / u).log2()
}

/// The Fellegi-Sunter "disagree" weight contribution for a field with
/// probabilities `m` and `u`.
///
/// Returns `log2((1 - m) / (1 - u))`. For a discriminating field (`m > u`),
/// the return value is negative — the field disagreeing among matches is
/// unusual, so it pushes the total weight down.
///
/// # Arguments
///
/// * `m` — the probability of agreement conditional on a true match.
///   Expected to lie in `(0, 1)`.
/// * `u` — the probability of agreement conditional on a true non-match.
///   Expected to lie in `(0, 1)`.
///
/// # Preconditions
///
/// See [`agree_weight`].
#[inline]
#[must_use]
pub fn disagree_weight(m: f64, u: f64) -> f64 {
    ((1.0 - m) / (1.0 - u)).log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agree_weight_matches_hand_computation() {
        // m=0.9, u=0.1 -> log2(9)
        let w = agree_weight(0.9, 0.1);
        let expected = 9.0_f64.log2();
        assert!(
            (w - expected).abs() < 1e-15,
            "agree_weight(0.9, 0.1) = {w}, expected {expected}"
        );
    }

    #[test]
    fn disagree_weight_matches_hand_computation() {
        // m=0.9, u=0.1 -> log2(0.1 / 0.9) = -log2(9)
        let w = disagree_weight(0.9, 0.1);
        let expected = -9.0_f64.log2();
        assert!(
            (w - expected).abs() < 1e-15,
            "disagree_weight(0.9, 0.1) = {w}, expected {expected}"
        );
    }

    #[test]
    fn nondiscriminating_field_yields_zero_weight() {
        // m == u -> agree_weight = log2(1) = 0
        let w = agree_weight(0.5, 0.5);
        assert_eq!(w.to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn nondiscriminating_field_yields_zero_disagree_weight() {
        // m == u -> disagree_weight = log2((1 - m) / (1 - u)) = log2(1) = 0
        let w = disagree_weight(0.3, 0.3);
        assert_eq!(w.to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn agree_and_disagree_have_expected_signs_for_discriminator() {
        // For a discriminating field (m > u), agree pushes weight up and
        // disagree pushes it down.
        let m = 0.95;
        let u = 0.05;
        assert!(agree_weight(m, u) > 0.0);
        assert!(disagree_weight(m, u) < 0.0);
    }

    #[test]
    fn extreme_u_produces_positive_infinity_agree() {
        // Documented behavior — validation is the model constructor's job.
        assert!(agree_weight(0.9, 0.0).is_infinite());
        assert!(agree_weight(0.9, 0.0).is_sign_positive());
    }
}
