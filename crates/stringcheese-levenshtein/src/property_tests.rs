//! Property-based and differential tests across all three kernels.
//!
//! These tests are the single most important thing in this crate: the whole
//! point of maintaining three implementations of the same recurrence is that
//! a bug in one is exceedingly unlikely to also be present in the other two.
//! Any disagreement between the oracle, the rolling-rows kernel, and the
//! banded kernel is an immediate signal.
//!
//! The metric axioms are also checked: [`Levenshtein`] declares itself a
//! true metric via [`MetricProperties::METRIC`], and this file backs that
//! claim with generated inputs.

use proptest::prelude::*;

use crate::algorithm::Levenshtein;
use crate::banded::distance_banded_with_workspace;
use crate::full_matrix::distance_full_matrix;
use crate::rolling_rows::distance_rolling_rows_with_workspace;
use crate::workspace::LevenshteinWorkspace;
use stringcheese_core::{BoundedDistance, Distance, DistanceMetric};

/// A short byte-slice strategy over a small alphabet.
///
/// A three-symbol alphabet gives a good mix of matches and mismatches within
/// short inputs, which is exactly what the metric axioms and the
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
    /// The rolling-rows kernel must agree with the oracle on every generated
    /// input.
    #[test]
    fn rolling_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let mut ws = LevenshteinWorkspace::new();
        let observed = distance_rolling_rows_with_workspace(&a, &b, &mut ws).into_inner();
        let expected = distance_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "rolling-rows disagreed with oracle");
    }

    /// The banded kernel with a wide cutoff must return the exact distance
    /// as `Within`.
    #[test]
    fn banded_matches_oracle_with_wide_cutoff(a in arb_bytes(), b in arb_bytes()) {
        let mut ws = LevenshteinWorkspace::new();
        let expected = distance_full_matrix(&a, &b);
        let observed = distance_banded_with_workspace(&a, &b, 100, &mut ws);
        prop_assert_eq!(observed, BoundedDistance::Within(Distance::new(expected)));
    }

    /// Cutoff correctness: for every input pair and cutoff, the banded
    /// kernel's answer must correctly reflect the oracle.
    #[test]
    fn banded_cutoff_semantics(
        a in arb_bytes(),
        b in arb_bytes(),
        k in 0u32..30,
    ) {
        let mut ws = LevenshteinWorkspace::new();
        let expected = distance_full_matrix(&a, &b);
        let observed = distance_banded_with_workspace(&a, &b, k, &mut ws);
        if expected <= k {
            prop_assert_eq!(
                observed,
                BoundedDistance::Within(Distance::new(expected)),
                "Within(exact) expected"
            );
        } else {
            prop_assert_eq!(
                observed,
                BoundedDistance::Exceeded { cutoff: k },
                "Exceeded(k) expected"
            );
        }
    }

    /// Metric axiom: symmetry — d(x, y) = d(y, x).
    #[test]
    fn symmetry(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(distance_full_matrix(&a, &b), distance_full_matrix(&b, &a));
    }

    /// Metric axiom: identity of indiscernibles — d(x, x) = 0.
    #[test]
    fn identity(a in arb_bytes()) {
        prop_assert_eq!(distance_full_matrix(&a, &a), 0);
    }

    /// Metric axiom: triangle inequality — d(x, z) <= d(x, y) + d(y, z).
    #[test]
    fn triangle_inequality(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
        c in arb_short_bytes(),
    ) {
        let d_ab = distance_full_matrix(&a, &b);
        let d_bc = distance_full_matrix(&b, &c);
        let d_ac = distance_full_matrix(&a, &c);
        prop_assert!(
            d_ac <= d_ab + d_bc,
            "triangle inequality violated: d(a,c)={d_ac} > d(a,b)+d(b,c)={d_ab}+{d_bc}"
        );
    }

    /// The `Levenshtein` trait-impl entry point must agree with the direct
    /// oracle call.
    #[test]
    fn public_api_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let alg = Levenshtein;
        prop_assert_eq!(
            alg.distance(&a, &b).into_inner(),
            distance_full_matrix(&a, &b)
        );
    }

    /// Cutoff monotonicity: raising the cutoff can only convert `Exceeded`
    /// into `Within`, never the other way around.
    #[test]
    fn cutoff_monotone(a in arb_short_bytes(), b in arb_short_bytes(), k in 0u32..10) {
        let mut ws = LevenshteinWorkspace::new();
        let small = distance_banded_with_workspace(&a, &b, k, &mut ws);
        let large = distance_banded_with_workspace(&a, &b, k + 5, &mut ws);
        if let BoundedDistance::Within(d) = small {
            prop_assert_eq!(large, BoundedDistance::Within(d),
                "raising the cutoff dropped a Within answer");
        }
    }
}

/// Non-property assertion of non-negativity on the empty/empty edge case;
/// `u32` makes negativity impossible everywhere else, but the empty/empty
/// pair is where "zero is really zero" is most easily wrong.
#[test]
fn non_negative_on_empty_pair() {
    assert_eq!(distance_full_matrix::<u8>(b"", b""), 0);
    let mut ws = LevenshteinWorkspace::new();
    assert_eq!(
        distance_rolling_rows_with_workspace::<u8>(b"", b"", &mut ws).into_inner(),
        0
    );
    assert_eq!(
        distance_banded_with_workspace::<u8>(b"", b"", 0, &mut ws),
        BoundedDistance::Within(Distance::new(0))
    );
}
