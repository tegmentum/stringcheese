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
//!
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/minhash.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-minhash
//! ```
//!
//! Two groups drive the two phases a real MinHash pipeline spends
//! time in: `sketch` (build) and `jaccard` (compare). Both sweep
//! three sketch widths (64 / 256 / 1024 permutations); `sketch`
//! adds a three-size sweep over gram count (100 / 1_000 / 10_000).
//! 12 measurement points total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`).
//! Wall-clock samples vary ±10-20 % on a laptop under load. Sketch
//! throughput reported in *grams / s* (input items absorbed);
//! Jaccard throughput reported in *positions / s* (sketch slots
//! compared).
//!
//! ```text
//! op       / width  / n_items    throughput
//! -------------------------------------------
//! sketch   / w64    / 100        ~5.0 M grams/s
//! sketch   / w64    / 1000       ~5.5 M grams/s
//! sketch   / w64    / 10000      ~5.6 M grams/s
//! sketch   / w256   / 100        ~1.3 M grams/s
//! sketch   / w256   / 1000       ~1.4 M grams/s
//! sketch   / w256   / 10000      ~1.4 M grams/s
//! sketch   / w1024  / 100        ~340 K grams/s
//! sketch   / w1024  / 1000       ~350 K grams/s
//! sketch   / w1024  / 10000      ~350 K grams/s
//! jaccard  / w64                 ~5 G positions/s
//! jaccard  / w256                ~10 G positions/s
//! jaccard  / w1024               ~15 G positions/s
//! ```
//!
//! Read:
//!
//! * **Sketch throughput scales inversely with width** — the O(width)
//!   inner loop dominates. Width 64 → ~5 M grams/s; width 1024 →
//!   ~350 K grams/s. This is expected; the "K independent hash
//!   functions" formulation trades sketch precision for build cost
//!   linearly.
//! * **Jaccard is memory-bound** — one `.zip().filter().count()` over
//!   the two `Vec<u64>`s. Width 1024 fits comfortably in L1 and
//!   throughput scales *up* with width because the loop amortises
//!   its per-call overhead over more positions.
//! * **Sketch throughput is flat in `n_items`** — the per-gram cost
//!   is dominated by hashing, not iteration; small startup jitter
//!   at n=100 disappears by n=1000.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression
//!   or a measurement environment change; rerun with
//!   `--sample-size 30` before filing a fix.

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
