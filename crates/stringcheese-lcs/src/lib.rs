//! Longest Common Subsequence and LCS distance for the StringCheese toolkit.
//!
//! This crate implements two closely related quantities on generic sequences:
//!
//! * [`Lcs`] — the *length* of the longest common subsequence. Given two
//!   sequences `a` and `b`, this is the length of the longest sequence that
//!   is a (not necessarily contiguous) subsequence of both. It is reported
//!   as a [`Score<u32>`](stringcheese_core::Score) because it is neither a
//!   distance nor a similarity in the canonical sense — larger values mean
//!   "more shared structure", but the value has no natural upper bound
//!   other than `min(|a|, |b|)`.
//! * [`LcsDistance`] — the true metric distance derived from the LCS length,
//!   defined as `|a| + |b| - 2 · lcs(a, b)`. This counts the minimum number
//!   of single-symbol insertions and deletions needed to transform `a` into
//!   `b`; substitutions are **not** allowed under this metric, which is the
//!   critical distinction from Levenshtein.
//!
//! Because substitutions are forbidden, the LCS distance of `"abcd"` and
//! `"abed"` is `2` (delete `c`, insert `e`) — whereas Levenshtein reports
//! `1` for the same pair. Every golden case in this crate that exercises
//! that difference is tagged accordingly so the two metrics cannot be
//! silently confused.
//!
//! # Kernels
//!
//! Two implementations of the same recurrence coexist here on purpose — any
//! two of them agreeing across a corpus is much stronger evidence of
//! correctness than any single one being fast:
//!
//! * [`full_matrix`] — the deliberately-simple `O(m · n)` time,
//!   `O(m · n)` space textbook implementation. This is the oracle
//!   [`rolling_rows`] is checked against.
//! * [`rolling_rows`] — the production kernel: `O(m · n)` time with
//!   `O(min(m, n))` scratch memory backed by a caller-owned
//!   [`LcsWorkspace`].
//!
//! # Semantics
//!
//! The LCS variant identifier is `"length-generic-eq"`; the LCS-distance
//! variant identifier is `"distance-generic-eq"`. Golden test cases
//! reference the algorithm by [`Lcs::DESCRIPTOR`] or
//! [`LcsDistance::DESCRIPTOR`] rather than by common name, so an LCS-length
//! case cannot silently be validated against an LCS-distance implementation
//! (or vice versa).
//!
//! # Sequence type
//!
//! Every kernel is generic over `&[T]` where `T: Eq`. String comparisons
//! pick their representation — bytes, `char`s, grapheme clusters, tokens —
//! at the call site, in keeping with StringCheese's rule that a representation
//! choice must never be made silently by the library.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. The kernels require heap
//! allocation for their scratch buffers, so they are gated on the `alloc`
//! feature; a build with neither `std` nor `alloc` compiles as an empty
//! no-alloc surface, which is what makes the crate safe to add as a
//! dependency in embedded configurations that only need to link against
//! the substrate crates.
//!
//! # References
//!
//! - Wagner, R. A., & Fischer, M. J. (1974). "The string-to-string correction
//!   problem." *Journal of the ACM*, 21(1), 168-173.
//!   <https://doi.org/10.1145/321796.321811> (the LCS DP is the "no
//!   substitution" specialization of the Wagner-Fischer recurrence.)
//! - Hirschberg, D. S. (1975). "A linear space algorithm for computing
//!   maximal common subsequences." *Communications of the ACM*, 18(6),
//!   341-343. <https://doi.org/10.1145/360825.360861>
//! - Bergroth, L., Hakonen, H., & Raita, T. (2000). "A survey of longest
//!   common subsequence algorithms." *Proceedings of the Seventh
//!   International Symposium on String Processing and Information Retrieval
//!   (SPIRE)*, 39-48. <https://doi.org/10.1109/SPIRE.2000.878178>
//!   (the reference for the metric-distance formulation
//!   `|a| + |b| - 2 · lcs(a, b)` this crate exposes as [`LcsDistance`].)

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod algorithm;
#[cfg(feature = "alloc")]
pub mod full_matrix;
#[cfg(feature = "alloc")]
pub mod rolling_rows;
#[cfg(feature = "alloc")]
pub mod workspace;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use algorithm::{Lcs, LcsDistance};
#[cfg(feature = "alloc")]
pub use full_matrix::{lcs_distance_full_matrix, lcs_length_full_matrix};
#[cfg(feature = "alloc")]
pub use rolling_rows::{
    lcs_distance_rolling_rows_with_workspace, lcs_length_rolling_rows_with_workspace,
};
#[cfg(feature = "alloc")]
pub use workspace::LcsWorkspace;
