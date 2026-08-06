//! Canonical golden cases for both algorithm variants in the crate.
//!
//! Each case's `descriptor` field pins the specific variant it validates.
//! A case that references [`Osa::DESCRIPTOR`] and a case that references
//! [`Damerau::DESCRIPTOR`] are validated against different implementations,
//! and mixing them up is a schema error rather than a silent mismatch —
//! that is the whole point of the variant registry in this crate.
//!
//! The distinguishing example `"ca"` vs `"abc"` appears in **both** golden
//! sets with different expected values (`3` under OSA, `2` under Damerau).
//! Any implementation that computes the wrong answer for either variant
//! fails at least one of the two cases; a fixture runner that dispatches on
//! `descriptor.family` cannot silently validate the OSA case against the
//! Damerau algorithm or vice versa.

use stringcheese_core::Distance;
use stringcheese_corpus::{GoldenCase, GoldenSource};

use crate::damerau::algorithm::{Damerau, Osa};

/// A byte-slice input pair, as stored in a golden case.
pub type BytesInput = (&'static [u8], &'static [u8]);

/// A `char`-slice input pair, as stored in a golden case.
pub type CharsInput = (&'static [char], &'static [char]);

/// The concrete `GoldenCase` type for byte-slice cases.
pub type BytesCase = GoldenCase<BytesInput, Distance<u32>>;

/// The concrete `GoldenCase` type for `char`-slice cases.
pub type CharsCase = GoldenCase<CharsInput, Distance<u32>>;

// Named `const`s for `char`-slice inputs so the `&'static [char]`
// references stored in [`GOLDEN_OSA_CHARS`] and [`GOLDEN_DAMERAU_CHARS`]
// have addresses that live long enough.
const CAFE_ACCENT: &[char] = &['c', 'a', 'f', 'é'];
const CAFE_PLAIN: &[char] = &['c', 'a', 'f', 'e'];

/// OSA golden cases whose inputs are byte slices.
pub const GOLDEN_OSA_BYTES: &[BytesCase] = &[
    GoldenCase {
        id: "osa/basic/empty-empty",
        descriptor: Osa::DESCRIPTOR,
        input: (b"", b""),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Both inputs empty; distance is zero by the boundary conditions of the DP.",
        tags: &["basic", "empty", "identity"],
    },
    GoldenCase {
        id: "osa/basic/identical",
        descriptor: Osa::DESCRIPTOR,
        input: (b"abcdef", b"abcdef"),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two equal inputs must have distance zero.",
        tags: &["basic", "identity"],
    },
    GoldenCase {
        id: "osa/basic/single-insertion",
        descriptor: Osa::DESCRIPTOR,
        input: (b"cat", b"cats"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single trailing insertion has distance one.",
        tags: &["basic", "insertion"],
    },
    GoldenCase {
        id: "osa/basic/single-deletion",
        descriptor: Osa::DESCRIPTOR,
        input: (b"cats", b"cat"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single trailing deletion has distance one.",
        tags: &["basic", "deletion"],
    },
    GoldenCase {
        id: "osa/basic/single-substitution",
        descriptor: Osa::DESCRIPTOR,
        input: (b"cat", b"cot"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single substitution has distance one.",
        tags: &["basic", "substitution"],
    },
    GoldenCase {
        id: "osa/basic/adjacent-transposition",
        descriptor: Osa::DESCRIPTOR,
        input: (b"ab", b"ba"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "An adjacent transposition has distance one under OSA (and under full Damerau); plain Levenshtein would score it as two.",
        tags: &["basic", "transposition"],
    },
    GoldenCase {
        id: "osa/canonical/kitten-sitting",
        descriptor: Osa::DESCRIPTOR,
        input: (b"kitten", b"sitting"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "Same score as plain Levenshtein — no adjacent transposition appears in the optimal edit sequence.",
        tags: &["canonical", "substitution", "insertion"],
    },
    GoldenCase {
        id: "osa/variant-boundary/ca-abc",
        descriptor: Osa::DESCRIPTOR,
        input: (b"ca", b"abc"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "THE distinguishing example versus full Damerau. Under OSA, the \"no substring edited twice\" restriction forbids reusing the transposed pair in a subsequent insertion, so the optimal path costs 3. Full Damerau scores this pair as 2 (see damerau/variant-boundary/ca-abc).",
        tags: &["variant-boundary", "transposition", "restriction"],
    },
    GoldenCase {
        id: "osa/structure/multi-transposition",
        descriptor: Osa::DESCRIPTOR,
        input: (b"abcd", b"badc"),
        expected: Distance::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two disjoint adjacent transpositions: swap (a,b) and swap (c,d). The two swaps involve disjoint substrings, so the OSA restriction does not fire; both count as one operation each.",
        tags: &["structure", "transposition"],
    },
    GoldenCase {
        id: "osa/structure/long-common-prefix",
        descriptor: Osa::DESCRIPTOR,
        input: (b"abcdefghijA", b"abcdefghijB"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A long shared prefix collapses to a single trailing substitution.",
        tags: &["structure", "prefix"],
    },
    GoldenCase {
        id: "osa/structure/all-different",
        descriptor: Osa::DESCRIPTOR,
        input: (b"abc", b"xyz"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "Same length with no matches: distance equals the length (three substitutions).",
        tags: &["structure", "worst-case"],
    },
    GoldenCase {
        id: "osa/basic/left-empty",
        descriptor: Osa::DESCRIPTOR,
        input: (b"", b"hello"),
        expected: Distance::new(5),
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty vs. non-empty: distance equals the non-empty side's length.",
        tags: &["basic", "empty"],
    },
];

/// OSA golden cases whose inputs are `char` slices.
pub const GOLDEN_OSA_CHARS: &[CharsCase] = &[GoldenCase {
    id: "osa/unicode/cafe-accent",
    descriptor: Osa::DESCRIPTOR,
    input: (CAFE_ACCENT, CAFE_PLAIN),
    expected: Distance::new(1),
    source: GoldenSource::IndependentlyDerived,
    notes: "Diacritic difference at char-slice granularity is one substitution. At byte granularity the answer would be two — exactly the kind of representation choice StringCheese refuses to make silently.",
    tags: &["unicode", "substitution", "representation"],
}];

/// Full-Damerau golden cases whose inputs are byte slices.
pub const GOLDEN_DAMERAU_BYTES: &[BytesCase] = &[
    GoldenCase {
        id: "damerau/basic/empty-empty",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"", b""),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Both inputs empty; distance is zero.",
        tags: &["basic", "empty", "identity"],
    },
    GoldenCase {
        id: "damerau/basic/identical",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"abcdef", b"abcdef"),
        expected: Distance::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two equal inputs must have distance zero.",
        tags: &["basic", "identity"],
    },
    GoldenCase {
        id: "damerau/basic/single-substitution",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"cat", b"cot"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single substitution has distance one.",
        tags: &["basic", "substitution"],
    },
    GoldenCase {
        id: "damerau/basic/single-insertion",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"cat", b"cats"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "A single trailing insertion has distance one.",
        tags: &["basic", "insertion"],
    },
    GoldenCase {
        id: "damerau/basic/adjacent-transposition",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"ab", b"ba"),
        expected: Distance::new(1),
        source: GoldenSource::IndependentlyDerived,
        notes: "An adjacent transposition has distance one under full Damerau (and under OSA); plain Levenshtein would score it as two.",
        tags: &["basic", "transposition"],
    },
    GoldenCase {
        id: "damerau/variant-boundary/ca-abc",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"ca", b"abc"),
        expected: Distance::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "THE distinguishing example versus OSA. Under full Damerau: transpose \"ca\" -> \"ac\" (1), then insert \"b\" between them -> \"abc\" (1). Total 2. OSA gives 3 for the same pair.",
        tags: &["variant-boundary", "transposition"],
    },
    GoldenCase {
        id: "damerau/structure/multi-transposition",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"abcd", b"badc"),
        expected: Distance::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two disjoint adjacent transpositions; each counts as one operation under full Damerau.",
        tags: &["structure", "transposition"],
    },
    GoldenCase {
        id: "damerau/canonical/kitten-sitting",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"kitten", b"sitting"),
        expected: Distance::new(3),
        source: GoldenSource::IndependentlyDerived,
        notes: "Same score as Levenshtein and OSA on this pair — no transpositions in the optimal path.",
        tags: &["canonical", "substitution", "insertion"],
    },
    GoldenCase {
        id: "damerau/structure/long-input",
        descriptor: Damerau::DESCRIPTOR,
        input: (b"the quick brown fox", b"teh quikc brown fox"),
        expected: Distance::new(2),
        source: GoldenSource::IndependentlyDerived,
        notes: "Two independent adjacent transpositions (\"he\" and \"kc\") in a longer input; each counts as one operation.",
        tags: &["structure", "transposition", "long"],
    },
];

/// Full-Damerau golden cases whose inputs are `char` slices.
pub const GOLDEN_DAMERAU_CHARS: &[CharsCase] = &[GoldenCase {
    id: "damerau/unicode/cafe-accent",
    descriptor: Damerau::DESCRIPTOR,
    input: (CAFE_ACCENT, CAFE_PLAIN),
    expected: Distance::new(1),
    source: GoldenSource::IndependentlyDerived,
    notes: "Diacritic difference at char-slice granularity is one substitution under both algorithms.",
    tags: &["unicode", "substitution"],
}];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damerau::algorithm::{Damerau, Osa};
    use crate::damerau::osa::full_matrix::distance_full_matrix as osa_oracle;
    use stringcheese_core::DistanceMetric;

    #[cfg(feature = "std")]
    use crate::damerau::damerau::full_matrix::distance_full_matrix as damerau_oracle;

    #[test]
    fn every_osa_case_uses_the_osa_descriptor() {
        for case in GOLDEN_OSA_BYTES {
            assert_eq!(
                case.descriptor,
                Osa::DESCRIPTOR,
                "OSA golden case {} references the wrong descriptor",
                case.id
            );
        }
        for case in GOLDEN_OSA_CHARS {
            assert_eq!(
                case.descriptor,
                Osa::DESCRIPTOR,
                "OSA golden case {} references the wrong descriptor",
                case.id
            );
        }
    }

    #[test]
    fn every_damerau_case_uses_the_damerau_descriptor() {
        for case in GOLDEN_DAMERAU_BYTES {
            assert_eq!(
                case.descriptor,
                Damerau::DESCRIPTOR,
                "Damerau golden case {} references the wrong descriptor",
                case.id
            );
        }
        for case in GOLDEN_DAMERAU_CHARS {
            assert_eq!(
                case.descriptor,
                Damerau::DESCRIPTOR,
                "Damerau golden case {} references the wrong descriptor",
                case.id
            );
        }
    }

    #[test]
    fn every_osa_byte_case_matches_the_algorithm() {
        let alg = Osa;
        for case in GOLDEN_OSA_BYTES {
            let (left, right) = case.input;
            let observed = alg.distance(left, right);
            assert_eq!(
                observed,
                case.expected,
                "OSA golden case {} disagreed: expected {expected}, observed {observed}",
                case.id,
                expected = case.expected,
            );
            // Also cross-check against the oracle directly, as a belt-and-
            // braces guard against a bug in the trait dispatch path.
            assert_eq!(
                osa_oracle(left, right),
                case.expected.into_inner(),
                "OSA oracle disagreed with expected for {}",
                case.id
            );
        }
    }

    #[test]
    fn every_osa_char_case_matches_the_algorithm() {
        let alg = Osa;
        for case in GOLDEN_OSA_CHARS {
            let (left, right) = case.input;
            let observed = alg.distance(left, right);
            assert_eq!(
                observed,
                case.expected,
                "OSA golden case {} disagreed: expected {expected}, observed {observed}",
                case.id,
                expected = case.expected,
            );
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn every_damerau_byte_case_matches_the_algorithm() {
        let alg = Damerau;
        for case in GOLDEN_DAMERAU_BYTES {
            let (left, right) = case.input;
            let observed = alg.distance(left, right);
            assert_eq!(
                observed,
                case.expected,
                "Damerau golden case {} disagreed: expected {expected}, observed {observed}",
                case.id,
                expected = case.expected,
            );
            assert_eq!(
                damerau_oracle(left, right),
                case.expected.into_inner(),
                "Damerau oracle disagreed with expected for {}",
                case.id
            );
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn every_damerau_char_case_matches_the_algorithm() {
        let alg = Damerau;
        for case in GOLDEN_DAMERAU_CHARS {
            let (left, right) = case.input;
            let observed = alg.distance(left, right);
            assert_eq!(
                observed,
                case.expected,
                "Damerau golden case {} disagreed: expected {expected}, observed {observed}",
                case.id,
                expected = case.expected,
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // The spec asks for at least 15 golden cases across the crate.
        let total = GOLDEN_OSA_BYTES.len()
            + GOLDEN_OSA_CHARS.len()
            + GOLDEN_DAMERAU_BYTES.len()
            + GOLDEN_DAMERAU_CHARS.len();
        assert!(
            total >= 15,
            "expected at least 15 golden cases across the crate; found {total}"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let mut ids: alloc::vec::Vec<&str> = GOLDEN_OSA_BYTES
            .iter()
            .map(|c| c.id)
            .chain(GOLDEN_OSA_CHARS.iter().map(|c| c.id))
            .chain(GOLDEN_DAMERAU_BYTES.iter().map(|c| c.id))
            .chain(GOLDEN_DAMERAU_CHARS.iter().map(|c| c.id))
            .collect();
        let n_total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n_total, "duplicate golden-case id detected");
    }

    #[test]
    fn distinguishing_example_appears_in_both_variants_with_correct_values() {
        // Both variants must include the "ca" vs "abc" case, with the
        // correct algorithm-specific expected value. This is the schema
        // guarantee the crate's whole variant-registry motivation rests on.
        let osa_case = GOLDEN_OSA_BYTES
            .iter()
            .find(|c| c.input == (b"ca".as_ref(), b"abc".as_ref()))
            .expect("OSA golden set must include ca-vs-abc");
        assert_eq!(osa_case.expected, Distance::new(3));

        let dam_case = GOLDEN_DAMERAU_BYTES
            .iter()
            .find(|c| c.input == (b"ca".as_ref(), b"abc".as_ref()))
            .expect("Damerau golden set must include ca-vs-abc");
        assert_eq!(dam_case.expected, Distance::new(2));
    }
}
