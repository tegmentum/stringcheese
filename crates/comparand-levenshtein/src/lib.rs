//! Unit-cost Levenshtein edit distance for the Comparand toolkit.
//!
//! This crate is the first vertical slice of Comparand's algorithm coverage.
//! Its job is twofold: to provide a correct, workspace-aware Levenshtein
//! implementation, and to exercise every piece of the `comparand-core`
//! substrate under real load so that any rough edge in the substrate is
//! surfaced before the rest of the algorithm crates depend on it.
//!
//! Named for V. I. Levenshtein, whose 1966 paper introduced the metric on
//! binary strings under insertion, deletion, and reversal (substitution
//! being a composition of a deletion and an insertion). The modern
//! dynamic-programming formulation universally used to compute the distance
//! is due to Wagner and Fischer (1974), and the banded cutoff kernel below
//! follows Ukkonen (1985).
//!
//! # Kernels
//!
//! Three implementations of the same recurrence coexist here on purpose —
//! any two of them agreeing across a corpus is much stronger evidence of
//! correctness than any single one being fast:
//!
//! * [`full_matrix`] — the deliberately-simple `O(m · n)`-time,
//!   `O(m · n)`-space textbook implementation (Wagner & Fischer 1974). This
//!   is the oracle every other kernel is checked against.
//! * [`rolling_rows`] — the production kernel: `O(m · n)` time with
//!   `O(min(m, n))` scratch memory backed by a caller-owned
//!   [`LevenshteinWorkspace`].
//! * [`banded`] — an Ukkonen-style banded kernel that touches
//!   `O(k · min(m, n))` cells when the caller supplies a distance cutoff `k`
//!   (Ukkonen 1985). Returns [`BoundedDistance`] so an early-terminated
//!   result cannot be silently mistaken for an exact one.
//!
//! # Semantics
//!
//! Substitutions, insertions, and deletions each cost `1`. A transposition
//! of two adjacent symbols costs `2` — Damerau–Levenshtein is a distinct
//! algorithm and ships in its own crate. The variant identifier for the
//! implementation is `unit-cost-generic-eq`; golden test cases reference the
//! algorithm by [`Levenshtein::DESCRIPTOR`] rather than by common name to
//! prevent a Levenshtein case from being silently validated against a
//! Damerau variant.
//!
//! # Sequence type
//!
//! Every kernel is generic over `&[T]` where `T: Eq`. String comparisons pick
//! their representation — bytes, `char`s, grapheme clusters, tokens — at the
//! call site, in keeping with Comparand's rule that a representation choice
//! must never be made silently by the library.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. The kernels require heap allocation
//! for their scratch buffers, so they are gated on the `alloc` feature; a
//! build with neither `std` nor `alloc` compiles as an empty no-alloc
//! surface, which is what makes the crate safe to add as a dependency in
//! embedded configurations that only need to link against the substrate
//! crates.
//!
//! [`BoundedDistance`]: comparand_core::BoundedDistance
//!
//! # References
//!
//! - Levenshtein, V. I. (1966). "Binary codes capable of correcting deletions,
//!   insertions, and reversals." *Soviet Physics Doklady*, 10(8), 707-710.
//!   (Originally published 1965 in Russian in *Doklady Akademii Nauk SSSR*.)
//! - Wagner, R. A., & Fischer, M. J. (1974). "The string-to-string correction
//!   problem." *Journal of the ACM*, 21(1), 168-173.
//!   <https://doi.org/10.1145/321796.321811>
//! - Ukkonen, E. (1985). "Algorithms for approximate string matching."
//!   *Information and Control*, 64(1-3), 100-118.
//!   <https://doi.org/10.1016/S0019-9958(85)80046-2>

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod algorithm;
#[cfg(feature = "alloc")]
pub mod banded;
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
pub use algorithm::Levenshtein;
#[cfg(feature = "alloc")]
pub use banded::distance_banded_with_workspace;
#[cfg(feature = "alloc")]
pub use full_matrix::distance_full_matrix;
#[cfg(feature = "alloc")]
pub use rolling_rows::distance_rolling_rows_with_workspace;
#[cfg(feature = "alloc")]
pub use workspace::LevenshteinWorkspace;
