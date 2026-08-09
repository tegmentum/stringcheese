//! # SimHash fingerprints and Hamming-distance similarity
//!
//! Charikar's SimHash (2002) projects a bag of weighted features
//! into a fixed-width bit vector such that similar bags produce
//! similar bit vectors under the Hamming metric. Two documents
//! that share many high-weight terms end up with bit vectors that
//! disagree in few positions; unrelated documents disagree in
//! roughly half.
//!
//! ## What ships
//!
//! - [`Sketch64`] / [`Sketch128`] — fixed-width fingerprints.
//!   Cheap to compare (`(a ^ b).count_ones()`), reasonably cheap
//!   to build (one hash per feature).
//! - [`Sketcher`] — builder that accumulates weighted features and
//!   finalises to a [`Sketch64`] or [`Sketch128`]. Fed by any
//!   `Hash + ?Sized` feature — plays directly with the gram
//!   producers in the `stringcheese-ngram` crate.
//! - [`Sketch64::similarity`] / [`Sketch128::similarity`] — cosine-
//!   similar signal in `[0.0, 1.0]`, computed as
//!   `1 - hamming / width`.
//! - [`lsh`] — permutation-banding helpers for near-duplicate
//!   candidate generation against a large corpus.
//!
//! ## When to reach for SimHash vs MinHash
//!
//! - **MinHash** — you have SETS. "How much do these two documents
//!   overlap in their character 3-grams?" Set-Jaccard exactly.
//! - **SimHash** — you have WEIGHTED FEATURE BAGS. "How similar
//!   are these documents' tf-idf-weighted term vectors?" Cosine-
//!   similar.
//!
//! Both are drop-in candidate generators for near-duplicate
//! pipelines; the choice is whether the underlying signal is a
//! set (unweighted) or a weighted feature vector.
//!
//! ## Determinism
//!
//! Sketches built with the same seed are bit-for-bit identical
//! across runs and machines. Default seed is a fixed named
//! constant; per-family seeding is via [`Sketcher::seeded`].
//!
//! ## Example
//!
//! ```
//! use stringcheese_simhash::Sketcher;
//!
//! let s1 = Sketcher::new().add_all(["foo", "bar", "baz"]).finalize_64();
//! let s2 = Sketcher::new().add_all(["foo", "bar", "qux"]).finalize_64();
//! let sim = s1.similarity(&s2);
//! // Two of three features shared → high similarity but not 1.0.
//! assert!(sim > 0.5 && sim < 1.0);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
// SimHash's similarity signal is `hamming / width` — a usize-to-
// f64 division whose result is what the caller wants. Tests use
// exact `assert_eq!(_, 1.0)` for identical fingerprints (all bits
// match by construction — the equality is the assertion, not
// accident). `doc_markdown` gets noisy about capitalised terms
// like SimHash, MinHash, LSH, Hamming — the docs are for humans.
#![allow(clippy::cast_precision_loss, clippy::float_cmp, clippy::doc_markdown)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod lsh;
pub mod sketch;

pub use sketch::{Sketch64, Sketch128, Sketcher};
