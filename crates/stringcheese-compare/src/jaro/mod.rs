//! Jaro and Jaro-Winkler similarity for the StringCheese toolkit.
//!
//! This crate is the third vertical slice of StringCheese's algorithm coverage,
//! and the first that returns a *similarity* rather than a distance. Every
//! design consequence of that flip — the [`SimilarityMetric`] trait rather
//! than [`DistanceMetric`], the [`NormalizedSimilarity`] range wrapper, the
//! [`FloatExpectation`] comparison policy on golden cases — is exercised
//! here for the first time. The intent is that the patterns established in
//! this crate become the template every future floating-point comparison
//! algorithm follows.
//!
//! # What lives here
//!
//! * [`jaro`] — the base [`Jaro`] similarity: `O(m + n)` matching-window
//!   traversal with `O(m + n)` scratch, producing a `Similarity<f64>` in
//!   `[0.0, 1.0]`. This is the classical Jaro (1989) score used by every
//!   record-linkage pipeline that pre-dates Winkler.
//! * [`jaro_winkler`] — [`JaroWinkler`], a *family* of similarities
//!   distinguished by their prefix limit, scaling factor, and boost
//!   threshold. Two named constructors pin the historically canonical
//!   variants ([`JaroWinkler::classic`] is Winkler 1990; [`JaroWinkler::with_threshold`]
//!   is Winkler's later refinement); [`JaroWinkler::new`] accepts arbitrary
//!   parameters and is validated against the invariant that keeps the
//!   output bounded to `[0.0, 1.0]`.
//!
//! # Sequence type
//!
//! Both algorithms are generic over `&[T]` where `T: Eq`. String comparisons
//! pick their representation — bytes, `char`s, grapheme clusters, tokens —
//! at the call site, in keeping with StringCheese's rule that a representation
//! choice must never be made silently by the library.
//!
//! # Mathematical properties
//!
//! Both algorithms are *similarities*, not metrics. The [`Jaro`] score is
//! symmetric, satisfies identity of indiscernibles (`sim(x, x) = 1.0`),
//! is non-negative, and is naturally normalized to `[0.0, 1.0]` — but does
//! not satisfy the triangle inequality (which is a metric-only axiom that
//! bounded similarities generally violate). The [`JaroWinkler`] boost
//! preserves all of these axioms.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. The kernels require heap allocation
//! for their per-call boolean scratch buffers, so they are gated on the
//! `alloc` feature; a build with neither `std` nor `alloc` compiles as an
//! empty no-alloc surface, which is what makes the crate safe to add as a
//! dependency in embedded configurations that only need the substrate
//! crates.
//!
//! [`DistanceMetric`]: stringcheese_core::DistanceMetric
//! [`SimilarityMetric`]: stringcheese_core::SimilarityMetric
//! [`NormalizedSimilarity`]: stringcheese_core::NormalizedSimilarity
//! [`FloatExpectation`]: https://docs.rs/stringcheese-corpus
//!
//! # References
//!
//! - Jaro, M. A. (1989). "Advances in record-linkage methodology as applied
//!   to matching the 1985 census of Tampa, Florida." *Journal of the American
//!   Statistical Association*, 84(406), 414-420.
//!   <https://doi.org/10.1080/01621459.1989.10478785>
//! - Winkler, W. E. (1990). "String comparator metrics and enhanced decision
//!   rules in the Fellegi-Sunter model of record linkage." *Proceedings of
//!   the Section on Survey Research Methods, American Statistical
//!   Association*, 354-359.

// Inner `jaro` submodule (base Jaro similarity) shares the enclosing
// module's name — inherited from when this crate was its own top-level
// `stringcheese-jaro` crate with `src/jaro.rs` sitting next to
// `src/jaro_winkler.rs`. Keeping the shape preserves the intra-doc link
// surface downstream code has been targeting.
#[cfg(feature = "alloc")]
#[allow(clippy::module_inception)]
pub mod jaro;
#[cfg(feature = "alloc")]
pub mod jaro_winkler;
#[cfg(feature = "alloc")]
pub mod workspace;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use jaro::Jaro;
#[cfg(feature = "alloc")]
pub use jaro_winkler::{JaroWinkler, JaroWinklerError};
#[cfg(feature = "alloc")]
pub use workspace::JaroWorkspace;
