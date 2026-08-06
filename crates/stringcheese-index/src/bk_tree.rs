//! The [`BkTree`] — Burkhard-Keller tree for metric-space range queries.
//!
//! # What it is
//!
//! A BK-tree is a metric-space index optimized for *range* queries: given
//! a query `q` and radius `r`, return every stored item whose distance to
//! `q` is at most `r`. Insertion is `O(log n)` on average; a range query
//! visits `O(n)` nodes in the worst case but far fewer when the radius is
//! small relative to the tree's diameter.
//!
//! # How the tree is organized
//!
//! Every node stores one item plus a map from *distance* to child subtree.
//! When a new item is inserted, we descend from the root: at each visited
//! node `n` we compute `d = metric.distance(item, n.item)`; if `n` has no
//! child indexed by `d`, we attach the new item there, otherwise we recurse
//! into that child. All items along any root-to-leaf path therefore satisfy
//! the invariant *the label on the edge to a child equals the distance from
//! the parent's item to every item in that child's subtree's root at least
//! for the root, and each subsequent edge is defined the same way*.
//!
//! Because Burkhard and Keller's original 1973 paper uses *integer* edge
//! labels, [`BkTree`] does the same: children of any node are keyed by
//! whatever `M::Output` value the wrapped metric returns, held in an
//! ordered map. Duplicate insertions (two items at distance zero) collide
//! at the same key and are threaded further down the tree, so identity of
//! indiscernibles is required only up to the metric's own definition of
//! equality.
//!
//! # How range queries prune
//!
//! Let `q` be the query and `r` the radius. At a node `n`:
//!
//! * Compute `d = metric.distance(q, n.item)`. If `d ≤ r`, include `n.item`
//!   in the result.
//! * For every child `c` attached with edge label `k`, the triangle
//!   inequality guarantees that for every item `x` in `c`'s subtree,
//!   `metric.distance(q, n.item) ≤ metric.distance(q, x) + metric.distance(x, n.item)`,
//!   which rearranges to
//!   `metric.distance(q, x) ≥ metric.distance(q, n.item) − metric.distance(x, n.item)`.
//!   Because every `x` in `c`'s subtree has `metric.distance(x, n.item) = k`,
//!   any candidate `x` with distance to `q` at most `r` must satisfy
//!   `|d − k| ≤ r`. Equivalently `d ≤ k + r` **and** `k ≤ d + r`. Children
//!   for which either check fails cannot contain an answer and are pruned.
//!
//! # Why the wrapped metric must be a true metric
//!
//! The pruning derivation above uses the triangle inequality. A semimetric —
//! for example OSA per `stringcheese-damerau` — does not satisfy it, and the
//! pruning will silently discard subtrees whose items are actually within
//! the requested radius. Constructing a BK-tree over a non-metric produces
//! a container whose answers are unrelated to the naive linear scan; there
//! is no way to make that safe short of refusing the construction.
//!
//! [`BkTree::new`] therefore panics on non-metric input. [`BkTree::try_new`]
//! returns a [`NotAMetricError`] instead, for callers that assemble a
//! metric dynamically and prefer to handle the rejection explicitly.
//!
//! # References
//!
//! * Burkhard, W. A., & Keller, R. M. (1973). "Some approaches to
//!   best-match file searching." *Communications of the ACM*, 16(4),
//!   230-236. <https://doi.org/10.1145/362003.362025>
//! * Baeza-Yates, R., & Ribeiro-Neto, B. (2011). *Modern Information
//!   Retrieval: The Concepts and Technology behind Search* (2nd ed.).
//!   Addison-Wesley. ISBN 978-0-321-41691-9 — chapter on similarity search
//!   for a modern textbook treatment.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::ops::Add;

use stringcheese_core::DistanceMetric;

use crate::error::NotAMetricError;

/// A single BK-tree node: one stored item plus a distance-keyed map of
/// children.
///
/// The map is a [`BTreeMap`] so iteration order is deterministic and range
/// operations are natural — the pruning step only needs to visit keys inside
/// the closed interval `[max(0, d − r), d + r]`, but for simplicity we scan
/// all children and let the range check reject the rest. The number of
/// distinct distances at any one node is bounded by the metric's range for
/// items in that subtree, which is typically small.
#[derive(Debug)]
struct Node<T, D: Ord> {
    /// The item stored at this node.
    item: Vec<T>,
    /// Children keyed by their distance to `item`.
    children: BTreeMap<D, Box<Node<T, D>>>,
}

impl<T, D: Ord> Node<T, D> {
    #[inline]
    fn new(item: Vec<T>) -> Self {
        Self {
            item,
            children: BTreeMap::new(),
        }
    }
}

/// A Burkhard-Keller tree indexing items compared by a true metric `M`.
///
/// See the [module-level docs][crate::bk_tree] for the tree's structure and
/// the pruning derivation.
///
/// # Type parameters
///
/// * `T` — the symbol type; items are stored as owned `Vec<T>`.
/// * `M` — a distance metric operating on `[T]`. Must be a true metric
///   (`properties().is_metric() == true`); non-metric input is rejected at
///   construction time.
#[derive(Debug)]
pub struct BkTree<T, M: DistanceMetric<[T]>>
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

impl<T, M> BkTree<T, M>
where
    T: Clone,
    M: DistanceMetric<[T]>,
    M::Output: Ord + Copy,
{
    /// Builds an empty BK-tree wrapping the supplied metric.
    ///
    /// # Panics
    ///
    /// Panics if `metric.properties()` does not satisfy
    /// [`MetricProperties::is_metric()`]. BK-tree correctness depends on the
    /// triangle inequality; using a semimetric would produce incorrect
    /// results. Use [`BkTree::try_new`] for a fallible version.
    ///
    /// [`MetricProperties::is_metric()`]: stringcheese_core::MetricProperties::is_metric
    #[must_use]
    pub fn new(metric: M) -> Self {
        match Self::try_new(metric) {
            Ok(t) => t,
            Err(e) => panic!("BkTree requires a true metric: {e}"),
        }
    }

    /// Builds an empty BK-tree wrapping the supplied metric, returning
    /// [`NotAMetricError`] if the metric is not a true metric.
    ///
    /// # Errors
    ///
    /// Returns [`NotAMetricError`] carrying the observed
    /// [`MetricProperties`] when `metric.properties().is_metric()` is
    /// `false` — most commonly for semimetrics (missing triangle
    /// inequality) or quasimetrics (missing symmetry).
    ///
    /// [`MetricProperties`]: stringcheese_core::MetricProperties
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
    /// If the tree already contains an item at distance zero from `item`
    /// (under the metric's notion of equality), the new item is nested
    /// deeper along the equal-distance child chain rather than being
    /// deduplicated. Callers who want set semantics must dedupe upstream.
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
    /// The result is unordered — callers who need it sorted by distance can
    /// sort the returned vector; the tree makes no ordering promise so it
    /// can prune subtrees in whichever order is cheapest. Distances are
    /// exactly the values the wrapped metric returned during traversal, so
    /// no re-computation is required at the call site.
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
}

/// Descend from `node`, attaching `item` at the first empty distance slot.
fn insert_into<T, M>(metric: &M, node: &mut Node<T, M::Output>, item: Vec<T>)
where
    T: Clone,
    M: DistanceMetric<[T]>,
    M::Output: Ord + Copy,
{
    let d = metric.distance(&node.item, &item).into_inner();
    if let Some(child) = node.children.get_mut(&d) {
        insert_into(metric, child, item);
    } else {
        node.children.insert(d, Box::new(Node::new(item)));
    }
}

/// Recursive range query.
///
/// The pruning condition is `|d − k| ≤ r`, expressed without subtraction
/// (to avoid underflow on unsigned distance types) as `d ≤ k + r` **and**
/// `k ≤ d + r`.
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
    let d = metric.distance(&node.item, query).into_inner();
    if d <= radius {
        out.push((node.item.clone(), d));
    }
    for (&k, child) in &node.children {
        // d ≤ k + r  &&  k ≤ d + r
        if d <= k + radius && k <= d + radius {
            find_within_at(metric, child, query, radius, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_core::{MetricClass, MetricProperties};

    // A tiny hand-rolled Hamming-on-equal-length metric — a true metric on
    // fixed-length inputs, used here to exercise `BkTree` without pulling
    // in a dev-dep just for a unit test that lives inside the crate.
    #[derive(Copy, Clone, Debug, Default)]
    struct EqLenHamming;

    impl DistanceMetric<[u8]> for EqLenHamming {
        type Output = u32;

        fn distance(&self, left: &[u8], right: &[u8]) -> stringcheese_core::Distance<u32> {
            // Same-length by construction in the tests below. Real code
            // would need a fallible variant; we take the simplest thing
            // here.
            assert_eq!(left.len(), right.len());
            let d = left
                .iter()
                .zip(right.iter())
                .filter(|(a, b)| a != b)
                .count();
            stringcheese_core::Distance::new(u32::try_from(d).unwrap_or(u32::MAX))
        }

        fn properties(&self) -> MetricProperties {
            MetricProperties::METRIC
        }

        fn class(&self) -> MetricClass {
            MetricClass::Metric
        }
    }

    #[test]
    fn empty_tree_has_no_matches() {
        let tree: BkTree<u8, EqLenHamming> = BkTree::new(EqLenHamming);
        assert!(tree.is_empty());
        assert_eq!(tree.find_within(b"abc", 5), alloc::vec![]);
    }

    #[test]
    fn single_item_within_radius() {
        let mut tree = BkTree::new(EqLenHamming);
        tree.insert(b"abc".to_vec());
        let hits = tree.find_within(b"abc", 0);
        assert_eq!(hits, alloc::vec![(b"abc".to_vec(), 0u32)]);
    }

    #[test]
    fn triangle_pruning_still_returns_all_matches() {
        // Small hand-picked corpus with easy-to-verify distances.
        let corpus: &[&[u8]] = &[b"aaaaa", b"aaaab", b"aaabb", b"aabbb", b"abbbb", b"bbbbb"];
        let mut tree = BkTree::new(EqLenHamming);
        for &s in corpus {
            tree.insert(s.to_vec());
        }
        // Radius 1 from "aaaab": items whose Hamming distance to
        // "aaaab" is at most 1. Those are "aaaab" itself (0), "aaaaa"
        // (1), and "aaabb" (1).
        let mut hits = tree.find_within(b"aaaab", 1);
        hits.sort();
        let mut expected: alloc::vec::Vec<(alloc::vec::Vec<u8>, u32)> = alloc::vec![
            (b"aaaab".to_vec(), 0),
            (b"aaaaa".to_vec(), 1),
            (b"aaabb".to_vec(), 1),
        ];
        expected.sort();
        assert_eq!(hits, expected);
    }
}
