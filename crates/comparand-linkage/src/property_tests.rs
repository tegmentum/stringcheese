//! Property-based tests for the [`LinkageModel`] classifier.
//!
//! The tests here exercise five properties of the Fellegi-Sunter classifier:
//!
//! * [Constructor validation][valid] — the constructor rejects every
//!   configuration outside the model's declared valid range and accepts
//!   every configuration inside it.
//! * [Threshold ordering][ordering] — the constructor rejects non-strict
//!   or reversed thresholds.
//! * [Monotonicity][monotone] — turning a disagreeing field into an
//!   agreeing field can only raise the total weight, provided every
//!   field has `m > u` (the typical discrimination-power condition).
//! * [Threshold consistency][consistency] — for any weight `w`,
//!   `classify_weight(w)` matches the two-threshold definition
//!   exactly.
//! * [Field-order symmetry][symmetry] — swapping two fields in the model
//!   and their positions in the similarity vector produces the same
//!   score.
//!
//! [valid]: proptest_constructor_accepts_valid_configurations
//! [ordering]: proptest_constructor_rejects_swapped_thresholds
//! [monotone]: proptest_monotonicity_with_positive_agree_weights
//! [consistency]: proptest_classifier_threshold_consistency
//! [symmetry]: proptest_field_order_symmetry

use alloc::vec::Vec;
use proptest::prelude::*;

use crate::classifier::LinkageDecision;
use crate::error::LinkageModelError;
use crate::field::{FieldComparator, FieldStrategy};
use crate::model::LinkageModel;

/// A strategy over `f64` values *strictly* inside `(0, 1)` — the valid
/// range for `m_i` and `u_i`.
fn arb_open_unit() -> impl Strategy<Value = f64> {
    // 1e-4..0.9999 stays comfortably clear of the exclusive bounds and
    // still exercises the useful working range of Fellegi-Sunter
    // probabilities.
    1e-4_f64..0.9999_f64
}

/// A strategy over `f64` values outside `(0, 1)`, plus non-finite
/// values — used to exercise the constructor's rejection logic.
fn arb_bad_probability() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(0.0),
        Just(1.0),
        Just(-0.1),
        Just(1.1),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
    ]
}

/// A strategy over a single field comparator with `m > u` (the typical
/// discrimination-power condition). Yields a comparator whose agree
/// weight is guaranteed positive and disagree weight guaranteed
/// negative, which the monotonicity property depends on.
fn arb_discriminating_field() -> impl Strategy<Value = FieldComparator> {
    (arb_open_unit(), arb_open_unit()).prop_map(|(a, b)| {
        // Force m > u so agree_weight = log2(m/u) > 0.
        let (m, u) = if a > b { (a, b) } else { (b, a) };
        // If they happened to coincide exactly, nudge them apart. This
        // is a probability-zero event from the strategy but guarding
        // costs nothing.
        #[allow(
            clippy::float_cmp,
            reason = "bit-exact equality is the exact condition being guarded — if two u64 mantissas coincide, m/u would be 1.0 and log2 would return 0.0, which the property test does not want to exercise"
        )]
        let coincide = m.to_bits() == u.to_bits();
        let m = if coincide { (m + u + 1e-3).min(0.9999) } else { m };
        let u = if coincide { (u - 1e-3).max(1e-4) } else { u };
        FieldComparator {
            name: "field",
            strategy: FieldStrategy::Custom,
            agreement_threshold: 0.5,
            m_probability: m,
            u_probability: u,
        }
    })
}

/// A strategy over a K-field model with all-discriminating fields.
fn arb_discriminating_model(field_count: usize) -> impl Strategy<Value = LinkageModel> {
    proptest::collection::vec(arb_discriminating_field(), field_count..=field_count).prop_map(
        move |fields| {
            // Choose thresholds well inside the possible-match middle
            // region so the model doesn't degenerate to Match-or-NonMatch
            // for typical inputs.
            LinkageModel::new(fields, 1e6, -1e6).unwrap()
        },
    )
}

proptest! {
    /// Constructor validation: any `m` or `u` outside `(0, 1)` (including
    /// `0` and `1` themselves) is rejected; any pair strictly inside is
    /// accepted.
    #[test]
    fn proptest_constructor_accepts_valid_configurations(
        m in arb_open_unit(),
        u in arb_open_unit(),
    ) {
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, m, u);
        prop_assert!(f.is_ok(), "valid probabilities rejected: m={m}, u={u}");
    }

    /// Constructor validation, mirror case: any bad `m` (including
    /// 0.0 and 1.0, which are the boundary cases the spec calls out) is
    /// rejected.
    #[test]
    fn proptest_constructor_rejects_bad_m(m in arb_bad_probability(), u in arb_open_unit()) {
        let result = FieldComparator::new("x", FieldStrategy::Exact, 0.5, m, u);
        prop_assert!(result.is_err(), "bad m={m} was accepted");
        let err = result.unwrap_err();
        let is_expected = matches!(err, LinkageModelError::InvalidMProbability { .. });
        prop_assert!(is_expected, "expected InvalidMProbability, got {:?}", err);
    }

    /// Same as above for bad `u`.
    #[test]
    fn proptest_constructor_rejects_bad_u(m in arb_open_unit(), u in arb_bad_probability()) {
        let result = FieldComparator::new("x", FieldStrategy::Exact, 0.5, m, u);
        prop_assert!(result.is_err(), "bad u={u} was accepted");
        let err = result.unwrap_err();
        let is_expected = matches!(err, LinkageModelError::InvalidUProbability { .. });
        prop_assert!(is_expected, "expected InvalidUProbability, got {:?}", err);
    }

    /// Threshold ordering: any pair with `non_match_threshold >=
    /// match_threshold` is rejected.
    #[test]
    fn proptest_constructor_rejects_swapped_thresholds(
        m in arb_open_unit(),
        u in arb_open_unit(),
        a in -100.0_f64..100.0,
        b in -100.0_f64..100.0,
    ) {
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, m, u).unwrap();
        // Force the non-match threshold to be >= the match threshold.
        let (upper, lower) = if a <= b { (a, b) } else { (b, a) };
        let result = LinkageModel::new(alloc::vec![f], upper, lower);
        prop_assert!(result.is_err(), "swapped thresholds ({upper}, {lower}) were accepted");
        let err = result.unwrap_err();
        let is_expected = matches!(err, LinkageModelError::InvalidThresholds { .. });
        prop_assert!(is_expected, "expected InvalidThresholds, got {:?}", err);
    }

    /// Monotonicity: within a discriminating model (m > u for every
    /// field), turning any subset of disagreements into agreements
    /// can only raise the score.
    ///
    /// The property picks a 3-field model and two similarity vectors
    /// where the second dominates the first field-by-field on the
    /// agreement pattern (agrees[i] >= less_agrees[i]). The model's
    /// score for the dominant vector must be at least that of the
    /// dominated vector.
    #[test]
    fn proptest_monotonicity_with_positive_agree_weights(
        model in arb_discriminating_model(3),
        pattern in proptest::collection::vec(any::<bool>(), 3..=3),
        extra_agrees in proptest::collection::vec(any::<bool>(), 3..=3),
    ) {
        // Build the dominated pattern.
        let base_pattern = pattern;
        // Build the dominant pattern: `dominant[i] = base[i] || extra[i]`.
        let dominant_pattern: Vec<bool> = base_pattern.iter()
            .zip(extra_agrees.iter())
            .map(|(&a, &b)| a || b)
            .collect();
        // Translate boolean patterns into per-field similarity values
        // that fall on the correct side of each field's agreement
        // threshold.
        let base_sims: Vec<f64> = base_pattern.iter().enumerate()
            .map(|(i, &a)| {
                let t = model.fields()[i].agreement_threshold;
                if a { t + 1e-3 } else { t - 1e-3 }
            })
            .collect();
        let dominant_sims: Vec<f64> = dominant_pattern.iter().enumerate()
            .map(|(i, &a)| {
                let t = model.fields()[i].agreement_threshold;
                if a { t + 1e-3 } else { t - 1e-3 }
            })
            .collect();
        let base_score = model.score(&base_sims);
        let dominant_score = model.score(&dominant_sims);
        prop_assert!(
            dominant_score >= base_score - 1e-12,
            "monotonicity violated: base={base_score}, dominant={dominant_score}"
        );
    }

    /// Threshold consistency: for any weight `w`, `classify_weight(w)`
    /// obeys the definition — Match iff `w >= T_μ`, NonMatch iff
    /// `w <= T_λ`, PossibleMatch otherwise.
    #[test]
    fn proptest_classifier_threshold_consistency(
        m in arb_open_unit(),
        u in arb_open_unit(),
        upper in -50.0_f64..50.0,
        gap in 1e-3_f64..100.0,
        weight in -1000.0_f64..1000.0,
    ) {
        let lower = upper - gap;
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, m, u).unwrap();
        let model = LinkageModel::new(alloc::vec![f], upper, lower).unwrap();
        let observed = model.classify_weight(weight);
        let expected = if weight >= upper {
            LinkageDecision::Match
        } else if weight <= lower {
            LinkageDecision::NonMatch
        } else {
            LinkageDecision::PossibleMatch
        };
        prop_assert_eq!(observed, expected,
            "classify_weight({}) with (T_μ={}, T_λ={}) returned {}, expected {}",
            weight, upper, lower, observed, expected);
    }

    /// Field-order symmetry: a model with fields (A, B) scoring
    /// similarities (s_A, s_B) equals a model with fields (B, A)
    /// scoring similarities (s_B, s_A).
    #[test]
    fn proptest_field_order_symmetry(
        m_a in arb_open_unit(),
        u_a in arb_open_unit(),
        t_a in 0.0_f64..1.0,
        s_a in 0.0_f64..1.0,
        m_b in arb_open_unit(),
        u_b in arb_open_unit(),
        t_b in 0.0_f64..1.0,
        s_b in 0.0_f64..1.0,
    ) {
        let field_a = FieldComparator::new("a", FieldStrategy::Custom, t_a, m_a, u_a).unwrap();
        let field_b = FieldComparator::new("b", FieldStrategy::Custom, t_b, m_b, u_b).unwrap();

        let model_ab = LinkageModel::new(alloc::vec![field_a, field_b], 1e6, -1e6).unwrap();
        let model_ba = LinkageModel::new(alloc::vec![field_b, field_a], 1e6, -1e6).unwrap();

        let score_ab = model_ab.score(&[s_a, s_b]);
        let score_ba = model_ba.score(&[s_b, s_a]);

        prop_assert!(
            (score_ab - score_ba).abs() < 1e-12,
            "field-order symmetry violated: score_ab={score_ab}, score_ba={score_ba}"
        );
    }

    /// Sanity: score is finite whenever the model was accepted (which
    /// requires m, u in (0, 1) strictly). The proof is analytic —
    /// log2 of any finite positive ratio is finite — but a property
    /// test catches accidental introduction of a non-finite path in
    /// a future refactor.
    #[test]
    fn proptest_score_is_finite_for_valid_models(
        m in arb_open_unit(),
        u in arb_open_unit(),
        sim in -10.0_f64..10.0,
    ) {
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, m, u).unwrap();
        let model = LinkageModel::new(alloc::vec![f], 1.0, -1.0).unwrap();
        let w = model.score(&[sim]);
        prop_assert!(w.is_finite(), "score {w} is not finite for m={m}, u={u}, sim={sim}");
    }

    /// Try_score's return value agrees with score's on inputs the
    /// panicking variant would accept.
    #[test]
    fn proptest_try_score_agrees_with_score(
        m in arb_open_unit(),
        u in arb_open_unit(),
        sim in -10.0_f64..10.0,
    ) {
        let f = FieldComparator::new("x", FieldStrategy::Exact, 0.5, m, u).unwrap();
        let model = LinkageModel::new(alloc::vec![f], 1.0, -1.0).unwrap();
        let scored = model.score(&[sim]);
        let tried = model.try_score(&[sim]).unwrap();
        prop_assert_eq!(scored.to_bits(), tried.to_bits(),
            "score and try_score disagreed: score={}, try_score={}", scored, tried);
    }
}
