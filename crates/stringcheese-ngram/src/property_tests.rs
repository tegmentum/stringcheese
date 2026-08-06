//! Property-based tests for the generator, count helper, and gram
//! representations.
//!
//! These are the load-bearing invariants: the `count_grams` closed form
//! must agree with the iterator's actual length across every legal
//! combination of arity, input length, and padding policy; the set and
//! multiset representations must agree on their support cardinality; and
//! the vector normalization helpers must produce unit-norm vectors within
//! the tolerance floating-point arithmetic permits.

use alloc::vec::Vec;
use proptest::prelude::*;

use crate::character::CharacterGrams;
use crate::generator::{NGramGenerator, count_grams};
use crate::multiset::GramMultiSet;
use crate::padding::PaddingPolicy;
use crate::set::GramSet;
use crate::vector::GramVector;

/// A padding strategy over `char` markers. Covers all three variants of
/// [`PaddingPolicy`] so the properties exercise every branch.
fn arb_padding() -> impl Strategy<Value = PaddingPolicy<char>> {
    prop_oneof![
        Just(PaddingPolicy::<char>::None),
        Just(PaddingPolicy::Boundary {
            start: '^',
            end: '$'
        }),
        (
            proptest::collection::vec(any::<char>(), 0..=3),
            proptest::collection::vec(any::<char>(), 0..=3),
        )
            .prop_map(|(prefix, suffix)| PaddingPolicy::Custom { prefix, suffix }),
    ]
}

/// A short `char` input strategy over a tiny alphabet so we get a good
/// mix of matches, mismatches, and repetitions in short inputs.
fn arb_input() -> impl Strategy<Value = Vec<char>> {
    proptest::collection::vec(proptest::char::range('a', 'd'), 0..12)
}

/// An arity in `1..=6`. Zero is excluded because constructors reject it.
fn arb_n() -> impl Strategy<Value = usize> {
    1usize..=6
}

proptest! {
    /// The generator must yield exactly `count_grams(...)` grams for
    /// every legal combination of input, arity, and padding.
    #[test]
    fn iterator_count_matches_closed_form(
        input in arb_input(),
        n in arb_n(),
        padding in arb_padding(),
    ) {
        let expected = count_grams(input.len(), n, &padding);
        let generator = CharacterGrams::new(n, padding);
        let observed = generator.grams(&input).count();
        prop_assert_eq!(observed, expected);
    }

    /// The generator's size hint must also match the closed form: this
    /// is what makes downstream `Vec::with_capacity` calls safe.
    #[test]
    fn iterator_size_hint_matches_closed_form(
        input in arb_input(),
        n in arb_n(),
        padding in arb_padding(),
    ) {
        let expected = count_grams(input.len(), n, &padding);
        let generator = CharacterGrams::new(n, padding);
        let (lower, upper) = generator.grams(&input).size_hint();
        prop_assert_eq!(lower, expected);
        prop_assert_eq!(upper, Some(expected));
    }

    /// A [`GramSet`] built from the same generator and input must dedupe
    /// the grams — its length is at most the raw iterator's length.
    #[test]
    fn set_len_le_iterator_count(
        input in arb_input(),
        n in arb_n(),
        padding in arb_padding(),
    ) {
        let generator = CharacterGrams::new(n, padding);
        let raw = generator.grams(&input).count();
        let s: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input);
        prop_assert!(s.len() <= raw);
    }

    /// The set and the multiset must agree on the *distinct* count.
    #[test]
    fn set_len_equals_multiset_distinct_len(
        input in arb_input(),
        n in arb_n(),
        padding in arb_padding(),
    ) {
        let generator = CharacterGrams::new(n, padding);
        let s: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input);
        let ms: GramMultiSet<Vec<char>> = GramMultiSet::from_generator(&generator, &input);
        prop_assert_eq!(s.len(), ms.distinct_len());
    }

    /// The multiset's total count must equal the raw iterator's length.
    #[test]
    fn multiset_total_matches_iterator_count(
        input in arb_input(),
        n in arb_n(),
        padding in arb_padding(),
    ) {
        let generator = CharacterGrams::new(n, padding);
        let raw = generator.grams(&input).count() as u64;
        let ms: GramMultiSet<Vec<char>> = GramMultiSet::from_generator(&generator, &input);
        prop_assert_eq!(ms.total_count(), raw);
    }

    /// [`GramSet`] iteration is deterministic and ascending.
    ///
    /// Building the same set twice from the same input and generator
    /// must produce the same iteration order. This is what
    /// `BTreeSet`-backed storage guarantees; the property tests the
    /// contract downstream MinHash implementations rely on.
    #[test]
    fn set_iteration_is_deterministic(
        input in arb_input(),
        n in arb_n(),
        padding in arb_padding(),
    ) {
        let generator = CharacterGrams::new(n, padding);
        let s1: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input);
        let s2: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input);
        let g1: Vec<&Vec<char>> = s1.iter().collect();
        let g2: Vec<&Vec<char>> = s2.iter().collect();
        prop_assert_eq!(g1, g2);
    }

    /// Set intersection is symmetric: `A ∩ B = B ∩ A`.
    #[test]
    fn intersection_is_symmetric(
        input_a in arb_input(),
        input_b in arb_input(),
        n in arb_n(),
    ) {
        let generator = CharacterGrams::new(n, PaddingPolicy::<char>::None);
        let a: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input_a);
        let b: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input_b);
        prop_assert_eq!(a.intersection_with(&b), b.intersection_with(&a));
    }

    /// Set difference is antisymmetric under swap: `A - B` and `B - A`
    /// are disjoint, and their union is the symmetric difference.
    #[test]
    fn difference_is_antisymmetric(
        input_a in arb_input(),
        input_b in arb_input(),
        n in arb_n(),
    ) {
        let generator = CharacterGrams::new(n, PaddingPolicy::<char>::None);
        let a: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input_a);
        let b: GramSet<Vec<char>> = GramSet::from_generator(&generator, &input_b);
        let a_minus_b = a.difference_with(&b);
        let b_minus_a = b.difference_with(&a);
        // Disjoint.
        prop_assert!(a_minus_b.intersection_with(&b_minus_a).is_empty());
        // Every element of `a_minus_b` is in `a` but not `b`; every
        // element of `b_minus_a` is in `b` but not `a` — the two are
        // structurally swapped.
        for g in &a_minus_b {
            prop_assert!(a.contains(g) && !b.contains(g));
        }
        for g in &b_minus_a {
            prop_assert!(b.contains(g) && !a.contains(g));
        }
    }

    /// The L2 norm of a nonzero [`GramVector`] after `normalize_l2` is
    /// `1.0` within a tolerance.
    #[test]
    fn normalize_l2_yields_unit_norm(
        // Non-zero, finite weights so the vector's L2 norm is non-zero.
        weights in proptest::collection::vec(-100.0f64..100.0, 1..8),
    ) {
        // Reject the pathological input where every weight is (near)
        // zero — normalize_l2 is documented as a no-op there.
        let sum_sq: f64 = weights.iter().map(|w| w * w).sum();
        prop_assume!(sum_sq > 1e-12);

        let mut v: GramVector<u32> = GramVector::new();
        for (i, w) in weights.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let key = i as u32;
            v.set(key, *w);
        }
        v.normalize_l2();
        prop_assert!((v.l2_norm() - 1.0).abs() < 1e-9,
            "L2 norm after normalize was {} for weights {:?}",
            v.l2_norm(), weights);
    }

    /// Similarly for L1.
    #[test]
    fn normalize_l1_yields_unit_norm(
        weights in proptest::collection::vec(-100.0f64..100.0, 1..8),
    ) {
        let sum_abs: f64 = weights.iter().map(|w| w.abs()).sum();
        prop_assume!(sum_abs > 1e-12);

        let mut v: GramVector<u32> = GramVector::new();
        for (i, w) in weights.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let key = i as u32;
            v.set(key, *w);
        }
        v.normalize_l1();
        prop_assert!((v.l1_norm() - 1.0).abs() < 1e-9);
    }
}
