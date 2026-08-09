//! # MinHash sketches and Jaccard-estimate similarity
//!
//! Given two sets `A` and `B`, the Jaccard similarity
//! `J(A, B) = |A ∩ B| / |A ∪ B|` costs `O(|A| + |B|)` to compute
//! exactly. MinHash [Broder 1997] gives an unbiased estimator at
//! fixed cost regardless of set size: hash every element `k`
//! ways, keep the minimum hash under each permutation, compare
//! two sketches by counting how many of the `k` positions agree.
//!
//! ## What ships
//!
//! - [`Sketch`] — a fixed-permutation MinHash sketch. Cheap to
//!   compare (one `.iter().zip()`), reasonably cheap to build
//!   (`k` hashes per input element).
//! - [`Sketcher`] — builder that hashes a stream of gram values
//!   into a [`Sketch`]. Fed by any `Hash + ?Sized` gram — plays
//!   directly with the `char_ngrams` / `token_ngrams` /
//!   `byte_ngrams` iterators from the `stringcheese-ngram` crate.
//! - [`Sketch::jaccard`] — Jaccard-similarity estimate between two
//!   sketches of the same width. Result is in `[0.0, 1.0]`.
//! - [`lsh`] — locality-sensitive hashing helpers for banded
//!   near-duplicate candidate generation against a large corpus.
//!
//! ## Determinism
//!
//! Sketches built with the same [`Sketcher`] seed are
//! bit-for-bit identical across runs and machines — matches the
//! guarantee `stringcheese-index` depends on. The default seed
//! is a fixed constant; supply your own via [`Sketcher::seeded`]
//! for pipelines that mix multiple sketch families.
//!
//! ## Example
//!
//! ```
//! use stringcheese_minhash::Sketcher;
//!
//! let s1 = Sketcher::new(128).sketch(["foo", "bar", "baz", "qux"]);
//! let s2 = Sketcher::new(128).sketch(["foo", "bar", "baz", "quux"]);
//! let j = s1.jaccard(&s2);
//! // Two of four unique elements shared out of five total → 2/5.
//! // MinHash's estimate is unbiased but noisy; wide sketches
//! // (many permutations) tighten the standard error.
//! assert!(j > 0.0 && j <= 1.0);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
// MinHash's Jaccard estimate is `matches / width` — a usize-to-f64
// division whose result is what the caller wants. The comparison
// tests use exact `assert_eq!(_, 1.0)` for identical sketches
// (all-min-hashes match position-for-position — the equality is
// the assertion, not accident). `doc_markdown` gets noisy about
// technical terms like MinHash / LSH / Jaccard / Broder — the
// docs are for humans; wrapping every capitalised term in
// backticks harms readability.
#![allow(clippy::cast_precision_loss, clippy::float_cmp, clippy::doc_markdown)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod lsh;
pub mod sketch;

pub use sketch::{Sketch, Sketcher};
