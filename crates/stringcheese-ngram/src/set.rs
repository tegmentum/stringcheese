//! The [`GramSet`] deduplicated gram representation.
//!
//! # Backing store
//!
//! [`GramSet`] is backed by [`BTreeSet`] rather than a hash set. Two
//! reasons:
//!
//! * **`no_std` + `alloc` compatibility.** `BTreeSet` lives in `alloc`; a
//!   hash set requires `std`. StringCheese's target of a `no_std`-plus-`alloc`
//!   build makes this a hard constraint.
//! * **Deterministic iteration order.** Downstream `MinHash` sketching and
//!   n-gram fingerprint indexing rely on cross-machine reproducibility.
//!   `BTreeSet` gives us ordered iteration for free; a hash set would need
//!   a deterministic hasher and a sort step to match.
//!
//! A `std`-only alternative backed by `HashSet` is a possible future
//! addition when a workload can absorb the cost of a deterministic hash
//! configuration; the shapes on this type are chosen so that variant can
//! land as a sibling without breaking the existing API.
//!
//! [`BTreeSet`]: alloc::collections::BTreeSet

use alloc::collections::BTreeSet;
use core::iter::FromIterator;

use crate::generator::NGramGenerator;

/// A deduplicated set of grams.
///
/// Iteration order is deterministic and follows [`BTreeSet`]'s ordered
/// traversal.
///
/// [`BTreeSet`]: alloc::collections::BTreeSet
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GramSet<G: Ord> {
    /// The ordered, deduplicated backing store.
    grams: BTreeSet<G>,
}

impl<G: Ord> GramSet<G> {
    /// Constructs an empty gram set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            grams: BTreeSet::new(),
        }
    }

    /// Materializes a gram set from a generator over the given input.
    ///
    /// This is the canonical entry point: rather than building a set
    /// piece-by-piece, callers hand the set a generator and an input and
    /// let it drain the generator once.
    #[must_use]
    pub fn from_generator<Gen>(generator: &Gen, input: &Gen::Input) -> Self
    where
        Gen: NGramGenerator<Gram = G>,
    {
        let mut grams = BTreeSet::new();
        for g in generator.grams(input) {
            grams.insert(g);
        }
        Self { grams }
    }

    /// Returns the number of distinct grams in the set.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.grams.len()
    }

    /// Returns `true` if the set contains no grams.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.grams.is_empty()
    }

    /// Returns `true` if the set contains `gram`.
    #[inline]
    #[must_use]
    pub fn contains(&self, gram: &G) -> bool {
        self.grams.contains(gram)
    }

    /// Iterates the set in ascending order.
    pub fn iter(&self) -> alloc::collections::btree_set::Iter<'_, G> {
        self.grams.iter()
    }

    /// Inserts `gram` into the set. Returns `true` if it was newly added.
    pub fn insert(&mut self, gram: G) -> bool {
        self.grams.insert(gram)
    }
}

impl<G: Ord + Clone> GramSet<G> {
    /// Returns the intersection of `self` and `other` as a fresh
    /// [`GramSet`].
    #[must_use]
    pub fn intersection_with(&self, other: &Self) -> Self {
        Self {
            grams: self.grams.intersection(&other.grams).cloned().collect(),
        }
    }

    /// Returns the union of `self` and `other` as a fresh [`GramSet`].
    #[must_use]
    pub fn union_with(&self, other: &Self) -> Self {
        Self {
            grams: self.grams.union(&other.grams).cloned().collect(),
        }
    }

    /// Returns the set difference `self - other` as a fresh [`GramSet`].
    /// Not symmetric: `self.difference_with(other) != other.difference_with(self)`
    /// in general.
    #[must_use]
    pub fn difference_with(&self, other: &Self) -> Self {
        Self {
            grams: self.grams.difference(&other.grams).cloned().collect(),
        }
    }
}

impl<G: Ord> FromIterator<G> for GramSet<G> {
    fn from_iter<I: IntoIterator<Item = G>>(iter: I) -> Self {
        Self {
            grams: iter.into_iter().collect(),
        }
    }
}

impl<G: Ord> IntoIterator for GramSet<G> {
    type Item = G;
    type IntoIter = alloc::collections::btree_set::IntoIter<G>;

    fn into_iter(self) -> Self::IntoIter {
        self.grams.into_iter()
    }
}

impl<'a, G: Ord> IntoIterator for &'a GramSet<G> {
    type Item = &'a G;
    type IntoIter = alloc::collections::btree_set::Iter<'a, G>;

    fn into_iter(self) -> Self::IntoIter {
        self.grams.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::CharacterGrams;
    use crate::padding::PaddingPolicy;
    use alloc::vec;
    use alloc::vec::Vec;

    fn set_of<const N: usize>(items: [&[char]; N]) -> GramSet<Vec<char>> {
        items.iter().map(|s| s.to_vec()).collect()
    }

    #[test]
    fn from_generator_dedupes() {
        let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
        let s = GramSet::from_generator(&generator, &['a', 'a', 'a']);
        // Bigrams over "aaa" are two copies of ['a','a']; the set has one.
        assert_eq!(s.len(), 1);
        assert!(s.contains(&vec!['a', 'a']));
    }

    #[test]
    fn from_generator_preserves_distinct_grams() {
        let generator = CharacterGrams::new(2, PaddingPolicy::<char>::None);
        let s = GramSet::from_generator(&generator, &['c', 'a', 't']);
        assert_eq!(s.len(), 2);
        assert!(s.contains(&vec!['c', 'a']));
        assert!(s.contains(&vec!['a', 't']));
    }

    #[test]
    fn iteration_order_is_ascending() {
        // BTreeSet iteration is ascending; this guards against a refactor
        // to a container without deterministic ordering.
        let s: GramSet<Vec<char>> = set_of([&['b'], &['a'], &['c']]);
        let grams: Vec<Vec<char>> = s.iter().cloned().collect();
        assert_eq!(grams, vec![vec!['a'], vec!['b'], vec!['c']]);
    }

    #[test]
    fn intersection_and_union_agree_with_hand_computed_sets() {
        let a: GramSet<Vec<char>> = set_of([&['a'], &['b'], &['c']]);
        let b: GramSet<Vec<char>> = set_of([&['b'], &['c'], &['d']]);
        let expected_inter: GramSet<Vec<char>> = set_of([&['b'], &['c']]);
        let expected_union: GramSet<Vec<char>> = set_of([&['a'], &['b'], &['c'], &['d']]);
        assert_eq!(a.intersection_with(&b), expected_inter);
        assert_eq!(a.union_with(&b), expected_union);
    }

    #[test]
    fn difference_is_asymmetric() {
        let a: GramSet<Vec<char>> = set_of([&['a'], &['b'], &['c']]);
        let b: GramSet<Vec<char>> = set_of([&['b'], &['c'], &['d']]);
        let a_minus_b: GramSet<Vec<char>> = set_of([&['a']]);
        let b_minus_a: GramSet<Vec<char>> = set_of([&['d']]);
        assert_eq!(a.difference_with(&b), a_minus_b);
        assert_eq!(b.difference_with(&a), b_minus_a);
    }

    #[test]
    fn from_iterator_produces_deduplicated_set() {
        let s: GramSet<u8> = [1u8, 2, 2, 3, 3, 3].iter().copied().collect();
        assert_eq!(s.len(), 3);
    }
}
