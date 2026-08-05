//! N-gram representation layer for the Comparand toolkit.
//!
//! # N-grams are a representation layer, not a metric input
//!
//! The common way to think about n-grams is as an input to Jaccard, Dice, or
//! cosine similarity. Comparand refuses that framing. An n-gram generator
//! produces a *representation* of the input, and the same representation
//! drives:
//!
//! - **Set similarity.** Jaccard, Dice, overlap coefficient, containment.
//! - **Weighted vector similarity.** Cosine similarity, weighted Jaccard.
//! - **`MinHash` and locality-sensitive hashing.**
//! - **N-gram inverted indexes.** Candidate generation for high-recall
//!   filtering before an expensive edit-distance rescore.
//! - **Substring-search preprocessing.** N-gram signatures gate Rabin–Karp
//!   scans over large corpora.
//! - **Explainability.** The intersection of two inputs' gram sets is a
//!   legible "why did these match" artifact.
//!
//! See [`docs/design/ngram-and-fingerprinting.md`][ng] for the definitive
//! specification. This crate is the reference implementation of that spec's
//! representation half; the fingerprinting half (rolling hashes, deterministic
//! `MinHash` bucketing, and Rabin variants) lives in `comparand-cdc` and its
//! consumers.
//!
//! [ng]: https://github.com/zacharywhitley/comparand/blob/main/docs/design/ngram-and-fingerprinting.md
//!
//! # What this crate provides
//!
//! * [`NGramGenerator`] — the trait every gram generator implements.
//! * [`PaddingPolicy`] — the sequence-boundary choice a generator makes.
//! * [`CharacterGrams`] — generic character (or byte) grams that yield owned
//!   windows, so padding markers absent from the input can still appear
//!   inside a gram.
//! * [`CharacterGramSlices`] — the zero-allocation fast path for pre-padded
//!   inputs. Yields borrowed slices; skips [`NGramGenerator`] because the
//!   trait's owned-gram associated type is not a fit for borrowed windows.
//! * [`TokenGrams`] — pre-tokenized token grams for the classical "shingle"
//!   representation.
//! * [`GramSet`] — the deduplicated set representation.
//! * [`GramMultiSet`] — the multiplicity-preserving representation. Jaccard
//!   on multisets is a distinct measure from Jaccard on sets; both are
//!   useful, and neither is silently substituted for the other.
//! * [`GramVector`] — the weighted-vector representation, the substrate for
//!   TF–IDF and cosine similarity that consumers land later.
//!
//! # What this crate does not do
//!
//! * **Similarity kernels.** Jaccard, Dice, cosine — those are separate
//!   algorithm crates that consume the representations built here. No
//!   representation type in this crate carries an [`AlgorithmDescriptor`]
//!   because n-gram *generation* is not itself an algorithm in Comparand's
//!   sense (it does not return a [`Distance`], [`Similarity`], or [`Score`]).
//! * **Tokenization.** [`TokenGrams`] consumes a pre-tokenized slice of
//!   `&str`s. Tokenization is a preprocessing-pipeline concern (see
//!   `docs/design/preprocessing-pipeline.md`).
//! * **Grapheme or phoneme grams.** Grapheme grams belong with the Unicode
//!   subsystem; phoneme grams belong with the phonetic subsystem. Both are
//!   planned but out of scope for the 0.1 representation layer.
//! * **`MinHash` / LSH / rolling hashes.** Those live in downstream crates.
//!
//! # Sequence type
//!
//! [`CharacterGrams`] is generic over `T: Ord + Clone`. String comparisons
//! pick their representation — bytes, `char`s, tokens — at the call site, in
//! keeping with Comparand's rule that a representation choice must never be
//! made silently by the library.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every representation type in it
//! requires heap allocation (a `Vec` window, a `BTreeSet`/`BTreeMap` backing
//! store), so **the entire public surface is behind the `alloc` feature.**
//! Under `--no-default-features` the crate compiles to an empty module,
//! which is what makes the crate safe to add as a dependency in embedded
//! configurations that only need to link against the substrate.
//!
//! [`Distance`]: comparand_core::Distance
//! [`Similarity`]: comparand_core::Similarity
//! [`Score`]: comparand_core::Score
//! [`AlgorithmDescriptor`]: comparand_core::AlgorithmDescriptor

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod character;
#[cfg(feature = "alloc")]
pub mod generator;
#[cfg(feature = "alloc")]
pub mod multiset;
#[cfg(feature = "alloc")]
pub mod padding;
#[cfg(feature = "alloc")]
pub mod set;
#[cfg(feature = "alloc")]
pub mod token;
#[cfg(feature = "alloc")]
pub mod vector;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use character::{CharacterGramSlices, CharacterGrams};
#[cfg(feature = "alloc")]
pub use generator::{NGramGenerator, count_grams};
#[cfg(feature = "alloc")]
pub use multiset::GramMultiSet;
#[cfg(feature = "alloc")]
pub use padding::{InvalidN, PaddingPolicy};
#[cfg(feature = "alloc")]
pub use set::GramSet;
#[cfg(feature = "alloc")]
pub use token::TokenGrams;
#[cfg(feature = "alloc")]
pub use vector::GramVector;
