//! The [`GramMultiSet`] multiplicity-preserving gram representation.
//!
//! # Why multisets matter
//!
//! Jaccard over a set and Jaccard over a multiset are two different
//! measures. A caller comparing `"aaaa"` against `"aa"` gets different
//! answers under the two, and neither answer is silently substitutable
//! for the other. StringCheese exposes both representations explicitly so
//! that the caller (or the algorithm crate that eventually consumes
//! this type) commits to one.
//!
//! # Backing store
//!
//! [`GramMultiSet`] is backed by [`BTreeMap<G, u32>`]. The rationale
//! matches [`GramSet`](crate::GramSet)'s: `no_std`+`alloc` compatibility,
//! and deterministic iteration order for downstream reproducibility. The
//! count type is `u32` — enough for corpora orders of magnitude larger
//! than a single string comparison would ever require, and portable
//! across native and Wasm targets without the `usize` platform-word
//! ambiguity.
//!
//! [`BTreeMap<G, u32>`]: alloc::collections::BTreeMap

use alloc::collections::BTreeMap;

use crate::ngram::generator::NGramGenerator;

/// A gram multiset — grams paired with their occurrence counts.
///
/// Iteration is over `(&G, u32)` pairs in ascending gram order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GramMultiSet<G: Ord> {
    /// Gram-to-count map. Absent entries have implicit count `0`.
    counts: BTreeMap<G, u32>,
}

impl<G: Ord> GramMultiSet<G> {
    /// Constructs an empty gram multiset.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counts: BTreeMap::new(),
        }
    }

    /// Materializes a gram multiset from a generator over the given input.
    #[must_use]
    pub fn from_generator<Gen>(generator: &Gen, input: &Gen::Input) -> Self
    where
        Gen: NGramGenerator<Gram = G>,
    {
        let mut counts = BTreeMap::new();
        for g in generator.grams(input) {
            *counts.entry(g).or_insert(0u32) += 1;
        }
        Self { counts }
    }

    /// Returns the number of *distinct* grams in the multiset — the
    /// cardinality of the support set, not the total count.
    #[inline]
    #[must_use]
    pub fn distinct_len(&self) -> usize {
        self.counts.len()
    }

    /// Returns the total number of gram occurrences — the sum of all
    /// counts. `u64` return type keeps the sum safe from overflow even
    /// when many `u32`s add up.
    #[must_use]
    pub fn total_count(&self) -> u64 {
        self.counts.values().map(|&c| u64::from(c)).sum()
    }

    /// Returns `true` if the multiset contains no grams.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Returns the occurrence count of `gram`, or `0` if it is absent.
    #[must_use]
    pub fn count(&self, gram: &G) -> u32 {
        self.counts.get(gram).copied().unwrap_or(0)
    }

    /// Iterates the multiset in ascending gram order, yielding
    /// `(&G, u32)` pairs.
    #[must_use]
    pub fn iter(&self) -> MultiSetIter<'_, G> {
        MultiSetIter {
            inner: self.counts.iter(),
        }
    }

    /// Adds a single occurrence of `gram` to the multiset and returns the
    /// resulting count.
    pub fn add(&mut self, gram: G) -> u32 {
        let entry = self.counts.entry(gram).or_insert(0u32);
        *entry = entry.saturating_add(1);
        *entry
    }
}

impl<G: Ord + Clone> GramMultiSet<G> {
    /// Multiset intersection with per-gram `min` counts.
    ///
    /// The count of each gram in the result is the *minimum* of its
    /// counts in `self` and `other` — the canonical multiset intersection.
    #[must_use]
    pub fn intersection_min(&self, other: &Self) -> Self {
        let mut counts = BTreeMap::new();
        for (g, &a) in &self.counts {
            if let Some(&b) = other.counts.get(g) {
                counts.insert(g.clone(), a.min(b));
            }
        }
        Self { counts }
    }

    /// Multiset union with per-gram `max` counts.
    ///
    /// The count of each gram in the result is the *maximum* of its
    /// counts in `self` and `other` — the canonical multiset union.
    #[must_use]
    pub fn union_max(&self, other: &Self) -> Self {
        let mut counts = self.counts.clone();
        for (g, &b) in &other.counts {
            let e = counts.entry(g.clone()).or_insert(0u32);
            if *e < b {
                *e = b;
            }
        }
        Self { counts }
    }

    /// Multiset sum: per-gram counts added together (the "bag" sum).
    #[must_use]
    pub fn union_sum(&self, other: &Self) -> Self {
        let mut counts = self.counts.clone();
        for (g, &b) in &other.counts {
            let e = counts.entry(g.clone()).or_insert(0u32);
            *e = e.saturating_add(b);
        }
        Self { counts }
    }

    /// Multiset difference: `max(0, self[g] - other[g])` for every gram.
    /// Not symmetric.
    #[must_use]
    pub fn difference_saturating(&self, other: &Self) -> Self {
        let mut counts = BTreeMap::new();
        for (g, &a) in &self.counts {
            let b = other.counts.get(g).copied().unwrap_or(0);
            let diff = a.saturating_sub(b);
            if diff > 0 {
                counts.insert(g.clone(), diff);
            }
        }
        Self { counts }
    }
}

/// Iterator over `(&G, u32)` pairs of a [`GramMultiSet`].
///
/// Yielded in ascending gram order.
#[derive(Clone, Debug)]
pub struct MultiSetIter<'a, G: Ord> {
    /// The wrapped [`BTreeMap`] iterator; `.next()` maps its `&u32` count
    /// through a copy.
    inner: alloc::collections::btree_map::Iter<'a, G, u32>,
}

impl<'a, G: Ord> Iterator for MultiSetIter<'a, G> {
    type Item = (&'a G, u32);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(g, &c)| (g, c))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<G: Ord> ExactSizeIterator for MultiSetIter<'_, G> {}

impl<'a, G: Ord> IntoIterator for &'a GramMultiSet<G> {
    type Item = (&'a G, u32);
    type IntoIter = MultiSetIter<'a, G>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ngram::character::CharacterGrams;
    use crate::ngram::padding::PaddingPolicy;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn from_generator_preserves_counts_for_repeated_grams() {
        // "aaa" with n=2 has two bigrams — both ['a','a'].
        let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
        let ms = GramMultiSet::from_generator(&generator, &['a', 'a', 'a']);
        assert_eq!(ms.distinct_len(), 1);
        assert_eq!(ms.total_count(), 2);
        assert_eq!(ms.count(&vec!['a', 'a']), 2);
    }

    #[test]
    fn count_returns_zero_for_absent_gram() {
        let ms: GramMultiSet<Vec<char>> = GramMultiSet::new();
        assert_eq!(ms.count(&vec!['x']), 0);
    }

    #[test]
    fn intersection_min_and_union_max_match_hand_computed_multisets() {
        let mut a: GramMultiSet<char> = GramMultiSet::new();
        a.add('x');
        a.add('x');
        a.add('y');
        let mut b: GramMultiSet<char> = GramMultiSet::new();
        b.add('x');
        b.add('y');
        b.add('y');
        b.add('z');
        let inter = a.intersection_min(&b);
        assert_eq!(inter.count(&'x'), 1);
        assert_eq!(inter.count(&'y'), 1);
        assert_eq!(inter.count(&'z'), 0);
        let uni = a.union_max(&b);
        assert_eq!(uni.count(&'x'), 2);
        assert_eq!(uni.count(&'y'), 2);
        assert_eq!(uni.count(&'z'), 1);
    }

    #[test]
    fn union_sum_adds_counts() {
        let mut a: GramMultiSet<char> = GramMultiSet::new();
        a.add('x');
        a.add('x');
        let mut b: GramMultiSet<char> = GramMultiSet::new();
        b.add('x');
        b.add('y');
        let s = a.union_sum(&b);
        assert_eq!(s.count(&'x'), 3);
        assert_eq!(s.count(&'y'), 1);
    }

    #[test]
    fn difference_saturating_is_asymmetric_and_never_negative() {
        let mut a: GramMultiSet<char> = GramMultiSet::new();
        a.add('x');
        let mut b: GramMultiSet<char> = GramMultiSet::new();
        b.add('x');
        b.add('x');
        assert!(a.difference_saturating(&b).is_empty());
        assert_eq!(b.difference_saturating(&a).count(&'x'), 1);
    }

    #[test]
    fn iter_yields_ascending_order() {
        let mut ms: GramMultiSet<char> = GramMultiSet::new();
        ms.add('c');
        ms.add('a');
        ms.add('b');
        let pairs: Vec<(char, u32)> = ms.iter().map(|(g, c)| (*g, c)).collect();
        assert_eq!(pairs, vec![('a', 1), ('b', 1), ('c', 1)]);
    }
}
