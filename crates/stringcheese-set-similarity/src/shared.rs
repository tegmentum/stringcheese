//! Shared helpers used by more than one similarity kernel.
//!
//! The two helpers here — [`set_intersection_size`] and
//! [`multiset_min_intersection`] — appear in every kernel that needs a
//! cardinality of the intersection. Both are written to avoid materializing
//! a fresh intersection collection: they walk the smaller side and probe
//! the larger, so the temporary state is a single running counter.

use stringcheese_ngram::{GramMultiSet, GramSet};

/// Returns `|A ∩ B|` for two [`GramSet`]s without allocating.
///
/// Iterates the smaller side and probes the larger with `.contains()`.
/// Runtime is `O(min(|A|, |B|) * log(max(|A|, |B|)))` — the log factor is
/// the [`GramSet`]'s underlying `BTreeSet` lookup. An in-parallel
/// merge over the two ordered iterators would drop the log, but the
/// simpler probe form is easier to audit and fast enough for realistic
/// gram-set sizes.
#[must_use]
pub(crate) fn set_intersection_size<G: Ord>(a: &GramSet<G>, b: &GramSet<G>) -> usize {
    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    small.iter().filter(|g| large.contains(*g)).count()
}

/// Returns `Σ min(a[g], b[g])` — the multiset intersection cardinality —
/// as a `u64` to keep the sum safe from `u32` overflow across a large
/// support set.
///
/// Iterates the multiset with fewer distinct grams and probes the other.
/// Undefined grams contribute zero and are naturally skipped.
#[must_use]
pub(crate) fn multiset_min_intersection<G: Ord>(a: &GramMultiSet<G>, b: &GramMultiSet<G>) -> u64 {
    let (small, large) = if a.distinct_len() <= b.distinct_len() {
        (a, b)
    } else {
        (b, a)
    };
    small
        .iter()
        .map(|(g, ca)| {
            let cb = large.count(g);
            u64::from(ca.min(cb))
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn set_of<const N: usize>(items: [char; N]) -> GramSet<Vec<char>> {
        items.iter().map(|c| vec![*c]).collect()
    }

    #[test]
    fn set_intersection_size_matches_hand_computation() {
        let a: GramSet<Vec<char>> = set_of(['a', 'b', 'c']);
        let b: GramSet<Vec<char>> = set_of(['b', 'c', 'd']);
        assert_eq!(set_intersection_size(&a, &b), 2);
        assert_eq!(set_intersection_size(&b, &a), 2);
    }

    #[test]
    fn set_intersection_size_of_empty_is_zero() {
        let a: GramSet<Vec<char>> = GramSet::new();
        let b: GramSet<Vec<char>> = set_of(['a']);
        assert_eq!(set_intersection_size(&a, &b), 0);
        assert_eq!(set_intersection_size(&b, &a), 0);
    }

    #[test]
    fn multiset_min_intersection_uses_per_gram_min() {
        let mut a: GramMultiSet<char> = GramMultiSet::new();
        a.add('x');
        a.add('x');
        a.add('y');
        let mut b: GramMultiSet<char> = GramMultiSet::new();
        b.add('x');
        b.add('y');
        b.add('y');
        b.add('z');
        // per-gram mins: x=1, y=1, z=0 → sum = 2
        assert_eq!(multiset_min_intersection(&a, &b), 2);
        assert_eq!(multiset_min_intersection(&b, &a), 2);
    }

    #[test]
    fn multiset_min_intersection_disjoint_is_zero() {
        let mut a: GramMultiSet<char> = GramMultiSet::new();
        a.add('a');
        let mut b: GramMultiSet<char> = GramMultiSet::new();
        b.add('b');
        assert_eq!(multiset_min_intersection(&a, &b), 0);
    }
}
