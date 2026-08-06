//! Sorted-neighborhood blocking — a compact candidate-generation helper
//! that is not itself an index tree.
//!
//! # What it is
//!
//! Given a corpus and a *key extractor* (a function that produces some
//! `Ord` value from each item — a Soundex code, a name prefix, a phonetic
//! key, a normalized birthdate), the blocker sorts the corpus by that key
//! and slides a fixed-size window across the sorted order. Two items whose
//! ranks in the sorted order are within `window_size` positions of each
//! other are emitted as a candidate pair for a full pairwise comparison
//! downstream.
//!
//! This is the classic Hernández & Stolfo (1995) sorted-neighborhood
//! method that entity-resolution pipelines have been using for thirty
//! years. It is complementary to the metric-space and set-similarity
//! indexes elsewhere in this crate: instead of pruning by distance or by
//! gram overlap, it prunes by *proximity in a key-derived total order.*
//! The interesting design tradeoff is entirely in the key — if the key
//! places likely matches near one another (a Double Metaphone code
//! grouping "Katherine" and "Catherine"; a birthdate rounded to the year
//! grouping records of the same person even when spellings vary), a small
//! window catches most real duplicates without an all-pairs scan.
//!
//! # Complexity
//!
//! * *Build.* `O(n log n)` for the key sort plus `O(n)` for the inverse
//!   permutation. The key extractor is called exactly `n` times.
//! * [`candidate_pairs`] returns at most `n * window_size` pairs (proof in
//!   the doc comment on that method).
//! * [`candidates_of`] returns at most `2 * window_size` items (window on
//!   each side of the item in the sorted order).
//!
//! # No metric assumption
//!
//! Unlike [`BkTree`] and [`VpTree`], nothing here requires a metric — the
//! key is chosen by the caller and the blocker only compares keys with
//! [`Ord`]. That means sorted-neighborhood plays fine with keys derived
//! from phonetic codes, prefix strings, or other non-metric encodings.
//! Correctness of the *downstream* comparison depends on that comparison's
//! properties; the blocker itself is a pure candidate-generation helper.
//!
//! [`BkTree`]: crate::bk_tree::BkTree
//! [`VpTree`]: crate::vp_tree::VpTree
//! [`candidate_pairs`]: SortedNeighborhoodBlocker::candidate_pairs
//! [`candidates_of`]: SortedNeighborhoodBlocker::candidates_of
//!
//! # References
//!
//! * Hernández, M. A., & Stolfo, S. J. (1995). "The merge/purge problem
//!   for large databases." *ACM SIGMOD Record*, 24(2), 127-138.
//!   <https://doi.org/10.1145/568271.223807> — the original
//!   sorted-neighborhood method for record linkage.
//! * Christen, P. (2012). *Data Matching: Concepts and Techniques for
//!   Record Linkage, Entity Resolution, and Duplicate Detection*.
//!   Springer. ISBN 978-3-642-31163-5 — modern reference for blocking
//!   techniques, including sorted-neighborhood variants and key-design
//!   considerations.

use alloc::vec::Vec;

/// Slides a fixed-size window over a corpus sorted by an extracted key,
/// emitting candidate pairs from within each window.
///
/// See the [module-level docs][crate::sorted_neighborhood] for the
/// algorithm and the tradeoffs of key choice.
///
/// # Type parameters
///
/// * `T` — the corpus item type. No trait bounds are imposed; items are
///   only accessed via their positional index.
/// * `K` — the key type. Must be [`Ord`] so the corpus can be sorted; the
///   blocker never inspects keys itself beyond that.
///
/// # Example
///
/// ```
/// use comparand_index::SortedNeighborhoodBlocker;
///
/// // Sort by the first byte of each item; window of 1 pairs each item
/// // with its immediate neighbor in the sorted order.
/// let items: Vec<&str> = vec!["banana", "apple", "cherry", "avocado"];
/// let blocker = SortedNeighborhoodBlocker::new(items, |s: &&str| s.as_bytes()[0]);
/// let pairs = blocker.candidate_pairs(1);
/// // Sorted order by first byte: apple, avocado, banana, cherry
/// //                              (1)    (3)      (0)     (2)
/// // Adjacent pairs (as (min, max)): (1,3), (0,3), (0,2)
/// assert_eq!(pairs.len(), 3);
/// ```
#[derive(Debug)]
pub struct SortedNeighborhoodBlocker<T, K: Ord> {
    /// The corpus, in insertion order. Kept so callers can look items up
    /// by the indices returned from [`candidate_pairs`] and
    /// [`candidates_of`].
    ///
    /// [`candidate_pairs`]: SortedNeighborhoodBlocker::candidate_pairs
    /// [`candidates_of`]: SortedNeighborhoodBlocker::candidates_of
    items: Vec<T>,
    /// Extracted keys, parallel to `items` (i.e. `keys[i]` is the key of
    /// `items[i]`). Preserved so debugging output can name the key that
    /// determined an item's position without re-running the extractor.
    keys: Vec<K>,
    /// The permutation that puts `items` into sorted-by-key order.
    /// `sort_indices[rank]` is the original position of the item at that
    /// rank in the sorted order. Stable-sorted, so ties fall in insertion
    /// order.
    sort_indices: Vec<usize>,
    /// The inverse of `sort_indices`: `rank_of[i]` is the position of
    /// `items[i]` in the sorted order. Precomputed so
    /// [`candidates_of`] can find an item's neighborhood in `O(1)`
    /// instead of scanning the permutation.
    ///
    /// [`candidates_of`]: SortedNeighborhoodBlocker::candidates_of
    rank_of: Vec<usize>,
}

impl<T, K: Ord> SortedNeighborhoodBlocker<T, K> {
    /// Builds a blocker from `items` and a per-item key extractor.
    ///
    /// The extractor is called exactly once per item at construction time.
    /// Sorting uses a *stable* sort by key, so items with equal keys
    /// retain their original insertion order in the sorted view — this is
    /// what makes [`candidate_pairs`] deterministic when many items share
    /// a key (a common case with coarse phonetic buckets).
    ///
    /// # Complexity
    ///
    /// `O(n)` extractor calls plus `O(n log n)` for the sort, plus `O(n)`
    /// for the inverse permutation.
    ///
    /// [`candidate_pairs`]: SortedNeighborhoodBlocker::candidate_pairs
    pub fn new<F>(items: Vec<T>, mut key_of: F) -> Self
    where
        F: FnMut(&T) -> K,
    {
        let keys: Vec<K> = items.iter().map(&mut key_of).collect();
        let n = items.len();
        let mut sort_indices: Vec<usize> = (0..n).collect();
        // Stable sort: ties among equal keys keep insertion order so the
        // sliding window is deterministic across builds with identical
        // input.
        sort_indices.sort_by(|&a, &b| keys[a].cmp(&keys[b]));
        let mut rank_of: Vec<usize> = alloc::vec![0; n];
        for (rank, &orig) in sort_indices.iter().enumerate() {
            rank_of[orig] = rank;
        }
        Self {
            items,
            keys,
            sort_indices,
            rank_of,
        }
    }

    /// Number of items in the corpus.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// `true` iff the corpus is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Read-only view of the corpus in insertion order. Indices returned
    /// from [`candidate_pairs`] and [`candidates_of`] index into this
    /// slice.
    ///
    /// [`candidate_pairs`]: Self::candidate_pairs
    /// [`candidates_of`]: Self::candidates_of
    #[inline]
    #[must_use]
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Read-only view of the extracted keys, parallel to [`items`].
    ///
    /// [`items`]: Self::items
    #[inline]
    #[must_use]
    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    /// The candidate pairs `(i, j)` with `i < j` such that `items[i]` and
    /// `items[j]` are within `window_size` positions of one another in the
    /// key-sorted order.
    ///
    /// The result is returned sorted lexicographically by `(i, j)`, which
    /// makes it stable to compare in tests and cheap to feed to a
    /// downstream deduplication step. When `window_size == 0` no pairs
    /// are emitted (an item's zero-radius neighborhood is itself only).
    ///
    /// # Bound
    ///
    /// For a corpus of size `N`, `|candidate_pairs(w)| ≤ N · w`. Proof:
    /// at each of the `N` positions in the sorted order the inner loop
    /// admits at most `w` successors (positions `rank + 1 ..= rank + w`,
    /// clamped to the corpus). Each admitted pair is produced by exactly
    /// one `(rank, successor)` iteration because the permutation is a
    /// bijection, so no duplicates are generated within a single call and
    /// the count is exactly the number of inner iterations, `≤ N · w`.
    ///
    /// # Coverage
    ///
    /// For `window_size ≥ N - 1`, every unordered pair of distinct
    /// positions appears in the result: rank `0` alone contributes pairs
    /// with every other rank.
    #[must_use]
    pub fn candidate_pairs(&self, window_size: usize) -> Vec<(usize, usize)> {
        let n = self.sort_indices.len();
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        if window_size == 0 || n < 2 {
            return pairs;
        }
        for a_rank in 0..n {
            // Successor ranks in [a_rank + 1, a_rank + window_size], clamped
            // to n. Upper bound (exclusive) is `a_rank + window_size + 1`.
            let end = a_rank.saturating_add(window_size).saturating_add(1).min(n);
            for b_rank in (a_rank + 1)..end {
                let a = self.sort_indices[a_rank];
                let b = self.sort_indices[b_rank];
                let pair = if a < b { (a, b) } else { (b, a) };
                pairs.push(pair);
            }
        }
        // Deterministic output order. No duplicates are produced by the
        // loop above (each unordered pair is generated by exactly one
        // (a_rank, b_rank) with a_rank < b_rank), so a plain sort is
        // sufficient.
        pairs.sort_unstable();
        pairs
    }

    /// The corpus positions `j` (other than `i`) whose sorted-order rank
    /// is within `window_size` of `i`'s rank — `window_size` positions on
    /// each side of `i`, clamped by the corpus boundaries.
    ///
    /// The returned slice contains at most `2 * window_size` items and is
    /// ordered by ascending rank in the sorted order (which is *not*
    /// necessarily ascending order of the position `j` itself).
    ///
    /// Returns an empty vector when `i` is out of bounds; this makes the
    /// method safe to call from generic pipeline code that has not
    /// verified indices.
    #[must_use]
    pub fn candidates_of(&self, i: usize, window_size: usize) -> Vec<usize> {
        let n = self.sort_indices.len();
        if i >= n || window_size == 0 {
            return Vec::new();
        }
        let rank = self.rank_of[i];
        let start = rank.saturating_sub(window_size);
        let end = rank.saturating_add(window_size).saturating_add(1).min(n);
        let mut out = Vec::with_capacity(end - start);
        for r in start..end {
            if r != rank {
                out.push(self.sort_indices[r]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_corpus_yields_no_pairs() {
        let blocker: SortedNeighborhoodBlocker<u32, u32> =
            SortedNeighborhoodBlocker::new(alloc::vec![], |&x| x);
        assert_eq!(blocker.len(), 0);
        assert!(blocker.is_empty());
        assert_eq!(blocker.candidate_pairs(5), alloc::vec![]);
        assert_eq!(blocker.candidates_of(0, 5), alloc::vec![]);
    }

    #[test]
    fn window_zero_yields_no_pairs() {
        let blocker = SortedNeighborhoodBlocker::new(alloc::vec![1u32, 2, 3, 4], |&x| x);
        assert_eq!(blocker.candidate_pairs(0), alloc::vec![]);
        assert_eq!(blocker.candidates_of(0, 0), alloc::vec![]);
    }

    #[test]
    fn window_one_pairs_adjacent_in_sorted_order() {
        // Sorted order by first byte: apple(1), avocado(3), banana(0), cherry(2)
        let items = alloc::vec!["banana", "apple", "cherry", "avocado"];
        let blocker = SortedNeighborhoodBlocker::new(items, |s: &&str| s.as_bytes()[0]);
        let pairs = blocker.candidate_pairs(1);
        // Adjacent pairs (normalized to min<max): (1,3), (0,3), (0,2)
        // Sorted: (0,2), (0,3), (1,3)
        assert_eq!(pairs, alloc::vec![(0, 2), (0, 3), (1, 3)]);
    }

    #[test]
    fn candidates_of_is_neighborhood_in_sorted_order() {
        // Same corpus as above. Ranks: apple=0, avocado=1, banana=2, cherry=3.
        // apple's original index is 1; rank_of[1] = 0.
        let items = alloc::vec!["banana", "apple", "cherry", "avocado"];
        let blocker = SortedNeighborhoodBlocker::new(items, |s: &&str| s.as_bytes()[0]);
        // apple (index 1) neighborhood with window=2: ranks 0..=2, excluding 0.
        // Ranks 1,2 → indices 3 (avocado), 0 (banana).
        assert_eq!(blocker.candidates_of(1, 2), alloc::vec![3, 0]);
    }

    #[test]
    fn stable_sort_preserves_insertion_order_for_ties() {
        // All items share the same key; sort must preserve insertion order.
        let items = alloc::vec!["a", "b", "c", "d"];
        let blocker = SortedNeighborhoodBlocker::new(items, |_| 0u32);
        // Sorted order = insertion order = [0, 1, 2, 3].
        // Window 1 pairs (0,1), (1,2), (2,3).
        assert_eq!(
            blocker.candidate_pairs(1),
            alloc::vec![(0, 1), (1, 2), (2, 3)]
        );
    }

    #[test]
    fn window_at_or_beyond_len_covers_all_pairs() {
        let items = alloc::vec![10u32, 30, 20, 40];
        let blocker = SortedNeighborhoodBlocker::new(items, |&x| x);
        let all_pairs = blocker.candidate_pairs(4);
        let n = 4;
        let expected_len = n * (n - 1) / 2;
        assert_eq!(all_pairs.len(), expected_len);
        // Check all 6 unordered pairs are present.
        for i in 0..n {
            for j in (i + 1)..n {
                assert!(
                    all_pairs.contains(&(i, j)),
                    "missing pair ({i},{j}) in full-window output {all_pairs:?}"
                );
            }
        }
    }

    #[test]
    fn out_of_bounds_index_returns_empty_candidates() {
        let blocker = SortedNeighborhoodBlocker::new(alloc::vec![1u32, 2], |&x| x);
        assert_eq!(blocker.candidates_of(99, 1), alloc::vec![]);
    }

    #[test]
    fn keys_accessor_returns_extracted_keys() {
        let items = alloc::vec!["banana", "apple"];
        let blocker = SortedNeighborhoodBlocker::new(items, |s: &&str| s.len());
        assert_eq!(blocker.keys(), &[6, 5][..]);
    }
}
