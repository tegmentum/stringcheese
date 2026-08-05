//! Property-based tests for the [`Hamming`] algorithm handle.
//!
//! Hamming has no second kernel to differential-check against — the algorithm
//! is a single fold — so the tests here exercise the four metric axioms
//! ([symmetry], [identity], non-negativity, [triangle inequality]) plus the
//! two integration properties that matter for the cutoff-aware and fallible
//! entry points ([cutoff correctness], [length-mismatch behavior]).
//!
//! Property inputs are capped at 30 items over a three-symbol alphabet. That
//! is small enough to keep property runs fast and large enough to produce a
//! healthy mix of exact matches and mismatches.
//!
//! [symmetry]: proptest_symmetry
//! [identity]: proptest_identity
//! [triangle inequality]: proptest_triangle_inequality
//! [cutoff correctness]: proptest_cutoff_correctness
//! [length-mismatch behavior]: proptest_length_mismatch_behavior

use proptest::prelude::*;

use crate::algorithm::Hamming;
use crate::error::LengthMismatch;
use comparand_core::{BoundedDistance, BoundedDistanceMetric, Distance, DistanceMetric};

/// A byte-slice strategy over a small alphabet with a length in `[0, 30]`.
///
/// A three-symbol alphabet gives a good mix of matches and mismatches within
/// short inputs, which is exactly what the metric axioms and the cutoff
/// property care about.
fn arb_bytes() -> impl Strategy<Value = std::vec::Vec<u8>> {
    proptest::collection::vec(0u8..3, 0..30)
}

/// A byte-slice strategy of a fixed length `n`, over the same three-symbol
/// alphabet. Used to generate three equal-length inputs for the
/// triangle-inequality property.
fn arb_bytes_of_len(n: usize) -> impl Strategy<Value = std::vec::Vec<u8>> {
    proptest::collection::vec(0u8..3, n..=n)
}

/// A three-tuple of equal-length byte-slices. Cap length at 20 to keep the
/// triangle-inequality search fast; each additional length grows the input
/// space by a factor of `3^3`.
fn arb_equal_length_triple(
) -> impl Strategy<Value = (std::vec::Vec<u8>, std::vec::Vec<u8>, std::vec::Vec<u8>)> {
    (0usize..=20).prop_flat_map(|n| {
        (
            arb_bytes_of_len(n),
            arb_bytes_of_len(n),
            arb_bytes_of_len(n),
        )
    })
}

/// A two-tuple of equal-length byte-slices, used wherever an equal-length
/// pair is a precondition for the property under test.
fn arb_equal_length_pair() -> impl Strategy<Value = (std::vec::Vec<u8>, std::vec::Vec<u8>)> {
    (0usize..=30).prop_flat_map(|n| (arb_bytes_of_len(n), arb_bytes_of_len(n)))
}

proptest! {
    /// Metric axiom: symmetry — d(x, y) = d(y, x) on equal-length inputs.
    #[test]
    fn proptest_symmetry((a, b) in arb_equal_length_pair()) {
        let alg = Hamming;
        prop_assert_eq!(alg.distance(&a, &b), alg.distance(&b, &a));
    }

    /// Metric axiom: identity of indiscernibles — d(x, x) = 0.
    #[test]
    fn proptest_identity(a in arb_bytes()) {
        let alg = Hamming;
        prop_assert_eq!(alg.distance(&a, &a), Distance::new(0));
    }

    /// Metric axiom: non-negativity. The output is `u32`, so negativity is
    /// impossible at the type level — but "zero really is zero" and "distance
    /// never exceeds the common length" are worth asserting on generated
    /// input. The upper bound guards against a counter that fires on matches
    /// as well as mismatches.
    #[test]
    fn proptest_non_negative_and_length_bounded((a, b) in arb_equal_length_pair()) {
        let alg = Hamming;
        let d = alg.distance(&a, &b).into_inner();
        prop_assert!(u64::from(d) <= a.len() as u64,
            "distance {d} exceeded common length {}", a.len());
    }

    /// Metric axiom: triangle inequality — d(x, z) <= d(x, y) + d(y, z) on
    /// equal-length inputs. Hamming is a true metric so this must hold
    /// unconditionally.
    #[test]
    fn proptest_triangle_inequality((a, b, c) in arb_equal_length_triple()) {
        let alg = Hamming;
        let d_ab = alg.distance(&a, &b).into_inner();
        let d_bc = alg.distance(&b, &c).into_inner();
        let d_ac = alg.distance(&a, &c).into_inner();
        prop_assert!(
            d_ac <= d_ab + d_bc,
            "triangle inequality violated: d(a,c)={d_ac} > d(a,b)+d(b,c)={d_ab}+{d_bc}"
        );
    }

    /// Cutoff correctness: for every equal-length pair and cutoff `k`,
    /// `distance_within` must be consistent with `try_distance`.
    ///
    /// If `Within(d)` is returned then `d <= k` and `d` equals the exact
    /// distance. If `Exceeded { cutoff: k }` is returned then the exact
    /// distance is strictly greater than `k`.
    #[test]
    fn proptest_cutoff_correctness(
        (a, b) in arb_equal_length_pair(),
        k in 0u32..40,
    ) {
        let alg = Hamming;
        let exact = alg.try_distance(&a, &b).unwrap().into_inner();
        let observed = alg.distance_within(&a, &b, k);
        match observed {
            BoundedDistance::Within(d) => {
                prop_assert!(d.into_inner() <= k,
                    "Within({d}) returned above cutoff {k}");
                prop_assert_eq!(d.into_inner(), exact,
                    "Within value disagreed with exact distance");
            }
            BoundedDistance::Exceeded { cutoff } => {
                prop_assert_eq!(cutoff, k,
                    "Exceeded cutoff {} disagreed with requested {}", cutoff, k);
                prop_assert!(exact > k,
                    "Exceeded returned but exact {} did not exceed cutoff {}", exact, k);
            }
        }
    }

    /// Length-mismatch behavior: `try_distance` returns `Err(LengthMismatch)`
    /// iff the input lengths differ, and reports the operand lengths
    /// verbatim.
    #[test]
    fn proptest_length_mismatch_behavior(a in arb_bytes(), b in arb_bytes()) {
        let alg = Hamming;
        match alg.try_distance(&a, &b) {
            Ok(_) => prop_assert_eq!(a.len(), b.len(),
                "try_distance returned Ok on unequal-length input"),
            Err(e) => {
                prop_assert_ne!(a.len(), b.len(),
                    "try_distance returned Err on equal-length input");
                prop_assert_eq!(e, LengthMismatch { left: a.len(), right: b.len() });
            }
        }
    }

    /// Length-mismatch behavior for the cutoff-aware fallible variant. Same
    /// contract as `try_distance`.
    #[test]
    fn proptest_length_mismatch_behavior_bounded(
        a in arb_bytes(),
        b in arb_bytes(),
        k in 0u32..30,
    ) {
        let alg = Hamming;
        match alg.try_distance_within(&a, &b, k) {
            Ok(_) => prop_assert_eq!(a.len(), b.len()),
            Err(e) => {
                prop_assert_ne!(a.len(), b.len());
                prop_assert_eq!(e, LengthMismatch { left: a.len(), right: b.len() });
            }
        }
    }

    /// Cutoff monotonicity: raising the cutoff can only convert `Exceeded`
    /// into `Within`, never the other way around. This is not a metric
    /// axiom — it is a sanity check specific to the cutoff-aware API.
    #[test]
    fn proptest_cutoff_monotone(
        (a, b) in arb_equal_length_pair(),
        k in 0u32..30,
    ) {
        let alg = Hamming;
        let small = alg.distance_within(&a, &b, k);
        let large = alg.distance_within(&a, &b, k + 5);
        if let BoundedDistance::Within(d) = small {
            prop_assert_eq!(large, BoundedDistance::Within(d),
                "raising the cutoff dropped a Within answer");
        }
    }
}
