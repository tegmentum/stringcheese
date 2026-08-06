//! Canonical Levenshtein golden cases wired to the `stringcheese-corpus` schema.
//!
//! Each entry is a [`GoldenCase`] whose `descriptor` field equals
//! [`Levenshtein::DESCRIPTOR`] and whose `expected` field is a
//! [`Distance<u32>`]. Cases whose input alphabet is `u8` live in
//! [`GOLDEN_BYTES`]; cases that require `char`-level indexing (Unicode
//! scalars) live in [`GOLDEN_CHARS`] because their two views of the same
//! string would have different lengths under UTF-8.
//!
//! Because this module is `#[cfg(test)]`, its cases are compiled only into
//! the crate's test binaries — `stringcheese-corpus` is declared as a
//! dev-dependency for exactly that reason.

use stringcheese_core::Distance;
use stringcheese_corpus::{GoldenCase, GoldenSource};

use crate::levenshtein::algorithm::Levenshtein;

/// A byte-slice input pair, as stored in a Levenshtein golden case.
pub type BytesInput = (&'static [u8], &'static [u8]);

/// A `char`-slice input pair, as stored in a Levenshtein golden case.
pub type CharsInput = (&'static [char], &'static [char]);

/// The concrete `GoldenCase` type for byte-slice Levenshtein cases.
pub type BytesCase = GoldenCase<BytesInput, Distance<u32>>;

/// The concrete `GoldenCase` type for `char`-slice Levenshtein cases.
pub type CharsCase = GoldenCase<CharsInput, Distance<u32>>;

// The `char`-level Unicode inputs need to be named `const`s so their
// addresses live long enough for the `&'static [char]` references stored in
// `GOLDEN_CHARS` below.
const CAFE_ACCENT: &[char] = &['c', 'a', 'f', 'é'];
const CAFE_PLAIN: &[char] = &['c', 'a', 'f', 'e'];
const NAIVE_ACCENT: &[char] = &['n', 'a', 'ï', 'v', 'e'];
const NAIVE_PLAIN: &[char] = &['n', 'a', 'i', 'v', 'e'];

/// Golden cases whose inputs are byte slices.
pub const GOLDEN_BYTES: &[BytesCase] = &[
    GoldenCase {
        id: "levenshtein/basic/empty-empty",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"", b""),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Both inputs empty; distance is zero by the boundary conditions of the DP.",
        tags: &["basic", "empty", "identity"],
    },
    GoldenCase {
        id: "levenshtein/basic/left-empty",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"", b"hello"),
        expected: Distance::new(5),
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty vs. non-empty: distance equals the non-empty side's length.",
        tags: &["basic", "empty"],
    },
    GoldenCase {
        id: "levenshtein/basic/right-empty",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"hello", b""),
        expected: Distance::new(5),
        source: GoldenSource::IndependentlyDerived,
        notes: "Non-empty vs. empty; symmetric to the left-empty case.",
        tags: &["basic", "empty"],
    },
    GoldenCase {
        id: "levenshtein/basic/identical",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"abcdef", b"abcdef"),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two equal inputs must have distance zero.",
        tags: &["basic", "identity"],
    },
    GoldenCase {
        id: "levenshtein/canonical/kitten-sitting",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"kitten", b"sitting"),
        expected: Distance::new(3),
        source: GoldenSource::PublishedPaper {
            citation: "R. A. Wagner and M. J. Fischer, \"The string-to-string correction problem\", Journal of the ACM 21(1), 1974, pp. 168-173.",
        },
        notes: "Wagner-Fischer's canonical worked example: substitute k->s, substitute e->i, insert g.",
        tags: &["canonical", "substitution", "insertion"],
    },
    GoldenCase {
        id: "levenshtein/basic/single-substitution",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"cat", b"cot"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single substitution has distance one under unit-cost Levenshtein.",
        tags: &["basic", "substitution"],
    },
    GoldenCase {
        id: "levenshtein/basic/single-insertion",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"cat", b"cats"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single trailing insertion has distance one.",
        tags: &["basic", "insertion"],
    },
    GoldenCase {
        id: "levenshtein/basic/single-deletion",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"cats", b"cat"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single trailing deletion has distance one; symmetric to single-insertion.",
        tags: &["basic", "deletion"],
    },
    GoldenCase {
        id: "levenshtein/basic/transposition-costs-two",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"ab", b"ba"),
        expected: Distance::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "Unit-cost Levenshtein scores a transposition as two operations; Damerau-Levenshtein would score it as one. This case pins that distinction.",
        tags: &["basic", "transposition", "variant-boundary"],
    },
    GoldenCase {
        id: "levenshtein/structure/long-common-prefix",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"abcdefghijA", b"abcdefghijB"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A long shared prefix collapses to a single trailing substitution.",
        tags: &["structure", "prefix"],
    },
    GoldenCase {
        id: "levenshtein/structure/long-common-suffix",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"Axyz-common-tail", b"Bxyz-common-tail"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A long shared suffix collapses to a single leading substitution.",
        tags: &["structure", "suffix"],
    },
    GoldenCase {
        id: "levenshtein/structure/all-different",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"abc", b"xyz"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "Same length with no matches: distance equals the length (three substitutions).",
        tags: &["structure", "substitution", "worst-case"],
    },
    GoldenCase {
        id: "levenshtein/canonical/saturday-sunday",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (b"Saturday", b"Sunday"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "Textbook example: delete a, delete t, substitute r->n. Independently derived by tracing the DP.",
        tags: &["canonical", "deletion", "substitution"],
    },
];

/// Golden cases whose inputs are `char` slices — inputs whose byte- and
/// character-level lengths differ, where the caller has to have chosen the
/// Unicode-scalar representation up front.
pub const GOLDEN_CHARS: &[CharsCase] = &[
    GoldenCase {
        id: "levenshtein/unicode/cafe-accent",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (CAFE_ACCENT, CAFE_PLAIN),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "Diacritic difference at char-slice granularity is one substitution; at byte granularity the answer would be two (é is two bytes in UTF-8), which is exactly the sort of silent representation choice StringCheese refuses to make.",
        tags: &["unicode", "substitution", "representation"],
    },
    GoldenCase {
        id: "levenshtein/unicode/naive-accent",
        descriptor: Levenshtein::DESCRIPTOR,
        input: (NAIVE_ACCENT, NAIVE_PLAIN),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "Second Unicode-scalar case to guard against a single-example fluke.",
        tags: &["unicode", "substitution"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levenshtein::algorithm::Levenshtein;
    use stringcheese_core::DistanceMetric;

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for case in GOLDEN_BYTES {
            assert_eq!(
                case.descriptor,
                Levenshtein::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
        for case in GOLDEN_CHARS {
            assert_eq!(
                case.descriptor,
                Levenshtein::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
    }

    #[test]
    fn every_byte_case_matches_the_algorithm() {
        let alg = Levenshtein;
        for case in GOLDEN_BYTES {
            let (left, right) = case.input;
            let observed = alg.distance(left, right);
            assert_eq!(
                observed,
                case.expected,
                "golden case {} disagreed: expected {expected}, observed {observed}",
                case.id,
                expected = case.expected,
            );
        }
    }

    #[test]
    fn every_char_case_matches_the_algorithm() {
        let alg = Levenshtein;
        for case in GOLDEN_CHARS {
            let (left, right) = case.input;
            let observed = alg.distance(left, right);
            assert_eq!(
                observed,
                case.expected,
                "golden case {} disagreed: expected {expected}, observed {observed}",
                case.id,
                expected = case.expected,
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // The spec asks for at least twelve golden cases across the crate.
        // Extra headroom guards against a future refactor accidentally
        // dropping one below the threshold.
        assert!(
            GOLDEN_BYTES.len() + GOLDEN_CHARS.len() >= 12,
            "expected at least 12 golden cases across the crate"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        // Duplicated identifiers would silently hide test failures — the
        // second case's expectation would only report the first case's id in
        // a fixture-driven runner.
        let ids: alloc::vec::Vec<&str> = GOLDEN_BYTES
            .iter()
            .map(|c| c.id)
            .chain(GOLDEN_CHARS.iter().map(|c| c.id))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }
}
