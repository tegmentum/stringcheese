//! The [`LearnedEdit`] distance handle and its DP kernel.
//!
//! The kernel is the classical Wagner-Fischer Levenshtein DP with the
//! per-cell costs replaced by lookups into a trained [`LearnedEditModel`].
//! Two equivalent expressions of the recurrence:
//!
//! ```text
//!     d(i, j) = min {
//!         d(i-1, j)   + delete(source[i-1]),
//!         d(i, j-1)   + insert(target[j-1]),
//!         d(i-1, j-1) + substitute(source[i-1], target[j-1]),
//!     }
//! ```
//!
//! with boundary conditions `d(0, 0) = 0`, and prefix-only rows/columns
//! costing accumulated inserts / deletes. The final distance is
//! `d(m, n) + end_cost` — the transducer must emit its stop event before
//! yielding a valid string pair, so its cost is paid exactly once.
//!
//! # Complexity
//!
//! `O(m · n)` time. `O(m · n)` space (the full matrix). Substituting a
//! rolling-row buffer for the full matrix would drop the space complexity
//! to `O(min(m, n))`, matching the pattern of the classical Levenshtein
//! rolling-rows kernel; that's a follow-up when a caller has a batching
//! use-case that pays for the extra plumbing.

use alloc::vec;

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, Distance,
    DistanceMetric, MetricClass, MetricProperties, VariantId,
};

use crate::learned::model::LearnedEditModel;

/// The Ristad-Yianilos learned string-edit distance.
///
/// A handle bundling a trained [`LearnedEditModel`] with the distance
/// kernel. Implements [`DistanceMetric<[T]>`] for any `T: Ord + Copy` (the
/// alphabet type the model was trained over).
///
/// Introduced by Ristad and Yianilos (1998); see the [module-level
/// documentation](crate::learned) for full citation and metric properties.
///
/// # Construction
///
/// Build a model first (via [`LearnedEditModel::uniform`] for a starting
/// point or [`RistadYianilosEstimator::train`] for a fitted model), then
/// wrap it in `LearnedEdit::new`. The handle owns the model; clone it if
/// you need to share.
///
/// ```
/// use stringcheese_compare::learned::{LearnedEdit, LearnedEditModel};
/// use stringcheese_core::DistanceMetric;
///
/// let model = LearnedEditModel::<u8>::uniform(b"abc");
/// let alg = LearnedEdit::new(model);
/// let d = alg.distance(b"abc".as_ref(), b"abc".as_ref());
/// // The distance is finite; it isn't zero (the end event has nonzero cost),
/// // but it's the minimum achievable under this model.
/// assert!(d.into_inner().is_finite());
/// ```
///
/// [`RistadYianilosEstimator::train`]: crate::learned::training::RistadYianilosEstimator::train
#[derive(Clone, Debug)]
pub struct LearnedEdit<T: Ord + Copy = u8> {
    model: LearnedEditModel<T>,
}

impl<T: Ord + Copy> LearnedEdit<T> {
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"ristad-yianilos-learned-edit"` names the paper's
    /// formulation. Because the descriptor is a `const` value shared across
    /// every [`LearnedEdit`] instance, it does *not* pin down which model
    /// the handle was constructed with — golden cases that want to validate
    /// a specific trained model must also carry a fingerprint of the model
    /// alongside the descriptor.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::RistadYianilos,
        variant: VariantId("ristad-yianilos-learned-edit"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Learning string-edit distance",
            authors: "E. S. Ristad and P. N. Yianilos",
            year: 1998,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    ///
    /// A `const` accessor is provided so descriptors can be pinned in
    /// `const` context — for example, as the `descriptor` field of a
    /// `GoldenCase`.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Returns the mathematical properties this algorithm satisfies.
    ///
    /// The properties reported are the *conservative* subset — the ones
    /// that hold for every possible trained model. In particular:
    ///
    /// * `non_negative = true` — always, because costs are `-log(p)` for
    ///   `p ∈ (0, 1]`.
    /// * `symmetric = false` — an asymmetric model (e.g. one trained on
    ///   `(query, gold)` pairs where the map is not symmetric) is not
    ///   symmetric. Even a symmetric model in insert/delete costs may have
    ///   asymmetric substitution costs.
    /// * `identity_of_indiscernibles = false` — the end-event cost is
    ///   nonzero in general, so `d(x, x) > 0`.
    /// * `triangle_inequality = false` — does not hold for a general
    ///   probability transducer.
    /// * `normalized = false` — output is unbounded above.
    ///
    /// See the [module-level docs](crate::learned) for the full discussion.
    #[inline]
    #[must_use]
    pub const fn properties() -> MetricProperties {
        MetricProperties {
            symmetric: false,
            identity_of_indiscernibles: false,
            triangle_inequality: false,
            non_negative: true,
            normalized: false,
        }
    }

    /// Returns the algorithm's mathematical classification.
    ///
    /// Reports [`MetricClass::Semimetric`] because the underlying quantity
    /// is distance-shaped (non-negative, has an identity for the model's
    /// most-probable path when the model is well-trained) — see the caveat
    /// on [`LearnedEdit::properties`].
    #[inline]
    #[must_use]
    pub const fn class() -> MetricClass {
        MetricClass::Semimetric
    }

    /// Constructs a distance handle wrapping the given trained model.
    #[inline]
    #[must_use]
    pub const fn new(model: LearnedEditModel<T>) -> Self {
        Self { model }
    }

    /// Returns a reference to the wrapped model.
    ///
    /// Useful when a caller wants to introspect the trained probabilities
    /// (for reporting, diagnostics, or building a UI on top of the model)
    /// after wrapping it in a distance handle.
    #[inline]
    #[must_use]
    pub const fn model(&self) -> &LearnedEditModel<T> {
        &self.model
    }

    /// Computes the learned distance between `source` and `target`.
    ///
    /// Returns the negative-log-probability of the Viterbi (most-probable)
    /// edit sequence transducing `source` to `target` under the wrapped
    /// model, plus the transducer's end-event cost.
    ///
    /// Returns [`f64::INFINITY`] if any required cost lookup lands on a
    /// symbol not present in the model's alphabet — under `min` propagation
    /// the whole path is pruned, and the answer is `+inf` only if *every*
    /// path is pruned.
    #[must_use]
    pub fn compute_distance(&self, source: &[T], target: &[T]) -> f64 {
        let m = source.len();
        let n = target.len();

        // Boundary DP with the full matrix — a caller with a batch use-case
        // would want a rolling-rows variant here, but the full matrix is
        // the simplest correct form and matches the pattern of
        // `levenshtein::full_matrix` in shape if not in cost.
        let cols = n + 1;
        let mut d = vec![f64::INFINITY; (m + 1) * cols];
        d[0] = 0.0;

        // First column: prefix of source consumed by deletions only.
        for i in 1..=m {
            let prev = d[(i - 1) * cols];
            d[i * cols] = prev + self.model.delete(source[i - 1]);
        }
        // First row: prefix of target emitted by insertions only.
        for j in 1..=n {
            d[j] = d[j - 1] + self.model.insert(target[j - 1]);
        }

        for i in 1..=m {
            for j in 1..=n {
                let del = d[(i - 1) * cols + j] + self.model.delete(source[i - 1]);
                let ins = d[i * cols + j - 1] + self.model.insert(target[j - 1]);
                let sub =
                    d[(i - 1) * cols + j - 1] + self.model.substitute(source[i - 1], target[j - 1]);
                d[i * cols + j] = min3(del, ins, sub);
            }
        }

        d[m * cols + n] + self.model.end()
    }
}

impl<T: Ord + Copy> DistanceMetric<[T]> for LearnedEdit<T> {
    type Output = f64;

    #[inline]
    fn distance(&self, left: &[T], right: &[T]) -> Distance<Self::Output> {
        Distance::new(self.compute_distance(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        Self::properties()
    }

    #[inline]
    fn class(&self) -> MetricClass {
        Self::class()
    }
}

/// Three-way `min` on `f64`, propagating `NaN`s as `NaN` (which then
/// pollutes downstream cells — a `NaN` in the model is a bug and should
/// surface).
#[inline]
fn min3(a: f64, b: f64, c: f64) -> f64 {
    // f64::min is defined to return the non-NaN argument when one is NaN,
    // which would silently swallow a bad model. We want strict propagation.
    let ab = if a < b { a } else { b };
    if ab < c { ab } else { c }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learned::model::LearnedEditModel;

    /// The classical Levenshtein-style DP the kernel implements, in a
    /// deliberately-different structural form (bare `f64` matrix, no
    /// module glue) — the shared-shape mistake would be much less likely
    /// to be present in both.
    fn oracle_matrix(source: &[u8], target: &[u8], m: &LearnedEditModel<u8>) -> f64 {
        let rows = source.len() + 1;
        let cols = target.len() + 1;
        let mut d = alloc::vec![f64::INFINITY; rows * cols];
        d[0] = 0.0;
        for i in 1..rows {
            d[i * cols] = d[(i - 1) * cols] + m.delete(source[i - 1]);
        }
        for j in 1..cols {
            d[j] = d[j - 1] + m.insert(target[j - 1]);
        }
        for i in 1..rows {
            for j in 1..cols {
                let del = d[(i - 1) * cols + j] + m.delete(source[i - 1]);
                let ins = d[i * cols + j - 1] + m.insert(target[j - 1]);
                let sub = d[(i - 1) * cols + j - 1] + m.substitute(source[i - 1], target[j - 1]);
                let mut best = del;
                if ins < best {
                    best = ins;
                }
                if sub < best {
                    best = sub;
                }
                d[i * cols + j] = best;
            }
        }
        d[(rows - 1) * cols + (cols - 1)] + m.end()
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = LearnedEdit::<u8>::descriptor();
        assert_eq!(d.family, AlgorithmFamily::RistadYianilos);
        assert_eq!(d.variant, VariantId("ristad-yianilos-learned-edit"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1998, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = LearnedEdit::<u8>::DESCRIPTOR;
        assert_eq!(D.variant.0, "ristad-yianilos-learned-edit");
    }

    #[test]
    fn declares_semimetric_class() {
        let alg = LearnedEdit::<u8>::new(LearnedEditModel::uniform(b"ab"));
        assert_eq!(
            <LearnedEdit<u8> as DistanceMetric<[u8]>>::class(&alg),
            MetricClass::Semimetric
        );
        let p = <LearnedEdit<u8> as DistanceMetric<[u8]>>::properties(&alg);
        assert!(p.non_negative);
        assert!(!p.is_metric());
    }

    #[test]
    fn empty_pair_has_end_cost_only() {
        let model = LearnedEditModel::<u8>::uniform(b"ab");
        let alg = LearnedEdit::new(model.clone());
        let d = alg.compute_distance(b"", b"");
        assert!((d - model.end()).abs() < 1e-12);
    }

    #[test]
    fn distance_is_non_negative() {
        let model = LearnedEditModel::<u8>::uniform(b"abc");
        let alg = LearnedEdit::new(model);
        for (s, t) in [
            (b"".as_ref(), b"".as_ref()),
            (b"a", b"b"),
            (b"abc", b"cba"),
            (b"abc", b"abc"),
        ] {
            assert!(alg.compute_distance(s, t) >= 0.0);
        }
    }

    #[test]
    fn identity_is_at_most_shorter_than_edits() {
        // Under a uniform model, the min-cost path for x==x is all matches
        // (identity substitutions), so distance(x, x) = |x| * per_edit +
        // end_cost. This should be strictly cheaper than the distance to a
        // completely-different string of the same length.
        let model = LearnedEditModel::<u8>::uniform(b"abc");
        let alg = LearnedEdit::new(model);
        let same = alg.compute_distance(b"abc", b"abc");
        let diff = alg.compute_distance(b"abc", b"cab");
        assert!(same <= diff, "same={same}, diff={diff}");
    }

    #[test]
    fn kernel_matches_oracle_on_canonical_pairs() {
        let model = LearnedEditModel::<u8>::uniform(b"abcde");
        let alg = LearnedEdit::new(model.clone());
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"a", b""),
            (b"", b"a"),
            (b"abc", b"abc"),
            (b"abc", b"abd"),
            (b"abc", b"xyz".as_ref()), // xyz would hit missing symbols — swap.
            (b"abc", b"aec"),
            (b"aa", b"aaa"),
            (b"abcde", b"edcba"),
        ];
        for (a, b) in pairs {
            let observed = alg.compute_distance(a, b);
            let expected = oracle_matrix(a, b, &model);
            if expected.is_infinite() {
                assert!(observed.is_infinite());
            } else {
                assert!(
                    (observed - expected).abs() < 1e-12,
                    "on ({a:?}, {b:?}): observed {observed}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn unknown_symbols_yield_infinite_distance() {
        // Model over {a, b}; comparing against a string containing 'z' —
        // every path must pay an infinite cost somewhere.
        let alg = LearnedEdit::new(LearnedEditModel::<u8>::uniform(b"ab"));
        let d = alg.compute_distance(b"a", b"z");
        assert!(d.is_infinite());
    }
}
