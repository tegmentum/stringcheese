//! Property-based tests for the index structures.
//!
//! These are the primary correctness tests: the tree pruning derivations
//! in the module docs are only *sound* if, on every random corpus and
//! every random query, the tree's answer set equals the naive linear scan's
//! answer set. Any disagreement means pruning has silently dropped a valid
//! candidate — which would make the tree strictly worse than a linear scan.

#![cfg(test)]

use alloc::vec::Vec;
use proptest::prelude::*;

use comparand_core::DistanceMetric;
use comparand_damerau::Osa;
use comparand_levenshtein::Levenshtein;

use crate::bk_tree::BkTree;
use crate::error::NotAMetricError;
use crate::prefix_filter::length_filter;
use crate::qgram_index::QgramIndex;
use crate::vp_tree::VpTree;

/// A short byte-slice strategy over a small alphabet.
fn arb_bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(b'a'..=b'c', 0..8)
}

/// A collection of short byte slices, giving us a small corpus per test.
fn arb_corpus() -> impl Strategy<Value = Vec<Vec<u8>>> {
    proptest::collection::vec(arb_bytes(), 1..12)
}

/// A larger corpus drawn from a slightly wider alphabet — used by the
/// depth-bound test where too many ties at the median would push the
/// bulk-built tree onto a degenerate path (a legitimate but uninteresting
/// case; the test's job is to check that *typical* inputs stay balanced).
fn arb_balanced_corpus() -> impl Strategy<Value = Vec<Vec<u8>>> {
    proptest::collection::vec(proptest::collection::vec(b'a'..=b'h', 2..10), 4..48)
}

/// Naive baseline: every corpus item within `r` of `query` under Levenshtein.
fn naive_within(corpus: &[Vec<u8>], query: &[u8], r: u32) -> Vec<(Vec<u8>, u32)> {
    let alg = Levenshtein;
    let mut out: Vec<(Vec<u8>, u32)> = corpus
        .iter()
        .filter_map(|s| {
            let d = alg.distance(query, s).into_inner();
            if d <= r { Some((s.clone(), d)) } else { None }
        })
        .collect();
    out.sort();
    out
}

/// Naive baseline: k nearest items under Levenshtein, sorted by ascending
/// distance. Ties are broken in insertion order, matching how the VP-tree's
/// max-heap resolves them.
fn naive_k_nearest(corpus: &[Vec<u8>], query: &[u8], k: usize) -> Vec<(Vec<u8>, u32)> {
    let alg = Levenshtein;
    let mut annotated: Vec<(Vec<u8>, u32)> = corpus
        .iter()
        .map(|s| (s.clone(), alg.distance(query, s).into_inner()))
        .collect();
    // Stable sort by distance so ties keep their insertion order.
    annotated.sort_by_key(|(_, d)| *d);
    annotated.truncate(k);
    annotated
}

proptest! {
    /// BK-tree completeness — the whole point of the structure. For every
    /// corpus, query, and radius, the tree's answer must equal the naive
    /// linear scan.
    #[test]
    fn bk_tree_completeness(
        corpus in arb_corpus(),
        query in arb_bytes(),
        r in 0u32..=6,
    ) {
        let mut tree = BkTree::new(Levenshtein);
        for item in &corpus {
            tree.insert(item.clone());
        }
        let mut got = tree.find_within(&query, r);
        got.sort();
        let expected = naive_within(&corpus, &query, r);
        prop_assert_eq!(got, expected);
    }

    /// VP-tree completeness for range queries.
    #[test]
    fn vp_tree_range_completeness(
        corpus in arb_corpus(),
        query in arb_bytes(),
        r in 0u32..=6,
    ) {
        let mut tree = VpTree::new(Levenshtein);
        for item in &corpus {
            tree.insert(item.clone());
        }
        let mut got = tree.find_within(&query, r);
        got.sort();
        let expected = naive_within(&corpus, &query, r);
        prop_assert_eq!(got, expected);
    }

    /// VP-tree k-NN correctness — the distance-sequence returned by the
    /// tree must equal the naive top-k distance sequence. Item identity is
    /// only checked up to distance because ties are legitimately
    /// exchangeable.
    #[test]
    fn vp_tree_k_nearest_matches_naive(
        corpus in arb_corpus(),
        query in arb_bytes(),
        k in 0usize..=6,
    ) {
        let mut tree = VpTree::new(Levenshtein);
        for item in &corpus {
            tree.insert(item.clone());
        }
        let got = tree.find_k_nearest(&query, k);
        let expected = naive_k_nearest(&corpus, &query, k);
        let got_d: Vec<u32> = got.iter().map(|(_, d)| *d).collect();
        let expected_d: Vec<u32> = expected.iter().map(|(_, d)| *d).collect();
        prop_assert_eq!(got_d, expected_d);
    }

    /// The bulk-built VP-tree must return the same range-query results as
    /// the incrementally-built VP-tree over the same corpus. Both should
    /// also equal the naive linear scan; if they agree on every input,
    /// we have real confidence both construction paths are correct.
    #[test]
    fn vp_tree_bulk_build_equivalence_range(
        corpus in arb_corpus(),
        query in arb_bytes(),
        r in 0u32..=6,
    ) {
        // Incremental tree.
        let mut incremental = VpTree::new(Levenshtein);
        for item in &corpus {
            incremental.insert(item.clone());
        }
        // Bulk-built tree.
        let bulk = VpTree::from_corpus(Levenshtein, corpus.clone());

        let mut got_incr = incremental.find_within(&query, r);
        got_incr.sort();
        let mut got_bulk = bulk.find_within(&query, r);
        got_bulk.sort();
        let expected = naive_within(&corpus, &query, r);

        prop_assert_eq!(&got_incr, &expected, "incremental disagreed with naive");
        prop_assert_eq!(&got_bulk, &expected, "bulk disagreed with naive");
        prop_assert_eq!(&got_bulk, &got_incr, "bulk disagreed with incremental");
    }

    /// The bulk-built VP-tree must also produce the same k-NN
    /// distance-sequence as the incrementally-built tree (and as the
    /// naive top-k). Item identity is only checked up to distance because
    /// ties are legitimately exchangeable across construction strategies.
    #[test]
    fn vp_tree_bulk_build_equivalence_k_nearest(
        corpus in arb_corpus(),
        query in arb_bytes(),
        k in 0usize..=6,
    ) {
        let mut incremental = VpTree::new(Levenshtein);
        for item in &corpus {
            incremental.insert(item.clone());
        }
        let bulk = VpTree::from_corpus(Levenshtein, corpus.clone());

        let got_incr: Vec<u32> = incremental
            .find_k_nearest(&query, k)
            .into_iter()
            .map(|(_, d)| d)
            .collect();
        let got_bulk: Vec<u32> = bulk
            .find_k_nearest(&query, k)
            .into_iter()
            .map(|(_, d)| d)
            .collect();
        let expected: Vec<u32> = naive_k_nearest(&corpus, &query, k)
            .into_iter()
            .map(|(_, d)| d)
            .collect();

        prop_assert_eq!(&got_incr, &expected, "incremental k-NN disagreed with naive");
        prop_assert_eq!(&got_bulk, &expected, "bulk k-NN disagreed with naive");
    }

    /// The bulk-built VP-tree's depth should be close to `log2(N)` — the
    /// whole point of median-partition construction over incremental
    /// insertion. The bound is `ceil(log2(N)) + 2`:
    ///   - `+1` because `max_depth` counts nodes, not edges (root is 1);
    ///   - `+1` slack for odd sizes where the split is off-by-one (inside
    ///     gets `n/2`, outside gets `n - n/2 = n/2 + 1`).
    ///
    /// The position-based partition (see [`build_bulk`] in
    /// [`vp_tree`][crate::vp_tree]) ensures ties at the median do not
    /// unbalance the tree, so no additional slack for ties is needed.
    #[test]
    fn vp_tree_bulk_build_balanced(corpus in arb_balanced_corpus()) {
        let n = corpus.len();
        prop_assume!(n >= 2);
        let bulk = VpTree::from_corpus(Levenshtein, corpus);

        let depth = bulk.max_depth();
        let ceil_log2 = (usize::BITS - (n - 1).leading_zeros()) as usize;
        let bound = ceil_log2 + 2;
        prop_assert!(
            depth <= bound,
            "bulk-built tree unexpectedly deep: n={n} depth={depth} bound={bound}"
        );
    }

    /// Length-filter soundness: the filter's output must be a *superset*
    /// of the items that could actually meet the threshold.
    ///
    /// We check the contrapositive over a random corpus: for every item
    /// whose true Jaccard similarity to the query set meets the threshold,
    /// the length filter must have kept it.
    #[test]
    fn length_filter_soundness(
        query_len in 1u32..30,
        item_len in 0u32..60,
        threshold in 0.1f64..=1.0,
    ) {
        let range = length_filter(query_len, threshold);
        // For an item of `item_len` grams to satisfy J >= θ against a
        // query of `query_len` grams, we need min(L,Q)/max(L,Q) >= θ, i.e.
        // both L >= θ·Q and L <= Q/θ. If that inequality holds, the item
        // must be in the filter's range.
        let l = f64::from(item_len);
        let q = f64::from(query_len);
        let can_meet = if item_len <= query_len {
            l >= threshold * q
        } else {
            l <= q / threshold
        };
        if can_meet && item_len > 0 {
            prop_assert!(
                range.contains(&item_len),
                "length filter dropped a candidate that could meet the threshold: item_len={item_len} query_len={query_len} θ={threshold} range={range:?}"
            );
        }
    }

    /// Q-gram overlap candidates are a superset of the items whose actual
    /// overlap meets the threshold. We construct a set of items with known
    /// grams, compute the naive intersection cardinality, and check the
    /// index's candidate list contains it.
    #[test]
    fn qgram_overlap_soundness(
        items in proptest::collection::vec(
            proptest::collection::vec(0u8..8, 1..8),
            1..8,
        ),
        query in proptest::collection::vec(0u8..8, 1..8),
        min_overlap in 1u32..=4,
    ) {
        let mut idx: QgramIndex<u8> = QgramIndex::new();
        for grams in &items {
            idx.insert(grams.iter().copied());
        }
        // Naive baseline: for each item, count distinct query grams that
        // appear in the item.
        let query_set: alloc::collections::BTreeSet<u8> = query.iter().copied().collect();
        let mut expected: Vec<usize> = items
            .iter()
            .enumerate()
            .filter_map(|(id, item_grams)| {
                let item_set: alloc::collections::BTreeSet<u8> =
                    item_grams.iter().copied().collect();
                let overlap = query_set.intersection(&item_set).count();
                if u32::try_from(overlap).unwrap_or(0) >= min_overlap {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();
        expected.sort_unstable();
        let got = idx.overlap_candidates(&query, min_overlap);
        // Every expected id must appear in the candidate list.
        for id in &expected {
            prop_assert!(
                got.contains(id),
                "overlap_candidates missed id {id}: got={got:?} expected superset of {expected:?}"
            );
        }
    }
}

// Non-proptest sanity checks for the panic-vs-fallible policy on
// non-metric input. These are cheap and worth being explicit about.

#[test]
fn try_new_bk_tree_rejects_semimetric() {
    // OSA is documented as MetricProperties::SEMIMETRIC.
    let err = BkTree::<u8, Osa>::try_new(Osa).unwrap_err();
    let expected = NotAMetricError::new(comparand_core::MetricProperties::SEMIMETRIC);
    assert_eq!(err, expected);
}

#[test]
fn try_new_vp_tree_rejects_semimetric() {
    let err = VpTree::<u8, Osa>::try_new(Osa).unwrap_err();
    let expected = NotAMetricError::new(comparand_core::MetricProperties::SEMIMETRIC);
    assert_eq!(err, expected);
}

#[test]
fn try_from_corpus_vp_tree_rejects_semimetric() {
    // Same policy as `try_new`: bulk construction over a semimetric
    // returns `NotAMetricError` instead of building a tree whose pruning
    // would be unsound.
    let err = VpTree::<u8, Osa>::try_from_corpus(Osa, Vec::new()).unwrap_err();
    let expected = NotAMetricError::new(comparand_core::MetricProperties::SEMIMETRIC);
    assert_eq!(err, expected);
}

#[test]
#[should_panic(expected = "VpTree requires a true metric")]
fn from_corpus_vp_tree_panics_on_semimetric() {
    let _tree: VpTree<u8, Osa> = VpTree::from_corpus(Osa, Vec::new());
}

#[test]
#[should_panic(expected = "BkTree requires a true metric")]
fn new_bk_tree_panics_on_semimetric() {
    let _tree: BkTree<u8, Osa> = BkTree::new(Osa);
}

#[test]
#[should_panic(expected = "VpTree requires a true metric")]
fn new_vp_tree_panics_on_semimetric() {
    let _tree: VpTree<u8, Osa> = VpTree::new(Osa);
}
