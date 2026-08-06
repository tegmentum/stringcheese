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
//!   the vantage (or `≥ threshold` for trees produced by
//!   [`VpTree::from_corpus`] — the pruning derivations below cover both
//!   cases).
//!
//! [`VpTree::insert`] descends the tree, computing the distance from the new
//! item to each vantage-point along the way and following whichever side of
//! the current threshold applies, until it reaches a slot without children —
//! at which point the new item becomes the vantage of a fresh leaf and the
//! threshold at the previous level is set to the distance we just computed.
//! This is a naive incremental construction: it is correct but does not
//! attempt to rebalance, so on adversarial insertion orders the tree may
//! degenerate into a path.
//!
//! [`VpTree::from_corpus`] (and its fallible cousin
//! [`VpTree::try_from_corpus`]) build a balanced tree from a full corpus in
//! one shot. At each recursive level the constructor picks the first item
//! as the vantage, computes its distance to every other item at that
//! level, partitions the items *by position* around the median (found in
//! expected linear time via [`slice::select_nth_unstable_by_key`]), and
//! recurses on each half. Because the split is by position rather than by
//! strict threshold, ties at the median do not unbalance the tree — inside
//! always receives exactly `n/2` items — and the depth stays `O(log n)`
//! even on highly repetitive corpora. The tradeoff is a slightly relaxed
//! invariant that outside items satisfy `d(x, v) ≥ threshold` (instead of
//! the strict `>` that incremental insertion produces); the pruning
//! derivations below use `≥` on the outside side either way, so the
//! [`VpTree::find_within`] and [`VpTree::find_k_nearest`] traversal is
//! unchanged and both construction strategies return the same result
//! sets.
//!
//! [`slice::select_nth_unstable_by_key`]: https://doc.rust-lang.org/std/primitive.slice.html#method.select_nth_unstable_by_key
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
//! # References
//!
//! * Yianilos, P. N. (1993). "Data structures and algorithms for nearest
//!   neighbor search in general metric spaces." *Proceedings of the fourth
//!   annual ACM-SIAM symposium on Discrete algorithms (SODA '93)*, 311-321.
//!   <https://dl.acm.org/doi/10.5555/313559.313789>
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

    /// Returns the maximum depth of the tree, in nodes, counting the root
    /// itself as depth `1`. An empty tree has depth `0`. Intended for tests
    /// and diagnostics that want to observe how balanced the constructed
    /// tree is.
    #[doc(hidden)]
    #[must_use]
    pub fn max_depth(&self) -> usize {
        fn depth<T, D: Ord>(node: &Node<T, D>) -> usize {
            let l = node.inside.as_deref().map_or(0, depth);
            let r = node.outside.as_deref().map_or(0, depth);
            1 + l.max(r)
        }
        self.root.as_ref().map_or(0, depth)
    }

    /// Builds a balanced VP-tree from a corpus in one shot.
    ///
    /// At each recursive level the first item is chosen as the vantage,
    /// distances from the vantage to every other item at that level are
    /// computed, and the *median* of those distances is picked as the
    /// threshold via [`slice::select_nth_unstable_by_key`] (expected linear
    /// time). Items with distance `≤ threshold` recurse into the inside
    /// subtree; items with distance `> threshold` recurse into the outside
    /// subtree — the same partition invariant that
    /// [`VpTree::insert`] maintains, so [`VpTree::find_within`] and
    /// [`VpTree::find_k_nearest`] work identically against both.
    ///
    /// Query results are guaranteed to match the incrementally-built tree's
    /// results as sets. Physical order of items in each result is not
    /// stable across construction strategies — sort at the call site if a
    /// specific ordering is required.
    ///
    /// # Vantage-point selection
    ///
    /// The constructor picks the *first* remaining item at each level as the
    /// vantage. This is the simplest correct choice; more sophisticated
    /// strategies — random sampling, farthest-from-parent — are documented
    /// in the literature (Yianilos '93) and can improve pruning in
    /// clustered data, but are not implemented here.
    ///
    /// # Complexity
    ///
    /// * *Time.* `O(n log² n)` on average: at each of the `O(log n)`
    ///   recursion levels the constructor computes `O(n)` distances and
    ///   partitions in expected `O(n)`; the `log n` factor comes from the
    ///   recursion depth on a balanced tree.
    /// * *Space.* `O(n)` for the tree plus `O(n)` transient allocations for
    ///   the per-level distance buffer.
    ///
    /// # Balance and pathological inputs
    ///
    /// Ties at the median (multiple items sharing the median distance from
    /// the vantage) all land on the inside side to preserve the `d ≤ t`
    /// invariant. On a corpus where every item has the same distance from
    /// every vantage the tree still degenerates into a path — correctness
    /// is unaffected, but average-depth guarantees are lost.
    ///
    /// # Panics
    ///
    /// Panics if `metric.properties()` does not satisfy
    /// [`MetricProperties::is_metric()`]. Use
    /// [`VpTree::try_from_corpus`] for a fallible version.
    ///
    /// [`MetricProperties::is_metric()`]: comparand_core::MetricProperties::is_metric
    /// [`slice::select_nth_unstable_by_key`]: https://doc.rust-lang.org/std/primitive.slice.html#method.select_nth_unstable_by_key
    #[must_use]
    pub fn from_corpus(metric: M, items: Vec<Vec<T>>) -> Self {
        match Self::try_from_corpus(metric, items) {
            Ok(t) => t,
            Err(e) => panic!("VpTree requires a true metric: {e}"),
        }
    }

    /// Fallible sibling of [`VpTree::from_corpus`].
    ///
    /// # Errors
    ///
    /// Returns [`NotAMetricError`] carrying the observed
    /// [`MetricProperties`] when `metric.properties().is_metric()` is
    /// `false`. The corpus is not consumed on error.
    ///
    /// [`MetricProperties`]: comparand_core::MetricProperties
    pub fn try_from_corpus(metric: M, items: Vec<Vec<T>>) -> Result<Self, NotAMetricError> {
        let props = metric.properties();
        if !props.is_metric() {
            return Err(NotAMetricError::new(props));
        }
        let len = items.len();
        let root = build_bulk(&metric, items);
        Ok(Self { metric, root, len })
    }

    /// Inserts `item` into the tree.
    ///
    /// This is a naive incremental construction: the descent follows the
    /// current threshold at each node without rebalancing. Adversarial
    /// insertion orders can produce a degenerate path; the correctness of
    /// range and k-NN queries is unaffected but the query cost approaches
    /// `O(n)`. For balanced construction from a known corpus use
    /// [`VpTree::from_corpus`].
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

/// Recursive median-partition builder backing [`VpTree::from_corpus`].
///
/// Picks the first remaining item as the vantage, computes distances from
/// it to every other item, then splits *by position* around the median via
/// [`slice::select_nth_unstable_by_key`] (expected linear time). Items at
/// sorted positions `[0, mid)` recurse into the inside subtree; items at
/// sorted positions `[mid, n)` recurse into the outside subtree.
///
/// # Splitting by position, not by threshold
///
/// The naive alternative — pick `threshold = median` and route items by
/// `d ≤ threshold` vs `d > threshold` — is unbalanced whenever multiple
/// items share the median distance from the vantage, because every tied
/// item is forced onto one side to preserve the strict invariant. Small
/// alphabets on integer metrics produce exactly this pathology.
///
/// Splitting by position instead lets ties fall across the boundary
/// naturally: inside always gets exactly `mid = n/2` items and outside
/// gets the rest, regardless of how many ties sit at the median. The
/// stored `threshold` is set to `annotated[mid].distance` (the smallest
/// distance in the outside partition), which
/// [`slice::select_nth_unstable_by_key`] guarantees is `≥` every distance
/// in the inside partition. The relaxed invariant this preserves is
///
/// * inside items have `d(x, v) ≤ threshold`;
/// * outside items have `d(x, v) ≥ threshold`;
///
/// which is exactly what [`find_within_at`] and [`k_nearest_at`] need for
/// their pruning bounds — the derivations in the module docs work
/// unchanged with `≥` in place of `>` on the outside side.
///
/// [`slice::select_nth_unstable_by_key`]: https://doc.rust-lang.org/std/primitive.slice.html#method.select_nth_unstable_by_key
fn build_bulk<T, M>(metric: &M, items: Vec<Vec<T>>) -> Option<Node<T, M::Output>>
where
    T: Clone,
    M: DistanceMetric<[T]>,
    M::Output: Ord + Copy,
{
    if items.is_empty() {
        return None;
    }
    let mut iter = items.into_iter();
    // Safe: we just checked non-emptiness above.
    let vantage = iter.next().expect("non-empty by prior check");
    let rest: Vec<Vec<T>> = iter.collect();
    if rest.is_empty() {
        return Some(Node::new(vantage));
    }

    // Annotate every remaining item with its distance from the vantage.
    let mut annotated: Vec<(Vec<T>, M::Output)> = rest
        .into_iter()
        .map(|item| {
            let d = metric.distance(&vantage, &item).into_inner();
            (item, d)
        })
        .collect();

    // Partition around the mid position via select_nth (expected O(n)).
    // Post-condition: annotated[..mid] all have distance ≤ annotated[mid],
    // and annotated[mid+1..] all have distance ≥ annotated[mid].
    let n = annotated.len();
    let mid = n / 2;
    if mid == 0 {
        // n == 1: only one non-vantage item. Put it in the outside
        // subtree, using its own distance as the threshold. Placement
        // side is arbitrary from a correctness standpoint — pruning
        // works either way — outside is used purely for a consistent
        // choice.
        let (only_item, only_d) = annotated.pop().expect("n == 1");
        return Some(Node {
            vantage,
            threshold: Some(only_d),
            inside: None,
            outside: Some(Box::new(Node::new(only_item))),
        });
    }
    annotated.select_nth_unstable_by_key(mid, |(_, d)| *d);
    // `annotated[mid].1` is the smallest distance in the outside
    // partition and an upper bound on every distance in the inside
    // partition — exactly the threshold we want to store.
    let threshold = annotated[mid].1;

    // Split at position `mid`. `Vec::split_off(mid)` moves positions
    // [mid, n) into a new vector and leaves [0, mid) in the original;
    // this is O(n - mid) and needs no temporary allocation for the tail.
    let outside_items: Vec<Vec<T>> = annotated
        .split_off(mid)
        .into_iter()
        .map(|(item, _)| item)
        .collect();
    let inside_items: Vec<Vec<T>> = annotated.into_iter().map(|(item, _)| item).collect();

    let inside = build_bulk(metric, inside_items).map(Box::new);
    let outside = build_bulk(metric, outside_items).map(Box::new);

    Some(Node {
        vantage,
        threshold: Some(threshold),
        inside,
        outside,
    })
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
