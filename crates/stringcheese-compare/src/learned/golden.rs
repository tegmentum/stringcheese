//! Golden cases for the Ristad-Yianilos learned edit distance.
//!
//! Because the learned distance's expected outputs depend on the trained
//! model, every golden case here fixes a specific hand-authored model as
//! its provenance and states expected distances *for that model*. Two
//! shared models are provided:
//!
//! * [`uniform_abc_model`] — the uniform 3-symbol model over `{a, b, c}`.
//!   Cases against this model probe the "uniform costs everywhere,
//!   distance is proportional to Levenshtein" edge of the parameter
//!   space.
//! * [`match_heavy_model`] — a hand-authored model with match
//!   substitutions strongly preferred over insertions, deletions, or
//!   mismatch substitutions. Cases against this model probe the "trained"
//!   edge, where matches are cheap and non-matches are expensive.
//!
//! Both models are legal probability distributions; the shared test
//! `models_are_valid_probability_distributions` asserts that. Every case's
//! expected distance is a [`FloatExpectation::Absolute`] with a tolerance
//! commensurate with the precision of the closed-form computation used to
//! derive it.

#![cfg(feature = "std")]

use alloc::collections::BTreeMap;
use stringcheese_corpus::{FloatExpectation, GoldenCase, GoldenSource};

use crate::learned::distance::LearnedEdit;
use crate::learned::model::LearnedEditModel;

/// A byte-slice input pair.
pub type BytesInput = (&'static [u8], &'static [u8]);

/// The concrete `GoldenCase` type for byte-slice learned-edit cases,
/// carrying a [`FloatExpectation`] for the expected distance under the
/// case's referenced model.
pub type BytesCase = GoldenCase<BytesInput, FloatExpectation>;

/// A uniform 3-symbol model. Every deletion, insertion, and substitution
/// (over `{a, b, c}`) and the end event carries equal probability. Under
/// this model, distance is proportional to the Levenshtein edit-sequence
/// length plus a constant end-event offset.
#[must_use]
pub fn uniform_abc_model() -> LearnedEditModel<u8> {
    LearnedEditModel::uniform(b"abc")
}

/// A hand-authored "match-heavy" model over `{a, b, c}`:
///
/// * Each identity substitution `(x, x)` has probability `0.20`.
/// * Each non-identity substitution `(x, y)` (`x != y`) has probability `0.01`.
/// * Each deletion probability is `0.05`.
/// * Each insertion probability is `0.05`.
/// * End probability is `0.04`.
///
/// Total probability mass: `3·0.20 + 6·0.01 + 3·0.05 + 3·0.05 + 0.04 =
/// 0.60 + 0.06 + 0.15 + 0.15 + 0.04 = 1.00`. This is what
/// [`models_are_valid_probability_distributions`] asserts.
#[must_use]
pub fn match_heavy_model() -> LearnedEditModel<u8> {
    let mut delete = BTreeMap::new();
    delete.insert(b'a', 0.05);
    delete.insert(b'b', 0.05);
    delete.insert(b'c', 0.05);
    let mut insert = BTreeMap::new();
    insert.insert(b'a', 0.05);
    insert.insert(b'b', 0.05);
    insert.insert(b'c', 0.05);
    let mut sub = BTreeMap::new();
    for &s in b"abc" {
        for &t in b"abc" {
            let p = if s == t { 0.20 } else { 0.01 };
            sub.insert((s, t), p);
        }
    }
    LearnedEditModel::from_probabilities(delete, insert, sub, 0.04)
}

// Precomputed constants for the match-heavy model's cases.
// -ln(0.20) ≈ 1.6094379124341003
// -ln(0.01) ≈ 4.605170185988091
// -ln(0.05) ≈ 2.995732273553991
// -ln(0.04) ≈ 3.2188758248682006

/// Golden cases for byte-slice learned-edit-distance computations.
///
/// Each case's expected value is derived by tracing the DP on paper under
/// the referenced model — the same procedure a reviewer can rerun.
pub const GOLDEN_BYTES: &[BytesCase] = &[
    // ---- Uniform model over {a, b, c} ----
    //
    // Under uniform costs (each event has probability 1/16), every edit
    // has cost ln(16) ≈ 2.772588722239781. distance(x, y) = k * ln(16) +
    // ln(16) for k = |edit sequence|, so distance(empty, empty) = ln(16),
    // distance(a, a) = 2*ln(16), distance(a, b) = 2*ln(16) (one
    // substitution), and so on.
    GoldenCase {
        id: "learned-edit/uniform-abc/empty-empty",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"", b""),
        expected: FloatExpectation::Absolute {
            value: 2.772_588_722_239_781,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Uniform 3-symbol model: distance(empty, empty) is just the end-event cost, ln(16).",
        tags: &["basic", "empty", "uniform"],
    },
    GoldenCase {
        id: "learned-edit/uniform-abc/identity-a",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"a", b"a"),
        expected: FloatExpectation::Absolute {
            // ln(16) for the identity sub + ln(16) for end.
            value: 5.545_177_444_479_563,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Uniform 3-symbol model: distance('a', 'a') = 2 · ln(16). Every edit is equally likely under uniform, so identity is not preferred.",
        tags: &["basic", "identity", "uniform"],
    },
    GoldenCase {
        id: "learned-edit/uniform-abc/one-substitution",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"a", b"b"),
        expected: FloatExpectation::Absolute {
            // Same as identity — under uniform every 1-step path has the same cost.
            value: 5.545_177_444_479_563,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Uniform 3-symbol model: distance('a', 'b') = 2 · ln(16), identical to distance('a', 'a'). Uniform models are Levenshtein up to a constant.",
        tags: &["basic", "substitution", "uniform"],
    },
    GoldenCase {
        id: "learned-edit/uniform-abc/one-insertion",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"", b"a"),
        expected: FloatExpectation::Absolute {
            // One insert + one end = 2 · ln(16).
            value: 5.545_177_444_479_563,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Uniform 3-symbol model: one insertion, then end.",
        tags: &["basic", "insertion", "uniform"],
    },
    GoldenCase {
        id: "learned-edit/uniform-abc/one-deletion",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"a", b""),
        expected: FloatExpectation::Absolute {
            value: 5.545_177_444_479_563,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Uniform 3-symbol model: one deletion, then end.",
        tags: &["basic", "deletion", "uniform"],
    },
    GoldenCase {
        id: "learned-edit/uniform-abc/two-mismatches",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"ab", b"cc"),
        expected: FloatExpectation::Absolute {
            // Two substitutions + end = 3 · ln(16).
            value: 8.317_766_166_719_343,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Uniform 3-symbol model: two substitutions + end = 3 · ln(16).",
        tags: &["basic", "substitution", "uniform"],
    },
    // ---- Match-heavy hand-authored model ----
    //
    // -ln(0.20) ≈ 1.6094379124341003    (identity substitution cost)
    // -ln(0.01) ≈ 4.605170185988091     (mismatch substitution cost)
    // -ln(0.05) ≈ 2.995732273553991     (insertion / deletion cost)
    // -ln(0.04) ≈ 3.2188758248682006    (end cost)
    GoldenCase {
        id: "learned-edit/match-heavy/empty-empty",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"", b""),
        expected: FloatExpectation::Absolute {
            value: 3.218_875_824_868_200_6,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Match-heavy model: distance(empty, empty) = -ln(0.04).",
        tags: &["basic", "empty", "match-heavy"],
    },
    GoldenCase {
        id: "learned-edit/match-heavy/identity-abc",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"abc", b"abc"),
        expected: FloatExpectation::Absolute {
            // 3 · identity-cost + end-cost = 3 · (-ln(0.20)) + (-ln(0.04))
            //   = 3 · 1.6094379... + 3.2188758... = 4.828314 + 3.218876 = 8.047190
            value: 8.047_189_562_170_502,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Match-heavy model: three identity substitutions + one end.",
        tags: &["identity", "match-heavy"],
    },
    GoldenCase {
        id: "learned-edit/match-heavy/one-substitution",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"abc", b"abd"),
        expected: FloatExpectation::Absolute {
            // Two identity subs + one mismatch sub — but 'd' is not in
            // the alphabet, so the substitution edit substitute('c', 'd')
            // is +inf. The DP has no valid path. This case exists to
            // document the "out-of-alphabet input yields +inf" behavior.
            value: f64::INFINITY,
            tolerance: 0.0,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Match-heavy model does not know 'd'; the DP has no finite path. The absolute-with-tolerance-0 check catches non-inf answers.",
        tags: &["out-of-alphabet", "match-heavy", "infinity"],
    },
    GoldenCase {
        id: "learned-edit/match-heavy/one-mismatch",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"abc", b"aec"),
        expected: FloatExpectation::Absolute {
            // 'e' is not in {a, b, c}. Same behavior as above.
            value: f64::INFINITY,
            tolerance: 0.0,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Match-heavy model: mismatch with an unknown target symbol yields infinity.",
        tags: &["out-of-alphabet", "match-heavy", "infinity"],
    },
    GoldenCase {
        id: "learned-edit/match-heavy/single-mismatch-in-alphabet",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"abc", b"abb"),
        expected: FloatExpectation::Absolute {
            // Two identity subs + one mismatch sub (c -> b) + end
            //   = 2 · 1.6094379124341003 + 4.605170185988091 + 3.2188758248682006
            //   = 3.2188758248682006 + 4.605170185988091 + 3.2188758248682006
            //   ≈ 11.042921835724492
            value: 11.042_921_835_724_492,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Match-heavy model: identity for a and b, mismatch for c -> b, plus end.",
        tags: &["substitution", "match-heavy"],
    },
    GoldenCase {
        id: "learned-edit/match-heavy/insert-then-match",
        descriptor: LearnedEdit::<u8>::DESCRIPTOR,
        input: (b"a", b"aa"),
        expected: FloatExpectation::Absolute {
            // One identity + one insert + end
            //   = 1.6094379 + 2.9957323 + 3.2188758 = 7.824046
            // Alternative path: one insert + one identity + end = same cost.
            value: 7.824_046_010_856_292,
            tolerance: 1e-9,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Match-heavy model: one identity + one insert + end.",
        tags: &["insertion", "match-heavy"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_core::DistanceMetric;

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for case in GOLDEN_BYTES {
            assert_eq!(
                case.descriptor,
                LearnedEdit::<u8>::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let ids: alloc::vec::Vec<&str> = GOLDEN_BYTES.iter().map(|c| c.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }

    #[test]
    fn corpus_meets_minimum_size() {
        assert!(
            GOLDEN_BYTES.len() >= 12,
            "expected at least 12 golden cases for learned-edit"
        );
    }

    #[test]
    fn models_are_valid_probability_distributions() {
        let uniform = uniform_abc_model();
        let mass_uniform = uniform.probability_mass();
        assert!(
            (mass_uniform - 1.0).abs() < 1e-10,
            "uniform model mass {mass_uniform} != 1"
        );
        let match_heavy = match_heavy_model();
        let mass_mh = match_heavy.probability_mass();
        assert!(
            (mass_mh - 1.0).abs() < 1e-10,
            "match-heavy model mass {mass_mh} != 1"
        );
    }

    #[test]
    fn every_uniform_case_matches_the_algorithm() {
        let alg = LearnedEdit::new(uniform_abc_model());
        for case in GOLDEN_BYTES {
            if !case.id.contains("uniform-abc") {
                continue;
            }
            let (source, target) = case.input;
            let observed = alg.distance(source, target).into_inner();
            assert!(
                case.expected.matches(observed),
                "case {}: expected {:?}, observed {}",
                case.id,
                case.expected,
                observed
            );
        }
    }

    #[test]
    fn every_match_heavy_case_matches_the_algorithm() {
        let alg = LearnedEdit::new(match_heavy_model());
        for case in GOLDEN_BYTES {
            if !case.id.contains("match-heavy") {
                continue;
            }
            let (source, target) = case.input;
            let observed = alg.distance(source, target).into_inner();
            // FloatExpectation::Absolute { tolerance: 0.0 } would reject
            // an infinite `observed` even against an infinite expected —
            // an absolute-value subtraction of inf minus inf is NaN. Handle
            // infinity comparisons explicitly.
            if let FloatExpectation::Absolute { value, .. } = case.expected {
                if value.is_infinite() {
                    assert!(
                        observed.is_infinite(),
                        "case {}: expected infinity, observed {}",
                        case.id,
                        observed
                    );
                    continue;
                }
            }
            assert!(
                case.expected.matches(observed),
                "case {}: expected {:?}, observed {}",
                case.id,
                case.expected,
                observed
            );
        }
    }
}
