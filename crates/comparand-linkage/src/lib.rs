//! Probabilistic record linkage (Fellegi-Sunter model) for the Comparand
//! sequence-comparison toolkit.
//!
//! This crate is the *record-linkage decision layer* of Comparand: the
//! primitive that combines per-field distances or similarities — produced by
//! the substrate metrics in [`comparand-core`], the string comparators in
//! [`comparand-jaro`] and [`comparand-levenshtein`], the phonetic encoders in
//! [`comparand-phonetic`], and the candidate generators in
//! [`comparand-index`] — into a match / non-match / possible-match decision
//! at record scale.
//!
//! The mathematical model is the one Fellegi and Sunter formalized in 1969
//! (see the crate-level `References` section for the full citation): given a
//! pair of records with `K` fields, each field's agreement pattern is
//! weighted by its discrimination power, and the sum of per-field weights is
//! thresholded against two analyst-supplied bounds to produce the final
//! decision.
//!
//! [`comparand-core`]: https://docs.rs/comparand-core
//! [`comparand-jaro`]: https://docs.rs/comparand-jaro
//! [`comparand-levenshtein`]: https://docs.rs/comparand-levenshtein
//! [`comparand-phonetic`]: https://docs.rs/comparand-phonetic
//! [`comparand-index`]: https://docs.rs/comparand-index
//!
//! # The Fellegi-Sunter model
//!
//! For each field `i` and a pair of records `(A, B)`, define an *agreement
//! pattern* `γ_i` by comparing the field's values through a per-field
//! comparator and thresholding the resulting similarity. The two
//! conditional probabilities that parameterize the model are
//!
//! * `m_i = P(γ_i = agree | (A, B) is a true match)`
//! * `u_i = P(γ_i = agree | (A, B) is a true non-match)`
//!
//! and the field's contribution to the record-pair log-likelihood weight is
//!
//! ```text
//!     w_i =  log2(m_i / u_i)               if γ_i = agree
//!         =  log2((1 - m_i) / (1 - u_i))   if γ_i = disagree
//! ```
//!
//! The record-pair weight is `W = Σ_i w_i`. The classification thresholds
//! `T_μ` (upper, chosen to bound the false-match rate) and `T_λ` (lower,
//! chosen to bound the false-non-match rate) partition the real line into
//! three decision regions:
//!
//! * `W >= T_μ` — [`LinkageDecision::Match`]
//! * `W <= T_λ` — [`LinkageDecision::NonMatch`]
//! * `T_λ < W < T_μ` — [`LinkageDecision::PossibleMatch`] (clerical review)
//!
//! # What lives here
//!
//! * [`field`] — [`FieldComparator`], the per-field agreement rule that
//!   turns a continuous similarity into a binary agree/disagree along with
//!   its `m_i` and `u_i` parameters; and [`FieldStrategy`], a
//!   metadata-only tag naming the comparator the analyst applied. The
//!   crate does not itself compute the similarity — the caller supplies
//!   `field_similarities` to [`LinkageModel::score`] — so [`FieldStrategy`]
//!   documents intent for downstream reports rather than driving
//!   dispatch. [`AgreementRule`] captures the same continuous-to-binary
//!   threshold as a first-class value for callers that want to reuse it
//!   outside of a full [`FieldComparator`].
//! * [`weight`] — the log-likelihood weight computation as pure functions
//!   [`agree_weight`] and [`disagree_weight`]. Exposed as free functions
//!   so callers can compute weights outside of a [`LinkageModel`] if
//!   desired.
//! * [`classifier`] — [`LinkageDecision`], the three-way decision the
//!   Fellegi-Sunter classifier emits.
//! * [`model`] — [`LinkageModel`], the classifier that combines per-field
//!   comparators into a scoring and decision pipeline. Weights are
//!   precomputed at construction so that [`LinkageModel::score`] does not
//!   pay for `log2` per call.
//! * [`estimation`] — [`PriorProbabilities`] for analyst-supplied
//!   parameterization and [`LabeledPairsEstimator`] for maximum-likelihood
//!   estimation with Jeffreys smoothing from labeled pairs. An
//!   [`EmEstimator`] stub reserves the unsupervised-estimation surface;
//!   the full EM loop is deferred to a subsequent release.
//! * [`error`] — [`LinkageModelError`] and [`estimation::EstimationError`],
//!   the two error types the crate surfaces on invalid parameterization
//!   or degenerate estimator input.
//!
//! # Why this crate has no [`AlgorithmDescriptor`]
//!
//! Record-linkage models are *infrastructure*: they wrap descriptor-carrying
//! comparators — a Jaro-Winkler surname comparator, a Levenshtein
//! address-line comparator — but do not implement a comparison themselves.
//! The [`AlgorithmDescriptor`] scheme in `comparand-core` identifies
//! algorithm variants, not the classifiers that combine their outputs. This
//! mirrors [`comparand-index`], whose BK-tree and VP-tree also wrap
//! descriptor-carrying metrics without carrying descriptors themselves.
//!
//! The [`AlgorithmFamily`] enum in `comparand-core` therefore does not
//! currently include a `FellegiSunter` variant. A future substrate release
//! is expected to add one; until then, this crate documents the Fellegi-
//! Sunter model in its top-level module docs and its type comments rather
//! than encoding it in a descriptor. Callers who need to record model
//! provenance in their own systems should do so alongside a
//! [`LinkageModel`] value — the model's per-field comparators, thresholds,
//! and citations are all inspectable.
//!
//! [`AlgorithmDescriptor`]: comparand_core::AlgorithmDescriptor
//! [`AlgorithmFamily`]: comparand_core::AlgorithmFamily
//! [`comparand-index`]: https://docs.rs/comparand-index
//!
//! # Sequence type
//!
//! [`LinkageModel::score`] and [`LinkageModel::classify`] take a slice of
//! per-field similarity scores rather than the raw records. This keeps
//! the classifier decoupled from any specific representation choice — the
//! caller pairs each field with the comparator best suited to it (bytes,
//! `char`s, grapheme clusters, phonetic codes) and hands off the resulting
//! scores. A future `TypedRecord<Field, Value>` abstraction is a
//! reasonable extension but is deliberately out of scope for the initial
//! release; the `&[f64]` interface makes the load-bearing decision
//! logic (the log-likelihood combination and the two-threshold
//! classifier) reviewable in isolation.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible in principle. In practice, the
//! Fellegi-Sunter log-likelihood weight requires `f64::log2`, which lives
//! in the standard library — no `core` alternative exists. **The entire
//! computational surface is therefore behind the `std` feature.** Under
//! `--no-default-features` (with or without `alloc`) the crate compiles
//! as an empty module, which is what makes the crate safe to add as a
//! dependency in embedded configurations that only need to link against
//! the substrate crates. Callers targeting `no_std` who need
//! record-linkage decisions must either enable `std` (typical) or add a
//! `libm` dependency and vendor the weight computation themselves (a
//! deliberate opt-in that a future release may automate behind an
//! additional feature).
//!
//! # References
//!
//! - Fellegi, I. P., & Sunter, A. B. (1969). "A theory for record linkage."
//!   *Journal of the American Statistical Association*, 64(328),
//!   1183-1210. <https://doi.org/10.1080/01621459.1969.10501049>
//! - Winkler, W. E. (1990). "String comparator metrics and enhanced decision
//!   rules in the Fellegi-Sunter model of record linkage." *Proceedings of
//!   the Section on Survey Research Methods, American Statistical
//!   Association*, 354-359.
//! - Jaro, M. A. (1989). "Advances in record-linkage methodology as applied
//!   to matching the 1985 census of Tampa, Florida." *Journal of the
//!   American Statistical Association*, 84(406), 414-420.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// Bring `alloc` into scope under any feature that enables it. The
// `#[allow(unused_extern_crates)]` is required for the `alloc`-only build:
// when `std` is disabled the crate's computational surface is empty, so
// no `alloc` types are referenced, yet the extern is retained so that a
// downstream release can add alloc-only content without another feature
// churn.
#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

#[cfg(feature = "std")]
pub mod classifier;
#[cfg(feature = "std")]
pub mod error;
#[cfg(feature = "std")]
pub mod estimation;
#[cfg(feature = "std")]
pub mod field;
#[cfg(feature = "std")]
pub mod model;
#[cfg(feature = "std")]
pub mod weight;

#[cfg(all(test, feature = "std"))]
mod golden;

#[cfg(all(test, feature = "std"))]
mod property_tests;

#[cfg(feature = "std")]
pub use classifier::LinkageDecision;
#[cfg(feature = "std")]
pub use error::LinkageModelError;
#[cfg(feature = "std")]
pub use estimation::{
    EmEstimator, EstimationError, LabeledPair, LabeledPairsEstimator, PriorProbabilities,
};
#[cfg(feature = "std")]
pub use field::{AgreementRule, FieldComparator, FieldStrategy};
#[cfg(feature = "std")]
pub use model::LinkageModel;
#[cfg(feature = "std")]
pub use weight::{agree_weight, disagree_weight};
