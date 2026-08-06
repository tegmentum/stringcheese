//! Expectation-Maximization training for the Ristad-Yianilos memoryless
//! transducer.
//!
//! Given a set of labeled `(source, target)` string pairs — every pair
//! interpreted as a positive (matching) example — EM iterates two steps
//! until the training-set log-likelihood stops improving:
//!
//! 1. **E-step**: for each pair, run the forward-backward algorithm and
//!    compute expected counts of every edit event (delete of each symbol,
//!    insert of each symbol, substitute of each ordered pair, end).
//! 2. **M-step**: replace the model's per-event probabilities with the
//!    normalized expected counts.
//!
//! The paper's Section 3 gives the equations; this module is a direct
//! translation, with two implementation choices worth calling out:
//!
//! * **Log-space arithmetic throughout.** The forward and backward
//!   probabilities decay geometrically with string length, so representing
//!   them as `f64` in linear space underflows before you get halfway
//!   through a hundred-character pair. Everything is stored and combined
//!   in log-space via the log-sum-exp trick, which is a hard requirement
//!   for numerical correctness — nothing else lets a straight
//!   implementation of the paper's equations produce meaningful results on
//!   realistic inputs.
//! * **Forward-backward memoized as flat matrices.** The alpha and beta
//!   tables are `(m + 1) × (n + 1)` `Vec<f64>`s laid out row-major. This
//!   is `O(m · n)` memory per pair, freed at the end of each pair — a
//!   caller with a very long single pair would prefer a streaming
//!   variant, but for training on realistic pair-lengths the flat form is
//!   easier to reason about than a rolling-row variant would be.
//!
//! # Std gate
//!
//! `f64::ln` and `f64::exp` live in `std`. The whole module is
//! `#![cfg(feature = "std")]`; under `--no-default-features --features
//! alloc` the estimator disappears entirely, and callers get only the
//! distance-side surface (model construction from log probabilities plus
//! the distance kernel).

#![cfg(feature = "std")]

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use crate::learned::model::LearnedEditModel;

/// EM estimator for a Ristad-Yianilos [`LearnedEditModel`].
///
/// Configured with a fixed alphabet (the set of symbols the model can
/// represent), a maximum iteration count, and a log-likelihood convergence
/// threshold. Runs [`RistadYianilosEstimator::train`] on a collection of
/// labeled pairs to fit a model.
///
/// # Alphabet
///
/// The estimator's alphabet fixes the model's rows and columns. Every
/// symbol in every training pair must be in the alphabet, or the pair
/// contributes zero probability (and the estimator skips it with a
/// warning by log). In practice, callers pre-scan their training set and
/// pass every distinct symbol seen.
///
/// # Convergence
///
/// The estimator tracks the training-set log-likelihood between iterations
/// and stops as soon as the improvement is below `convergence_threshold`
/// (default `1e-4`) or `max_iterations` iterations have run (default
/// `100`). EM's log-likelihood is monotone non-decreasing across
/// iterations by construction — that invariant is exercised by the
/// property tests.
///
/// # Builder pattern
///
/// The tuning knobs are exposed as consuming setters that return `Self`,
/// so they can be chained fluently:
///
/// ```
/// use stringcheese_compare::learned::RistadYianilosEstimator;
///
/// let estimator = RistadYianilosEstimator::<u8>::new(b"abc".to_vec())
///     .max_iterations(50)
///     .convergence_threshold(1e-5);
/// ```
#[derive(Clone, Debug)]
pub struct RistadYianilosEstimator<T: Ord + Copy = u8> {
    alphabet: Vec<T>,
    max_iterations: usize,
    convergence_threshold: f64,
}

impl<T: Ord + Copy> RistadYianilosEstimator<T> {
    /// Constructs an estimator over the given alphabet.
    ///
    /// Duplicate entries in `alphabet` are treated as a single symbol.
    ///
    /// Defaults: `max_iterations = 100`, `convergence_threshold = 1e-4`.
    #[must_use]
    pub fn new(alphabet: Vec<T>) -> Self {
        Self {
            alphabet,
            max_iterations: 100,
            convergence_threshold: 1e-4,
        }
    }

    /// Sets the maximum number of EM iterations.
    ///
    /// Ignored during a single [`train`](Self::train) call once the
    /// convergence threshold is hit. Setting this to `0` runs no iterations
    /// and returns the uniform initialization unchanged.
    #[must_use]
    pub const fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// Sets the log-likelihood convergence threshold.
    ///
    /// Training stops as soon as `log_likelihood[k] - log_likelihood[k-1]
    /// < eps`. Set to `0.0` (or negative) to always run
    /// `max_iterations`.
    #[must_use]
    pub const fn convergence_threshold(mut self, eps: f64) -> Self {
        self.convergence_threshold = eps;
        self
    }

    /// Returns the alphabet the estimator will train over.
    #[inline]
    #[must_use]
    pub fn alphabet(&self) -> &[T] {
        &self.alphabet
    }

    /// Trains a [`LearnedEditModel`] from the given labeled pairs.
    ///
    /// Iterates E-step (forward-backward over each pair) and M-step
    /// (renormalize edit probabilities from expected counts) until
    /// convergence or `max_iterations` iterations have run. Returns the
    /// final trained model.
    ///
    /// Pairs that the current model assigns zero probability to (typically
    /// because they contain a symbol not in the alphabet) are skipped
    /// silently — a warning-log surface is a follow-up when the crate
    /// grows one.
    ///
    /// # Panics
    ///
    /// Does not panic on any well-formed input. An `alphabet` that is
    /// empty produces a degenerate model whose `end_cost` is `0` and every
    /// other cost is `+inf`; the pair-loop skips every nonempty pair and
    /// the model is unchanged.
    #[must_use]
    pub fn train(&self, pairs: &[(&[T], &[T])]) -> LearnedEditModel<T> {
        let mut model = LearnedEditModel::<T>::uniform(&self.alphabet);

        if self.max_iterations == 0 || pairs.is_empty() || self.alphabet.is_empty() {
            return model;
        }

        let mut prev_log_likelihood = f64::NEG_INFINITY;
        for _iter in 0..self.max_iterations {
            let (next_model, log_likelihood) = self.one_em_step(&model, pairs);
            model = next_model;
            // The very first iteration cannot improve on -inf, so we always
            // accept it. Subsequent iterations check the improvement threshold.
            if prev_log_likelihood.is_finite()
                && (log_likelihood - prev_log_likelihood) < self.convergence_threshold
            {
                break;
            }
            prev_log_likelihood = log_likelihood;
        }

        model
    }

    /// One EM step: E-step accumulates expected counts, M-step renormalizes
    /// them into a fresh model. Returns the new model and the log-likelihood
    /// of the training set under the *old* model (the standard EM
    /// convention — this is the value that must be non-decreasing across
    /// iterations).
    ///
    /// Crate-visible so the property-test module can exercise the
    /// iteration-by-iteration log-likelihood monotonicity invariant
    /// directly rather than reconstructing it from a series of `train`
    /// calls.
    #[allow(
        clippy::too_many_lines,
        reason = "the E-step and M-step read most naturally as a single continuous function — splitting them mid-derivation would force the reader to jump between two symbols on every trace-through of the algorithm"
    )]
    pub(crate) fn one_em_step(
        &self,
        model: &LearnedEditModel<T>,
        pairs: &[(&[T], &[T])],
    ) -> (LearnedEditModel<T>, f64) {
        // Log-space accumulators; every entry starts at -inf ("no count yet").
        let mut log_del_count: BTreeMap<T, f64> = BTreeMap::new();
        let mut log_ins_count: BTreeMap<T, f64> = BTreeMap::new();
        let mut log_sub_count: BTreeMap<(T, T), f64> = BTreeMap::new();
        let mut log_end_count = f64::NEG_INFINITY;
        for &c in &self.alphabet {
            log_del_count.entry(c).or_insert(f64::NEG_INFINITY);
            log_ins_count.entry(c).or_insert(f64::NEG_INFINITY);
            for &d in &self.alphabet {
                log_sub_count.entry((c, d)).or_insert(f64::NEG_INFINITY);
            }
        }

        let mut total_log_likelihood = 0.0;
        for &(source, target) in pairs {
            let m = source.len();
            let n = target.len();
            let alpha = forward(source, target, model);
            let beta = backward(source, target, model);
            let cols = n + 1;
            let log_z = alpha[m * cols + n] + (-model.end());

            if !log_z.is_finite() {
                // Pair has zero probability under the current model — skip.
                continue;
            }
            total_log_likelihood += log_z;

            // Delete-count contributions: for every (i, j) with i >= 1,
            //   γ_del(i, j) = α(i-1, j) · P(del s[i-1]) · β(i, j) / Z.
            for i in 1..=m {
                let c = source[i - 1];
                let log_p = -model.delete(c);
                if !log_p.is_finite() {
                    continue;
                }
                for j in 0..=n {
                    let contribution =
                        alpha[(i - 1) * cols + j] + log_p + beta[i * cols + j] - log_z;
                    let entry = log_del_count.entry(c).or_insert(f64::NEG_INFINITY);
                    *entry = log_sum_exp(*entry, contribution);
                }
            }

            // Insert-count contributions.
            for j in 1..=n {
                let c = target[j - 1];
                let log_p = -model.insert(c);
                if !log_p.is_finite() {
                    continue;
                }
                for i in 0..=m {
                    let contribution =
                        alpha[i * cols + (j - 1)] + log_p + beta[i * cols + j] - log_z;
                    let entry = log_ins_count.entry(c).or_insert(f64::NEG_INFINITY);
                    *entry = log_sum_exp(*entry, contribution);
                }
            }

            // Substitute-count contributions.
            for i in 1..=m {
                for j in 1..=n {
                    let s = source[i - 1];
                    let t = target[j - 1];
                    let log_p = -model.substitute(s, t);
                    if !log_p.is_finite() {
                        continue;
                    }
                    let contribution =
                        alpha[(i - 1) * cols + (j - 1)] + log_p + beta[i * cols + j] - log_z;
                    let entry = log_sub_count.entry((s, t)).or_insert(f64::NEG_INFINITY);
                    *entry = log_sum_exp(*entry, contribution);
                }
            }

            // End-event: exactly one expected end per pair. log(1) = 0.
            log_end_count = log_sum_exp(log_end_count, 0.0);
        }

        // M-step: normalize into probabilities. Compute the total log-count
        // as the log-sum-exp of every event's log-count, then subtract to
        // get per-event log probabilities and negate for costs.
        let mut all_log_counts: Vec<f64> =
            Vec::with_capacity(log_del_count.len() + log_ins_count.len() + log_sub_count.len() + 1);
        all_log_counts.extend(log_del_count.values().copied());
        all_log_counts.extend(log_ins_count.values().copied());
        all_log_counts.extend(log_sub_count.values().copied());
        all_log_counts.push(log_end_count);
        let log_total = log_sum_exp_slice(&all_log_counts);

        // Guard against a degenerate total (every count -inf — shouldn't
        // happen after we always add 1 for end unless no pair contributed).
        let log_total = if log_total.is_finite() {
            log_total
        } else {
            // Fall back to the old model so we don't produce NaN costs.
            return (model.clone(), total_log_likelihood);
        };

        let mut new_delete = BTreeMap::new();
        for (c, log_count) in log_del_count {
            let cost = if log_count.is_finite() {
                log_total - log_count
            } else {
                f64::INFINITY
            };
            new_delete.insert(c, cost);
        }
        let mut new_insert = BTreeMap::new();
        for (c, log_count) in log_ins_count {
            let cost = if log_count.is_finite() {
                log_total - log_count
            } else {
                f64::INFINITY
            };
            new_insert.insert(c, cost);
        }
        let mut new_sub = BTreeMap::new();
        for (k, log_count) in log_sub_count {
            let cost = if log_count.is_finite() {
                log_total - log_count
            } else {
                f64::INFINITY
            };
            new_sub.insert(k, cost);
        }
        let end_cost = if log_end_count.is_finite() {
            log_total - log_end_count
        } else {
            f64::INFINITY
        };

        let next =
            LearnedEditModel::from_log_probabilities(new_delete, new_insert, new_sub, end_cost);
        (next, total_log_likelihood)
    }
}

/// Forward algorithm in log-space.
///
/// Returns `alpha[i * (n + 1) + j] = log P(consumed first i of source, emitted
/// first j of target, not yet ended)`.
fn forward<T: Ord + Copy>(source: &[T], target: &[T], model: &LearnedEditModel<T>) -> Vec<f64> {
    let m = source.len();
    let n = target.len();
    let cols = n + 1;
    let mut alpha = vec![f64::NEG_INFINITY; (m + 1) * cols];
    alpha[0] = 0.0; // log(1) = 0

    // First column: only deletions.
    for i in 1..=m {
        let log_p = -model.delete(source[i - 1]);
        alpha[i * cols] = alpha[(i - 1) * cols] + log_p;
    }
    // First row: only insertions.
    for j in 1..=n {
        let log_p = -model.insert(target[j - 1]);
        alpha[j] = alpha[j - 1] + log_p;
    }

    for i in 1..=m {
        for j in 1..=n {
            let log_p_del = -model.delete(source[i - 1]);
            let log_p_ins = -model.insert(target[j - 1]);
            let log_p_sub = -model.substitute(source[i - 1], target[j - 1]);
            let del = alpha[(i - 1) * cols + j] + log_p_del;
            let ins = alpha[i * cols + (j - 1)] + log_p_ins;
            let sub = alpha[(i - 1) * cols + (j - 1)] + log_p_sub;
            alpha[i * cols + j] = log_sum_exp3(del, ins, sub);
        }
    }
    alpha
}

/// Backward algorithm in log-space.
///
/// Returns `beta[i * (n + 1) + j] = log P(will consume source[i..], emit
/// target[j..], then end)`.
fn backward<T: Ord + Copy>(source: &[T], target: &[T], model: &LearnedEditModel<T>) -> Vec<f64> {
    let m = source.len();
    let n = target.len();
    let cols = n + 1;
    let mut beta = vec![f64::NEG_INFINITY; (m + 1) * cols];
    beta[m * cols + n] = -model.end();

    // Last column: only deletions from (i, n) back to (m, n).
    for i in (0..m).rev() {
        let log_p = -model.delete(source[i]);
        beta[i * cols + n] = beta[(i + 1) * cols + n] + log_p;
    }
    // Last row: only insertions.
    for j in (0..n).rev() {
        let log_p = -model.insert(target[j]);
        beta[m * cols + j] = beta[m * cols + (j + 1)] + log_p;
    }

    for i in (0..m).rev() {
        for j in (0..n).rev() {
            let log_p_del = -model.delete(source[i]);
            let log_p_ins = -model.insert(target[j]);
            let log_p_sub = -model.substitute(source[i], target[j]);
            let del = beta[(i + 1) * cols + j] + log_p_del;
            let ins = beta[i * cols + (j + 1)] + log_p_ins;
            let sub = beta[(i + 1) * cols + (j + 1)] + log_p_sub;
            beta[i * cols + j] = log_sum_exp3(del, ins, sub);
        }
    }
    beta
}

/// The log-sum-exp trick: computes `log(exp(a) + exp(b))` without
/// intermediate overflow or underflow.
///
/// Handles the sentinel cases explicitly:
///   * If either argument is `-inf` the other wins.
///   * If both are `-inf` the result is `-inf` (log(0)).
#[inline]
fn log_sum_exp(a: f64, b: f64) -> f64 {
    if a == f64::NEG_INFINITY {
        return b;
    }
    if b == f64::NEG_INFINITY {
        return a;
    }
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    hi + (1.0 + (lo - hi).exp()).ln()
}

/// Three-way log-sum-exp — the DP cell recurrence's arithmetic.
#[inline]
fn log_sum_exp3(a: f64, b: f64, c: f64) -> f64 {
    log_sum_exp(log_sum_exp(a, b), c)
}

/// Log-sum-exp over an arbitrary slice.
///
/// Reduces via the pairwise combinator; O(n) work, and numerically the same
/// as the classical "subtract the max, exp, sum, log, add back the max"
/// formulation modulo associativity of the intermediate steps.
fn log_sum_exp_slice(xs: &[f64]) -> f64 {
    let mut acc = f64::NEG_INFINITY;
    for &x in xs {
        acc = log_sum_exp(acc, x);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_sum_exp_matches_definition() {
        // log(exp(1) + exp(2)) = 2 + log(1 + exp(-1))
        let observed = log_sum_exp(1.0, 2.0);
        let expected = (1.0_f64.exp() + 2.0_f64.exp()).ln();
        assert!((observed - expected).abs() < 1e-12);
    }

    #[test]
    fn log_sum_exp_handles_infinities() {
        // Bit-exact comparisons for the sentinel-handling paths — this
        // isn't computing anything, it's plumbing.
        assert_eq!(
            log_sum_exp(f64::NEG_INFINITY, 3.0).to_bits(),
            3.0_f64.to_bits()
        );
        assert_eq!(
            log_sum_exp(3.0, f64::NEG_INFINITY).to_bits(),
            3.0_f64.to_bits()
        );
        assert_eq!(
            log_sum_exp(f64::NEG_INFINITY, f64::NEG_INFINITY).to_bits(),
            f64::NEG_INFINITY.to_bits()
        );
    }

    #[test]
    fn forward_backward_agree_on_total() {
        // For a valid pair, log_alpha(m, n) + log(P_end) should equal
        // log_beta(0, 0). This is the classical forward-backward
        // consistency check — a bug in either would silently pass unit
        // tests otherwise.
        let model = LearnedEditModel::<u8>::uniform(b"abc");
        for (s, t) in [
            (b"a".as_ref(), b"a".as_ref()),
            (b"ab", b"ac"),
            (b"abc", b"cba"),
            (b"aab", b"abb"),
        ] {
            let alpha = forward(s, t, &model);
            let beta = backward(s, t, &model);
            let m = s.len();
            let n = t.len();
            let cols = n + 1;
            let log_z_forward = alpha[m * cols + n] + (-model.end());
            let log_z_backward = beta[0];
            assert!(
                (log_z_forward - log_z_backward).abs() < 1e-10,
                "on ({s:?}, {t:?}): forward={log_z_forward}, backward={log_z_backward}"
            );
        }
    }

    #[test]
    fn training_on_identity_pairs_makes_matches_cheap() {
        // Train on many (x, x) pairs. The model should learn that identity
        // substitutions are much more likely than insertion, deletion, or
        // non-identity substitutions.
        let alphabet: Vec<u8> = b"abc".to_vec();
        let est = RistadYianilosEstimator::new(alphabet)
            .max_iterations(30)
            .convergence_threshold(1e-6);
        let pairs: Vec<(&[u8], &[u8])> = vec![
            (b"a", b"a"),
            (b"b", b"b"),
            (b"c", b"c"),
            (b"ab", b"ab"),
            (b"bc", b"bc"),
            (b"ac", b"ac"),
            (b"abc", b"abc"),
            (b"bca", b"bca"),
            (b"cab", b"cab"),
        ];
        let trained = est.train(&pairs);

        // For each symbol c, substitute(c, c) should be strictly cheaper
        // than substitute(c, d) for any d != c.
        for &c in b"abc" {
            let identity_cost = trained.substitute(c, c);
            for &d in b"abc" {
                if d != c {
                    let mismatch_cost = trained.substitute(c, d);
                    assert!(
                        identity_cost < mismatch_cost,
                        "identity ({c}->{c}) cost {identity_cost} is not cheaper than mismatch ({c}->{d}) cost {mismatch_cost}"
                    );
                }
            }
            // And cheaper than insert or delete.
            let del_cost = trained.delete(c);
            let ins_cost = trained.insert(c);
            assert!(
                identity_cost < del_cost,
                "identity ({c}->{c}) cost {identity_cost} is not cheaper than delete {del_cost}"
            );
            assert!(
                identity_cost < ins_cost,
                "identity ({c}->{c}) cost {identity_cost} is not cheaper than insert {ins_cost}"
            );
        }
    }

    #[test]
    fn trained_model_probabilities_sum_to_one() {
        let est = RistadYianilosEstimator::new(b"abc".to_vec()).max_iterations(10);
        let pairs: Vec<(&[u8], &[u8])> = vec![
            (b"abc", b"abc"),
            (b"abc", b"acb"),
            (b"ab", b"aab"),
            (b"bc", b"bbc"),
        ];
        let trained = est.train(&pairs);
        let mass = trained.probability_mass();
        assert!((mass - 1.0).abs() < 1e-9, "trained model mass {mass} != 1");
    }

    #[test]
    fn zero_iterations_returns_uniform() {
        let est = RistadYianilosEstimator::<u8>::new(b"ab".to_vec()).max_iterations(0);
        let pairs: Vec<(&[u8], &[u8])> = vec![(b"a", b"a")];
        let trained = est.train(&pairs);
        let uniform = LearnedEditModel::<u8>::uniform(b"ab");
        // Every entry should match the uniform initialization.
        for &c in b"ab" {
            assert!((trained.delete(c) - uniform.delete(c)).abs() < 1e-12);
            assert!((trained.insert(c) - uniform.insert(c)).abs() < 1e-12);
            for &d in b"ab" {
                assert!((trained.substitute(c, d) - uniform.substitute(c, d)).abs() < 1e-12);
            }
        }
        assert!((trained.end() - uniform.end()).abs() < 1e-12);
    }

    #[test]
    fn log_likelihood_non_decreasing() {
        // Explicit iteration-by-iteration check — the load-bearing
        // correctness invariant of EM.
        let est = RistadYianilosEstimator::new(b"abc".to_vec()).max_iterations(1);
        let pairs: Vec<(&[u8], &[u8])> = vec![(b"abc", b"abc"), (b"abc", b"aac"), (b"ab", b"abb")];
        let mut model = LearnedEditModel::<u8>::uniform(b"abc");
        let mut previous_ll = f64::NEG_INFINITY;
        for _ in 0..15 {
            let (next, ll) = est.one_em_step(&model, &pairs);
            if previous_ll.is_finite() {
                // Allow a tiny slack for floating-point roundoff.
                assert!(
                    ll >= previous_ll - 1e-9,
                    "log-likelihood decreased: {previous_ll} -> {ll}"
                );
            }
            model = next;
            previous_ll = ll;
        }
    }
}
