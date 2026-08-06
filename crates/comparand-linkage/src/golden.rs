//! Canonical Fellegi-Sunter golden cases.
//!
//! Each entry is a [`LinkageGoldenCase`] pairing a model specification with
//! a per-field similarity vector, an expected record-pair weight
//! (compared under a [`FloatExpectation`] policy), and an expected
//! [`LinkageDecision`]. The tests below build a [`LinkageModel`] from each
//! specification and verify that the model reproduces both the numeric
//! weight and the classified decision.
//!
//! # Provenance
//!
//! Every case is [`GoldenSource::IndependentlyDerived`] — the numeric
//! weights are derived by hand from the Fellegi-Sunter formula rather
//! than lifted from a specific reference implementation. The 1969 paper
//! itself introduces the mathematical framework but does not publish a
//! single small numeric worked example that could be reproduced verbatim;
//! the parameter choices below (`m=0.9, u=0.1` for the single-field
//! discriminator, three-field surname/given/DOB models with plausible
//! probabilities) are the ones the record-linkage literature has
//! converged on as illustrative textbook examples.

use comparand_corpus::{FloatExpectation, GoldenSource};

use crate::classifier::LinkageDecision;
use crate::field::{FieldComparator, FieldStrategy};
use crate::model::LinkageModel;

/// A canonical Fellegi-Sunter test case.
///
/// The case owns its model specification (as slices of primitives so the
/// full case can live in `const` context) plus the per-field similarity
/// vector and both the expected weight and expected decision.
#[derive(Copy, Clone, Debug)]
pub struct LinkageGoldenCase {
    /// Hierarchically named case id, e.g.
    /// `"linkage/one-field/perfect-agreement"`.
    pub id: &'static str,
    /// Provenance for the expected values.
    pub source: GoldenSource,
    /// Free-form notes about what the case exercises.
    pub notes: &'static str,
    /// Names of each field, in declaration order.
    pub field_names: &'static [&'static str],
    /// Comparator-strategy tags for each field.
    pub strategies: &'static [FieldStrategy],
    /// Per-field agreement thresholds.
    pub agreement_thresholds: &'static [f64],
    /// Per-field `m_i` probabilities.
    pub m_probabilities: &'static [f64],
    /// Per-field `u_i` probabilities.
    pub u_probabilities: &'static [f64],
    /// Model upper threshold `T_μ`.
    pub match_threshold: f64,
    /// Model lower threshold `T_λ`.
    pub non_match_threshold: f64,
    /// Per-field similarity scores for the candidate pair.
    pub field_similarities: &'static [f64],
    /// Expected record-pair weight (as a [`FloatExpectation`] because
    /// `log2` results are not exactly representable in `f64`).
    pub expected_weight: FloatExpectation,
    /// Expected classification.
    pub expected_decision: LinkageDecision,
}

impl LinkageGoldenCase {
    /// Builds a [`LinkageModel`] from this case's parameters. Used by
    /// the test loop and available to downstream consumers that want to
    /// exercise a golden fixture end-to-end.
    ///
    /// # Panics
    ///
    /// Panics if the parameters violate a Fellegi-Sunter invariant.
    /// Golden cases in this module are hand-checked to be valid, so a
    /// panic here indicates a fixture bug and should fail loudly.
    #[must_use]
    pub fn build_model(&self) -> LinkageModel {
        let mut fields = alloc::vec::Vec::with_capacity(self.field_names.len());
        for i in 0..self.field_names.len() {
            let f = FieldComparator::new(
                self.field_names[i],
                self.strategies[i],
                self.agreement_thresholds[i],
                self.m_probabilities[i],
                self.u_probabilities[i],
            )
            .expect("golden-case fixture supplies valid FieldComparator parameters");
            fields.push(f);
        }
        LinkageModel::new(fields, self.match_threshold, self.non_match_threshold)
            .expect("golden-case fixture supplies valid LinkageModel parameters")
    }
}

// The tolerance for weight comparisons. log2 is not exactly representable
// for the small rational arguments the golden cases use (log2(9),
// log2(6), log2(99), etc.), so we compare within a modest relative
// tolerance. 1e-12 is comfortably above the accumulated rounding of half
// a dozen f64 log2 calls and their sum.
const WEIGHT_TOLERANCE: f64 = 1e-12;

const ONE_FIELD_NAMES: &[&str] = &["surname"];
const ONE_FIELD_STRATEGIES: &[FieldStrategy] = &[FieldStrategy::JaroWinklerSimilarity];
const ONE_FIELD_THRESHOLDS: &[f64] = &[0.85];
const ONE_FIELD_M: &[f64] = &[0.9];
const ONE_FIELD_U: &[f64] = &[0.1];

const SIM_AGREE_1: &[f64] = &[1.0];
const SIM_DISAGREE_1: &[f64] = &[0.0];
const SIM_AT_THRESHOLD_1: &[f64] = &[0.85];
const SIM_JUST_BELOW_THRESHOLD_1: &[f64] = &[0.849_999_999];

const THREE_FIELD_NAMES: &[&str] = &["surname", "given_name", "dob"];
const THREE_FIELD_STRATEGIES: &[FieldStrategy] = &[
    FieldStrategy::JaroWinklerSimilarity,
    FieldStrategy::LevenshteinNormalized,
    FieldStrategy::Exact,
];
const THREE_FIELD_THRESHOLDS: &[f64] = &[0.85, 0.8, 0.5];
// Realistic-ish parameters — surname is a strong discriminator, given
// name slightly weaker, DOB the strongest because exact-match birthdate
// is highly discriminatory in typical demographics.
const THREE_FIELD_M: &[f64] = &[0.9, 0.85, 0.99];
const THREE_FIELD_U: &[f64] = &[0.1, 0.15, 0.01];

const THREE_FIELD_ALL_AGREE: &[f64] = &[1.0, 1.0, 1.0];
const THREE_FIELD_ONE_AGREE: &[f64] = &[1.0, 0.0, 0.0];
const THREE_FIELD_NONE_AGREE: &[f64] = &[0.0, 0.0, 0.0];

/// The full canonical Fellegi-Sunter corpus.
pub const GOLDEN_CASES: &[LinkageGoldenCase] = &[
    // Case 1: single-field perfect agreement.
    // log2(0.9/0.1) = log2(9).
    LinkageGoldenCase {
        id: "linkage/one-field/perfect-agreement",
        source: GoldenSource::IndependentlyDerived,
        notes: "Single-field model with m=0.9, u=0.1. Similarity above threshold produces the agree weight log2(9) ≈ 3.17, which exceeds T_μ=1.0 so the classifier returns Match.",
        field_names: ONE_FIELD_NAMES,
        strategies: ONE_FIELD_STRATEGIES,
        agreement_thresholds: ONE_FIELD_THRESHOLDS,
        m_probabilities: ONE_FIELD_M,
        u_probabilities: ONE_FIELD_U,
        match_threshold: 1.0,
        non_match_threshold: -1.0,
        field_similarities: SIM_AGREE_1,
        expected_weight: FloatExpectation::Relative {
            value: 3.169_925_001_442_312,
            tolerance: WEIGHT_TOLERANCE,
        },
        expected_decision: LinkageDecision::Match,
    },
    // Case 2: single-field perfect disagreement.
    // log2(0.1/0.9) = -log2(9).
    LinkageGoldenCase {
        id: "linkage/one-field/zero-agreement",
        source: GoldenSource::IndependentlyDerived,
        notes: "Same single-field model as above; similarity below threshold produces the disagree weight -log2(9) ≈ -3.17, below T_λ=-1.0 so the classifier returns NonMatch.",
        field_names: ONE_FIELD_NAMES,
        strategies: ONE_FIELD_STRATEGIES,
        agreement_thresholds: ONE_FIELD_THRESHOLDS,
        m_probabilities: ONE_FIELD_M,
        u_probabilities: ONE_FIELD_U,
        match_threshold: 1.0,
        non_match_threshold: -1.0,
        field_similarities: SIM_DISAGREE_1,
        expected_weight: FloatExpectation::Relative {
            value: -3.169_925_001_442_312,
            tolerance: WEIGHT_TOLERANCE,
        },
        expected_decision: LinkageDecision::NonMatch,
    },
    // Case 3: similarity exactly at the agreement threshold. The
    // AgreementRule's convention is inclusive, so this still counts
    // as agreement.
    LinkageGoldenCase {
        id: "linkage/one-field/similarity-at-threshold",
        source: GoldenSource::IndependentlyDerived,
        notes: "Similarity exactly at the agreement threshold (0.85) counts as agreement (inclusive lower bound) — this locks in the AgreementRule::agrees semantics.",
        field_names: ONE_FIELD_NAMES,
        strategies: ONE_FIELD_STRATEGIES,
        agreement_thresholds: ONE_FIELD_THRESHOLDS,
        m_probabilities: ONE_FIELD_M,
        u_probabilities: ONE_FIELD_U,
        match_threshold: 1.0,
        non_match_threshold: -1.0,
        field_similarities: SIM_AT_THRESHOLD_1,
        expected_weight: FloatExpectation::Relative {
            value: 3.169_925_001_442_312,
            tolerance: WEIGHT_TOLERANCE,
        },
        expected_decision: LinkageDecision::Match,
    },
    // Case 4: similarity just below the agreement threshold — the
    // exclusive complement of case 3.
    LinkageGoldenCase {
        id: "linkage/one-field/similarity-just-below-threshold",
        source: GoldenSource::IndependentlyDerived,
        notes: "Similarity strictly below the agreement threshold counts as disagreement, locking in the exclusive-upper-bound side of the AgreementRule::agrees semantics.",
        field_names: ONE_FIELD_NAMES,
        strategies: ONE_FIELD_STRATEGIES,
        agreement_thresholds: ONE_FIELD_THRESHOLDS,
        m_probabilities: ONE_FIELD_M,
        u_probabilities: ONE_FIELD_U,
        match_threshold: 1.0,
        non_match_threshold: -1.0,
        field_similarities: SIM_JUST_BELOW_THRESHOLD_1,
        expected_weight: FloatExpectation::Relative {
            value: -3.169_925_001_442_312,
            tolerance: WEIGHT_TOLERANCE,
        },
        expected_decision: LinkageDecision::NonMatch,
    },
    // Case 5: three-field model, all fields agree. Weight is the sum
    // of three agree_weights. log2(9) + log2(85/15) + log2(99).
    LinkageGoldenCase {
        id: "linkage/three-field/all-agree",
        source: GoldenSource::IndependentlyDerived,
        notes: "Three-field surname/given/DOB model with all fields agreeing; weight = log2(9) + log2(85/15) + log2(99) ≈ 12.38 -> Match.",
        field_names: THREE_FIELD_NAMES,
        strategies: THREE_FIELD_STRATEGIES,
        agreement_thresholds: THREE_FIELD_THRESHOLDS,
        m_probabilities: THREE_FIELD_M,
        u_probabilities: THREE_FIELD_U,
        match_threshold: 5.0,
        non_match_threshold: -5.0,
        field_similarities: THREE_FIELD_ALL_AGREE,
        expected_weight: FloatExpectation::Absolute {
            // Precomputed: log2(0.9/0.1) + log2(0.85/0.15) + log2(0.99/0.01)
            //            ≈ 3.169925001442312 + 2.5025003405291826 + 6.629356620105241
            //            ≈ 12.301781962076735
            value: 12.301_781_962_076_735,
            tolerance: 1e-9,
        },
        expected_decision: LinkageDecision::Match,
    },
    // Case 6: three-field model, exactly one field agrees. Weight is
    // one agree contribution and two disagree contributions.
    LinkageGoldenCase {
        id: "linkage/three-field/one-agree",
        source: GoldenSource::IndependentlyDerived,
        notes: "Surname agrees; given name and DOB do not. Weight is one positive + two negative contributions — enough to land in the possible-match middle region.",
        field_names: THREE_FIELD_NAMES,
        strategies: THREE_FIELD_STRATEGIES,
        agreement_thresholds: THREE_FIELD_THRESHOLDS,
        m_probabilities: THREE_FIELD_M,
        u_probabilities: THREE_FIELD_U,
        match_threshold: 5.0,
        non_match_threshold: -5.0,
        field_similarities: THREE_FIELD_ONE_AGREE,
        expected_weight: FloatExpectation::Absolute {
            // log2(0.9/0.1) + log2(0.15/0.85) + log2(0.01/0.99)
            //   ≈ 3.169925001442312 + (-2.5025003405291826) + (-6.629356620105241)
            //   ≈ -5.961931959192112
            value: -5.961_931_959_192_112,
            tolerance: 1e-9,
        },
        expected_decision: LinkageDecision::NonMatch,
    },
    // Case 7: three-field model, no fields agree.
    LinkageGoldenCase {
        id: "linkage/three-field/none-agree",
        source: GoldenSource::IndependentlyDerived,
        notes: "All three fields disagree; weight is the sum of three disagree contributions and is strongly negative.",
        field_names: THREE_FIELD_NAMES,
        strategies: THREE_FIELD_STRATEGIES,
        agreement_thresholds: THREE_FIELD_THRESHOLDS,
        m_probabilities: THREE_FIELD_M,
        u_probabilities: THREE_FIELD_U,
        match_threshold: 5.0,
        non_match_threshold: -5.0,
        field_similarities: THREE_FIELD_NONE_AGREE,
        expected_weight: FloatExpectation::Absolute {
            // log2(0.1/0.9) + log2(0.15/0.85) + log2(0.01/0.99)
            //   ≈ -3.169925001442312 + -2.5025003405291826 + -6.629356620105241
            //   ≈ -12.301781962076735
            value: -12.301_781_962_076_735,
            tolerance: 1e-9,
        },
        expected_decision: LinkageDecision::NonMatch,
    },
    // Case 8: possible-match middle-region case. Wide thresholds so
    // even the all-agree weight lands strictly between them.
    LinkageGoldenCase {
        id: "linkage/one-field/possible-match-wide-thresholds",
        source: GoldenSource::IndependentlyDerived,
        notes: "Wide thresholds T_μ=10.0, T_λ=-10.0 leave the agree weight (~3.17) strictly in the middle region -> PossibleMatch (clerical review).",
        field_names: ONE_FIELD_NAMES,
        strategies: ONE_FIELD_STRATEGIES,
        agreement_thresholds: ONE_FIELD_THRESHOLDS,
        m_probabilities: ONE_FIELD_M,
        u_probabilities: ONE_FIELD_U,
        match_threshold: 10.0,
        non_match_threshold: -10.0,
        field_similarities: SIM_AGREE_1,
        expected_weight: FloatExpectation::Relative {
            value: 3.169_925_001_442_312,
            tolerance: WEIGHT_TOLERANCE,
        },
        expected_decision: LinkageDecision::PossibleMatch,
    },
    // Case 9: threshold-boundary Match. Chosen so the weight equals
    // T_μ exactly; the classifier's inclusive comparison at the
    // upper bound must return Match.
    LinkageGoldenCase {
        id: "linkage/one-field/boundary-at-upper-threshold",
        source: GoldenSource::IndependentlyDerived,
        notes: "T_μ equals the agree weight bit-exactly (log2(9)); the classifier's `weight >= T_μ` semantics must return Match at the boundary.",
        field_names: ONE_FIELD_NAMES,
        strategies: ONE_FIELD_STRATEGIES,
        agreement_thresholds: ONE_FIELD_THRESHOLDS,
        m_probabilities: ONE_FIELD_M,
        u_probabilities: ONE_FIELD_U,
        match_threshold: 3.169_925_001_442_312,
        non_match_threshold: -3.169_925_001_442_312_5,
        field_similarities: SIM_AGREE_1,
        expected_weight: FloatExpectation::Relative {
            value: 3.169_925_001_442_312,
            tolerance: WEIGHT_TOLERANCE,
        },
        expected_decision: LinkageDecision::Match,
    },
    // Case 10: threshold-boundary NonMatch.
    LinkageGoldenCase {
        id: "linkage/one-field/boundary-at-lower-threshold",
        source: GoldenSource::IndependentlyDerived,
        notes: "T_λ equals the disagree weight bit-exactly (-log2(9)); the classifier's `weight <= T_λ` semantics must return NonMatch at the boundary.",
        field_names: ONE_FIELD_NAMES,
        strategies: ONE_FIELD_STRATEGIES,
        agreement_thresholds: ONE_FIELD_THRESHOLDS,
        m_probabilities: ONE_FIELD_M,
        u_probabilities: ONE_FIELD_U,
        match_threshold: 3.169_925_001_442_312_5,
        non_match_threshold: -3.169_925_001_442_312,
        field_similarities: SIM_DISAGREE_1,
        expected_weight: FloatExpectation::Relative {
            value: -3.169_925_001_442_312,
            tolerance: WEIGHT_TOLERANCE,
        },
        expected_decision: LinkageDecision::NonMatch,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn every_case_is_a_valid_model_specification() {
        for case in GOLDEN_CASES {
            let _ = case.build_model();
        }
    }

    #[test]
    fn every_case_matches_its_expected_weight() {
        for case in GOLDEN_CASES {
            let model = case.build_model();
            let observed = model.score(case.field_similarities);
            assert!(
                case.expected_weight.matches(observed),
                "golden case {} weight mismatch: observed {observed}, expected {:?}",
                case.id,
                case.expected_weight,
            );
        }
    }

    #[test]
    fn every_case_matches_its_expected_decision() {
        for case in GOLDEN_CASES {
            let model = case.build_model();
            let observed = model.classify(case.field_similarities);
            assert_eq!(
                observed, case.expected_decision,
                "golden case {} decision mismatch: observed {observed}, expected {}",
                case.id, case.expected_decision,
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // Spec asks for at least 8 golden cases.
        assert!(
            GOLDEN_CASES.len() >= 8,
            "expected at least 8 golden cases, got {}",
            GOLDEN_CASES.len()
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let ids: Vec<&str> = GOLDEN_CASES.iter().map(|c| c.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }

    #[test]
    fn every_case_records_provenance() {
        // Every case should have a non-empty notes field so the failure
        // diagnostic is self-explanatory, and every case should declare
        // a `GoldenSource` (that the field is read at all is the point;
        // this test also keeps the compiler from flagging it as dead).
        for case in GOLDEN_CASES {
            assert!(
                !case.notes.is_empty(),
                "golden case {} has empty notes",
                case.id
            );
            assert_eq!(
                case.source,
                GoldenSource::IndependentlyDerived,
                "golden case {} unexpectedly changed source",
                case.id,
            );
        }
    }

    /// Integration test using `comparand-jaro`: the crate's declared
    /// natural per-field comparator. Verifies the golden-case model
    /// integrates with a real similarity source, not just hand-supplied
    /// f64s.
    #[test]
    fn integrates_with_comparand_jaro() {
        use comparand_core::SimilarityMetric;
        use comparand_jaro::JaroWinkler;

        // Single-surname model, m=0.9, u=0.1, threshold 0.85.
        let field = FieldComparator::new(
            "surname",
            FieldStrategy::JaroWinklerSimilarity,
            0.85,
            0.9,
            0.1,
        )
        .unwrap();
        let model = LinkageModel::new(alloc::vec![field], 1.0, -1.0).unwrap();

        let jw = JaroWinkler::classic();
        // MARTHA vs MARHTA is a canonical high-similarity example from
        // Jaro (1989).
        let sim = jw.similarity(b"MARTHA", b"MARHTA").into_inner();
        assert!(sim >= 0.85, "MARTHA vs MARHTA should score above 0.85");
        assert_eq!(model.classify(&[sim]), LinkageDecision::Match);

        // Disjoint surnames — well below the threshold.
        let sim_bad = jw.similarity(b"SMITH", b"XYZQW").into_inner();
        assert!(sim_bad < 0.85);
        assert_eq!(model.classify(&[sim_bad]), LinkageDecision::NonMatch);
    }
}
