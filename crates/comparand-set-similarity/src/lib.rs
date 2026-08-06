//! Set-similarity metrics for the Comparand toolkit.
//!
//! This crate is Comparand's fifth vertical slice, and the first that
//! *consumes* another algorithm crate's output: every algorithm here takes
//! one of `comparand-ngram`'s representation types ([`GramSet`],
//! [`GramMultiSet`], or [`GramVector`]) rather than a raw slice. Callers
//! commit to a representation up-front — a strict application of Comparand's
//! rule that the library never silently promotes one representation to
//! another (see the [n-gram design document][ng] for the reasoning).
//!
//! # What lives here
//!
//! Four algorithm families, each pinned by its own [`AlgorithmDescriptor`]:
//!
//! * [`dice`] — the Sørensen–Dice coefficient. Ships in two variants:
//!   [`DiceOverSet`] uses `GramSet` (distinct grams only) and
//!   [`DiceOverMultiSet`] uses `GramMultiSet` (with multiplicity).
//! * [`jaccard`] — the Jaccard coefficient. Ships in two variants:
//!   [`JaccardOverSet`] uses `GramSet` and [`JaccardOverMultiSet`] uses
//!   `GramMultiSet` (the "weighted Jaccard" formulation, `Σ min / Σ max`).
//!   Both variants expose a companion `distance` method returning a
//!   [`NormalizedDistance`]; the set-form distance is a **true metric**
//!   (well-known result, useful for BK-tree consumers), and the multiset
//!   form is a metric on non-negative multiset counts.
//! * [`overlap`] — the Szymkiewicz–Simpson [`Overlap`] coefficient over
//!   `GramSet`. NOT a metric: it violates identity of indiscernibles
//!   (`overlap(A, B) = 1` whenever one side is a subset of the other,
//!   regardless of whether the two sides are equal).
//! * [`cosine`] — [`Cosine`] similarity over `GramVector`. Requires the
//!   `std` feature for `f64::sqrt`. Ships with a companion `distance`
//!   method returning a [`NormalizedDistance`] — but note this is the
//!   arithmetic complement `1 - cosine`, **not** angular distance, and it
//!   is not a true metric (the arccos-then-divide-by-pi construction that
//!   IS a metric is deferred to a future variant).
//!
//! Every algorithm returns [`NormalizedSimilarity`]-adjacent values in
//! `[0.0, 1.0]`; the boundary cases are documented per algorithm in the
//! module docs.
//!
//! # Empty-vs-empty convention
//!
//! The literature is genuinely ambiguous about the meaning of
//! `sim(∅, ∅)`. Comparand adopts the identity-of-indiscernibles convention
//! uniformly: two empty representations are considered identical, and every
//! algorithm in this crate returns `1.0` for the empty-empty case. This
//! matches the convention Jaro uses ([`comparand_jaro::Jaro`] returns
//! `1.0` for two empty inputs) and gives property tests a clean starting
//! point. The choice is documented on each algorithm's kernel so a reader
//! coming to a single similarity does not need the crate-level rationale.
//!
//! One-empty-one-not always yields `0.0` — that case is unambiguous in
//! every published formulation.
//!
//! # Sequence type
//!
//! The gram type `G` is generic under a `G: Ord` bound (character grams
//! typically instantiate it with `Vec<char>`; byte grams with `Vec<u8>`).
//! No algorithm in this crate makes a representation choice on the caller's
//! behalf — the caller commits to a representation when they build the
//! [`GramSet`], [`GramMultiSet`], or [`GramVector`] with
//! [`GramSet::from_generator`][gs] or a sibling constructor.
//!
//! # Metric-class summary
//!
//! | Algorithm                | [`SimilarityMetric`] class | Distance form                                    |
//! |--------------------------|----------------------------|--------------------------------------------------|
//! | [`DiceOverSet`]          | `Similarity`               | `1 - dice` is a semimetric (NOT a true metric)   |
//! | [`DiceOverMultiSet`]     | `Similarity`               | same as above, over multiset counts              |
//! | [`JaccardOverSet`]       | `Similarity`               | `1 - jaccard` IS a true metric                   |
//! | [`JaccardOverMultiSet`]  | `Similarity`               | `1 - weighted_jaccard` IS a metric on non-neg. w |
//! | [`Overlap`]              | `Similarity`               | not applicable (identity of indiscernibles fails)|
//! | [`Cosine`]               | `Similarity`               | `1 - cosine` is NOT a true metric; angular is    |
//!
//! The [`class`] field returned by every algorithm is `MetricClass::Similarity`.
//! The design document's [n-gram type-system table][ng] describes Jaccard's
//! class as `Metric` — a labelling that captures the *distance form*'s
//! metric-hood — but this crate uses the stricter reading of the
//! [type-system spec][ts]: `class = Metric` requires
//! `properties = METRIC` or `NORMALIZED_METRIC`, and the similarity itself
//! does not satisfy the triangle inequality (bounded similarities generally
//! do not). Consumers wanting a metric distance from Jaccard should call
//! [`JaccardOverSet::distance`], which returns a [`NormalizedDistance`]
//! that IS a metric.
//!
//! [`class`]: comparand_core::SimilarityMetric::class
//! [ng]: https://github.com/tegmentum/comparand/blob/main/docs/design/ngram-and-fingerprinting.md
//! [ts]: https://github.com/tegmentum/comparand/blob/main/docs/design/type-system.md
//! [gs]: comparand_ngram::GramSet::from_generator
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every representation the crate
//! consumes requires heap allocation (a `BTreeSet` / `BTreeMap` backing
//! store), so **the entire public surface is behind the `alloc` feature.**
//! [`Cosine`] additionally requires `std` because `f64::sqrt` is not part
//! of `core`. Under `--no-default-features` the crate compiles to an empty
//! module, matching the ngram crate's own no-alloc surface.
//!
//! [`AlgorithmDescriptor`]: comparand_core::AlgorithmDescriptor
//! [`GramSet`]: comparand_ngram::GramSet
//! [`GramMultiSet`]: comparand_ngram::GramMultiSet
//! [`GramVector`]: comparand_ngram::GramVector
//! [`NormalizedDistance`]: comparand_core::NormalizedDistance
//! [`NormalizedSimilarity`]: comparand_core::NormalizedSimilarity
//! [`SimilarityMetric`]: comparand_core::SimilarityMetric
//! [`comparand_jaro::Jaro`]: https://docs.rs/comparand-jaro
//!
//! # References
//!
//! - Jaccard, P. (1912). "The distribution of the flora in the alpine zone."
//!   *New Phytologist*, 11(2), 37-50.
//!   <https://doi.org/10.1111/j.1469-8137.1912.tb05611.x>
//! - Dice, L. R. (1945). "Measures of the amount of ecologic association
//!   between species." *Ecology*, 26(3), 297-302.
//!   <https://doi.org/10.2307/1932409>
//! - Sørensen, T. A. (1948). "A method of establishing groups of equal
//!   amplitude in plant sociology based on similarity of species content,
//!   and its application to analyses of the vegetation on Danish commons."
//!   *Kongelige Danske Videnskabernes Selskab, Biologiske Skrifter*, 5(4),
//!   1-34. (Cited alongside Dice for the Sørensen-Dice coefficient.)
//! - Szymkiewicz, D. (1934). "Une contribution statistique à la géographie
//!   floristique." *Acta Societatis Botanicorum Poloniae*, 11(3), 249-265.
//!   (Cited alongside Simpson for the overlap coefficient.)
//! - Simpson, G. G. (1943). "Mammals and the nature of continents."
//!   *American Journal of Science*, 241, 1-31.
//! - Salton, G., & `McGill`, M. J. (1983). *Introduction to Modern Information
//!   Retrieval*. McGraw-Hill. ISBN: 0-07-054484-0. (The canonical IR
//!   reference for cosine similarity over term-frequency vectors.)

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

// `extern crate alloc;` is a `no_std` idiom needed to bring the `alloc`
// crate into scope. Sub-modules name `alloc::` freely; lib.rs itself does
// not, which fires the `unused_extern_crates` lint on every configuration
// unless we suppress it. The declaration is load-bearing for the sub-
// modules (which the lint does not see), so the allow-attribute here is
// correct rather than a paper-over.
#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

#[cfg(all(feature = "std", feature = "alloc"))]
pub mod cosine;
#[cfg(feature = "alloc")]
pub mod dice;
#[cfg(feature = "alloc")]
pub mod jaccard;
#[cfg(feature = "alloc")]
pub mod overlap;
#[cfg(feature = "alloc")]
pub(crate) mod shared;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
mod property_tests;

#[cfg(all(feature = "std", feature = "alloc"))]
pub use cosine::Cosine;
#[cfg(feature = "alloc")]
pub use dice::{DiceOverMultiSet, DiceOverSet};
#[cfg(feature = "alloc")]
pub use jaccard::{JaccardOverMultiSet, JaccardOverSet};
#[cfg(feature = "alloc")]
pub use overlap::Overlap;
