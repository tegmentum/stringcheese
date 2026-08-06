//! Canonical golden cases for the index structures.
//!
//! Unlike the algorithm crates (where a `GoldenCase` records an expected
//! distance for a specific input pair), the natural golden case for an
//! index is *the entire result set of a query against a fixed corpus*.
//! Each test below fixes a small corpus, builds the appropriate index, and
//! checks that queries at various radii or overlaps agree with a
//! hand-verified expected set — the "independently derived" oracle in
//! [`GoldenSource::IndependentlyDerived`] terms.
//!
//! The BK-tree and VP-tree tests use [`Levenshtein`] as the wrapped metric
//! because Levenshtein is a documented true metric (see
//! `stringcheese-levenshtein`'s golden set). The q-gram tests use hand-built
//! `Vec<u8>` grams so the expected candidate lists can be enumerated by
//! eye without pulling in the n-gram crate.
//!
//! [`GoldenSource::IndependentlyDerived`]: stringcheese_corpus::GoldenSource::IndependentlyDerived
//! [`Levenshtein`]: stringcheese_compare::levenshtein::Levenshtein

#![cfg(test)]

use alloc::vec::Vec;

use stringcheese_compare::levenshtein::Levenshtein;

use crate::bk_tree::BkTree;
use crate::qgram_index::QgramIndex;
use crate::vp_tree::VpTree;

/// A small hand-picked corpus of short English words. Chosen so that
/// distances at radii 0, 1, and 2 have easy-to-verify expected sets and
/// distinct answers.
const CORPUS: &[&[u8]] = &[
    b"cat", b"cot", b"cut", b"cab", b"car", b"bat", b"bot", b"but", b"bit", b"hat",
];

/// Naive baseline for BK-tree / VP-tree completeness golden checks.
fn linear_scan_within(query: &[u8], radius: u32) -> Vec<(Vec<u8>, u32)> {
    use stringcheese_core::DistanceMetric;
    let alg = Levenshtein;
    let mut out: Vec<(Vec<u8>, u32)> = CORPUS
        .iter()
        .filter_map(|s| {
            let d = alg.distance(query, *s).into_inner();
            if d <= radius {
                Some((s.to_vec(), d))
            } else {
                None
            }
        })
        .collect();
    out.sort();
    out
}

#[test]
fn bk_tree_matches_linear_scan_on_hand_corpus() {
    let mut tree = BkTree::new(Levenshtein);
    for &s in CORPUS {
        tree.insert(s.to_vec());
    }
    for query in [b"cat".as_ref(), b"bit".as_ref(), b"zzz".as_ref()] {
        for r in 0u32..=3 {
            let mut got = tree.find_within(query, r);
            got.sort();
            let expected = linear_scan_within(query, r);
            assert_eq!(got, expected, "BK-tree disagreed at query {query:?} r={r}");
        }
    }
}

#[test]
fn vp_tree_matches_linear_scan_on_hand_corpus() {
    let mut tree = VpTree::new(Levenshtein);
    for &s in CORPUS {
        tree.insert(s.to_vec());
    }
    for query in [b"cat".as_ref(), b"bit".as_ref(), b"zzz".as_ref()] {
        for r in 0u32..=3 {
            let mut got = tree.find_within(query, r);
            got.sort();
            let expected = linear_scan_within(query, r);
            assert_eq!(got, expected, "VP-tree disagreed at query {query:?} r={r}");
        }
    }
}

#[test]
fn vp_tree_k_nearest_matches_naive_top_k() {
    use stringcheese_core::DistanceMetric;
    let alg = Levenshtein;
    let mut tree = VpTree::new(Levenshtein);
    for &s in CORPUS {
        tree.insert(s.to_vec());
    }
    let query: &[u8] = b"cat";
    let mut naive: Vec<(Vec<u8>, u32)> = CORPUS
        .iter()
        .map(|s| (s.to_vec(), alg.distance(query, *s).into_inner()))
        .collect();
    naive.sort_by_key(|(_, d)| *d);
    for k in 1..=CORPUS.len() {
        let got = tree.find_k_nearest(query, k);
        let got_d: Vec<u32> = got.iter().map(|(_, d)| *d).collect();
        let exp_d: Vec<u32> = naive.iter().take(k).map(|(_, d)| *d).collect();
        assert_eq!(got_d, exp_d, "k={k}");
    }
}

// A hand-built 3-gram corpus for the q-gram tests. Grams are two-byte
// slices held as `Vec<u8>` so they compare with `Ord`.
fn char_bigrams(input: &[u8]) -> Vec<Vec<u8>> {
    input.windows(2).map(<[u8]>::to_vec).collect()
}

#[test]
fn qgram_length_filter_matches_hand_computed_range() {
    // Item lengths: cat=2, catamaran=8, catalog=6, catfish=6.
    let words: &[&[u8]] = &[b"cat", b"catamaran", b"catalog", b"catfish"];
    let mut idx: QgramIndex<Vec<u8>> = QgramIndex::new();
    for w in words {
        idx.insert(char_bigrams(w));
    }
    // Query length 6, θ = 0.6 → L in [4, 10] → items 1, 2, 3.
    let got = idx.length_filter_candidates(6, 0.6);
    assert_eq!(got, alloc::vec![1, 2, 3]);
}

#[test]
fn qgram_overlap_matches_hand_computed_overlap() {
    // Small corpus with overlapping bigrams.
    let words: &[&[u8]] = &[b"cat", b"cats", b"scat", b"dog"];
    let mut idx: QgramIndex<Vec<u8>> = QgramIndex::new();
    for w in words {
        idx.insert(char_bigrams(w));
    }
    // Query "cat" has bigrams {ca, at}. Items sharing:
    //   cat  → {ca, at} (2)
    //   cats → {ca, at, ts} (2 shared)
    //   scat → {sc, ca, at} (2 shared)
    //   dog  → {do, og} (0 shared)
    let query = char_bigrams(b"cat");
    let got = idx.overlap_candidates(&query, 2);
    assert_eq!(got, alloc::vec![0, 1, 2]);
    let got = idx.overlap_candidates(&query, 3);
    assert_eq!(got, alloc::vec![]);
}
