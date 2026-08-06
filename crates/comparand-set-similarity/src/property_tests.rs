//! Property-based tests for the four similarity families in this crate.
//!
//! Every mathematical claim any [`SimilarityMetric`] impl in this crate
//! makes is backed by a generator here — a future change that violates a
//! claim will produce a shrunk counterexample rather than a silent lie in
//! the descriptor.
//!
//! # Tolerance rationale
//!
//! * Bit-exact `.to_bits()` equality is asserted wherever the arithmetic
//!   is symmetric by construction — the two sides compute the same
//!   floating-point expression, so IEEE 754 associativity is not at
//!   play. Identity, symmetric intersection-counting, and disjoint-vs-empty
//!   all fall here.
//! * A small ULP margin (`4` ULPs) is used where two mathematically-equal
//!   quantities are computed by different arithmetic routes and round-off
//!   can push them one or two ULPs apart. The margin is deliberately
//!   loose; a real bug would produce a difference many orders of
//!   magnitude larger.
//!
//! # Alphabet size
//!
//! Small alphabets (3–5 symbols) generate frequent overlaps — the cases
//! that exercise the arithmetic. Larger alphabets would waste most cases
//! on disjoint pairs.
//!
//! [`SimilarityMetric`]: comparand_core::SimilarityMetric

use proptest::prelude::*;

use crate::cosine::{Cosine, cosine};
use crate::dice::{DiceOverMultiSet, DiceOverSet, dice_multiset, dice_set};
use crate::jaccard::{JaccardOverMultiSet, JaccardOverSet, jaccard_multiset, jaccard_set};
use crate::overlap::{Overlap, overlap_set};
use comparand_core::SimilarityMetric;
use comparand_ngram::{GramMultiSet, GramSet, GramVector};

/// Small `u8` alphabet — generates enough overlaps to exercise the
/// intersection arithmetic in most cases.
fn arb_set() -> impl Strategy<Value = GramSet<u8>> {
    proptest::collection::vec(0u8..5, 0..12).prop_map(|v| v.into_iter().collect())
}

fn arb_multiset() -> impl Strategy<Value = GramMultiSet<u8>> {
    proptest::collection::vec(0u8..5, 0..12).prop_map(|v| {
        let mut ms = GramMultiSet::new();
        for x in v {
            ms.add(x);
        }
        ms
    })
}

fn arb_vector() -> impl Strategy<Value = GramVector<u8>> {
    proptest::collection::vec(0u8..5, 0..12).prop_map(|v| {
        let mut gv: GramVector<u8> = GramVector::new();
        for x in v {
            gv.add(x, 1.0);
        }
        gv
    })
}

/// `4`-ULP absolute-equality check on two positive `f64`s. Mirrors the
/// helper used in `comparand-jaro`'s property tests.
#[allow(
    clippy::float_cmp,
    reason = "IEEE 754 equality is the fast-path check here; the ULP fallback handles genuine round-off"
)]
fn nearly_equal(x: f64, y: f64) -> bool {
    if x == y {
        return true;
    }
    if x.is_nan() || y.is_nan() {
        return false;
    }
    if x.is_sign_negative() != y.is_sign_negative() {
        return false;
    }
    x.to_bits().abs_diff(y.to_bits()) <= 4
}

proptest! {
    // -------- Bounded range --------

    #[test]
    fn dice_set_range(a in arb_set(), b in arb_set()) {
        let s = dice_set(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {s}");
    }

    #[test]
    fn dice_multiset_range(a in arb_multiset(), b in arb_multiset()) {
        let s = dice_multiset(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {s}");
    }

    #[test]
    fn jaccard_set_range(a in arb_set(), b in arb_set()) {
        let s = jaccard_set(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {s}");
    }

    #[test]
    fn jaccard_multiset_range(a in arb_multiset(), b in arb_multiset()) {
        let s = jaccard_multiset(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {s}");
    }

    #[test]
    fn overlap_range(a in arb_set(), b in arb_set()) {
        let s = overlap_set(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {s}");
    }

    /// Non-negative-weight gram vectors keep cosine in `[0, 1]`, not
    /// the general `[-1, 1]` cosine range. This is the crate's own
    /// non-negativity claim under test.
    #[test]
    fn cosine_nonneg_range(a in arb_vector(), b in arb_vector()) {
        let s = cosine(&a, &b);
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {s}");
    }

    // -------- Identity --------

    #[test]
    fn dice_set_identity_bit_exact(a in arb_set()) {
        prop_assume!(!a.is_empty());
        let s = dice_set(&a, &a);
        prop_assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn dice_multiset_identity_bit_exact(a in arb_multiset()) {
        prop_assume!(!a.is_empty());
        let s = dice_multiset(&a, &a);
        prop_assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn jaccard_set_identity_bit_exact(a in arb_set()) {
        prop_assume!(!a.is_empty());
        let s = jaccard_set(&a, &a);
        prop_assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn jaccard_multiset_identity_bit_exact(a in arb_multiset()) {
        prop_assume!(!a.is_empty());
        let s = jaccard_multiset(&a, &a);
        prop_assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn overlap_identity_bit_exact(a in arb_set()) {
        prop_assume!(!a.is_empty());
        let s = overlap_set(&a, &a);
        prop_assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn cosine_identity_bit_exact(a in arb_vector()) {
        prop_assume!(!a.is_empty());
        let s = cosine(&a, &a);
        prop_assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    // -------- Symmetry --------
    //
    // The intersection helpers walk the smaller side, so `sim(a, b)` and
    // `sim(b, a)` compute the identical sum in the identical order and
    // must agree bit-exactly. If a future refactor introduces an
    // order-sensitive summation, these assertions catch it.

    #[test]
    fn dice_set_symmetry_bit_exact(a in arb_set(), b in arb_set()) {
        prop_assert_eq!(dice_set(&a, &b).to_bits(), dice_set(&b, &a).to_bits());
    }

    #[test]
    fn dice_multiset_symmetry_bit_exact(a in arb_multiset(), b in arb_multiset()) {
        prop_assert_eq!(
            dice_multiset(&a, &b).to_bits(),
            dice_multiset(&b, &a).to_bits()
        );
    }

    #[test]
    fn jaccard_set_symmetry_bit_exact(a in arb_set(), b in arb_set()) {
        prop_assert_eq!(jaccard_set(&a, &b).to_bits(), jaccard_set(&b, &a).to_bits());
    }

    #[test]
    fn jaccard_multiset_symmetry_bit_exact(a in arb_multiset(), b in arb_multiset()) {
        prop_assert_eq!(
            jaccard_multiset(&a, &b).to_bits(),
            jaccard_multiset(&b, &a).to_bits()
        );
    }

    #[test]
    fn overlap_symmetry_bit_exact(a in arb_set(), b in arb_set()) {
        prop_assert_eq!(overlap_set(&a, &b).to_bits(), overlap_set(&b, &a).to_bits());
    }

    /// Cosine's `dot` product iterates the smaller side of the two
    /// vectors; because the choice is size-driven, `cos(a, b)` and
    /// `cos(b, a)` produce identical bit-patterns. This is guaranteed by
    /// `GramVector::dot`'s implementation choice and is exercised here.
    #[test]
    fn cosine_symmetry_bit_exact(a in arb_vector(), b in arb_vector()) {
        prop_assert_eq!(cosine(&a, &b).to_bits(), cosine(&b, &a).to_bits());
    }

    // -------- Jaccard distance IS a metric --------
    //
    // The whole reason to expose `JaccardOverSet::distance` alongside the
    // similarity is that `1 - jaccard` is a true metric. Triangle
    // inequality on random triples is the load-bearing property test.

    #[test]
    fn jaccard_set_distance_triangle_inequality(
        a in arb_set(),
        b in arb_set(),
        c in arb_set(),
    ) {
        let d_ab = 1.0 - jaccard_set(&a, &b);
        let d_bc = 1.0 - jaccard_set(&b, &c);
        let d_ac = 1.0 - jaccard_set(&a, &c);
        // Allow a tiny ULP slack — the three distances are computed by
        // different rational reductions and a strict `<=` can trip on the
        // last bit. Every discrepancy here is well under 1 ULP; the 1e-12
        // slack is many orders of magnitude larger than that but many
        // orders of magnitude smaller than any real violation would be.
        prop_assert!(
            d_ac <= d_ab + d_bc + 1e-12,
            "Jaccard-set triangle-inequality violated: d(a,c) = {d_ac} > d(a,b) + d(b,c) = {d_ab} + {d_bc}"
        );
    }

    /// Same property on the weighted-multiset form — its distance is also
    /// a metric on non-negative counts.
    #[test]
    fn jaccard_multiset_distance_triangle_inequality(
        a in arb_multiset(),
        b in arb_multiset(),
        c in arb_multiset(),
    ) {
        let d_ab = 1.0 - jaccard_multiset(&a, &b);
        let d_bc = 1.0 - jaccard_multiset(&b, &c);
        let d_ac = 1.0 - jaccard_multiset(&a, &c);
        prop_assert!(
            d_ac <= d_ab + d_bc + 1e-12,
            "Jaccard-multiset triangle-inequality violated: d(a,c) = {d_ac} > d(a,b) + d(b,c) = {d_ab} + {d_bc}"
        );
    }

    // -------- Dice >= Jaccard --------
    //
    // Algebraic identity on sets: `2|inter| / (|A| + |B|) >= |inter| / (|A| + |B| - |inter|)`,
    // which reduces to `|A| + |B| >= 2|inter|` — always true because
    // `|inter| <= min(|A|, |B|) <= (|A| + |B|) / 2`.

    #[test]
    fn dice_ge_jaccard_on_sets(a in arb_set(), b in arb_set()) {
        let d = dice_set(&a, &b);
        let j = jaccard_set(&a, &b);
        prop_assert!(d >= j || nearly_equal(d, j),
            "dice {d} < jaccard {j}");
    }

    /// Same relationship on multisets. Follows from the min–max identity
    /// once you replace set cardinality with total count.
    #[test]
    fn dice_ge_jaccard_on_multisets(a in arb_multiset(), b in arb_multiset()) {
        let d = dice_multiset(&a, &b);
        let j = jaccard_multiset(&a, &b);
        prop_assert!(d >= j || nearly_equal(d, j),
            "dice-multiset {d} < jaccard-multiset {j}");
    }
}

// -------- Non-generative regressions --------

/// Overlap is **not** a metric — and, more importantly, does not satisfy
/// identity of indiscernibles. Hand-coded counterexample: `{a, b}` is a
/// strict subset of `{a, b, c}`, yet `overlap(A, B) = 1.0`. This
/// trip-wire documents the class claim in [`Overlap::properties`]; a
/// future change that mistakenly declares identity of indiscernibles will
/// fail here.
#[test]
fn overlap_fails_identity_of_indiscernibles_on_subset() {
    use alloc::vec::Vec;
    let a: GramSet<Vec<char>> = ['a', 'b'].iter().map(|c| alloc::vec![*c]).collect();
    let b: GramSet<Vec<char>> = ['a', 'b', 'c'].iter().map(|c| alloc::vec![*c]).collect();
    let s = overlap_set(&a, &b);
    assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    // and yet
    assert_ne!(a, b);

    // Also confirm the reported metric-properties correctly refuse the
    // identity-of-indiscernibles claim.
    let props = <Overlap as SimilarityMetric<GramSet<Vec<char>>>>::properties(&Overlap);
    assert!(!props.identity_of_indiscernibles);
}

/// Empty-vs-empty is `1.0` bit-exactly for every algorithm — spelled out
/// so the corner cannot be silently changed by a future refactor.
#[test]
fn empty_pair_yields_one_bit_exact_for_every_algorithm() {
    use alloc::vec::Vec;
    let a_set: GramSet<Vec<char>> = GramSet::new();
    let a_ms: GramMultiSet<char> = GramMultiSet::new();
    let a_vec: GramVector<char> = GramVector::new();
    assert_eq!(dice_set(&a_set, &a_set).to_bits(), 1.0_f64.to_bits());
    assert_eq!(dice_multiset(&a_ms, &a_ms).to_bits(), 1.0_f64.to_bits());
    assert_eq!(jaccard_set(&a_set, &a_set).to_bits(), 1.0_f64.to_bits());
    assert_eq!(jaccard_multiset(&a_ms, &a_ms).to_bits(), 1.0_f64.to_bits());
    assert_eq!(overlap_set(&a_set, &a_set).to_bits(), 1.0_f64.to_bits());
    assert_eq!(cosine(&a_vec, &a_vec).to_bits(), 1.0_f64.to_bits());
}

/// `nearly_equal`'s own contract: never accept mixed signs.
#[test]
fn nearly_equal_rejects_opposite_signs() {
    assert!(!nearly_equal(1e-300, -1e-300));
}

// The trait handles exist here so a future refactor that renames the
// unit structs breaks these lines rather than silently accepting a wrong
// alias.
#[allow(dead_code)]
fn _handles_exist() {
    let _ = DiceOverSet;
    let _ = DiceOverMultiSet;
    let _ = JaccardOverSet;
    let _ = JaccardOverMultiSet;
    let _ = Overlap;
    let _ = Cosine;
}
