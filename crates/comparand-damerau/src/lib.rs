//! Damerau-Levenshtein and Optimal String Alignment edit distances for the
//! Comparand toolkit.
//!
//! This crate is the fourth vertical slice of Comparand's algorithm coverage
//! and the first where the algorithm-variant registry is genuinely
//! load-bearing at the *definitional* level rather than at the tuning level.
//! The reason: "Damerau-Levenshtein" in the wild refers to at least two
//! different algorithms — and they produce different distances on the same
//! inputs.
//!
//! # The two variants
//!
//! Two distinct algorithms both go by "Damerau-Levenshtein" in the
//! literature and in most third-party libraries. Comparand distinguishes
//! them explicitly by `AlgorithmDescriptor`; a golden case that references
//! the wrong variant is a schema error, not a silent mismatch.
//!
//! * **Optimal String Alignment (OSA)** — sometimes called "restricted
//!   Damerau-Levenshtein". Same four operations as Damerau (insertion,
//!   deletion, substitution, transposition of adjacent symbols), with one
//!   critical restriction: **no substring of either input may be edited more
//!   than once**. This makes the DP a small extension of standard Levenshtein
//!   — one extra branch checking the two-back diagonal cell. It is fast and
//!   in most cases indistinguishable from full Damerau, but the restriction
//!   causes OSA to violate the triangle inequality: OSA is a *semimetric*,
//!   not a metric. The variant handle is [`Osa`].
//! * **Unrestricted Damerau-Levenshtein** — the algorithm from Damerau's
//!   1964 paper, first given a polynomial DP formulation by Lowrance and
//!   Wagner in 1975. A character may be involved in a transposition and then
//!   later be part of other edits — no per-substring cap. Requires an
//!   auxiliary "last position of symbol in `a`" table. Full Damerau *is* a
//!   true metric under unit costs. The variant handle is [`Damerau`].
//!
//! The distinguishing example, present in both variants' golden sets: for
//! `"ca"` vs `"abc"`, OSA gives `3` while full Damerau gives `2`. Under OSA,
//! transposing "ca" into "ac" and then inserting a "b" between them would
//! edit the same overlapping substring twice, so the restriction forbids it;
//! full Damerau allows the same two-step edit and returns `2`. This is
//! exactly the kind of case that would silently miscompare if a golden
//! dataset referred to "Damerau-Levenshtein" by common name.
//!
//! # Kernels
//!
//! Each variant ships three-or-two independent implementations, so any two
//! agreeing across a corpus is far stronger evidence of correctness than
//! any single one being fast:
//!
//! * [`osa::full_matrix`] — OSA oracle: the deliberately-simple textbook
//!   `O(m · n)`-time, `O(m · n)`-space implementation. Every other OSA
//!   kernel is checked against this.
//! * [`osa::rolling_rows`] — OSA production kernel: `O(m · n)` time with
//!   `O(min(m, n))` scratch memory, backed by a caller-owned
//!   [`OsaWorkspace`]. Uses three rolling rows because the transposition
//!   check reaches two rows back.
//! * [`osa::banded`] — OSA cutoff-aware kernel: `O(k · min(m, n))` cells
//!   for cutoff `k`. Returns [`BoundedDistance`] so an early-terminated
//!   result cannot be silently mistaken for an exact one.
//! * [`damerau::full_matrix`] — full Damerau oracle: Lowrance-Wagner
//!   `O(m² · n)`-time (linear scan over previous rows to locate the "last
//!   position of symbol" without a hash table). Deliberately slow;
//!   deliberately simple.
//! * [`damerau::production`] — full Damerau production kernel: same
//!   algorithm as the oracle but uses a `HashMap<&T, usize>` for the
//!   auxiliary lookup, giving `O(m · n)` time. Requires `T: Eq + Hash`.
//!
//! [`BoundedDistance`]: comparand_core::BoundedDistance
//!
//! # Semantics
//!
//! Every operation costs `1`; there is no weighted-cost variant here. The
//! two variant identifiers are `"unit-cost-generic-eq"` (OSA) and
//! `"unrestricted-unit-cost-generic-eq"` (full Damerau); golden test cases
//! reference [`Osa::DESCRIPTOR`] or [`Damerau::DESCRIPTOR`] rather than the
//! common name.
//!
//! # Metric-class summary
//!
//! | Variant   | Symmetric | Identity | Non-neg. | Triangle | Class        |
//! | --------- | :-------: | :------: | :------: | :------: | ------------ |
//! | OSA       | yes       | yes      | yes      | **no**   | `Semimetric` |
//! | Damerau   | yes       | yes      | yes      | yes      | `Metric`     |
//!
//! The triangle-inequality violation for OSA is not academic: the classical
//! counterexample family is `d(x, y) + d(y, z) < d(x, z)` for triples like
//! `(x = "ca", y = "ac", z = "abc")`, where OSA gives distances `1`, `2`,
//! and `3` — but `1 + 2 = 3`, and the strict inequality is broken by
//! `(x = "ca", y = "abc", z = "ac")` when phrased as a counterexample of
//! reachability under the "one edit per substring" restriction. Comparand's
//! property tests include a hard-coded triangle-violation assertion that
//! documents the fact as *known behavior* rather than a bug.
//!
//! # Sequence type
//!
//! OSA kernels are generic over `&[T]` with `T: Eq`. The full-Damerau
//! production kernel additionally requires `T: Hash` for the auxiliary
//! last-position table; the full-Damerau *oracle* requires only `T: Eq` and
//! uses a linear scan through the source in place of a hash lookup. The
//! two implementations therefore differ *structurally* in how they compute
//! the transposition candidate, which is what makes the differential test
//! between them meaningful.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every kernel requires heap
//! allocation for its scratch buffers, so they are gated on the `alloc`
//! feature; a build with neither `std` nor `alloc` compiles as an empty
//! no-alloc surface, matching the pattern of every other algorithm crate
//! in the workspace.
//!
//! # References
//!
//! - Damerau, F. J. (1964). "A technique for computer detection and
//!   correction of spelling errors." *Communications of the ACM*, 7(3),
//!   171-176. <https://doi.org/10.1145/363958.363994>
//! - Lowrance, R., & Wagner, R. A. (1975). "An extension of the
//!   string-to-string correction problem." *Journal of the ACM*, 22(2),
//!   177-183. <https://doi.org/10.1145/321879.321880>
//! - Wagner, R. A., & Fischer, M. J. (1974). "The string-to-string correction
//!   problem." *Journal of the ACM*, 21(1), 168-173.
//!   <https://doi.org/10.1145/321796.321811> (the OSA recurrence is a
//!   one-branch extension of the Wagner-Fischer Levenshtein DP.)

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod algorithm;
#[cfg(feature = "alloc")]
pub mod damerau;
#[cfg(feature = "alloc")]
pub mod osa;
#[cfg(feature = "alloc")]
pub mod workspace;

#[cfg(all(test, feature = "alloc"))]
mod golden;

// The property tests exercise the full-Damerau production kernel, which
// lives behind the `std` feature (it uses `std::collections::HashMap`).
// Under `--no-default-features --features alloc` the crate still tests its
// OSA kernels via each module's inline `#[cfg(test)]` sub-modules; only
// the cross-variant property suite is gated on `std`.
#[cfg(all(test, feature = "std"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use algorithm::{Damerau, Osa};
#[cfg(feature = "std")]
pub use workspace::DamerauWorkspace;
#[cfg(feature = "alloc")]
pub use workspace::OsaWorkspace;
