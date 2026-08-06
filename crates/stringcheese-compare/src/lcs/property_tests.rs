//! Property-based and differential tests across the LCS kernels.
//!
//! The point of maintaining two implementations of the same recurrence is
//! that a bug in one is exceedingly unlikely to also be present in the
//! other. Any disagreement between the full-matrix oracle and the
//! rolling-rows kernel is an immediate signal.
//!
//! The metric axioms are also checked: [`LcsDistance`] declares itself a
//! true metric via [`MetricProperties::METRIC`], and this file backs that
//! claim with generated inputs. The triangle inequality is the notable
//! one — it is the only axiom for which a counterexample would not be
//! caught by a single-pair test.
//!
//! [`MetricProperties::METRIC`]: stringcheese_core::MetricProperties::METRIC

use proptest::prelude::*;

use crate::lcs::algorithm::{Lcs, LcsDistance};
use crate::lcs::full_matrix::{lcs_distance_full_matrix, lcs_length_full_matrix};
use crate::lcs::rolling_rows::{
    lcs_distance_rolling_rows_with_workspace, lcs_length_rolling_rows_with_workspace,
};
use crate::lcs::workspace::LcsWorkspace;

/// A short byte-slice strategy over a small alphabet.
///
/// A three-symbol alphabet gives a good mix of matches and mismatches
/// within short inputs, which is exactly what the metric axioms and the
/// cross-kernel differential care about.
fn arb_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..3, 0..20)
}

/// A very short byte-slice strategy, used for the triangle-inequality test
/// where three inputs are multiplied together and the search space grows
/// cubically.
fn arb_short_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..3, 0..10)
}

proptest! {
    /// The rolling-rows length kernel must agree with the oracle on every
    /// generated input.
    #[test]
    fn rolling_length_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let mut ws = LcsWorkspace::new();
        let observed = lcs_length_rolling_rows_with_workspace(&a, &b, &mut ws).into_inner();
        let expected = lcs_length_full_matrix(&a, &b).into_inner();
        prop_assert_eq!(observed, expected, "rolling-rows LCS length disagreed with oracle");
    }

    /// The rolling-rows distance kernel must agree with the oracle on every
    /// generated input.
    #[test]
    fn rolling_distance_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let mut ws = LcsWorkspace::new();
        let observed = lcs_distance_rolling_rows_with_workspace(&a, &b, &mut ws).into_inner();
        let expected = lcs_distance_full_matrix(&a, &b).into_inner();
        prop_assert_eq!(observed, expected, "rolling-rows LCS distance disagreed with oracle");
    }

    /// LCS length must be bounded: `0 ≤ lcs(a, b) ≤ min(|a|, |b|)`.
    #[test]
    fn length_is_in_range(a in arb_bytes(), b in arb_bytes()) {
        let lcs = lcs_length_full_matrix(&a, &b).into_inner();
        let bound = u32::try_from(a.len().min(b.len())).unwrap();
        prop_assert!(lcs <= bound, "lcs({}) > min(|a|, |b|)={bound}", lcs);
    }

    /// LCS length identity: `lcs(x, x) = |x|`.
    #[test]
    fn length_identity(a in arb_bytes()) {
        let lcs = lcs_length_full_matrix(&a, &a).into_inner();
        prop_assert_eq!(lcs, u32::try_from(a.len()).unwrap());
    }

    /// LCS length with an empty side is zero: `lcs(x, []) = 0`.
    #[test]
    fn length_with_empty_is_zero(a in arb_bytes()) {
        let empty: &[u8] = &[];
        prop_assert_eq!(lcs_length_full_matrix(&a, empty).into_inner(), 0);
        prop_assert_eq!(lcs_length_full_matrix(empty, &a).into_inner(), 0);
    }

    /// LCS length symmetry: `lcs(x, y) = lcs(y, x)`.
    #[test]
    fn length_symmetry(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(
            lcs_length_full_matrix(&a, &b).into_inner(),
            lcs_length_full_matrix(&b, &a).into_inner()
        );
    }

    /// LCS distance metric axiom: symmetry — d(x, y) = d(y, x).
    #[test]
    fn distance_symmetry(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(
            lcs_distance_full_matrix(&a, &b).into_inner(),
            lcs_distance_full_matrix(&b, &a).into_inner()
        );
    }

    /// LCS distance metric axiom: identity of indiscernibles — d(x, x) = 0.
    #[test]
    fn distance_identity(a in arb_bytes()) {
        prop_assert_eq!(lcs_distance_full_matrix(&a, &a).into_inner(), 0);
    }

    /// LCS distance is non-negative. `u32` makes negativity representationally
    /// impossible; this axiom instead checks that the derived formula never
    /// underflows for real inputs (i.e., `2 · lcs ≤ |a| + |b|` always holds).
    #[test]
    fn distance_non_negative(a in arb_bytes(), b in arb_bytes()) {
        // Any observation is non-negative by type; the interesting question
        // is whether `distance_full_matrix` panicked, which the proptest
        // harness would report as a failure. This test would therefore
        // catch a regression that flipped the subtraction order.
        let _ = lcs_distance_full_matrix(&a, &b).into_inner();
    }

    /// LCS distance metric axiom: triangle inequality — d(x, z) ≤ d(x, y) + d(y, z).
    #[test]
    fn distance_triangle_inequality(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
        c in arb_short_bytes(),
    ) {
        let d_ab = lcs_distance_full_matrix(&a, &b).into_inner();
        let d_bc = lcs_distance_full_matrix(&b, &c).into_inner();
        let d_ac = lcs_distance_full_matrix(&a, &c).into_inner();
        prop_assert!(
            d_ac <= d_ab + d_bc,
            "triangle inequality violated: d(a,c)={d_ac} > d(a,b)+d(b,c)={d_ab}+{d_bc}"
        );
    }

    /// LCS distance is even iff `|a| + |b|` is even. Because
    /// `distance = |a| + |b| - 2·lcs`, the parity of the distance is
    /// entirely determined by the parity of `|a| + |b|`. A regression that
    /// added or removed a substitution operation would break this parity.
    #[test]
    fn distance_parity_matches_length_sum(a in arb_bytes(), b in arb_bytes()) {
        let dist = lcs_distance_full_matrix(&a, &b).into_inner();
        let sum = u32::try_from(a.len() + b.len()).unwrap();
        prop_assert_eq!(dist & 1, sum & 1, "distance parity != (|a| + |b|) parity");
    }

    /// The `LcsDistance` trait impl must agree with the direct oracle
    /// call.
    #[test]
    fn public_distance_api_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let alg = LcsDistance;
        prop_assert_eq!(
            alg.distance(&a, &b).into_inner(),
            lcs_distance_full_matrix(&a, &b).into_inner()
        );
    }

    /// The `Lcs::length` public method must agree with the direct oracle
    /// call.
    #[test]
    fn public_length_api_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let alg = Lcs;
        prop_assert_eq!(
            alg.length(&a, &b).into_inner(),
            lcs_length_full_matrix(&a, &b).into_inner()
        );
    }
}

/// Non-property assertion of non-negativity on the empty/empty edge case;
/// `u32` makes negativity impossible everywhere else, but the empty/empty
/// pair is where "zero is really zero" is most easily wrong.
#[test]
fn non_negative_on_empty_pair() {
    assert_eq!(lcs_length_full_matrix::<u8>(b"", b"").into_inner(), 0);
    assert_eq!(lcs_distance_full_matrix::<u8>(b"", b"").into_inner(), 0);
    let mut ws = LcsWorkspace::new();
    assert_eq!(
        lcs_length_rolling_rows_with_workspace::<u8>(b"", b"", &mut ws).into_inner(),
        0
    );
    assert_eq!(
        lcs_distance_rolling_rows_with_workspace::<u8>(b"", b"", &mut ws).into_inner(),
        0
    );
}
