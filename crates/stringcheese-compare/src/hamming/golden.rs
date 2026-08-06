//! Canonical Hamming golden cases wired to the `stringcheese-corpus` schema.
//!
//! Each entry is a [`GoldenCase`] whose `descriptor` field equals
//! [`Hamming::DESCRIPTOR`] and whose `expected` field is a
//! [`Distance<u32>`] or a [`BoundedDistance<u32>`]. Cases whose input
//! alphabet is `u8` live in [`GOLDEN_BYTES`]; cases that require `char`-level
//! indexing (Unicode scalars) live in [`GOLDEN_CHARS`] because their two
//! views of the same string would have different lengths under UTF-8. Cases
//! that exercise the cutoff-aware entry point live in [`GOLDEN_BOUNDED`]
//! because the expected type is different.
//!
//! Because this module is `#[cfg(test)]`, its cases are compiled only into
//! the crate's test binaries — `stringcheese-corpus` is declared as a
//! dev-dependency for exactly that reason.

use stringcheese_core::{BoundedDistance, Distance};
use stringcheese_corpus::{GoldenCase, GoldenSource};

use crate::hamming::algorithm::Hamming;

/// A byte-slice input pair, as stored in a Hamming golden case.
pub type BytesInput = (&'static [u8], &'static [u8]);

/// A `char`-slice input pair, as stored in a Hamming golden case.
pub type CharsInput = (&'static [char], &'static [char]);

/// A byte-slice-plus-cutoff input triple, as stored in a bounded golden case.
pub type BoundedBytesInput = (&'static [u8], &'static [u8], u32);

/// The concrete `GoldenCase` type for byte-slice Hamming cases.
pub type BytesCase = GoldenCase<BytesInput, Distance<u32>>;

/// The concrete `GoldenCase` type for `char`-slice Hamming cases.
pub type CharsCase = GoldenCase<CharsInput, Distance<u32>>;

/// The concrete `GoldenCase` type for cutoff-aware Hamming cases.
pub type BoundedCase = GoldenCase<BoundedBytesInput, BoundedDistance<u32>>;

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
        id: "hamming/basic/empty-empty",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"", b""),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two empty inputs are equal-length (zero) and share zero mismatching positions.",
        tags: &["basic", "empty", "identity"],
    },
    GoldenCase {
        id: "hamming/basic/identical-single-char",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"a", b"a"),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single identical symbol on both sides is distance zero.",
        tags: &["basic", "identity"],
    },
    GoldenCase {
        id: "hamming/basic/identical-multi-char",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"abcdef", b"abcdef"),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two equal multi-character inputs must have distance zero.",
        tags: &["basic", "identity"],
    },
    GoldenCase {
        id: "hamming/positional/single-mismatch-at-start",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"Xbcdef", b"abcdef"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "One mismatch at position 0; verifies the first-position edge is counted.",
        tags: &["positional", "single-mismatch"],
    },
    GoldenCase {
        id: "hamming/positional/single-mismatch-in-middle",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"abcXef", b"abcdef"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "One mismatch at an interior position; verifies interior positions are counted.",
        tags: &["positional", "single-mismatch"],
    },
    GoldenCase {
        id: "hamming/positional/single-mismatch-at-end",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"abcdeX", b"abcdef"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "One mismatch at the last position; verifies the tail-position edge is counted.",
        tags: &["positional", "single-mismatch"],
    },
    GoldenCase {
        id: "hamming/structure/all-different",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"abc", b"xyz"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "Same length with no matches: distance equals the length. This is the per-length worst case.",
        tags: &["structure", "worst-case"],
    },
    GoldenCase {
        id: "hamming/structure/long-common-prefix-tail-mismatch",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"abcdefghijA", b"abcdefghijB"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A long shared prefix collapses to a single trailing mismatch.",
        tags: &["structure", "prefix"],
    },
    GoldenCase {
        id: "hamming/canonical/karolin-kathrin",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"karolin", b"kathrin"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "A widely quoted textbook example: positions 2 (r vs t), 3 (o vs h), and 4 (l vs r) differ. Independently derived by hand-counting rather than lifted from a specific reference.",
        tags: &["canonical", "textbook"],
    },
    GoldenCase {
        id: "hamming/canonical/karolin-kerstin",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"karolin", b"kerstin"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "Second textbook pair: positions 1 (a vs e), 3 (o vs s), and 4 (l vs t) differ.",
        tags: &["canonical", "textbook"],
    },
    GoldenCase {
        id: "hamming/binary/1011101-vs-1001001",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"1011101", b"1001001"),
        expected: Distance::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "Binary example inspired by Hamming's 1950 paper on error-detecting codes. Positions 2 and 4 differ. Cited as IndependentlyDerived because the paper's specific numeric examples are not reproduced verbatim here.",
        tags: &["binary", "hamming-1950"],
    },
];

/// Golden cases whose inputs are `char` slices — inputs whose byte- and
/// character-level lengths differ, where the caller has to have chosen the
/// Unicode-scalar representation up front.
pub const GOLDEN_CHARS: &[CharsCase] = &[
    GoldenCase {
        id: "hamming/unicode/cafe-accent",
        descriptor: Hamming::DESCRIPTOR,
        input: (CAFE_ACCENT, CAFE_PLAIN),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "Diacritic difference at char-slice granularity is one mismatch. At byte granularity the two inputs would have different lengths (é is two bytes in UTF-8) and the trait method would panic — exactly the sort of silent representation choice StringCheese refuses to make.",
        tags: &["unicode", "representation"],
    },
    GoldenCase {
        id: "hamming/unicode/naive-accent",
        descriptor: Hamming::DESCRIPTOR,
        input: (NAIVE_ACCENT, NAIVE_PLAIN),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "Second Unicode-scalar case to guard against a single-example fluke.",
        tags: &["unicode"],
    },
];

/// Golden cases that exercise the cutoff-aware entry point.
pub const GOLDEN_BOUNDED: &[BoundedCase] = &[
    GoldenCase {
        id: "hamming/bounded/within-at-cutoff",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"karolin", b"kathrin", 3),
        expected: BoundedDistance::Within(Distance::new(3)),
        source: GoldenSource::IndependentlyDerived,
        notes: "Distance equals the cutoff — the cutoff is inclusive, so the result must be Within, not Exceeded.",
        tags: &["bounded", "boundary"],
    },
    GoldenCase {
        id: "hamming/bounded/exceeded-below-cutoff",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"karolin", b"kathrin", 2),
        expected: BoundedDistance::Exceeded { cutoff: 2 },
        source: GoldenSource::IndependentlyDerived,
        notes: "Distance = 3, cutoff = 2 -> Exceeded { cutoff: 2 }. Verifies early termination at the third mismatch and that the cutoff value is preserved.",
        tags: &["bounded", "exceeded"],
    },
    GoldenCase {
        id: "hamming/bounded/within-well-under-cutoff",
        descriptor: Hamming::DESCRIPTOR,
        input: (b"abcdef", b"abcdef", 5),
        expected: BoundedDistance::Within(Distance::new(0)),
        source: GoldenSource::IndependentlyDerived,
        notes: "Zero distance well under the cutoff returns Within(0). Guards against sign-off-by-one bugs at the zero end.",
        tags: &["bounded", "identity"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hamming::algorithm::Hamming;
    use stringcheese_core::{BoundedDistanceMetric, DistanceMetric};

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for case in GOLDEN_BYTES {
            assert_eq!(
                case.descriptor,
                Hamming::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
        for case in GOLDEN_CHARS {
            assert_eq!(
                case.descriptor,
                Hamming::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
        for case in GOLDEN_BOUNDED {
            assert_eq!(
                case.descriptor,
                Hamming::DESCRIPTOR,
                "golden case {} references the wrong algorithm descriptor",
                case.id
            );
        }
    }

    #[test]
    fn every_byte_case_matches_the_algorithm() {
        let alg = Hamming;
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
        let alg = Hamming;
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
    fn every_bounded_case_matches_the_algorithm() {
        let alg = Hamming;
        for case in GOLDEN_BOUNDED {
            let (left, right, cutoff) = case.input;
            let observed = alg.distance_within(left, right, cutoff);
            assert_eq!(
                observed, case.expected,
                "bounded golden case {} disagreed",
                case.id,
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // The spec asks for at least ten golden cases across the crate.
        // Extra headroom guards against a future refactor accidentally
        // dropping one below the threshold.
        let total = GOLDEN_BYTES.len() + GOLDEN_CHARS.len() + GOLDEN_BOUNDED.len();
        assert!(
            total >= 10,
            "expected at least 10 golden cases across the crate, got {total}"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        // Duplicated identifiers would silently hide test failures — the
        // second case's expectation would only report the first case's id in
        // a fixture-driven runner.
        let ids: std::vec::Vec<&str> = GOLDEN_BYTES
            .iter()
            .map(|c| c.id)
            .chain(GOLDEN_CHARS.iter().map(|c| c.id))
            .chain(GOLDEN_BOUNDED.iter().map(|c| c.id))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }
}
