//! Canonical LCS golden cases wired to the `stringcheese-corpus` schema.
//!
//! LCS is packaged in this crate as two closely related algorithms with
//! distinct descriptors: [`Lcs`] reports the LCS *length* as a
//! [`Score<u32>`](stringcheese_core::Score); [`LcsDistance`] reports the
//! derived edit metric as a [`Distance<u32>`](stringcheese_core::Distance).
//! The two share an [`AlgorithmFamily`] but have distinct [`VariantId`]s,
//! and the golden fixtures below are split into two arrays so that a
//! length case cannot silently be validated by the distance algorithm
//! (or vice versa).
//!
//! Because this module is `#[cfg(test)]`, its cases are compiled only
//! into the crate's test binaries — `stringcheese-corpus` is declared as a
//! dev-dependency for exactly that reason.
//!
//! [`AlgorithmFamily`]: stringcheese_core::AlgorithmFamily
//! [`VariantId`]: stringcheese_core::VariantId

use stringcheese_core::{Distance, Score};
use stringcheese_corpus::{GoldenCase, GoldenSource};

use crate::lcs::algorithm::{Lcs, LcsDistance};

/// A byte-slice input pair, as stored in an LCS golden case.
pub type BytesInput = (&'static [u8], &'static [u8]);

/// A `char`-slice input pair, as stored in an LCS golden case.
pub type CharsInput = (&'static [char], &'static [char]);

/// The concrete `GoldenCase` type for LCS-length byte cases.
pub type LengthBytesCase = GoldenCase<BytesInput, Score<u32>>;

/// The concrete `GoldenCase` type for LCS-length `char` cases.
pub type LengthCharsCase = GoldenCase<CharsInput, Score<u32>>;

/// The concrete `GoldenCase` type for LCS-distance byte cases.
pub type DistanceBytesCase = GoldenCase<BytesInput, Distance<u32>>;

// The `char`-level Unicode inputs need to be named `const`s so their
// addresses live long enough for the `&'static [char]` references stored
// in `LENGTH_CHARS_CASES` below.
const CAFE_ACCENT: &[char] = &['c', 'a', 'f', 'é'];
const CAFE_PLAIN: &[char] = &['c', 'a', 'f', 'e'];

/// LCS-length golden cases whose inputs are byte slices.
pub const LENGTH_BYTES_CASES: &[LengthBytesCase] = &[
    GoldenCase {
        id: "lcs/length/empty-empty",
        descriptor: Lcs::DESCRIPTOR,
        input: (b"", b""),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Both inputs empty; LCS length is zero by the boundary conditions of the DP.",
        tags: &["basic", "empty", "length"],
    },
    GoldenCase {
        id: "lcs/length/one-empty",
        descriptor: Lcs::DESCRIPTOR,
        input: (b"", b"hello"),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty vs. non-empty: no symbol is shared, so the LCS length is zero.",
        tags: &["basic", "empty", "length"],
    },
    GoldenCase {
        id: "lcs/length/identical",
        descriptor: Lcs::DESCRIPTOR,
        input: (b"abcdef", b"abcdef"),
        expected: Score::new(6),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two equal inputs have LCS length equal to the input length.",
        tags: &["basic", "identity", "length"],
    },
    GoldenCase {
        id: "lcs/length/disjoint-alphabet",
        descriptor: Lcs::DESCRIPTOR,
        input: (b"abc", b"xyz"),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "No shared symbols, so no common subsequence exists.",
        tags: &["basic", "disjoint", "length"],
    },
    GoldenCase {
        id: "lcs/length/textbook-abcbdab-bdcab",
        descriptor: Lcs::DESCRIPTOR,
        input: (b"ABCBDAB", b"BDCAB"),
        expected: Score::new(4),
        source: GoldenSource::PublishedPaper {
            citation: "T. H. Cormen, C. E. Leiserson, R. L. Rivest, C. Stein, \"Introduction to Algorithms\", 3rd ed., MIT Press, 2009, section 15.4.",
        },
        notes: "Cormen et al.'s worked example: the LCS of \"ABCBDAB\" and \"BDCAB\" is length 4 (e.g. \"BCAB\" or \"BDAB\").",
        tags: &["canonical", "length"],
    },
    GoldenCase {
        id: "lcs/length/textbook-agcat-gac",
        descriptor: Lcs::DESCRIPTOR,
        input: (b"AGCAT", b"GAC"),
        expected: Score::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "Textbook worked example: the LCS of \"AGCAT\" and \"GAC\" has length 2 (e.g. \"GA\", \"AC\", or \"GC\").",
        tags: &["canonical", "length"],
    },
    GoldenCase {
        id: "lcs/length/kitten-sitting",
        descriptor: Lcs::DESCRIPTOR,
        input: (b"kitten", b"sitting"),
        expected: Score::new(4),
        source: GoldenSource::IndependentlyDerived,
        notes: "Derived by hand: the LCS of \"kitten\" and \"sitting\" is \"ittn\" of length 4. Cross-referenced with the Levenshtein distance of 3 via |a|+|b|-2*lcs = 6+7-8 = 5, which does not match Levenshtein's 3 — the intended demonstration that LCS distance and Levenshtein disagree in the presence of substitutions.",
        tags: &["canonical", "length", "cross-reference"],
    },
];

/// LCS-length golden cases whose inputs are `char` slices.
pub const LENGTH_CHARS_CASES: &[LengthCharsCase] = &[GoldenCase {
    id: "lcs/length/unicode/cafe-accent",
    descriptor: Lcs::DESCRIPTOR,
    input: (CAFE_ACCENT, CAFE_PLAIN),
    expected: Score::new(3),
    source: GoldenSource::IndependentlyDerived,
    notes: "Diacritic difference at char-slice granularity: the LCS is \"caf\" of length 3. At byte granularity the answer would differ (é is two bytes in UTF-8), which is exactly the sort of silent representation choice StringCheese refuses to make.",
    tags: &["unicode", "length", "representation"],
}];

/// LCS-distance golden cases whose inputs are byte slices.
pub const DISTANCE_BYTES_CASES: &[DistanceBytesCase] = &[
    GoldenCase {
        id: "lcs/distance/empty-empty",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"", b""),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Both inputs empty; distance is zero by identity.",
        tags: &["basic", "empty", "distance"],
    },
    GoldenCase {
        id: "lcs/distance/one-empty",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"", b"hello"),
        expected: Distance::new(5),
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty vs. non-empty: five insertions bridge the gap. |a|+|b|-2*lcs = 0+5-0 = 5.",
        tags: &["basic", "empty", "distance"],
    },
    GoldenCase {
        id: "lcs/distance/single-insertion",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"abc", b"abcd"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single trailing insertion: LCS = 3, distance = 3+4-6 = 1.",
        tags: &["basic", "insertion", "distance"],
    },
    GoldenCase {
        id: "lcs/distance/single-deletion",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"abcd", b"abc"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single trailing deletion: LCS = 3, distance = 4+3-6 = 1. Symmetric to single-insertion.",
        tags: &["basic", "deletion", "distance"],
    },
    GoldenCase {
        id: "lcs/distance/substitution-costs-two",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"abcd", b"abed"),
        expected: Distance::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "LCS distance forbids substitutions, so replacing a single character costs one delete plus one insert. LCS = 3 (\"abd\"), distance = 4+4-6 = 2. Contrast with Levenshtein, which would return 1 for the same pair; this case pins the difference between the two metrics.",
        tags: &["variant-boundary", "substitution", "distance"],
    },
    GoldenCase {
        id: "lcs/distance/agcat-gac",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"AGCAT", b"GAC"),
        expected: Distance::new(4),
        source: GoldenSource::PublishedPaper {
            citation: "L. Bergroth, H. Hakonen, T. Raita, \"A survey of longest common subsequence algorithms\", Proc. Seventh International Symposium on String Processing Information Retrieval (SPIRE 2000), pp. 39-48.",
        },
        notes: "Derived from the LCS-length worked example: LCS = 2, distance = 5+3-4 = 4.",
        tags: &["canonical", "distance"],
    },
    GoldenCase {
        id: "lcs/distance/identical",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"abcdef", b"abcdef"),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two equal inputs must have distance zero. Identity of indiscernibles.",
        tags: &["basic", "identity", "distance"],
    },
    GoldenCase {
        id: "lcs/distance/disjoint-alphabet",
        descriptor: LcsDistance::DESCRIPTOR,
        input: (b"abc", b"xyz"),
        expected: Distance::new(6),
        source: GoldenSource::IndependentlyDerived,
        notes: "Disjoint alphabet: LCS = 0, distance = 3+3-0 = 6. Every symbol must be deleted and re-inserted.",
        tags: &["structure", "worst-case", "distance"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcs::algorithm::{Lcs, LcsDistance};

    #[test]
    fn every_length_case_uses_the_correct_descriptor() {
        for case in LENGTH_BYTES_CASES {
            assert_eq!(
                case.descriptor,
                Lcs::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
        for case in LENGTH_CHARS_CASES {
            assert_eq!(
                case.descriptor,
                Lcs::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
    }

    #[test]
    fn every_distance_case_uses_the_correct_descriptor() {
        for case in DISTANCE_BYTES_CASES {
            assert_eq!(
                case.descriptor,
                LcsDistance::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
    }

    #[test]
    fn every_length_bytes_case_matches_the_algorithm() {
        let alg = Lcs;
        for case in LENGTH_BYTES_CASES {
            let (left, right) = case.input;
            let observed = alg.length(left, right);
            assert_eq!(observed, case.expected, "golden case {} disagreed", case.id);
        }
    }

    #[test]
    fn every_length_chars_case_matches_the_algorithm() {
        let alg = Lcs;
        for case in LENGTH_CHARS_CASES {
            let (left, right) = case.input;
            let observed = alg.length(left, right);
            assert_eq!(observed, case.expected, "golden case {} disagreed", case.id);
        }
    }

    #[test]
    fn every_distance_bytes_case_matches_the_algorithm() {
        let alg = LcsDistance;
        for case in DISTANCE_BYTES_CASES {
            let (left, right) = case.input;
            let observed = alg.distance(left, right);
            assert_eq!(observed, case.expected, "golden case {} disagreed", case.id);
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // The task specification asks for at least ten golden cases across
        // the crate, spread across the LCS-length and LCS-distance
        // fixtures.
        let total =
            LENGTH_BYTES_CASES.len() + LENGTH_CHARS_CASES.len() + DISTANCE_BYTES_CASES.len();
        assert!(
            total >= 10,
            "expected at least 10 golden cases across the crate, found {total}"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        // Duplicated identifiers would silently hide test failures — the
        // second case's expectation would only report the first case's id
        // in a fixture-driven runner.
        let ids: alloc::vec::Vec<&str> = LENGTH_BYTES_CASES
            .iter()
            .map(|c| c.id)
            .chain(LENGTH_CHARS_CASES.iter().map(|c| c.id))
            .chain(DISTANCE_BYTES_CASES.iter().map(|c| c.id))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }
}
