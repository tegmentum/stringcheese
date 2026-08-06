//! Ristad-Yianilos (1998) learned string-edit distance.
//!
//! Classical Levenshtein charges every insertion, deletion, and substitution
//! the same cost of `1`. That's a reasonable prior in the absence of any
//! task-specific evidence, but for many real problems — OCR post-correction,
//! genotype comparison, keyboard-typo modelling, phonetic matching — the
//! per-character *shape* of the errors is not uniform, and treating them as
//! if it were throws away useful signal.
//!
//! Ristad and Yianilos (1998) formulate string-edit distance as the
//! negative-log-likelihood of transducing a source string to a target string
//! under a memoryless stochastic transducer whose per-character edit
//! probabilities are *learned* from labelled string pairs via
//! Expectation-Maximization. The learned distance is Levenshtein-shaped
//! dynamic programming, but every cost is looked up from the trained model
//! instead of being nailed to `1`.
//!
//! # Layout
//!
//! The module is split three ways:
//!
//! * [`LearnedEditModel`] — the trained cost table itself. Owns per-symbol
//!   deletion, insertion, and substitution costs together with the
//!   transducer's end-of-string cost. Constructed either uniformly (a
//!   sensible starting point before training) or as the output of an
//!   estimator's fit. Stored in log-space (as negative log probabilities) so
//!   the distance kernel is a straight `min` DP with `+` accumulation.
//! * [`LearnedEdit`] — the distance metric parameterized by a
//!   [`LearnedEditModel`]. Implements the same [`DistanceMetric`] trait as
//!   the classical kernels; the recurrence is the Wagner-Fischer DP with the
//!   per-cell cost replaced by the model lookup. `O(m · n)` time and space.
//! * [`RistadYianilosEstimator`] — EM training over a set of labelled
//!   (source, target) pairs, following Section 3 of the paper. Uses the
//!   forward-backward algorithm in log-space with the log-sum-exp trick, so
//!   nothing underflows even for long strings. Requires `std` because it
//!   needs `f64::ln` and `f64::exp`, both of which are `std`-only.
//!
//! # Metric properties
//!
//! Ristad-Yianilos learned edit distance is *not* a metric in the classical
//! sense. Specifically:
//!
//! * **Non-negative** — costs are `-log(p)` for `p ∈ (0, 1]`, so every cost
//!   is `>= 0` and every distance is non-negative. This one is unconditional.
//! * **Identity of indiscernibles** — the transducer's end event carries a
//!   cost, so `d(x, x) > 0` in general. Even a "perfectly identity" trained
//!   model has a nonzero baseline. This axiom does *not* hold.
//! * **Symmetric** — only holds when the model is symmetric in insertion
//!   vs. deletion costs *and* in substitution costs (`s(a, b) == s(b, a)`).
//!   A model trained on asymmetric data (say, mapping abbreviations to their
//!   expansions) is not generally symmetric.
//! * **Triangle inequality** — does *not* hold in general. The paper's
//!   formulation is a divergence, not a metric.
//!
//! The [`LearnedEdit`] handle declares [`MetricClass::Semimetric`] and
//! [`MetricProperties`] accordingly; the reported flags are the strongest
//! guarantees that hold for *every* possible trained model rather than the
//! strongest that might hold for a specific one. Downstream infrastructure
//! that requires stricter properties (BK-trees, some clustering algorithms)
//! should not consume [`LearnedEdit`] blindly.
//!
//! # Alphabet type
//!
//! The types in this module are generic over a symbol type `T: Ord + Copy`;
//! the default is `u8`, matching the crate's byte-first orientation. `char`
//! and integer token types also work. `T: Ord` is required by the
//! [`BTreeMap`]-backed cost tables — see below.
//!
//! # `BTreeMap` vs `HashMap`
//!
//! `alloc::collections::HashMap` does not exist; `HashMap` lives in `std`
//! only. Because [`LearnedEditModel`] and the [`LearnedEdit`] distance
//! kernel must work under `--no-default-features --features alloc` (they
//! need only `+` and `min` on `f64`, both of which live in `core`), the
//! cost tables are [`BTreeMap`]s. Under `std` a `HashMap` would be a
//! constant-factor
//! win on lookups, but the model's alphabet is usually small enough
//! (`|Σ| ≤ 256` for byte-oriented cases) that the difference is not
//! load-bearing.
//!
//! # Model persistence
//!
//! For `v0.1` there is no serialization surface: models are constructed
//! in-memory (via [`LearnedEditModel::uniform`] or
//! [`RistadYianilosEstimator::train`]) and consumed live. A serde-based
//! save/load pass is a follow-up.
//!
//! # References
//!
//! - Ristad, E. S., & Yianilos, P. N. (1998). "Learning string-edit distance."
//!   *IEEE Transactions on Pattern Analysis and Machine Intelligence*, 20(5),
//!   522-532. <https://doi.org/10.1109/34.682181>
//! - Wagner, R. A., & Fischer, M. J. (1974). "The string-to-string correction
//!   problem." *Journal of the ACM*, 21(1), 168-173. — the underlying DP
//!   whose cost table Ristad-Yianilos generalizes.
//!
//! [`BTreeMap`]: alloc::collections::BTreeMap
//! [`MetricClass::Semimetric`]: stringcheese_core::MetricClass::Semimetric
//! [`MetricProperties`]: stringcheese_core::MetricProperties
//! [`DistanceMetric`]: stringcheese_core::DistanceMetric

#[cfg(feature = "alloc")]
pub mod distance;
#[cfg(feature = "alloc")]
pub mod model;
#[cfg(feature = "std")]
pub mod training;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "std"))]
#[cfg(not(target_family = "wasm"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use distance::LearnedEdit;
#[cfg(feature = "alloc")]
pub use model::LearnedEditModel;
#[cfg(feature = "std")]
pub use training::RistadYianilosEstimator;
