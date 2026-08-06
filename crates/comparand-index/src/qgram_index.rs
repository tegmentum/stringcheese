//! The [`QgramIndex`] — an inverted index over gram sets for set-similarity
//! candidate generation.
//!
//! # What it is
//!
//! A q-gram inverted index maps each gram `g` to the list of items that
//! contain it, together with each item's total gram count. For a query
//! whose gram set is `G`, two cheap operations produce a candidate set that
//! is a *superset* of the items whose true Jaccard similarity to the query
//! could meet a threshold:
//!
//! * [`QgramIndex::length_filter_candidates`] — length-only pruning. Uses
//!   the Jaccard length bound to keep items whose gram count could
//!   conceivably meet the threshold. Does not read the postings at all;
//!   costs `O(n)` in the number of stored items.
//! * [`QgramIndex::overlap_candidates`] — postings-driven pruning. Walks
//!   the postings for each query gram and returns items sharing at least
//!   `min_overlap` distinct grams with the query. Costs
//!   `O(query_len · avg_posting_len)`.
//!
//! Both are candidate generators: they never compute the full similarity.
//! Callers are expected to rescore the returned items with a real
//! similarity kernel (Jaccard, Dice, cosine, weighted Jaccard) to produce
//! the final match list.
//!
//! # Why generic over the gram type
//!
//! The index deliberately does not depend on `comparand-ngram`. The gram
//! type `G` is any `Ord + Clone`, so the same index serves
//! character grams, byte grams, token shingles, and future phoneme grams
//! without a per-representation adapter. Callers pass grams in whichever
//! representation their pipeline produces.
//!
//! # Backing store
//!
//! Postings are held in an ordered [`BTreeMap`] keyed by the gram. Two
//! reasons, matching `comparand-ngram`:
//!
//! * **`no_std` + `alloc` compatibility.** [`BTreeMap`] lives in `alloc`;
//!   a hash-map alternative would require `std`. The whole crate targets
//!   the `no_std`-plus-`alloc` configuration.
//! * **Deterministic iteration order.** Downstream sketching and index
//!   verification depend on cross-machine reproducibility; [`BTreeMap`]
//!   provides that for free.
//!
//! # Count widths
//!
//! Per-item gram counts and per-posting item gram counts are stored as
//! `u32`. This is plenty for practical corpora (billions of grams per item
//! is far outside what any single-shot indexing pipeline would run), and
//! keeps the postings compact. Callers indexing pathologically large
//! individual documents can pre-slice them; a `u64` variant is future
//! work.
//!
//! [`BTreeMap`]: alloc::collections::BTreeMap
//!
//! # References
//!
//! * Ukkonen, E. (1992). "Approximate string-matching with q-grams and
//!   maximal matches." *Theoretical Computer Science*, 92(1), 191-211.
//!   <https://doi.org/10.1016/0304-3975(92)90143-4> — establishes the
//!   q-gram overlap bound that
//!   [`QgramIndex::overlap_candidates`] exploits.
//! * Sarawagi, S., & Kirpal, A. (2004). "Efficient set joins on similarity
//!   predicates." *Proceedings of the 2004 ACM SIGMOD international
//!   conference on Management of data*, 743-754.
//!   <https://doi.org/10.1145/1007568.1007652> — the length-filter bound
//!   used by
//!   [`QgramIndex::length_filter_candidates`], derived in
//!   [`crate::prefix_filter`].

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::prefix_filter::length_filter;

/// An inverted index over gram sets.
///
/// See the [module-level documentation][crate::qgram_index] for the two
/// candidate-generation modes and their intended use.
///
/// # Type parameters
///
/// * `G` — the gram type. `Ord + Clone` is enough; iteration order over
///   postings is deterministic because the backing store is a
///   [`BTreeMap`].
///
/// [`BTreeMap`]: alloc::collections::BTreeMap
#[derive(Debug, Clone, Default)]
pub struct QgramIndex<G: Ord + Clone> {
    /// Inverted index: for each gram, an ascending-by-`item_id` list of
    /// `(item_id, item_gram_count)` postings.
    ///
    /// The count is the *multiplicity* of the gram in the item, not the
    /// count in the query. It is preserved for downstream consumers that
    /// want multiset semantics (weighted Jaccard, TF-IDF); the built-in
    /// [`QgramIndex::overlap_candidates`] uses only presence.
    postings: BTreeMap<G, Vec<(usize, u32)>>,
    /// Total gram count per item (multiplicity-preserving), used by
    /// [`QgramIndex::length_filter_candidates`].
    item_lens: Vec<u32>,
}

impl<G: Ord + Clone> QgramIndex<G> {
    /// Builds an empty index.
    #[must_use]
    pub fn new() -> Self {
        Self {
            postings: BTreeMap::new(),
            item_lens: Vec::new(),
        }
    }

    /// Adds an item's gram sequence, returning the assigned item id.
    ///
    /// The iterator is drained once. Duplicate grams contribute to the
    /// item's total length (used by length filtering) and to the postings'
    /// per-item count (used by future multiset consumers); each distinct
    /// gram produces exactly one posting for this item.
    ///
    /// Item ids are assigned sequentially starting from zero, so the id
    /// returned equals `len() − 1` after the call.
    pub fn insert(&mut self, grams: impl IntoIterator<Item = G>) -> usize {
        let id = self.item_lens.len();
        let mut counts: BTreeMap<G, u32> = BTreeMap::new();
        let mut total: u32 = 0;
        for g in grams {
            total = total.saturating_add(1);
            *counts.entry(g).or_insert(0) = counts.get(&g).copied().unwrap_or(0).saturating_add(1);
        }
        for (g, c) in counts {
            self.postings.entry(g).or_default().push((id, c));
        }
        self.item_lens.push(total);
        id
    }

    /// Returns the number of stored items.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.item_lens.len()
    }

    /// Returns `true` if no items are stored.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.item_lens.is_empty()
    }

    /// Returns the ids of items whose gram count could conceivably meet a
    /// Jaccard similarity of `threshold` with a query of `query_len`
    /// grams.
    ///
    /// This is a length-only pre-filter (see [`length_filter`]); it never
    /// consults the postings. The returned ids are ascending.
    #[must_use]
    pub fn length_filter_candidates(&self, query_len: u32, threshold: f64) -> Vec<usize> {
        let range = length_filter(query_len, threshold);
        let mut out = Vec::new();
        for (id, &len) in self.item_lens.iter().enumerate() {
            if range.contains(&len) {
                out.push(id);
            }
        }
        out
    }

    /// Returns the ids of items sharing at least `min_overlap` distinct
    /// grams with `query_grams`.
    ///
    /// The query is deduplicated on the fly, so query multiplicity does not
    /// inflate the overlap count. Items are counted at most once per
    /// distinct query gram. When `min_overlap == 0` every stored item is a
    /// candidate; this is a degenerate case and the returned list is the
    /// same as `0..len()`.
    #[must_use]
    pub fn overlap_candidates(&self, query_grams: &[G], min_overlap: u32) -> Vec<usize> {
        if min_overlap == 0 {
            return (0..self.item_lens.len()).collect();
        }
        // Deduplicate the query grams so a duplicate in the query cannot
        // inflate the tally.
        let mut distinct: BTreeSet<&G> = BTreeSet::new();
        for g in query_grams {
            distinct.insert(g);
        }
        // Tally: for each item id, count how many distinct query grams
        // appear in its postings.
        let mut tally: BTreeMap<usize, u32> = BTreeMap::new();
        for g in distinct {
            if let Some(list) = self.postings.get(g) {
                for &(id, _count) in list {
                    *tally.entry(id).or_insert(0) += 1;
                }
            }
        }
        let mut out: Vec<usize> = tally
            .into_iter()
            .filter_map(|(id, n)| (n >= min_overlap).then_some(id))
            .collect();
        // `tally` iteration is ascending by id already because BTreeMap is
        // ordered, but `filter_map` preserves that; the sort here is a
        // safety net that costs nothing when the input is already ordered.
        out.sort_unstable();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_index_returns_nothing() {
        let idx: QgramIndex<u8> = QgramIndex::new();
        assert_eq!(idx.length_filter_candidates(5, 0.5), alloc::vec![]);
        assert_eq!(idx.overlap_candidates(&[1u8, 2, 3], 1), alloc::vec![]);
    }

    #[test]
    fn insert_assigns_sequential_ids() {
        let mut idx: QgramIndex<u8> = QgramIndex::new();
        assert_eq!(idx.insert([1u8, 2, 3]), 0);
        assert_eq!(idx.insert([4u8, 5, 6]), 1);
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn length_filter_uses_total_gram_count() {
        // Item 0 has 4 grams, item 1 has 8 grams, item 2 has 12 grams.
        let mut idx: QgramIndex<u8> = QgramIndex::new();
        idx.insert([1u8, 2, 3, 4]);
        idx.insert([1u8, 2, 3, 4, 5, 6, 7, 8]);
        idx.insert([1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        // Query length 8, θ = 0.75 → L in [6, 10] → only item 1 (8 grams).
        assert_eq!(idx.length_filter_candidates(8, 0.75), alloc::vec![1]);
    }

    #[test]
    fn overlap_uses_distinct_query_grams() {
        let mut idx: QgramIndex<u8> = QgramIndex::new();
        idx.insert([1u8, 2, 3]);
        idx.insert([1u8, 2, 4]);
        idx.insert([5u8, 6, 7]);
        // Query has grams {1, 2}; item 0 shares {1, 2} (2 overlap), item 1
        // shares {1, 2} (2 overlap), item 2 shares nothing.
        let got = idx.overlap_candidates(&[1u8, 2, 2, 2], 2);
        assert_eq!(got, alloc::vec![0, 1]);
    }

    #[test]
    fn overlap_min_one_only_drops_no_share() {
        let mut idx: QgramIndex<u8> = QgramIndex::new();
        idx.insert([1u8, 2]);
        idx.insert([3u8, 4]);
        // Query = {1}; only item 0 shares anything.
        assert_eq!(idx.overlap_candidates(&[1u8], 1), alloc::vec![0]);
    }
}
