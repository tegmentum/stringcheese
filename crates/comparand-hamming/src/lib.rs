//! Hamming distance for the Comparand toolkit.
//!
//! This crate is the second vertical slice of Comparand's algorithm coverage.
//! Where the Levenshtein crate demonstrated that a full dynamic-programming
//! algorithm fits the `comparand-core` substrate cleanly, this crate is a
//! deliberate stress test in the opposite direction: does a *simple*
//! algorithm — one that is a single fold and needs no scratch memory — fit the
//! same substrate without ceremony? The answer here is yes, and the file
//! layout is intentionally the same as Levenshtein's so that a reader who
//! learns one learns the other.
//!
//! # What Hamming distance is
//!
//! For two sequences of *equal length*, the Hamming distance is the number of
//! positions at which the corresponding symbols differ. As originally defined
//! in [Hamming's 1950 paper](https://ieeexplore.ieee.org/document/6772729) for
//! binary error-detecting codes, the distance is not defined for sequences of
//! different length. Comparand takes that restriction seriously — see the
//! unequal-length policy below.
//!
//! # Semantics
//!
//! The variant identifier for the implementation is
//! `"equal-length-generic-eq"`; golden test cases reference the algorithm by
//! [`Hamming::DESCRIPTOR`] rather than by common name so that a future
//! variant (edit-distance-compatible padding semantics, weighted mismatch
//! costs, …) cannot silently be validated against the pure equal-length
//! definition.
//!
//! Hamming is a true metric on the space of equal-length sequences: it is
//! symmetric, non-negative, satisfies the identity of indiscernibles under
//! `T: Eq`, and satisfies the triangle inequality. It is *not* naturally
//! normalized — the maximum distance for a pair of length-`n` sequences is
//! `n`, not `1.0`.
//!
//! # Unequal-length policy
//!
//! Hamming distance is only defined for equal-length sequences. Comparand
//! offers two entry points that handle the mismatch differently, and the
//! caller must pick one deliberately:
//!
//! * [`Hamming::try_distance`] and [`Hamming::try_distance_within`] are the
//!   **fallible** entry points. They return
//!   `Result<_, LengthMismatch>` and are the correct choice when the caller
//!   cannot statically prove that both inputs have the same length.
//! * The [`DistanceMetric`] and [`BoundedDistanceMetric`] trait
//!   implementations are the **infallible** entry points; they **panic** with
//!   a clear message on a length mismatch. They are the correct choice when
//!   the caller has already established equal length (for example by taking
//!   both inputs from `&[T; N]` at the same `N`, or by handing off to a
//!   Hamming-code decoder that only accepts fixed-width codewords).
//!
//! No third option is offered: silently padding the shorter side (option
//! (c) in the design discussion) would redefine Hamming distance to something
//! it is not. Rejected.
//!
//! [`DistanceMetric`]: comparand_core::DistanceMetric
//! [`BoundedDistanceMetric`]: comparand_core::BoundedDistanceMetric
//!
//! # Sequence type
//!
//! Every entry point is generic over `&[T]` where `T: Eq`. String comparisons
//! pick their representation — bytes, `char`s, grapheme clusters, tokens — at
//! the call site, in keeping with Comparand's rule that a representation
//! choice must never be made silently by the library.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible with **no allocation requirement** —
//! Hamming's kernel is a single fold with a running counter. Under
//! `--no-default-features` the crate compiles as pure `core`; the only
//! observable effect of the `std` feature is that [`LengthMismatch`]
//! implements [`std::error::Error`].
//!
//! [`LengthMismatch`]: error::LengthMismatch

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod algorithm;
pub mod error;
pub mod kernel;

#[cfg(test)]
mod golden;

#[cfg(test)]
mod property_tests;

pub use algorithm::Hamming;
pub use error::LengthMismatch;
pub use kernel::{hamming_distance, hamming_distance_within};
