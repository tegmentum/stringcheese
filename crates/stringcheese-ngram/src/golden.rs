//! Canonical n-gram generation cases — the reference implementation of
//! the design document's specification.
//!
//! N-gram *generation* does not produce a [`Distance`], [`Similarity`], or
//! [`Score`], so the [`stringcheese_corpus::GoldenCase`] schema does not fit
//! it directly (the schema keys on an [`AlgorithmDescriptor`], and this
//! crate produces no descriptors — see the crate-level docs). This module
//! stores the same *shape* of case — id, input, expected output, notes,
//! source — as an in-file `const` table, and the tests below check every
//! implementation against every case.
//!
//! Every case in this file cites the design document
//! (`docs/design/ngram-and-fingerprinting.md`) as its source; the
//! canonical examples are pulled directly from that document's worked
//! example section, and the edge cases are the ones the document commits
//! to (empty input, arity greater than input length, and the multiset vs
//! set distinction on `"aaa"`).
//!
//! [`Distance`]: stringcheese_core::Distance
//! [`Similarity`]: stringcheese_core::Similarity
//! [`Score`]: stringcheese_core::Score
//! [`AlgorithmDescriptor`]: stringcheese_core::AlgorithmDescriptor

use alloc::vec;
use alloc::vec::Vec;

use crate::character::CharacterGrams;
use crate::generator::{NGramGenerator, count_grams};
use crate::multiset::GramMultiSet;
use crate::padding::PaddingPolicy;
use crate::set::GramSet;

/// A single canonical case for the character generator.
///
/// This is the structural analog of a [`stringcheese_corpus::GoldenCase`]:
/// the `expected` field records the exact output the generator must
/// produce, and `notes` documents the reasoning.
#[derive(Debug)]
struct CharCase {
    /// Hierarchical case identifier for filtering test runs.
    id: &'static str,
    /// The input the generator is applied to.
    input: &'static [char],
    /// The generator's arity.
    n: usize,
    /// The padding policy under test. Uses `char` markers because this
    /// case set exercises the character generator.
    padding: PaddingPolicy<char>,
    /// The exact ordered sequence of grams the generator must produce.
    expected: Vec<Vec<char>>,
    /// The source (always the design document for this file).
    #[allow(dead_code)]
    source: &'static str,
    /// Human-readable notes about what the case exercises.
    #[allow(dead_code)]
    notes: &'static str,
}

#[allow(clippy::too_many_lines)]
fn canonical_char_cases() -> Vec<CharCase> {
    vec![
        CharCase {
            id: "character/cat/unigrams-unpadded",
            input: &['c', 'a', 't'],
            n: 1,
            padding: PaddingPolicy::None,
            expected: vec![vec!['c'], vec!['a'], vec!['t']],
            source: "docs/design/ngram-and-fingerprinting.md § 2 (Fixed n) + § 3 (No padding)",
            notes: "n=1 with no padding is the trivial base case: one gram per input symbol.",
        },
        CharCase {
            id: "character/cat/bigrams-unpadded",
            input: &['c', 'a', 't'],
            n: 2,
            padding: PaddingPolicy::None,
            expected: vec![vec!['c', 'a'], vec!['a', 't']],
            source: "docs/design/ngram-and-fingerprinting.md § 3 (No padding)",
            notes: "The worked bigram example: `\"kit\"` under bigrams yields `{ki, it}`; \
                    `\"cat\"` under the same policy is structurally identical.",
        },
        CharCase {
            id: "character/cat/bigrams-boundary",
            input: &['c', 'a', 't'],
            n: 2,
            padding: PaddingPolicy::Boundary {
                start: '^',
                end: '$',
            },
            expected: vec![
                vec!['^', 'c'],
                vec!['c', 'a'],
                vec!['a', 't'],
                vec!['t', '$'],
            ],
            source: "docs/design/ngram-and-fingerprinting.md § 3 (Boundary markers)",
            notes: "The worked padded-bigram example: `\"kit\"` becomes `\"$kit$\"` under \
                    bigrams; `\"cat\"` here mirrors that. Distinct start and end markers \
                    prevent artificial palindromic collisions.",
        },
        CharCase {
            id: "character/cat/n4-unpadded-empty",
            input: &['c', 'a', 't'],
            n: 4,
            padding: PaddingPolicy::None,
            expected: vec![],
            source: "docs/design/ngram-and-fingerprinting.md § 3 (No padding)",
            notes: "Input length 3 with n=4 under no padding yields zero grams — the case \
                    that motivates boundary padding in the first place.",
        },
        CharCase {
            id: "character/cat/n4-boundary",
            input: &['c', 'a', 't'],
            n: 4,
            padding: PaddingPolicy::Boundary {
                start: '^',
                end: '$',
            },
            expected: vec![
                vec!['^', '^', '^', 'c'],
                vec!['^', '^', 'c', 'a'],
                vec!['^', 'c', 'a', 't'],
                vec!['c', 'a', 't', '$'],
                vec!['a', 't', '$', '$'],
                vec!['t', '$', '$', '$'],
            ],
            source: "docs/design/ngram-and-fingerprinting.md § 3 (Boundary markers)",
            notes: "The same input at n=4 with n-1=3 markers each side: input plus \
                    padding is 9 characters, 6 four-grams.",
        },
        CharCase {
            id: "character/empty/n3-unpadded",
            input: &[],
            n: 3,
            padding: PaddingPolicy::None,
            expected: vec![],
            source: "docs/design/ngram-and-fingerprinting.md § 3",
            notes: "Empty input, no padding: zero grams — the degenerate base case.",
        },
        CharCase {
            id: "character/empty/n3-boundary",
            input: &[],
            n: 3,
            padding: PaddingPolicy::Boundary {
                start: '^',
                end: '$',
            },
            expected: vec![vec!['^', '^', '$'], vec!['^', '$', '$']],
            source: "docs/design/ngram-and-fingerprinting.md § 3 (Boundary markers)",
            notes: "Empty input still yields grams under boundary padding: the padded \
                    sequence `^^$$` has two length-3 windows.",
        },
        CharCase {
            id: "character/aaa/bigrams-unpadded",
            input: &['a', 'a', 'a'],
            n: 2,
            padding: PaddingPolicy::None,
            expected: vec![vec!['a', 'a'], vec!['a', 'a']],
            source: "docs/design/ngram-and-fingerprinting.md § 4 (Set vs multiset)",
            notes: "The multiset case: `\"aaa\"` at n=2 produces two identical bigrams. \
                    The set representation collapses them; the multiset preserves the count.",
        },
    ]
}

/// Cases whose set vs multiset distinction is the point.
///
/// These do not fit the `CharCase` schema because the expected value is
/// not the raw gram sequence — it is the derived set or multiset shape.
#[test]
fn cat_and_act_have_distinct_gram_sets() {
    // "cat" and "act" — same characters, different order — must not
    // produce the same bigram set. Bigrams are position-sensitive within
    // a window, so `ca ≠ ac` even though the underlying character
    // multisets agree.
    let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
    let s_cat: GramSet<Vec<char>> = GramSet::from_generator(&generator, &['c', 'a', 't']);
    let s_act: GramSet<Vec<char>> = GramSet::from_generator(&generator, &['a', 'c', 't']);
    assert_ne!(
        s_cat, s_act,
        "bigram sets of `cat` and `act` must differ — order matters within a gram"
    );
}

#[test]
fn aaa_bigrams_collapse_in_set_and_persist_in_multiset() {
    let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
    let input: &[char] = &['a', 'a', 'a'];
    let s = GramSet::from_generator(&generator, input);
    let ms = GramMultiSet::from_generator(&generator, input);
    assert_eq!(s.len(), 1);
    assert_eq!(ms.count(&vec!['a', 'a']), 2);
    assert_eq!(ms.total_count(), 2);
}

#[test]
fn every_canonical_case_matches_the_generator() {
    for case in canonical_char_cases() {
        let generator = CharacterGrams::new(case.n, case.padding.clone());
        let observed: Vec<Vec<char>> = generator.grams(case.input).collect();
        assert_eq!(
            observed, case.expected,
            "golden case `{}` disagreed with the generator",
            case.id
        );
    }
}

#[test]
fn count_grams_matches_iterator_count_for_every_canonical_case() {
    // The `count_grams` closed form must agree exactly with the iterator's
    // length; this is the load-bearing invariant every consumer that
    // preallocates a backing store relies on.
    for case in canonical_char_cases() {
        let expected = count_grams(case.input.len(), case.n, &case.padding);
        let generator = CharacterGrams::new(case.n, case.padding.clone());
        let observed_len = generator.grams(case.input).count();
        assert_eq!(
            observed_len,
            expected,
            "count_grams disagreed with iterator for `{}` (input_len={}, n={})",
            case.id,
            case.input.len(),
            case.n,
        );
    }
}

#[test]
fn every_case_has_a_unique_id() {
    let cases = canonical_char_cases();
    let mut ids: Vec<&str> = cases.iter().map(|c| c.id).collect();
    ids.sort_unstable();
    let n = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), n, "duplicate canonical case id detected");
}
