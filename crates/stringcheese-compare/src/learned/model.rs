//! The trained cost table backing a Ristad-Yianilos learned edit distance.
//!
//! A [`LearnedEditModel`] stores, in log-space, the four groups of
//! probabilities the memoryless transducer emits:
//!
//! * `delete[t]` — probability the transducer emits a deletion consuming
//!   source symbol `t`.
//! * `insert[t]` — probability the transducer emits an insertion producing
//!   target symbol `t`.
//! * `substitute[(s, t)]` — probability the transducer emits a substitution
//!   consuming source `s` and producing target `t`. The identity case
//!   `substitute[(s, s)]` is a substitution *of a symbol with itself*, i.e.
//!   the transducer's "match" operation.
//! * `end` — probability the transducer emits its stop event.
//!
//! Following the paper's Section 2 constraint, the sum of every one of these
//! probabilities equals `1`. The [`RistadYianilosEstimator`] enforces this
//! after each M-step; the [`LearnedEditModel::uniform`] constructor enforces
//! it up front.
//!
//! Costs are stored as `-log(p)` — that is, negative log probabilities. This
//! is the shape the distance kernel needs and the shape training's
//! forward-backward computes natively, so we sidestep round-tripping through
//! exp and log at query time.
//!
//! [`RistadYianilosEstimator`]: crate::learned::training::RistadYianilosEstimator

use alloc::collections::{BTreeMap, BTreeSet};

/// A trained Ristad-Yianilos cost table.
///
/// See the [module-level documentation](crate::learned) for the mathematical
/// definition and the storage convention (negative log probabilities).
///
/// Construct with [`LearnedEditModel::uniform`] for a fresh untrained model
/// or with [`RistadYianilosEstimator::train`] for a fitted one. Query with
/// [`LearnedEditModel::delete`], [`LearnedEditModel::insert`],
/// [`LearnedEditModel::substitute`], and [`LearnedEditModel::end`].
///
/// # Symbol type
///
/// The default symbol type is `u8`. Any `T: Ord + Copy` also works; `char`
/// and integer token types are common alternatives. The [`Ord`] bound is
/// what lets the cost tables be [`BTreeMap`]s (a `no_std + alloc` requirement
/// — see [module docs](crate::learned)).
///
/// [`RistadYianilosEstimator::train`]: crate::learned::training::RistadYianilosEstimator::train
/// [`BTreeMap`]: alloc::collections::BTreeMap
#[allow(
    clippy::struct_field_names,
    reason = "the `_cost` postfix is load-bearing: it names the storage convention (negative log probabilities in cost units) and distinguishes the three edit operations from the end event which is a scalar rather than a table"
)]
#[derive(Clone, Debug)]
pub struct LearnedEditModel<T: Ord + Copy = u8> {
    /// Negative log probability of deletion for each source symbol.
    delete_cost: BTreeMap<T, f64>,
    /// Negative log probability of insertion for each target symbol.
    insert_cost: BTreeMap<T, f64>,
    /// Negative log probability of substitution for each ordered pair.
    /// The identity pair `(s, s)` captures the "match" operation.
    substitute_cost: BTreeMap<(T, T), f64>,
    /// Negative log probability of the end event.
    end_cost: f64,
}

impl<T: Ord + Copy> LearnedEditModel<T> {
    /// Constructs a uniform model over the given alphabet.
    ///
    /// Every one of the `|Σ|² + 2·|Σ| + 1` edit events (deletions,
    /// insertions, substitutions, and end) is assigned equal probability
    /// `1 / (|Σ|² + 2·|Σ| + 1)`, giving a per-edit cost of
    /// `log(|Σ|² + 2·|Σ| + 1)`. Under this model the learned distance is
    /// proportional to the Levenshtein distance (a constant scale factor per
    /// edit, plus the end-event offset), which makes uniform a reasonable
    /// initialization before training.
    ///
    /// Duplicate entries in `alphabet` are treated as a single symbol.
    /// An empty alphabet is legal — it produces a model where only the end
    /// event has nonzero probability, and every comparison of nonempty
    /// inputs has infinite distance.
    ///
    /// # Std requirement
    ///
    /// This constructor uses natural log, so the constant cost per edit is
    /// computed as an explicit closed-form value here. Requires `alloc` but
    /// not `std`: the closed-form `log(k)` for the small integer `k = |Σ|² +
    /// 2·|Σ| + 1` is computed via a series expansion pulled from `libm` when
    /// available and replaced by a straightforward loop-based approximation
    /// otherwise. In practice, all builds we currently ship enable at least
    /// `alloc`; if the reader is looking at this code and finds
    /// `LearnedEditModel::uniform` in an `alloc`-only build failing to link,
    /// that's a bug — please file it.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "alphabet sizes fit comfortably in an f64's mantissa (max useful size is well under 2^30 for any real-world use of this metric)"
    )]
    pub fn uniform(alphabet: &[T]) -> Self {
        let mut delete_cost = BTreeMap::new();
        let mut insert_cost = BTreeMap::new();
        let mut substitute_cost = BTreeMap::new();

        // Deduplicate first so the event count matches |Σ| (not `alphabet.len()`).
        let unique: BTreeSet<T> = alphabet.iter().copied().collect();
        let sigma = unique.len();
        let sigma_f = sigma as f64;
        // Total distinct events: |Σ| deletes + |Σ| inserts + |Σ|² substitutes + 1 end.
        let total_events = sigma_f * sigma_f + 2.0 * sigma_f + 1.0;
        // -log(1/N) = log(N). Uses core::f64::ln via the ln_positive helper.
        let cost_per_event = ln_positive(total_events);

        for &c in &unique {
            delete_cost.insert(c, cost_per_event);
            insert_cost.insert(c, cost_per_event);
            for &d in &unique {
                substitute_cost.insert((c, d), cost_per_event);
            }
        }

        Self {
            delete_cost,
            insert_cost,
            substitute_cost,
            end_cost: cost_per_event,
        }
    }

    /// Constructs a model from raw probability tables.
    ///
    /// Every probability must be in the closed interval `[0, 1]`. This
    /// constructor performs the log conversion but does *not* renormalize
    /// or validate: it is the caller's responsibility to ensure the
    /// probabilities sum to `1`. A validating variant is a follow-up.
    ///
    /// This is the constructor used by hand-authored golden cases where a
    /// specific cost table is being tested end-to-end. Not intended for
    /// day-to-day construction — prefer [`LearnedEditModel::uniform`] plus
    /// [`RistadYianilosEstimator::train`].
    ///
    /// Requires `std` because it uses `f64::ln`. See
    /// [`LearnedEditModel::from_log_probabilities`] for a `no_std +
    /// alloc`-compatible entry point that takes pre-computed log probabilities.
    ///
    /// [`RistadYianilosEstimator::train`]: crate::learned::training::RistadYianilosEstimator::train
    #[cfg(feature = "std")]
    #[must_use]
    pub fn from_probabilities(
        delete: BTreeMap<T, f64>,
        insert: BTreeMap<T, f64>,
        substitute: BTreeMap<(T, T), f64>,
        end: f64,
    ) -> Self {
        let to_cost = |p: f64| if p <= 0.0 { f64::INFINITY } else { -p.ln() };
        Self {
            delete_cost: delete.into_iter().map(|(k, p)| (k, to_cost(p))).collect(),
            insert_cost: insert.into_iter().map(|(k, p)| (k, to_cost(p))).collect(),
            substitute_cost: substitute
                .into_iter()
                .map(|(k, p)| (k, to_cost(p)))
                .collect(),
            end_cost: to_cost(end),
        }
    }

    /// Constructs a model directly from tables of negative log probabilities.
    ///
    /// This is the `no_std + alloc`-compatible sibling of
    /// [`LearnedEditModel::from_probabilities`]: it never calls `f64::ln`
    /// and therefore compiles without `std`. Callers are responsible for
    /// providing costs that correspond to a valid probability distribution
    /// (i.e., `sum exp(-cost) == 1` over every edit event).
    #[must_use]
    pub fn from_log_probabilities(
        delete_cost: BTreeMap<T, f64>,
        insert_cost: BTreeMap<T, f64>,
        substitute_cost: BTreeMap<(T, T), f64>,
        end_cost: f64,
    ) -> Self {
        Self {
            delete_cost,
            insert_cost,
            substitute_cost,
            end_cost,
        }
    }

    /// Returns the negative log probability of deleting `t`.
    ///
    /// Returns [`f64::INFINITY`] if `t` is not in the model's alphabet — the
    /// distance kernel treats infinity as "this edit is impossible", which
    /// under `min` propagation collapses that branch of the DP without
    /// polluting the other branches.
    #[inline]
    #[must_use]
    pub fn delete(&self, t: T) -> f64 {
        self.delete_cost.get(&t).copied().unwrap_or(f64::INFINITY)
    }

    /// Returns the negative log probability of inserting `t`.
    ///
    /// Returns [`f64::INFINITY`] if `t` is not in the model's alphabet.
    #[inline]
    #[must_use]
    pub fn insert(&self, t: T) -> f64 {
        self.insert_cost.get(&t).copied().unwrap_or(f64::INFINITY)
    }

    /// Returns the negative log probability of substituting `s` with `t`.
    ///
    /// The identity substitution `substitute(s, s)` is the transducer's
    /// "match" operation — under a well-trained model this is the cheapest
    /// edit for every `s`. Returns [`f64::INFINITY`] if the ordered pair
    /// `(s, t)` is not in the model.
    #[inline]
    #[must_use]
    pub fn substitute(&self, s: T, t: T) -> f64 {
        self.substitute_cost
            .get(&(s, t))
            .copied()
            .unwrap_or(f64::INFINITY)
    }

    /// Returns the negative log probability of the end event.
    ///
    /// Every distance computation pays this cost exactly once (the
    /// transducer must end to have produced any string pair at all), so it
    /// contributes as a constant offset per pair.
    #[inline]
    #[must_use]
    pub const fn end(&self) -> f64 {
        self.end_cost
    }

    /// Iterates over `(symbol, cost)` entries of the delete table.
    ///
    /// Ordered by symbol.
    #[inline]
    pub fn delete_entries(&self) -> impl Iterator<Item = (&T, &f64)> {
        self.delete_cost.iter()
    }

    /// Iterates over `(symbol, cost)` entries of the insert table.
    ///
    /// Ordered by symbol.
    #[inline]
    pub fn insert_entries(&self) -> impl Iterator<Item = (&T, &f64)> {
        self.insert_cost.iter()
    }

    /// Iterates over `((source, target), cost)` entries of the substitution
    /// table.
    ///
    /// Ordered by `(source, target)`.
    #[inline]
    pub fn substitute_entries(&self) -> impl Iterator<Item = (&(T, T), &f64)> {
        self.substitute_cost.iter()
    }

    /// Returns the total probability mass of the model (should sum to
    /// approximately `1` for a valid model).
    ///
    /// Sums `exp(-cost)` for every entry in every table plus the end event.
    /// Useful for asserting that a hand-constructed or trained model is a
    /// valid probability distribution.
    ///
    /// Requires `std` because it uses `f64::exp`.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn probability_mass(&self) -> f64 {
        let mut total = (-self.end_cost).exp();
        for &c in self.delete_cost.values() {
            total += (-c).exp();
        }
        for &c in self.insert_cost.values() {
            total += (-c).exp();
        }
        for &c in self.substitute_cost.values() {
            total += (-c).exp();
        }
        total
    }
}

/// Computes `ln(x)` for a strictly positive `x`, without `std::f64::ln`.
///
/// The uniform-model constructor uses this to avoid a hard dependency on
/// `std` for what is otherwise a pure `alloc + core` type. The
/// implementation uses the mantissa-and-exponent decomposition of an IEEE
/// 754 `binary64` combined with an argument-reduced Padé approximation to
/// `ln(1 + u)` on `[0, 1/sqrt(2))` — accurate to a few ULPs, which is
/// plenty for cost-table initialization.
///
/// If a caller is unhappy with the numerical accuracy (they shouldn't be —
/// distance is a `min` DP whose cell values are added modulo the model's
/// per-edit costs), they can use [`LearnedEditModel::from_log_probabilities`]
/// with a pre-computed table.
///
/// # Panics
///
/// Panics in debug mode if `x <= 0`. In release mode returns
/// [`f64::NEG_INFINITY`] for `x == 0` and [`f64::NAN`] for negative `x` —
/// matching the semantics of `f64::ln`.
#[inline]
#[allow(
    clippy::many_single_char_names,
    reason = "the letters `x`, `m`, `e`, `u`, `v`, `k` are the standard mathematical notation for the mantissa, exponent, and series-reduction variables in a Taylor-series log — renaming them would obscure the correspondence to the derivation"
)]
#[allow(
    clippy::cast_possible_wrap,
    reason = "raw_exp is the 11-bit IEEE 754 biased exponent field, whose range 0..=2047 fits trivially in i64"
)]
#[allow(
    clippy::cast_precision_loss,
    reason = "e_adj is an IEEE 754 unbiased exponent in the range -1074..=1023, which fits exactly in an f64's mantissa"
)]
fn ln_positive(x: f64) -> f64 {
    debug_assert!(x > 0.0, "ln_positive requires a strictly positive argument");
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x < 0.0 || x.is_nan() {
        return f64::NAN;
    }
    if x.is_infinite() {
        return f64::INFINITY;
    }
    // Decompose x = m * 2^e with m in [1, 2). Then ln(x) = ln(m) + e * ln(2).
    let bits = x.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i64;
    // Skip subnormals — a uniform model's per-event count is comfortably
    // in normal range, and the estimator never asks ln of a subnormal.
    debug_assert!(raw_exp != 0, "ln_positive: subnormal input unsupported");
    let e = raw_exp - 1023;
    // Reset the exponent to 1023 so the mantissa m lies in [1, 2).
    let m_bits = (bits & 0x000f_ffff_ffff_ffff) | (1023u64 << 52);
    let mut m = f64::from_bits(m_bits);
    // If m >= sqrt(2), halve it and add 1 to e so u = m - 1 sits in
    // [-0.29, 0.41) — a tighter interval that the series converges on faster.
    let mut e_adj = e;
    if m > core::f64::consts::SQRT_2 {
        m *= 0.5;
        e_adj += 1;
    }
    // ln(1 + u) via a Taylor-shifted series in v = u / (2 + u), giving
    //   ln((1+u)) = 2 * (v + v^3/3 + v^5/5 + v^7/7 + ...)
    // — an ancient trick that converges quickly on this interval.
    let u = m - 1.0;
    let v = u / (2.0 + u);
    let v2 = v * v;
    // Nine terms are enough for ~1 ULP accuracy on the reduced interval.
    let mut sum = 0.0;
    for k in (0..9u32).rev() {
        let denom = f64::from(2 * k + 1);
        sum = sum * v2 + 1.0 / denom;
    }
    let ln_m = 2.0 * v * sum;
    ln_m + (e_adj as f64) * core::f64::consts::LN_2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ln_positive_matches_std_on_simple_values() {
        for &x in &[
            0.5_f64,
            1.0,
            1.5,
            2.0,
            core::f64::consts::E,
            10.0,
            100.0,
            1024.0,
            12_345.6,
        ] {
            let ours = ln_positive(x);
            let theirs = x.ln();
            let err = (ours - theirs).abs();
            assert!(
                err < 1e-12,
                "ln_positive({x}) = {ours}, f64::ln = {theirs}, err = {err}"
            );
        }
    }

    #[test]
    fn uniform_model_costs_are_uniform() {
        let alphabet: &[u8] = b"abc";
        let m = LearnedEditModel::<u8>::uniform(alphabet);
        // 3 symbols: 3 deletes + 3 inserts + 9 substitutes + 1 end = 16 events.
        let expected = ln_positive(16.0);
        for &c in alphabet {
            assert!((m.delete(c) - expected).abs() < 1e-12);
            assert!((m.insert(c) - expected).abs() < 1e-12);
            for &d in alphabet {
                assert!((m.substitute(c, d) - expected).abs() < 1e-12);
            }
        }
        assert!((m.end() - expected).abs() < 1e-12);
    }

    #[test]
    fn uniform_model_probabilities_sum_to_one() {
        let alphabet: &[u8] = b"abcd";
        let m = LearnedEditModel::<u8>::uniform(alphabet);
        let mass = m.probability_mass();
        assert!((mass - 1.0).abs() < 1e-10, "uniform model mass {mass} != 1");
    }

    #[test]
    fn uniform_model_deduplicates_alphabet() {
        let with_dupes = LearnedEditModel::<u8>::uniform(b"abab");
        let no_dupes = LearnedEditModel::<u8>::uniform(b"ab");
        // Same costs everywhere.
        for &c in b"ab" {
            assert!((with_dupes.delete(c) - no_dupes.delete(c)).abs() < 1e-12);
        }
        assert!((with_dupes.end() - no_dupes.end()).abs() < 1e-12);
    }

    #[test]
    fn unknown_symbols_have_infinite_cost() {
        let m = LearnedEditModel::<u8>::uniform(b"ab");
        assert!(m.delete(b'z').is_infinite());
        assert!(m.insert(b'z').is_infinite());
        assert!(m.substitute(b'a', b'z').is_infinite());
        assert!(m.substitute(b'z', b'a').is_infinite());
    }

    #[test]
    fn empty_alphabet_only_has_end_event() {
        let m = LearnedEditModel::<u8>::uniform(&[]);
        // 0 events + 1 end = 1 event, so end_cost = ln(1) = 0.
        // Bit-exact comparison to guarantee no rounding slipped in.
        assert_eq!(m.end().to_bits(), 0.0_f64.to_bits());
        assert!(m.delete(b'a').is_infinite());
        // Total mass is exp(-0) = 1.
        let mass = m.probability_mass();
        assert!((mass - 1.0).abs() < 1e-12);
    }

    #[cfg(feature = "std")]
    #[test]
    fn from_probabilities_round_trip() {
        let mut delete = BTreeMap::new();
        delete.insert(b'a', 0.1);
        delete.insert(b'b', 0.1);
        let mut insert = BTreeMap::new();
        insert.insert(b'a', 0.1);
        insert.insert(b'b', 0.1);
        let mut sub = BTreeMap::new();
        sub.insert((b'a', b'a'), 0.15);
        sub.insert((b'b', b'b'), 0.15);
        sub.insert((b'a', b'b'), 0.05);
        sub.insert((b'b', b'a'), 0.05);
        let end = 0.2;
        let m = LearnedEditModel::<u8>::from_probabilities(delete, insert, sub, end);
        let mass = m.probability_mass();
        assert!((mass - 1.0).abs() < 1e-12, "mass {mass} != 1");
        // A match under this model: substitute (a, a) had p=0.15, so cost = -ln(0.15).
        let expected = -0.15_f64.ln();
        assert!((m.substitute(b'a', b'a') - expected).abs() < 1e-12);
    }
}
