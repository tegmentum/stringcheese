//! Property-based and differential tests across every kernel and both
//! variants.
//!
//! The tests here are the most important thing in this crate: three
//! independent OSA implementations must agree pixel-for-pixel on every
//! generated input, and the full-Damerau oracle and production must
//! likewise agree. Any disagreement is an immediate signal that one of the
//! kernels has a bug — a shared bug across independently-authored code
//! paths is unlikely enough that a single differential run is stronger
//! evidence of correctness than any single implementation ever proves on
//! its own.
//!
//! The tests also encode the crate's metric-class claims:
//!
//! * OSA: symmetric, identity, non-negative, cutoff-correct.
//!   Explicitly **not** the triangle inequality — the crate declares OSA a
//!   [`MetricClass::Semimetric`] and a hard-coded test below documents the
//!   canonical triangle-inequality violation as *known behavior*.
//! * Full Damerau: all four metric axioms including the triangle inequality.
//! * Cross-variant: for every input pair, `damerau <= osa` (any OSA edit
//!   sequence is a valid full-Damerau edit sequence, so full Damerau is
//!   weakly less on every pair).
//!
//! [`MetricClass::Semimetric`]: stringcheese_core::MetricClass::Semimetric

use proptest::prelude::*;

use crate::damerau::algorithm::{Damerau, Osa};
use crate::damerau::damerau::full_matrix::distance_full_matrix as damerau_full_matrix;
use crate::damerau::damerau::production::distance_production_with_workspace as damerau_production;
use crate::damerau::osa::banded::distance_banded_with_workspace as osa_banded;
use crate::damerau::osa::full_matrix::distance_full_matrix as osa_full_matrix;
use crate::damerau::osa::rolling_rows::distance_rolling_rows_with_workspace as osa_rolling_rows;
use crate::damerau::workspace::{DamerauWorkspace, OsaWorkspace};
use stringcheese_core::{BoundedDistance, Distance, DistanceMetric};

/// A short byte-slice strategy over a small alphabet.
///
/// A three-symbol alphabet gives a good mix of matches and mismatches
/// within short inputs, which is exactly what the metric axioms and the
/// cross-kernel differential care about.
fn arb_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..3, 0..20)
}

/// A very short byte-slice strategy, used for tests that combine three
/// inputs (triangle inequality) or that would otherwise take a long time
/// on longer inputs.
fn arb_short_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(0u8..3, 0..10)
}

proptest! {
    // -----------------------------------------------------------------
    // OSA cross-kernel differential
    // -----------------------------------------------------------------

    /// The OSA rolling-rows kernel must agree with the OSA oracle on every
    /// generated input.
    #[test]
    fn osa_rolling_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let mut ws = OsaWorkspace::new();
        let observed = osa_rolling_rows(&a, &b, &mut ws).into_inner();
        let expected = osa_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "OSA rolling-rows disagreed with oracle");
    }

    /// The OSA banded kernel with a wide cutoff must return the exact
    /// distance as `Within`.
    #[test]
    fn osa_banded_matches_oracle_with_wide_cutoff(a in arb_bytes(), b in arb_bytes()) {
        let mut ws = OsaWorkspace::new();
        let expected = osa_full_matrix(&a, &b);
        let observed = osa_banded(&a, &b, 100, &mut ws);
        prop_assert_eq!(observed, BoundedDistance::Within(Distance::new(expected)));
    }

    /// OSA banded cutoff semantics: for every generated pair and every
    /// generated cutoff, the banded kernel's answer must correctly reflect
    /// the oracle.
    #[test]
    fn osa_banded_cutoff_semantics(
        a in arb_bytes(),
        b in arb_bytes(),
        k in 0u32..30,
    ) {
        let mut ws = OsaWorkspace::new();
        let expected = osa_full_matrix(&a, &b);
        let observed = osa_banded(&a, &b, k, &mut ws);
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

    /// OSA cutoff monotonicity: raising the cutoff can only convert
    /// `Exceeded` into `Within`, never the other way around.
    #[test]
    fn osa_cutoff_monotone(a in arb_short_bytes(), b in arb_short_bytes(), k in 0u32..10) {
        let mut ws = OsaWorkspace::new();
        let small = osa_banded(&a, &b, k, &mut ws);
        let large = osa_banded(&a, &b, k + 5, &mut ws);
        if let BoundedDistance::Within(d) = small {
            prop_assert_eq!(large, BoundedDistance::Within(d),
                "raising the cutoff dropped a Within answer");
        }
    }

    // -----------------------------------------------------------------
    // OSA metric axioms (except triangle inequality — see below)
    // -----------------------------------------------------------------

    /// OSA metric axiom: symmetry.
    #[test]
    fn osa_symmetry(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(osa_full_matrix(&a, &b), osa_full_matrix(&b, &a));
    }

    /// OSA metric axiom: identity of indiscernibles — `d(x, x) = 0`.
    #[test]
    fn osa_identity(a in arb_bytes()) {
        prop_assert_eq!(osa_full_matrix(&a, &a), 0);
    }

    /// OSA output is bounded above by `max(m, n)`: no edit sequence can
    /// exceed converting one input into the other via insertions,
    /// deletions, or substitutions alone.
    #[test]
    fn osa_bounded_by_max_length(a in arb_bytes(), b in arb_bytes()) {
        let observed = osa_full_matrix(&a, &b);
        let upper = u32::try_from(a.len().max(b.len()))
            .expect("proptest strategy caps length well below u32::MAX");
        prop_assert!(u64::from(observed) <= u64::from(upper),
            "OSA {observed} exceeded max length {upper}");
    }

    /// The `Osa` trait-impl entry point must agree with the direct oracle.
    #[test]
    fn osa_public_api_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let alg = Osa;
        prop_assert_eq!(
            alg.distance(&a, &b).into_inner(),
            osa_full_matrix(&a, &b)
        );
    }

    // -----------------------------------------------------------------
    // Full-Damerau cross-kernel differential
    // -----------------------------------------------------------------

    /// The Damerau production kernel must agree with the Damerau oracle on
    /// every generated input.
    #[test]
    fn damerau_production_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let mut ws: DamerauWorkspace<u8> = DamerauWorkspace::new();
        let observed = damerau_production(&a, &b, &mut ws).into_inner();
        let expected = damerau_full_matrix(&a, &b);
        prop_assert_eq!(observed, expected, "Damerau production disagreed with oracle");
    }

    /// Workspace-reuse correctness: for any two generated pairs, running
    /// them back-to-back through a single reused workspace must produce the
    /// same answers as running each with a fresh workspace. This is the
    /// load-bearing test that the HashMap-in-workspace reuse from Item 1
    /// preserves correctness — a stale key or a partial clear would surface
    /// here as a shrunk counterexample.
    #[test]
    fn damerau_production_workspace_reuse_matches_fresh(
        a1 in arb_bytes(),
        b1 in arb_bytes(),
        a2 in arb_bytes(),
        b2 in arb_bytes(),
    ) {
        let mut fresh1: DamerauWorkspace<u8> = DamerauWorkspace::new();
        let mut fresh2: DamerauWorkspace<u8> = DamerauWorkspace::new();
        let fresh_d1 = damerau_production(&a1, &b1, &mut fresh1).into_inner();
        let fresh_d2 = damerau_production(&a2, &b2, &mut fresh2).into_inner();

        let mut hot: DamerauWorkspace<u8> = DamerauWorkspace::new();
        let hot_d1 = damerau_production(&a1, &b1, &mut hot).into_inner();
        let hot_d2 = damerau_production(&a2, &b2, &mut hot).into_inner();

        prop_assert_eq!(hot_d1, fresh_d1, "reused workspace disagreed on first call");
        prop_assert_eq!(hot_d2, fresh_d2, "reused workspace disagreed on second call");
    }

    /// The `Damerau` trait-impl entry point must agree with the direct oracle.
    #[test]
    fn damerau_public_api_matches_oracle(a in arb_bytes(), b in arb_bytes()) {
        let alg = Damerau;
        prop_assert_eq!(
            alg.distance(&a, &b).into_inner(),
            damerau_full_matrix(&a, &b)
        );
    }

    // -----------------------------------------------------------------
    // Full-Damerau metric axioms — all four including triangle inequality
    // -----------------------------------------------------------------

    /// Damerau metric axiom: symmetry.
    #[test]
    fn damerau_symmetry(a in arb_bytes(), b in arb_bytes()) {
        prop_assert_eq!(damerau_full_matrix(&a, &b), damerau_full_matrix(&b, &a));
    }

    /// Damerau metric axiom: identity of indiscernibles.
    #[test]
    fn damerau_identity(a in arb_bytes()) {
        prop_assert_eq!(damerau_full_matrix(&a, &a), 0);
    }

    /// Damerau output is bounded above by `max(m, n)`.
    #[test]
    fn damerau_bounded_by_max_length(a in arb_bytes(), b in arb_bytes()) {
        let observed = damerau_full_matrix(&a, &b);
        let upper = u32::try_from(a.len().max(b.len()))
            .expect("proptest strategy caps length well below u32::MAX");
        prop_assert!(u64::from(observed) <= u64::from(upper),
            "Damerau {observed} exceeded max length {upper}");
    }

    /// Damerau metric axiom: triangle inequality —
    /// `d(x, z) <= d(x, y) + d(y, z)`. Unlike OSA, full Damerau *is* a
    /// true metric so this holds unconditionally.
    #[test]
    fn damerau_triangle_inequality(
        a in arb_short_bytes(),
        b in arb_short_bytes(),
        c in arb_short_bytes(),
    ) {
        let d_ab = damerau_full_matrix(&a, &b);
        let d_bc = damerau_full_matrix(&b, &c);
        let d_ac = damerau_full_matrix(&a, &c);
        prop_assert!(
            d_ac <= d_ab + d_bc,
            "triangle inequality violated: d(a,c)={d_ac} > d(a,b)+d(b,c)={d_ab}+{d_bc}"
        );
    }

    // -----------------------------------------------------------------
    // Cross-variant relationship
    // -----------------------------------------------------------------

    /// Full Damerau is weakly less than OSA on every input pair.
    ///
    /// This holds because any OSA edit sequence is also a valid full-Damerau
    /// edit sequence — full Damerau is strictly more permissive about how
    /// edits may compose.
    #[test]
    fn damerau_weakly_below_osa(a in arb_bytes(), b in arb_bytes()) {
        let osa_d = osa_full_matrix(&a, &b);
        let dam_d = damerau_full_matrix(&a, &b);
        prop_assert!(
            dam_d <= osa_d,
            "damerau({a:?}, {b:?}) = {dam_d} exceeded osa({a:?}, {b:?}) = {osa_d}"
        );
    }
}

// -----------------------------------------------------------------------
// Non-property assertions
// -----------------------------------------------------------------------

/// Non-negativity on the empty/empty edge case; `u32` makes negativity
/// impossible everywhere else, but the empty/empty pair is where "zero is
/// really zero" is most easily wrong.
#[test]
fn non_negative_on_empty_pair() {
    assert_eq!(osa_full_matrix::<u8>(b"", b""), 0);
    assert_eq!(damerau_full_matrix::<u8>(b"", b""), 0);
    let mut osa_ws = OsaWorkspace::new();
    assert_eq!(
        osa_rolling_rows::<u8>(b"", b"", &mut osa_ws).into_inner(),
        0
    );
    let mut dam_ws: DamerauWorkspace<u8> = DamerauWorkspace::new();
    assert_eq!(
        damerau_production::<u8>(b"", b"", &mut dam_ws).into_inner(),
        0
    );
}

/// The distinguishing example, hard-coded: `damerau("ca", "abc") <
/// osa("ca", "abc")`.
///
/// A property test would find this eventually, but a proptest shrunk
/// counterexample is a shakier reason to trust the ordering than a
/// hard-coded value that any reader can verify by hand.
#[test]
fn distinguishing_example_ca_abc() {
    let osa_d = osa_full_matrix(b"ca", b"abc");
    let dam_d = damerau_full_matrix(b"ca", b"abc");
    assert_eq!(osa_d, 3, "OSA(ca, abc) should be 3");
    assert_eq!(dam_d, 2, "Damerau(ca, abc) should be 2");
    assert!(
        dam_d < osa_d,
        "the distinguishing example must have Damerau strictly less than OSA"
    );
}

/// OSA violates the triangle inequality on a known family.
///
/// This test is the crate's contract with its `MetricClass::Semimetric`
/// declaration: OSA is *known* to violate the triangle inequality. If some
/// future refactor accidentally made OSA a true metric (perhaps by silently
/// changing the transposition semantics), *this* test would fail — the
/// documented, deliberate departure from metric behavior would have been
/// silently altered.
///
/// The canonical violating triple used here: with `x = "ab"`,
/// `y = "acb"`, `z = "ca"`,
/// `osa(x, y) = 1`, `osa(y, z) = 2`, and `osa(x, z) = 3` — but the DP over
/// `osa(x, z)` computed via the intermediate `y = "acb"` would prefer a
/// path costing only `1 + 2 = 3`, so this specific triple is a boundary
/// case. The strict violation used in the assertion below is a different
/// well-documented triple.
#[test]
#[allow(
    clippy::similar_names,
    reason = "`d_xy`, `d_yz`, `d_xz` are the three pairwise distances between the same three inputs; the naming mirrors the mathematical statement of the triangle-inequality axiom."
)]
fn osa_violates_triangle_inequality_on_known_family() {
    // Boytsov 2011 discusses the family of OSA triangle-violating triples
    // that arise from the "no substring edited twice" restriction. One
    // simple three-string counterexample:
    //   x = "ab", y = "ba", z = "abc"
    //   osa(x, y) = 1 (swap a and b)
    //   osa(y, z) = 3 (subst b->a, subst a->b, insert c — no valid
    //                  transposition sequence to convert "ba" into "abc"
    //                  under the OSA restriction using fewer than 3 ops)
    //   osa(x, z) = 1 (insert c)
    // For triangle inequality we'd need d(x, z) <= d(x, y) + d(y, z).
    // Here 1 <= 1 + 3 → holds. So this is not the counterexample.
    //
    // The classical counterexample: use the "ca"/"abc" chain.
    //   x = "ca", y = "ac", z = "abc"
    //   osa(x, y) = 1 (transpose)
    //   osa(y, z) = 1 (insert b)
    //   osa(x, z) = 3 (OSA can't chain a transposition with a later
    //                  insertion on the same substring)
    // Triangle inequality would demand osa(x, z) <= osa(x, y) + osa(y, z):
    // 3 <= 1 + 1 = 2. That is FALSE — OSA violates the triangle inequality
    // on this triple.
    let d_xy = osa_full_matrix(b"ca", b"ac");
    let d_yz = osa_full_matrix(b"ac", b"abc");
    let d_xz = osa_full_matrix(b"ca", b"abc");
    assert_eq!(d_xy, 1, "OSA(ca, ac) should be 1");
    assert_eq!(d_yz, 1, "OSA(ac, abc) should be 1");
    assert_eq!(d_xz, 3, "OSA(ca, abc) should be 3");
    assert!(
        d_xz > d_xy + d_yz,
        "OSA is documented to violate the triangle inequality on (ca, ac, abc); \
         found d(x,z)={d_xz}, d(x,y)+d(y,z)={d_xy}+{d_yz}. If this assertion \
         fails, either the OSA semantics have quietly changed, or the crate's \
         MetricClass::Semimetric declaration has become a lie."
    );
}
