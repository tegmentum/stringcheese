//! The [`VpTree`] — Vantage-Point tree for metric-space range and k-NN
//! queries.
//!
//! # What it is
//!
//! A VP-tree, introduced by Peter Yianilos in 1993, is a binary metric-space
//! index. At each internal node one item is chosen as the *vantage point*
//! and a *threshold distance* is picked; every other item at that node is
//! sent to the *inside* subtree if its distance to the vantage is less than
//! or equal to the threshold, and to the *outside* subtree otherwise.
//!
//! # How the tree is organized
//!
//! Each node stores:
//!
//! * one *vantage-point* item;
//! * an optional *threshold* distance, absent iff the node is a leaf with no
//!   children yet;
//! * an *inside* subtree containing items at distance `≤ threshold` from the
//!   vantage;
//! * an *outside* subtree containing items at distance `> threshold` from
//!   the vantage.
//!
//! [`VpTree::insert`] descends the tree, computing the distance from the new
//! item to each vantage-point along the way and following whichever side of
//! the current threshold applies, until it reaches a slot without children —
//! at which point the new item becomes the vantage of a fresh leaf and the
//! threshold at the previous level is set to the distance we just computed.
//! This is a naive incremental construction: it is correct but does not
//! attempt to rebalance, so on adversarial insertion orders the tree may
//! degenerate into a path. Bulk construction with median-based thresholds
//! is a natural extension and lands as future work.
//!
//! # How range queries prune
//!
//! Let `q` be the query, `r` the radius, `v` the vantage, `t` the threshold,
//! and `d = metric.distance(q, v)`. Every item `x` in the inside subtree has
//! `metric.distance(x, v) ≤ t`, and by the triangle inequality
//! `metric.distance(q, x) ≥ |d − metric.distance(x, v)|`.
//!
//! * **Inside subtree.** In the worst case `metric.distance(x, v)` is chosen
//!   to minimize `|d − metric.distance(x, v)|` for values in `[0, t]`. That
//!   minimum is zero when `d ≤ t` and `d − t` when `d > t`. So an inside
//!   item can be within radius `r` only when `d − t ≤ r`, equivalently
//!   `d ≤ t + r`.
//! * **Outside subtree.** Items have `metric.distance(x, v) > t`. The
//!   worst-case lower bound on `metric.distance(q, x)` is zero when `d > t`
//!   and `t − d` when `d ≤ t`. An outside item can be within radius `r`
//!   only when `t − d ≤ r`, equivalently `d + r ≥ t`.
//!
//! Both checks use only addition, so they never underflow on unsigned
//! distance types.
//!
//! # How k-NN queries prune
//!
//! [`VpTree::find_k_nearest`] maintains a max-heap of the best-`k`
//! candidates seen so far and updates the current cutoff radius from the
//! heap's top whenever a new candidate is admitted. The visit order at each
//! node is "closer subtree first" — this admits improved candidates earlier,
//! tightens the cutoff sooner, and prunes more aggressively on the far
//! side.
//!
//! # Why the wrapped metric must be a true metric
//!
//! Both range and k-NN pruning invoke the triangle inequality; a semimetric
//! makes the bounds above unsound and the tree will silently drop items
//! whose true distance is within the requested radius. As with
//! [`BkTree`], [`VpTree::new`] panics on non-metric input and
//! [`VpTree::try_new`] returns a [`NotAMetricError`] instead.
//!
//! # Source
//!
//! P. N. Yianilos, *Data structures and algorithms for nearest neighbor
//! search in general metric spaces*, Proceedings of the fourth annual
//! ACM-SIAM Symposium on Discrete algorithms (SODA '93), pp. 311-321.
//!
//! [`BkTree`]: crate::bk_tree::BkTree

use alloc::boxed::Box;
use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::ops::Add;

use comparand_core::DistanceMetric;

use crate::error::NotAMetricError;

/// A single VP-tree node: a vantage-point item plus an optional threshold
/// and two child subtrees.
///
/// The threshold is `None` until the second item is inserted into this
/// subtree — at that point we finally have a distance to partition on and
/// the node picks up its `Some(t)` split.
#[derive(Debug)]
struct Node<T, D: Ord> {
    /// The vantage-point item stored at this node.
    vantage: Vec<T>,
    /// Threshold distance separating inside from outside; `None` until this
    /// node acquires its first child.
    threshold: Option<D>,
    /// Subtree of items at distance `≤ threshold` from `vantage`.
    inside: Option<Box<Node<T, D>>>,
    /// Subtree of items at distance `> threshold` from `vantage`.
    outside: Option<Box<Node<T, D>>>,
}

impl<T, D: Ord> Node<T, D> {
    #[inline]
    fn new(vantage: Vec<T>) -> Self {
        Self {
            vantage,
            threshold: None,
            inside: None,
            outside: None,
        }
    }
}

/// A Vantage-Point tree indexing items compared by a true metric `M`.
///
/// See the [module-level docs][crate::vp_tree] for the tree's structure and
/// the pruning derivations. Both range and k-NN queries are supported.
///
/// # Type parameters
///
/// * `T` — the symbol type; items are stored as owned `Vec<T>`.
/// * `M` — a distance metric operating on `[T]`. Must be a true metric
///   (`properties().is_metric() == true`); non-metric input is rejected at
///   construction time.
#[derive(Debug)]
pub struct VpTree<T, M: DistanceMetric<[T]>>
where
    M::Output: Ord,
{
    /// The metric used to compare items.
    metric: M,
    /// The root node, if any items have been inserted.
    root: Option<Node<T, M::Output>>,
    /// Total number of stored items.
    len: usize,
}

impl<T, M> VpTree<T, M>
where
    T: Clone,
    M: DistanceMetric<[T]>,
    M::Output: Ord + Copy,
{
    /// Builds an empty VP-tree wrapping the supplied metric.
    ///
    /// # Panics
    ///
    /// Panics if `metric.properties()` does not satisfy
    /// [`MetricProperties::is_metric()`]. VP-tree pruning depends on the
    /// triangle inequality; using a semimetric would produce incorrect
    /// results. Use [`VpTree::try_new`] for a fallible version.
    ///
    /// [`MetricProperties::is_metric()`]: comparand_core::MetricProperties::is_metric
    #[must_use]
    pub fn new(metric: M) -> Self {
        match Self::try_new(metric) {
            Ok(t) => t,
            Err(e) => panic!("VpTree requires a true metric: {e}"),
        }
    }

    /// Builds an empty VP-tree wrapping the supplied metric, returning
    /// [`NotAMetricError`] if the metric is not a true metric.
    ///
    /// # Errors
    ///
    /// Returns [`NotAMetricError`] carrying the observed
    /// [`MetricProperties`] when `metric.properties().is_metric()` is
    /// `false`.
    ///
    /// [`MetricProperties`]: comparand_core::MetricProperties
    pub fn try_new(metric: M) -> Result<Self, NotAMetricError> {
        let props = metric.properties();
        if props.is_metric() {
            Ok(Self {
                metric,
                root: None,
                len: 0,
            })
        } else {
            Err(NotAMetricError::new(props))
        }
    }

    /// Returns the number of items stored in the tree.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if no items are stored.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns a reference to the wrapped metric.
    #[inline]
    pub fn metric(&self) -> &M {
        &self.metric
    }

    /// Inserts `item` into the tree.
    ///
    /// This is a naive incremental construction: the descent follows the
    /// current threshold at each node without rebalancing. Adversarial
    /// insertion orders can produce a degenerate path; the correctness of
    /// range and k-NN queries is unaffected but the query cost approaches
    /// `O(n)`.
    pub fn insert(&mut self, item: Vec<T>) {
        let Some(root) = self.root.as_mut() else {
            self.root = Some(Node::new(item));
            self.len = 1;
            return;
        };
        insert_into(&self.metric, root, item);
        self.len += 1;
    }

    /// Returns every stored item within `radius` of `query`, along with its
    /// distance.
    ///
    /// The result is unordered; sort at the call site if a ranking is
    /// required.
    #[must_use]
    pub fn find_within(&self, query: &[T], radius: M::Output) -> Vec<(Vec<T>, M::Output)>
    where
        M::Output: Add<Output = M::Output>,
    {
        let mut out = Vec::new();
        if let Some(root) = self.root.as_ref() {
            find_within_at(&self.metric, root, query, radius, &mut out);
        }
        out
    }

    /// Returns the `k` items nearest to `query`, ordered from nearest to
    /// farthest.
    ///
    /// If the tree holds fewer than `k` items the entire tree is returned
    /// (still sorted). Ties are broken in insertion order.
    #[must_use]
    pub fn find_k_nearest(&self, query: &[T], k: usize) -> Vec<(Vec<T>, M::Output)>
    where
        M::Output: Add<Output = M::Output>,
    {
        if k == 0 {
            return Vec::new();
        }
        let mut heap: BinaryHeap<HeapEntry<T, M::Output>> = BinaryHeap::with_capacity(k);
        if let Some(root) = self.root.as_ref() {
            k_nearest_at(&self.metric, root, query, k, &mut heap);
        }
        // Convert to a sorted-ascending Vec.
        let mut collected: Vec<(Vec<T>, M::Output)> = heap
            .into_sorted_vec()
            .into_iter()
            .map(|e| (e.item, e.distance))
            .collect();
        // `into_sorted_vec` sorts by the type's `Ord`, which for `HeapEntry`
        // is ascending distance (see the impl below), so no extra sort is
        // needed.
        // Truncate defensively; `into_sorted_vec` already respects heap
        // size.
        collected.truncate(k);
        collected
    }
}

/// Entry in the k-NN max-heap.
///
/// [`BinaryHeap`] is a *max*-heap; we want the largest current distance to
/// be at the top so we can pop it when we admit a closer candidate. The
/// [`Ord`] impl below therefore orders entries by distance in the usual
/// direction (ascending), which makes [`BinaryHeap::peek`] return the
/// *farthest* current candidate. `into_sorted_vec` then produces the
/// results in ascending order — which is exactly what we hand back to the
/// caller.
#[derive(Debug, Clone)]
struct HeapEntry<T, D> {
    /// Distance from the query to `item`.
    distance: D,
    /// The stored item.
    item: Vec<T>,
}

impl<T, D: PartialEq> PartialEq for HeapEntry<T, D> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}
impl<T, D: Eq> Eq for HeapEntry<T, D> {}

impl<T, D: Ord> PartialOrd for HeapEntry<T, D> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T, D: Ord> Ord for HeapEntry<T, D> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Ascending order on distance — the max-heap top is the farthest
        // candidate.
        self.distance.cmp(&other.distance)
    }
}

fn insert_into<T, M>(metric: &M, node: &mut Node<T, M::Output>, item: Vec<T>)
where
    T: Clone,
    M: DistanceMetric<[T]>,
    M::Output: Ord + Copy,
{
    let d = metric.distance(&node.vantage, &item).into_inner();
    match node.threshold {
        None => {
            // First child: set the split at this distance and place the
            // new item inside.
            node.threshold = Some(d);
            node.inside = Some(Box::new(Node::new(item)));
        }
        Some(t) => {
            if d <= t {
                match node.inside.as_mut() {
                    Some(child) => insert_into(metric, child, item),
                    None => node.inside = Some(Box::new(Node::new(item))),
                }
            } else {
                match node.outside.as_mut() {
                    Some(child) => insert_into(metric, child, item),
                    None => node.outside = Some(Box::new(Node::new(item))),
                }
            }
        }
    }
}

fn find_within_at<T, M>(
    metric: &M,
    node: &Node<T, M::Output>,
    query: &[T],
    radius: M::Output,
    out: &mut Vec<(Vec<T>, M::Output)>,
) where
    T: Clone,
    M: DistanceMetric<[T]>,
    M::Output: Ord + Copy + Add<Output = M::Output>,
{
    let d = metric.distance(&node.vantage, query).into_inner();
    if d <= radius {
        out.push((node.vantage.clone(), d));
    }
    let Some(t) = node.threshold else {
        return;
    };
    // Inside subtree: search if d ≤ t + r.
    if let Some(inside) = node.inside.as_ref()
        && d <= t + radius
    {
        find_within_at(metric, inside, query, radius, out);
    }
    // Outside subtree: search if d + r ≥ t.
    if let Some(outside) = node.outside.as_ref()
        && d + radius >= t
    {
        find_within_at(metric, outside, query, radius, out);
    }
}

fn k_nearest_at<T, M>(
    metric: &M,
    node: &Node<T, M::Output>,
    query: &[T],
    k: usize,
    heap: &mut BinaryHeap<HeapEntry<T, M::Output>>,
) where
    T: Clone,
    M: DistanceMetric<[T]>,
    M::Output: Ord + Copy + Add<Output = M::Output>,
{
    let d = metric.distance(&node.vantage, query).into_inner();
    admit_candidate(heap, k, &node.vantage, d);

    let Some(t) = node.threshold else {
        return;
    };

    // Visit closer subtree first — this tightens the cutoff earlier and
    // prunes the far side more aggressively.
    let inside_first = d <= t;
    let (first, second) = if inside_first {
        (node.inside.as_ref(), node.outside.as_ref())
    } else {
        (node.outside.as_ref(), node.inside.as_ref())
    };

    if let Some(child) = first {
        if child_may_contain_better(heap, k, inside_first, d, t) {
            k_nearest_at(metric, child, query, k, heap);
        }
    }
    if let Some(child) = second {
        if child_may_contain_better(heap, k, !inside_first, d, t) {
            k_nearest_at(metric, child, query, k, heap);
        }
    }
}

/// Admit `(item, distance)` into the top-k heap.
///
/// If the heap is not yet full, insert. Otherwise insert only if the
/// candidate strictly improves on the current farthest entry; pop the old
/// farthest first in that case.
fn admit_candidate<T, D>(heap: &mut BinaryHeap<HeapEntry<T, D>>, k: usize, item: &[T], distance: D)
where
    T: Clone,
    D: Ord + Copy,
{
    let entry = HeapEntry {
        distance,
        item: item.to_vec(),
    };
    if heap.len() < k {
        heap.push(entry);
    } else if let Some(top) = heap.peek()
        && distance < top.distance
    {
        heap.pop();
        heap.push(entry);
    }
}

/// Pruning test for the k-NN search: can this subtree possibly hold an item
/// nearer than the current cutoff radius?
///
/// The cutoff radius is the current heap's top distance if the heap is
/// full, otherwise infinity (represented by "always true"). The pruning
/// bounds are the same as in `find_within_at`, applied with the current
/// cutoff as the radius.
fn child_may_contain_better<T, D>(
    heap: &BinaryHeap<HeapEntry<T, D>>,
    k: usize,
    is_inside: bool,
    d: D,
    t: D,
) -> bool
where
    T: Clone,
    D: Ord + Copy + Add<Output = D>,
{
    if heap.len() < k {
        return true; // radius is effectively +∞
    }
    let cutoff = heap.peek().expect("heap non-empty when len >= k").distance;
    if is_inside {
        d <= t + cutoff
    } else {
        d + cutoff >= t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use comparand_core::{MetricClass, MetricProperties};

    #[derive(Copy, Clone, Debug, Default)]
    struct EqLenHamming;

    impl DistanceMetric<[u8]> for EqLenHamming {
        type Output = u32;

        fn distance(&self, left: &[u8], right: &[u8]) -> comparand_core::Distance<u32> {
            assert_eq!(left.len(), right.len());
            let d = left
                .iter()
                .zip(right.iter())
                .filter(|(a, b)| a != b)
                .count();
            comparand_core::Distance::new(u32::try_from(d).unwrap_or(u32::MAX))
        }

        fn properties(&self) -> MetricProperties {
            MetricProperties::METRIC
        }

        fn class(&self) -> MetricClass {
            MetricClass::Metric
        }
    }

    #[test]
    fn empty_tree_returns_nothing() {
        let tree: VpTree<u8, EqLenHamming> = VpTree::new(EqLenHamming);
        assert_eq!(tree.find_within(b"abc", 5), alloc::vec![]);
        assert_eq!(tree.find_k_nearest(b"abc", 3), alloc::vec![]);
    }

    #[test]
    fn range_query_matches_linear_scan() {
        let corpus: &[&[u8]] = &[b"aaaaa", b"aaaab", b"aaabb", b"aabbb", b"abbbb", b"bbbbb"];
        let mut tree = VpTree::new(EqLenHamming);
        for &s in corpus {
            tree.insert(s.to_vec());
        }
        let query: &[u8] = b"aaaaa";
        for r in 0u32..=5 {
            let mut got = tree.find_within(query, r);
            got.sort();
            let mut naive: alloc::vec::Vec<(alloc::vec::Vec<u8>, u32)> = corpus
                .iter()
                .map(|&s| {
                    let d = EqLenHamming.distance(query, s).into_inner();
                    (s.to_vec(), d)
                })
                .filter(|(_, d)| *d <= r)
                .collect();
            naive.sort();
            assert_eq!(got, naive, "range disagreed at radius {r}");
        }
    }

    #[test]
    fn k_nearest_matches_naive_top_k() {
        let corpus: &[&[u8]] = &[b"aaaaa", b"aaaab", b"aaabb", b"aabbb", b"abbbb", b"bbbbb"];
        let mut tree = VpTree::new(EqLenHamming);
        for &s in corpus {
            tree.insert(s.to_vec());
        }
        let query: &[u8] = b"aabaa";
        let mut naive: alloc::vec::Vec<(alloc::vec::Vec<u8>, u32)> = corpus
            .iter()
            .map(|&s| (s.to_vec(), EqLenHamming.distance(query, s).into_inner()))
            .collect();
        naive.sort_by_key(|(_, d)| *d);
        for k in 1..=corpus.len() {
            let got = tree.find_k_nearest(query, k);
            let got_distances: alloc::vec::Vec<u32> = got.iter().map(|(_, d)| *d).collect();
            let expected_distances: alloc::vec::Vec<u32> =
                naive.iter().take(k).map(|(_, d)| *d).collect();
            assert_eq!(got_distances, expected_distances, "k={k} distances differ");
        }
    }
}
