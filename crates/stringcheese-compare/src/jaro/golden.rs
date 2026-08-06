//! Canonical Jaro and Jaro-Winkler golden cases wired to the
//! `stringcheese-corpus` [`FloatExpectation`] schema.
//!
//! This is the first crate in StringCheese that exercises `FloatExpectation`
//! seriously. The pattern established here — every floating-point case
//! declares its comparison policy explicitly, and cases derived from the
//! algorithm's paper cite the paper via `PublishedPaper` while cases
//! independently traced by hand cite `IndependentlyDerived` — is the
//! template every future floating-point algorithm crate should follow.
//!
//! # Tolerance choices
//!
//! * **[`FloatExpectation::ExactBits`]** for cases whose expected value is a
//!   representable constant such as `0.0` or `1.0`. These are boundary
//!   conditions (identity, empty pair, no matches) where any deviation is a
//!   bug, not floating-point noise.
//! * **[`FloatExpectation::Absolute`] with tolerance `1e-4`** for cases
//!   whose expected value comes from a paper published to four decimal
//!   digits. Tightening the tolerance below what the paper committed to
//!   would fail on values the paper itself is ambiguous about.
//! * **[`FloatExpectation::Absolute`] with tolerance `1e-12`** for cases
//!   whose expected value is a closed-form rational (like `17/18`) that we
//!   compute here as well. The tolerance covers the small rounding
//!   differences between `(17.0/18.0) - (17.0_f64/18.0_f64)`-style
//!   reformulations without accepting genuine algorithmic drift.
//!
//! Every case's descriptor matches the specific variant being tested; a
//! case tagged with [`Jaro::DESCRIPTOR`] cannot silently be validated
//! against [`JaroWinkler::CLASSIC_DESCRIPTOR`], because
//! `every_case_uses_the_correct_descriptor` at the bottom of this file
//! will reject a mismatch.
//!
//! [`FloatExpectation`]: stringcheese_corpus::FloatExpectation

use stringcheese_corpus::{FloatExpectation, GoldenCase, GoldenSource};

use crate::jaro::jaro::Jaro;
use crate::jaro::jaro_winkler::JaroWinkler;

/// A byte-slice input pair, as stored in a Jaro golden case.
pub type BytesInput = (&'static [u8], &'static [u8]);

/// Concrete `GoldenCase` type for byte-slice Jaro-family cases carrying a
/// [`FloatExpectation`].
pub type BytesCase = GoldenCase<BytesInput, FloatExpectation>;

// Citations kept as short strings so each case reads at one glance. Each
// `PublishedPaper` citation is quoted with enough detail to locate the paper
// in any library index; `IndependentlyDerived` cases carry the arithmetic
// derivation in the case's `notes` field.
const JARO_1989: GoldenSource = GoldenSource::PublishedPaper {
    citation: "Matthew A. Jaro, \"Advances in Record-Linkage Methodology as Applied to Matching the 1985 Census of Tampa, Florida\", Journal of the American Statistical Association 84(406), 1989, pp. 414-420.",
};

const WINKLER_1990: GoldenSource = GoldenSource::PublishedPaper {
    citation: "William E. Winkler, \"String Comparator Metrics and Enhanced Decision Rules in the Fellegi-Sunter Model of Record Linkage\", Proceedings of the Section on Survey Research Methods, American Statistical Association, 1990, pp. 354-359.",
};

/// Golden cases for the base [`Jaro`] similarity.
pub const GOLDEN_JARO: &[BytesCase] = &[
    GoldenCase {
        id: "jaro/basic/empty-empty",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"", b""),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Two empty inputs are treated as identical by the boundary convention; similarity is exactly 1.0.",
        tags: &["basic", "empty", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "jaro/basic/left-empty",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"", b"hello"),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "One empty and one non-empty input have zero possible matches; similarity is exactly 0.0.",
        tags: &["basic", "empty", "exact-bits"],
    },
    GoldenCase {
        id: "jaro/basic/right-empty",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"hello", b""),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Symmetric to left-empty; similarity is exactly 0.0.",
        tags: &["basic", "empty", "exact-bits"],
    },
    GoldenCase {
        id: "jaro/basic/identical",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"kitten", b"kitten"),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Identical inputs: m = |a| = |b|, t = 0, so similarity = (1 + 1 + 1)/3 = 1.0 exactly.",
        tags: &["basic", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "jaro/basic/no-matches",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"abc", b"xyz"),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Disjoint alphabets under a matching window of zero. The m = 0 short-circuit returns exactly 0.0.",
        tags: &["basic", "no-match", "exact-bits"],
    },
    GoldenCase {
        id: "jaro/single-char/match",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"a", b"a"),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Single matching character; the matching-window arithmetic collapses to a single pair.",
        tags: &["basic", "single-char", "exact-bits"],
    },
    GoldenCase {
        id: "jaro/single-char/mismatch",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"a", b"b"),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Single mismatched character: m = 0, similarity = 0.0.",
        tags: &["basic", "single-char", "exact-bits"],
    },
    GoldenCase {
        id: "jaro/canonical/martha-marhta",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"MARTHA", b"MARHTA"),
        // 17/18 = 0.94444...; cite the paper to four decimals.
        expected: FloatExpectation::Absolute {
            value: 0.9444,
            tolerance: 1e-4,
        },
        source: JARO_1989,
        notes: "Jaro's canonical worked example: m = 6, t = 1, similarity = (1 + 1 + 5/6)/3 = 17/18 ≈ 0.9444.",
        tags: &["canonical", "paper", "transposition"],
    },
    GoldenCase {
        id: "jaro/canonical/dwayne-duane",
        descriptor: Jaro::DESCRIPTOR,
        // Winkler (1990) cites this pair on p. 356 as a running example.
        input: (b"DWAYNE", b"DUANE"),
        expected: FloatExpectation::Absolute {
            value: 0.822,
            tolerance: 1e-3,
        },
        source: WINKLER_1990,
        notes: "Winkler's running example: m = 4, t = 0, similarity ≈ 0.822. Paper committed to three decimals.",
        tags: &["canonical", "paper", "unequal-length"],
    },
    GoldenCase {
        id: "jaro/canonical/dixon-dicksonx",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"DIXON", b"DICKSONX"),
        expected: FloatExpectation::Absolute {
            value: 0.767,
            tolerance: 1e-3,
        },
        source: WINKLER_1990,
        notes: "Winkler (1990) example: m = 4, t = 0, similarity = (4/5 + 4/8 + 4/4)/3 ≈ 0.7667.",
        tags: &["canonical", "paper", "unequal-length"],
    },
    GoldenCase {
        id: "jaro/window/includes-transposition",
        descriptor: Jaro::DESCRIPTOR,
        input: (b"abcd", b"bacd"),
        // max_len = 4, window = 4/2 - 1 = 1. `a` (pos 0) finds `a` at
        // b-pos 1 (within window). All four match; matched b positions in
        // a-order are 1, 0, 2, 3, so the matched sequences abcd vs abcd
        // disagree at positions 0 and 1 → 1 transposition.
        // similarity = (4/4 + 4/4 + 3/4)/3 = 11/12.
        expected: FloatExpectation::Absolute {
            value: 11.0_f64 / 12.0_f64,
            tolerance: 1e-12,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Adjacent transposition captured by a window of 1. Illustrates that the transposition count is derived from post-matching order, not directly from the input.",
        tags: &["window", "transposition", "derivation"],
    },
    GoldenCase {
        id: "jaro/window/excludes-otherwise-matching",
        descriptor: Jaro::DESCRIPTOR,
        // max_len = 4, window = 1. The pair (a[0]='a', b[3]='a') is
        // outside the window; only b's 'b' (at index 1) and its 'x'
        // (at index 2) fall within reach of a's 'b' (at index 1) and 'x'
        // (at index 3) respectively. m = 2, t = 0.
        // similarity = (2/4 + 2/4 + 2/2)/3 = 2/3.
        input: (b"abcx", b"cbxa"),
        expected: FloatExpectation::Absolute {
            value: 2.0_f64 / 3.0_f64,
            tolerance: 1e-12,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "The trailing 'a' in b is outside the matching window from a[0]='a' and does not contribute. Guards against a bug that would silently widen the window.",
        tags: &["window", "boundary", "derivation"],
    },
];

/// Golden cases for [`JaroWinkler::classic`] — Winkler 1990.
pub const GOLDEN_JW_CLASSIC: &[BytesCase] = &[
    GoldenCase {
        id: "jw-classic/canonical/martha-marhta",
        descriptor: JaroWinkler::CLASSIC_DESCRIPTOR,
        input: (b"MARTHA", b"MARHTA"),
        // Jaro = 17/18; prefix = 3 (MAR); boost = 3 * 0.1 * (1 - 17/18) = 1/60.
        // jw = 17/18 + 1/60 = 170/180 + 3/180 = 173/180 ≈ 0.96111.
        expected: FloatExpectation::Absolute {
            value: 173.0_f64 / 180.0_f64,
            tolerance: 1e-4,
        },
        source: WINKLER_1990,
        notes: "Winkler's headline example: Jaro ≈ 0.9444, prefix length 3, JW ≈ 0.9611.",
        tags: &["canonical", "paper"],
    },
    GoldenCase {
        id: "jw-classic/canonical/dwayne-duane",
        descriptor: JaroWinkler::CLASSIC_DESCRIPTOR,
        input: (b"DWAYNE", b"DUANE"),
        // Jaro ≈ 0.822; prefix = 1 (D); boost = 1 * 0.1 * (1 - 0.822) ≈ 0.0178.
        // jw ≈ 0.840. Winkler's paper commits to three decimals.
        expected: FloatExpectation::Absolute {
            value: 0.840,
            tolerance: 1e-3,
        },
        source: WINKLER_1990,
        notes: "Winkler's running example: Jaro 0.822 + 0.018 boost = 0.840.",
        tags: &["canonical", "paper", "single-char-prefix"],
    },
    GoldenCase {
        id: "jw-classic/canonical/dixon-dicksonx",
        descriptor: JaroWinkler::CLASSIC_DESCRIPTOR,
        input: (b"DIXON", b"DICKSONX"),
        // Jaro ≈ 0.7667; prefix = 2 (DI); boost = 2 * 0.1 * (1 - 0.7667).
        // jw ≈ 0.8133.
        expected: FloatExpectation::Absolute {
            value: 0.813,
            tolerance: 1e-3,
        },
        source: WINKLER_1990,
        notes: "Winkler (1990): Jaro 0.767 with a two-character common prefix boosts to 0.813.",
        tags: &["canonical", "paper", "two-char-prefix"],
    },
    GoldenCase {
        id: "jw-classic/structure/long-common-prefix",
        descriptor: JaroWinkler::CLASSIC_DESCRIPTOR,
        input: (b"abcdefx", b"abcdefy"),
        // Jaro = (6/7 + 6/7 + 1)/3 = 19/21 ≈ 0.9048.
        // Common prefix = 6, capped at 4. Boost = 4 * 0.1 * (1 - 19/21) = 0.8/21.
        // jw = 19/21 + 0.8/21 ≈ 0.9429. This case is chosen to demonstrate
        // that the boost differentiates JW from Jaro significantly on
        // inputs sharing a long prefix.
        expected: FloatExpectation::Absolute {
            value: 19.0_f64 / 21.0_f64 + 4.0 * 0.1 * (1.0 - 19.0_f64 / 21.0_f64),
            tolerance: 1e-12,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Long common prefix (six chars, capped at the prefix limit of four) demonstrates JW's characteristic upward departure from Jaro.",
        tags: &["structure", "prefix", "derivation"],
    },
    GoldenCase {
        id: "jw-classic/derivation/three-char-prefix",
        descriptor: JaroWinkler::CLASSIC_DESCRIPTOR,
        input: (b"abcde", b"abcxe"),
        // len_a = len_b = 5, window = 5/2 - 1 = 1.
        // Matches: a(0)=b(0), b(1)=b(1), c(2)=b(2), e(4)=b(4). d(3) has no
        // match. m = 4, t = 0. Jaro = (4/5 + 4/5 + 4/4)/3 = 13/15.
        // Prefix = 3 (abc). Boost = 3 * 0.1 * (1 - 13/15).
        // jw = 13/15 + 3 * 0.1 * (1 - 13/15) = 13.6/15 ≈ 0.90667.
        expected: FloatExpectation::Absolute {
            value: 13.0_f64 / 15.0_f64 + 3.0 * 0.1 * (1.0 - 13.0_f64 / 15.0_f64),
            tolerance: 1e-12,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Three-character common prefix drives a boost that lifts a Jaro of 13/15 up to about 0.907. Exercises the boost with a non-trivial prefix shorter than the cap.",
        tags: &["structure", "prefix-3", "derivation"],
    },
];

/// Golden cases for [`JaroWinkler::with_threshold`] — Winkler's later
/// threshold-gated modification.
pub const GOLDEN_JW_THRESHOLD: &[BytesCase] = &[
    GoldenCase {
        id: "jw-threshold/below-threshold-equals-jaro",
        descriptor: JaroWinkler::WITH_THRESHOLD_DESCRIPTOR,
        // "abc" vs "xyz" has Jaro similarity 0.0, which is well below the
        // 0.7 threshold, so no boost is applied and jw equals jaro.
        input: (b"abc", b"xyz"),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Jaro score 0.0 is below the 0.7 threshold; the boost is skipped and jw equals jaro bit-exactly.",
        tags: &["threshold", "gate-inactive", "exact-bits"],
    },
    GoldenCase {
        id: "jw-threshold/above-threshold-matches-classic",
        descriptor: JaroWinkler::WITH_THRESHOLD_DESCRIPTOR,
        input: (b"MARTHA", b"MARHTA"),
        // Jaro ≈ 0.944 >= 0.7 threshold; the boost is applied and yields
        // the same 173/180 as the classic variant.
        expected: FloatExpectation::Absolute {
            value: 173.0_f64 / 180.0_f64,
            tolerance: 1e-4,
        },
        source: GoldenSource::IndependentlyDerived,
        notes: "Jaro exceeds 0.7 so the boost fires; result matches JaroWinkler::classic on this input.",
        tags: &["threshold", "gate-active"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jaro::jaro::Jaro;
    use crate::jaro::jaro_winkler::JaroWinkler;
    use stringcheese_core::SimilarityMetric;

    /// Runs a single case against the appropriate algorithm and returns the
    /// observed value alongside a match flag. Split out for clarity of the
    /// per-suite driver tests below.
    fn run_case_jaro(case: &BytesCase) -> (f64, bool) {
        let alg = Jaro;
        let (l, r) = case.input;
        let observed = alg.similarity(l, r).into_inner();
        (observed, case.expected.matches(observed))
    }

    fn run_case_jw(alg: JaroWinkler, case: &BytesCase) -> (f64, bool) {
        let (l, r) = case.input;
        let observed = alg.similarity(l, r).into_inner();
        (observed, case.expected.matches(observed))
    }

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for c in GOLDEN_JARO {
            assert_eq!(
                c.descriptor,
                Jaro::DESCRIPTOR,
                "golden case {} references the wrong descriptor",
                c.id
            );
        }
        for c in GOLDEN_JW_CLASSIC {
            assert_eq!(
                c.descriptor,
                JaroWinkler::CLASSIC_DESCRIPTOR,
                "golden case {} references the wrong descriptor",
                c.id
            );
        }
        for c in GOLDEN_JW_THRESHOLD {
            assert_eq!(
                c.descriptor,
                JaroWinkler::WITH_THRESHOLD_DESCRIPTOR,
                "golden case {} references the wrong descriptor",
                c.id
            );
        }
    }

    #[test]
    fn every_jaro_case_matches_algorithm() {
        for case in GOLDEN_JARO {
            let (observed, ok) = run_case_jaro(case);
            assert!(
                ok,
                "golden case {} disagreed: expected {:?}, observed {observed}",
                case.id, case.expected
            );
        }
    }

    #[test]
    fn every_jw_classic_case_matches_algorithm() {
        let alg = JaroWinkler::classic();
        for case in GOLDEN_JW_CLASSIC {
            let (observed, ok) = run_case_jw(alg, case);
            assert!(
                ok,
                "golden case {} disagreed: expected {:?}, observed {observed}",
                case.id, case.expected
            );
        }
    }

    #[test]
    fn every_jw_threshold_case_matches_algorithm() {
        let alg = JaroWinkler::with_threshold();
        for case in GOLDEN_JW_THRESHOLD {
            let (observed, ok) = run_case_jw(alg, case);
            assert!(
                ok,
                "golden case {} disagreed: expected {:?}, observed {observed}",
                case.id, case.expected
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // The spec asks for at least twelve golden cases across the crate.
        assert!(
            GOLDEN_JARO.len() + GOLDEN_JW_CLASSIC.len() + GOLDEN_JW_THRESHOLD.len() >= 12,
            "expected at least 12 golden cases across the crate"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let ids: alloc::vec::Vec<&str> = GOLDEN_JARO
            .iter()
            .map(|c| c.id)
            .chain(GOLDEN_JW_CLASSIC.iter().map(|c| c.id))
            .chain(GOLDEN_JW_THRESHOLD.iter().map(|c| c.id))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }
}
