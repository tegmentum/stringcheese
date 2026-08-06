//! Property-based tests for the Ristad-Yianilos learned edit distance.
//!
//! Three invariants matter enough to be exercised on generated inputs:
//!
//! 1. **Non-negativity** — `distance(s, t) >= 0` for every pair `(s, t)`.
//!    This is unconditional; a violation is a bug in the DP kernel or a
//!    corrupted model.
//! 2. **Identity-vs-mismatch under a trained model** — after training on
//!    identity pairs, `distance(x, x) <= distance(x, y)` for `y != x`
//!    over the alphabet. This is the load-bearing behavioral claim of the
//!    whole learning procedure — if it fails, the estimator hasn't
//!    learned anything.
//! 3. **EM log-likelihood monotonicity** — across successive EM
//!    iterations on the same training set, the log-likelihood is
//!    non-decreasing. This is EM's classical correctness invariant, and
//!    a failure means the E- or M-step is wrong.

use proptest::prelude::*;

use crate::learned::distance::LearnedEdit;
use crate::learned::model::LearnedEditModel;
use crate::learned::training::RistadYianilosEstimator;

fn arb_abc_bytes() -> impl Strategy<Value = alloc::vec::Vec<u8>> {
    proptest::collection::vec(prop_oneof![Just(b'a'), Just(b'b'), Just(b'c')], 0..8)
}

proptest! {
    /// Distance is always non-negative — the invariant that must hold for
    /// every trained model, every pair.
    #[test]
    fn distance_non_negative(a in arb_abc_bytes(), b in arb_abc_bytes()) {
        let alg = LearnedEdit::new(LearnedEditModel::<u8>::uniform(b"abc"));
        let d = alg.compute_distance(&a, &b);
        prop_assert!(d >= 0.0, "distance was negative: {d}");
    }

    /// Under a model trained on identity pairs, `distance(s, s) <=
    /// distance(s, t)` for any t drawn from the alphabet.
    ///
    /// This is a weaker form of "identity of indiscernibles" — d(s, s)
    /// isn't zero (the end event has nonzero cost), but it should be the
    /// *minimum* over any t of the same alphabet-shape.
    #[test]
    fn identity_at_most_other(s in arb_abc_bytes(), t in arb_abc_bytes()) {
        // Pre-train inside the property so proptest's shrinker doesn't
        // reuse a stale model. Training is fast on this alphabet.
        let est = RistadYianilosEstimator::new(b"abc".to_vec())
            .max_iterations(25)
            .convergence_threshold(1e-6);
        let identity_pairs: &[(&[u8], &[u8])] = &[
            (b"a", b"a"),
            (b"b", b"b"),
            (b"c", b"c"),
            (b"ab", b"ab"),
            (b"ba", b"ba"),
            (b"abc", b"abc"),
            (b"cba", b"cba"),
            (b"aa", b"aa"),
            (b"bb", b"bb"),
        ];
        let model = est.train(identity_pairs);
        let alg = LearnedEdit::new(model);
        let d_ss = alg.compute_distance(&s, &s);
        let d_st = alg.compute_distance(&s, &t);
        prop_assert!(
            d_ss <= d_st + 1e-9,
            "identity vs mismatch violated: d(s,s)={d_ss}, d(s,t)={d_st}, s={s:?}, t={t:?}"
        );
    }
}

/// EM log-likelihood is non-decreasing across iterations. This is a
/// deterministic assertion rather than a property test — the training
/// data is fixed and there is no randomness — but it's the single most
/// important correctness invariant of the estimator, so it lives here
/// alongside the property tests where a reader looking for correctness
/// tests will find it.
#[test]
fn em_log_likelihood_is_non_decreasing_over_many_iterations() {
    let est = RistadYianilosEstimator::new(b"abc".to_vec());
    let pairs: &[(&[u8], &[u8])] = &[
        (b"abc", b"abc"),
        (b"abc", b"acb"),
        (b"ab", b"aab"),
        (b"bc", b"bbc"),
        (b"acbcab", b"acbcab"),
    ];
    let mut model = LearnedEditModel::<u8>::uniform(b"abc");
    let mut previous_ll = f64::NEG_INFINITY;
    for iter in 0..30 {
        let (next_model, ll) = est.one_em_step(&model, pairs);
        if previous_ll.is_finite() {
            assert!(
                ll >= previous_ll - 1e-9,
                "log-likelihood decreased at iter {iter}: {previous_ll} -> {ll}"
            );
        }
        model = next_model;
        previous_ll = ll;
    }
}
