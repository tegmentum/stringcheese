//! Property-based tests for the Jaro and Jaro-Winkler similarities.
//!
//! Properties are the primary vehicle for exercising the metric-property
//! claims made in [`Jaro`] and [`JaroWinkler`]. Every axiom the algorithms
//! report through [`MetricProperties`] is backed by a generator here — any
//! future code change that violates a claim will show up as a shrunk
//! counterexample rather than as a silent lie in the descriptor.
//!
//! # Tolerance rationale
//!
//! Where two paths compute the same expression by different routes (Jaro
//! then boost vs. Jaro alone; forward vs. reversed inputs), the arithmetic
//! is bit-exact by construction and we assert equality on `to_bits()`. That
//! is stronger than any tolerance and catches accidental use of a
//! non-associative reordering.
//!
//! Where two paths compute closely related but not identical expressions
//! (e.g., the ordering `jw >= jaro` between two `f64` values), we use ULP
//! tolerance on the boundary so the assertion is robust to the round-off
//! that could push a mathematically-equal pair one ULP apart. The chosen
//! `4` ULP margin is deliberately loose — a real bug would produce a
//! difference many orders of magnitude larger.
//!
//! [`Jaro`]: crate::jaro::Jaro
//! [`JaroWinkler`]: crate::jaro_winkler::JaroWinkler
//! [`MetricProperties`]: stringcheese_core::MetricProperties

use proptest::prelude::*;

use crate::jaro::Jaro;
use crate::jaro_winkler::JaroWinkler;
use crate::workspace::JaroWorkspace;
use stringcheese_core::SimilarityMetric;

/// A short `char` slice over a three-symbol alphabet.
///
/// Char-level generation matches how downstream string callers pick the
/// Unicode-scalar representation. The alphabet is small enough that generated
/// pairs commonly share prefixes and produce transpositions, which are the
/// cases where subtle Jaro bugs (window computation, matching bookkeeping,
/// transposition count) surface.
fn arb_chars() -> impl Strategy<Value = alloc::vec::Vec<char>> {
    proptest::collection::vec(prop_oneof![Just('a'), Just('b'), Just('c')], 0..20)
}

/// A `4`-ULP absolute-equality check for `f64`s. Small enough that a real
/// bug is many orders of magnitude out of reach; loose enough that
/// mathematically equal expressions reformulated slightly do not fail.
#[allow(
    clippy::float_cmp,
    reason = "IEEE 754 equality is exactly what we want here: it treats +0.0 and -0.0 as equal (correct: zero ULPs apart) and short-circuits the common exact-match case; the ULP fallback handles genuine round-off"
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
    // -------- Jaro properties --------

    /// Identity: `sim(x, x) = 1.0` bit-exactly for non-empty `x`.
    #[test]
    fn jaro_identity_bit_exact(x in arb_chars()) {
        prop_assume!(!x.is_empty());
        let alg = Jaro;
        let s = alg.similarity(&x, &x).into_inner();
        prop_assert_eq!(s.to_bits(), 1.0_f64.to_bits());
    }

    /// Symmetry: `sim(x, y) = sim(y, x)`. The arithmetic
    /// `m/|a| + m/|b| + (m - t)/m` is bit-exactly commutative in `|a|`
    /// and `|b|` because IEEE 754 addition is commutative; combined with
    /// symmetric matching that yields identical `m` and `t`, the two
    /// results must agree bit-for-bit.
    #[test]
    fn jaro_symmetry_bit_exact(x in arb_chars(), y in arb_chars()) {
        let alg = Jaro;
        let ab = alg.similarity(&x, &y).into_inner();
        let ba = alg.similarity(&y, &x).into_inner();
        prop_assert_eq!(ab.to_bits(), ba.to_bits(),
            "asymmetric Jaro: sim({:?}, {:?}) = {} vs sim({:?}, {:?}) = {}",
            x, y, ab, y, x, ba);
    }

    /// Range: every generated pair produces a similarity in `[0.0, 1.0]`.
    #[test]
    fn jaro_range(x in arb_chars(), y in arb_chars()) {
        let alg = Jaro;
        let s = alg.similarity(&x, &y).into_inner();
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {}", s);
    }

    /// Empty-vs-nonempty: exactly `0.0`.
    #[test]
    fn jaro_empty_vs_nonempty_bit_exact(x in arb_chars()) {
        prop_assume!(!x.is_empty());
        let alg = Jaro;
        let s = alg.similarity(&x, &[] as &[char]).into_inner();
        prop_assert_eq!(s.to_bits(), 0.0_f64.to_bits());
    }

    // -------- JaroWinkler::classic properties --------

    /// When the two inputs disagree at the first position, prefix length is
    /// zero and the boost degenerates to `+0.0 * (1 - jaro) = 0`, so JW
    /// classic reduces to Jaro bit-exactly.
    #[test]
    fn jw_classic_reduces_to_jaro_without_prefix(x in arb_chars(), y in arb_chars()) {
        // Force different first characters.
        prop_assume!(!x.is_empty() && !y.is_empty() && x[0] != y[0]);
        let jaro_alg = Jaro;
        let jw_alg = JaroWinkler::classic();
        let j = jaro_alg.similarity(&x, &y).into_inner();
        let w = jw_alg.similarity(&x, &y).into_inner();
        prop_assert_eq!(j.to_bits(), w.to_bits());
    }

    /// Range: JW is in `[0.0, 1.0]` — the invariant that the
    /// `scaling * prefix_limit <= 1` check protects.
    #[test]
    fn jw_classic_range(x in arb_chars(), y in arb_chars()) {
        let alg = JaroWinkler::classic();
        let s = alg.similarity(&x, &y).into_inner();
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {}", s);
    }

    /// Symmetry: JW classic is symmetric under bit-exact equality, because
    /// the boost multiplies through symmetric-in-inputs values (jaro,
    /// prefix length).
    #[test]
    fn jw_classic_symmetry_bit_exact(x in arb_chars(), y in arb_chars()) {
        let alg = JaroWinkler::classic();
        let ab = alg.similarity(&x, &y).into_inner();
        let ba = alg.similarity(&y, &x).into_inner();
        prop_assert_eq!(ab.to_bits(), ba.to_bits());
    }

    /// Monotonicity: JW classic never drops below Jaro; the boost is
    /// always non-negative in the range `[0, 1]`. Uses a small ULP margin
    /// because the boost formula is a different arithmetic expression
    /// from the Jaro-only baseline.
    #[test]
    fn jw_classic_at_least_jaro(x in arb_chars(), y in arb_chars()) {
        let jaro_alg = Jaro;
        let jw_alg = JaroWinkler::classic();
        let j = jaro_alg.similarity(&x, &y).into_inner();
        let w = jw_alg.similarity(&x, &y).into_inner();
        prop_assert!(w >= j || nearly_equal(w, j), "JW {} < Jaro {}", w, j);
    }

    // -------- JaroWinkler::with_threshold(0.7) properties --------

    /// Below-threshold identity: when the base Jaro score is below 0.7,
    /// JW returns Jaro bit-exactly.
    #[test]
    fn jw_threshold_below_equals_jaro(x in arb_chars(), y in arb_chars()) {
        let jaro_alg = Jaro;
        let j = jaro_alg.similarity(&x, &y).into_inner();
        prop_assume!(j < 0.7);
        let jw_alg = JaroWinkler::with_threshold();
        let w = jw_alg.similarity(&x, &y).into_inner();
        prop_assert_eq!(w.to_bits(), j.to_bits());
    }

    /// Above-threshold monotonicity: when Jaro >= 0.7, JW is >= Jaro.
    #[test]
    fn jw_threshold_above_at_least_jaro(x in arb_chars(), y in arb_chars()) {
        let jaro_alg = Jaro;
        let j = jaro_alg.similarity(&x, &y).into_inner();
        prop_assume!(j >= 0.7);
        let jw_alg = JaroWinkler::with_threshold();
        let w = jw_alg.similarity(&x, &y).into_inner();
        prop_assert!(w >= j || nearly_equal(w, j), "JW {} < Jaro {}", w, j);
    }

    /// Range under the threshold variant.
    #[test]
    fn jw_threshold_range(x in arb_chars(), y in arb_chars()) {
        let alg = JaroWinkler::with_threshold();
        let s = alg.similarity(&x, &y).into_inner();
        prop_assert!((0.0..=1.0).contains(&s), "out of range: {}", s);
    }

    // -------- Workspace-reuse correctness (Item 2) --------

    /// For random inputs, running a Jaro comparison with a workspace that
    /// has already been used on a previous call must return the same
    /// `f64` bits as running with a fresh workspace. Any leaked residue
    /// in the reused bitmaps (or a partial reset in `split_bitmaps_mut`)
    /// would surface here as a shrunk counterexample.
    #[test]
    fn jaro_workspace_reuse_matches_fresh(
        a1 in arb_chars(),
        b1 in arb_chars(),
        a2 in arb_chars(),
        b2 in arb_chars(),
    ) {
        let alg = Jaro;

        // Per-call fresh workspaces for the baseline.
        let mut fresh1 = JaroWorkspace::new();
        let mut fresh2 = JaroWorkspace::new();
        let fresh_v1 = alg.similarity_with_workspace(&a1, &b1, &mut fresh1).into_inner();
        let fresh_v2 = alg.similarity_with_workspace(&a2, &b2, &mut fresh2).into_inner();

        // Single hot workspace across both calls.
        let mut hot = JaroWorkspace::new();
        let hot_v1 = alg.similarity_with_workspace(&a1, &b1, &mut hot).into_inner();
        let hot_v2 = alg.similarity_with_workspace(&a2, &b2, &mut hot).into_inner();

        prop_assert_eq!(hot_v1.to_bits(), fresh_v1.to_bits(),
            "reused workspace disagreed on first call");
        prop_assert_eq!(hot_v2.to_bits(), fresh_v2.to_bits(),
            "reused workspace disagreed on second call");
    }

    /// Jaro-Winkler workspace reuse must be bit-exact against a fresh
    /// workspace across arbitrary input pairs. The JW boost is deterministic
    /// on top of a bit-exact base Jaro score, so the equality is bit-for-bit.
    #[test]
    fn jw_classic_workspace_reuse_matches_fresh(
        a1 in arb_chars(),
        b1 in arb_chars(),
        a2 in arb_chars(),
        b2 in arb_chars(),
    ) {
        let alg = JaroWinkler::classic();

        let mut fresh1 = JaroWorkspace::new();
        let mut fresh2 = JaroWorkspace::new();
        let fresh_v1 = alg.similarity_with_workspace(&a1, &b1, &mut fresh1).into_inner();
        let fresh_v2 = alg.similarity_with_workspace(&a2, &b2, &mut fresh2).into_inner();

        let mut hot = JaroWorkspace::new();
        let hot_v1 = alg.similarity_with_workspace(&a1, &b1, &mut hot).into_inner();
        let hot_v2 = alg.similarity_with_workspace(&a2, &b2, &mut hot).into_inner();

        prop_assert_eq!(hot_v1.to_bits(), fresh_v1.to_bits());
        prop_assert_eq!(hot_v2.to_bits(), fresh_v2.to_bits());
    }

    /// The workspace-aware Jaro entry point must be bit-exactly equivalent
    /// to the allocating [`SimilarityMetric::similarity`] entry point.
    #[test]
    fn jaro_workspace_and_trait_agree_bit_exact(x in arb_chars(), y in arb_chars()) {
        let alg = Jaro;
        let via_trait = alg.similarity(&x, &y).into_inner();
        let mut ws = JaroWorkspace::new();
        let via_ws = alg.similarity_with_workspace(&x, &y, &mut ws).into_inner();
        prop_assert_eq!(via_trait.to_bits(), via_ws.to_bits());
    }

    /// Same equivalence for JaroWinkler::classic.
    #[test]
    fn jw_classic_workspace_and_trait_agree_bit_exact(x in arb_chars(), y in arb_chars()) {
        let alg = JaroWinkler::classic();
        let via_trait = alg.similarity(&x, &y).into_inner();
        let mut ws = JaroWorkspace::new();
        let via_ws = alg.similarity_with_workspace(&x, &y, &mut ws).into_inner();
        prop_assert_eq!(via_trait.to_bits(), via_ws.to_bits());
    }
}

/// Non-generative regression: the ULP helper's own contract. Catches an
/// accidental sign-check regression that would spuriously accept opposite
/// signs as "nearly equal".
#[test]
fn nearly_equal_rejects_opposite_signs() {
    assert!(!nearly_equal(1e-300, -1e-300));
}

/// Non-generative regression on the empty-empty corner: the identity
/// property test above uses `prop_assume!(!x.is_empty())`, which would
/// silently mask a bug in the empty-empty branch. Pin the empty-empty
/// convention here.
#[test]
fn jaro_empty_pair_is_one_bit_exact() {
    let alg = Jaro;
    let s = alg.similarity(&[] as &[char], &[] as &[char]).into_inner();
    assert_eq!(s.to_bits(), 1.0_f64.to_bits());
}
