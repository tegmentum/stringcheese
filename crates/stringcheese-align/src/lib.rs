//! Global (Needleman-Wunsch) and local (Smith-Waterman) sequence alignment.
//!
//! This crate is the v0.2 StringCheese alignment subsystem. Unlike an edit-
//! distance kernel, an alignment algorithm produces a *score* plus an
//! *edit script* — a step-by-step account of how each symbol of the two
//! inputs was paired (matched, substituted, or aligned to a gap). A
//! downstream consumer can use the script to visualize the alignment,
//! render a diff, or reconstruct one sequence from the other.
//!
//! # Algorithms
//!
//! * [`NeedlemanWunsch`] — global alignment. Every symbol of both inputs
//!   appears in the resulting alignment, aligned to a symbol or to a gap.
//! * [`SmithWaterman`] — local alignment. Returns the highest-scoring pair
//!   of substrings and the alignment between them; low-scoring regions of
//!   both inputs are discarded (their scores are floored to zero during the
//!   DP).
//!
//! Both algorithms share the same scoring vocabulary and can be
//! parameterized by any [`ScoringScheme`]. Two concrete schemes ship with
//! the crate:
//!
//! * [`LinearGap`] — every gap symbol costs the same amount. Total
//!   `k`-symbol gap cost is `k * gap_penalty`.
//! * [`AffineGap`] — opening a gap costs `gap_open`; each additional symbol
//!   in the same gap costs `gap_extend`. Total `k`-symbol gap cost is
//!   `gap_open + (k - 1) * gap_extend`. When the scheme is affine
//!   (`gap_open != gap_extend`) both aligners select a three-matrix
//!   Gotoh 1982 DP; otherwise they use a simpler single-matrix DP.
//!
//! # Reconstructed alignments
//!
//! [`Alignment`] holds the score, the ordered [`EditOp`] script, and — for
//! local alignment — the start indices of the aligned substrings in the
//! two inputs. Utility methods on [`Alignment`] rebuild the aligned
//! substrings ([`Alignment::extract_a`], [`Alignment::extract_b`]) or apply
//! the script to a witness ([`Alignment::apply_to`]).
//!
//! # Sequence type
//!
//! All alignment operations are generic over `&[T]` where `T: Eq` (and
//! `T: Clone` for edit-script reconstruction). Representation is chosen at
//! the call site — bytes, `char`s, `u32` code points, tokens, or any
//! `Eq + Clone` type.
//!
//! # Score type
//!
//! Alignment scores are reported as [`Score<i32>`](stringcheese_core::Score).
//! `i32` was chosen instead of a generic numeric type because every scoring
//! scheme currently expresses match, mismatch and gap costs as small
//! integers, and `i32` gives ample headroom (a `10 000 x 10 000` alignment
//! with `match_score = i16::MAX` still fits). See the "Design choices"
//! section of the crate's README for the trade-off.
//!
//! # `no_std`
//!
//! `no_std`-compatible; the `alloc` feature gates the DP scratch buffers,
//! the edit-script [`Vec`], and both alignment algorithms
//! themselves (which allocate their DP matrices).
//!
//! # Metric class
//!
//! Alignment scores are not distances: they can be positive or negative and
//! do not satisfy the triangle inequality without further transformation.
//! Both algorithms therefore report
//! [`MetricClass::Score`](stringcheese_core::MetricClass::Score) and deliberately
//! implement *neither* [`DistanceMetric`](stringcheese_core::DistanceMetric)
//! *nor* [`SimilarityMetric`](stringcheese_core::SimilarityMetric) — the
//! entry points are inherent methods only.
//!
//! # What's not here
//!
//! * SIMD backends — out of scope for v0.2.
//! * Multiple sequence alignment (MSA) — v0.3+.
//! * Profile-based alignment (PSSMs, HMMs) — v0.3+.
//! * Banded alignment for constrained-similarity inputs — future work.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates, reason = "consumed by submodules")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod edit_script;
#[cfg(feature = "alloc")]
pub mod needleman_wunsch;
pub mod scoring;
#[cfg(feature = "alloc")]
pub mod smith_waterman;
#[cfg(feature = "alloc")]
pub mod workspace;

#[cfg(all(test, feature = "alloc"))]
mod golden;
#[cfg(all(test, feature = "alloc"))]
#[cfg(not(target_family = "wasm"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use edit_script::{Alignment, EditOp};
#[cfg(feature = "alloc")]
pub use needleman_wunsch::NeedlemanWunsch;
pub use scoring::{AffineGap, LinearGap, ScoringScheme};
#[cfg(feature = "alloc")]
pub use smith_waterman::SmithWaterman;
#[cfg(feature = "alloc")]
pub use workspace::AlignmentWorkspace;
