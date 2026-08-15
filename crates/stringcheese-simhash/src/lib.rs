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
//!
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/simhash.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-simhash
//! ```
//!
//! Three groups drive the three phases a real SimHash pipeline
//! spends time in: `hash` (build via `add_all().finalize_64()`),
//! `hamming` (`hamming_distance`), and `similar` (`similarity`).
//! Each group sweeps three feature counts (100 / 1_000 / 10_000)
//! crossed with two flavors (`short` — 8-byte feature strings —
//! and `long` — 64-byte feature strings). 18 measurement points
//! total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`).
//! Wall-clock samples vary ±10-20 % on a laptop under load. `hash`
//! throughput reported in *features / s*; the fixed-cost `hamming`
//! and `similar` surfaces are reported as per-call latency (each
//! measurement is one comparison).
//!
//! ```text
//! op       / flavor  / n            throughput
//! -----------------------------------------------
//! hash     / short   / 100          ~31 M feat/s
//! hash     / short   / 1000         ~31 M feat/s
//! hash     / short   / 10000        ~28 M feat/s
//! hash     / long    / 100          ~27 M feat/s
//! hash     / long    / 1000         ~26 M feat/s
//! hash     / long    / 10000        ~27 M feat/s
//! hamming  / short   / *            ~1.4 ns / call
//! hamming  / long    / *            ~1.5 ns / call
//! similar  / short   / *            ~1.5 ns / call
//! similar  / long    / *            ~1.5 ns / call
//! ```
//!
//! Read:
//!
//! * **`hash` throughput sits at ~27-31 M features/s across every
//!   cell** — two 64-bit `ahash` hashes per feature plus the
//!   128-way accumulator update. The `long`-flavor slowdown vs
//!   `short` is only ~15 %, not the 2× a per-byte-hash cost would
//!   predict; both feature lengths fit under the ahash "one 64-bit
//!   word" fast path threshold.
//! * **`hash` throughput is flat in feature count** — the per-feature
//!   loop dwarfs any startup cost by n=1000; small variance at
//!   n=100 disappears at higher counts.
//! * **`hamming_distance` is essentially free** — one xor, one
//!   `popcount`. `similarity` adds one division and lands at the
//!   same nanosecond count. This is the load-bearing property that
//!   makes SimHash worth building: candidate scans at billion-pair
//!   scales are bottlenecked on memory, not compute.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression
//!   or a measurement environment change; rerun with
//!   `--sample-size 30` before filing a fix.

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
