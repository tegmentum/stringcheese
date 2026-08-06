//! N-gram representation layer for the StringCheese toolkit.
//!
//! # N-grams are a representation layer, not a metric input
//!
//! The common way to think about n-grams is as an input to Jaccard, Dice, or
//! cosine similarity. StringCheese refuses that framing. An n-gram generator
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
//! `MinHash` bucketing, and Rabin variants) lives in `stringcheese-cdc` and its
//! consumers.
//!
//! [ng]: https://github.com/tegmentum/stringcheese/blob/main/docs/design/ngram-and-fingerprinting.md
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
//!   because n-gram *generation* is not itself an algorithm in StringCheese's
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
//! keeping with StringCheese's rule that a representation choice must never be
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
//! [`Distance`]: stringcheese_core::Distance
//! [`Similarity`]: stringcheese_core::Similarity
//! [`Score`]: stringcheese_core::Score
//! [`AlgorithmDescriptor`]: stringcheese_core::AlgorithmDescriptor
//!
//! # References
//!
//! * Broder, A. Z. (1997). "On the resemblance and containment of documents."
//!   *Proceedings of the Compression and Complexity of Sequences 1997*,
//!   21-29. DOI: <https://doi.org/10.1109/SEQUEN.1997.666900> — the seminal
//!   analysis of n-gram (shingle) resemblance and containment that motivates
//!   set and multiset representations for approximate document similarity.
//! * Ukkonen, E. (1992). "Approximate string-matching with q-grams and
//!   maximal matches." *Theoretical Computer Science*, 92(1), 191-211.
//!   DOI: <https://doi.org/10.1016/0304-3975(92)90143-4> — q-gram-based
//!   approximate string comparison, the theoretical grounding for using
//!   n-gram profiles as a distance surrogate.
//! * Manning, C. D., Raghavan, P., & Schütze, H. (2008). *Introduction to
//!   Information Retrieval*. Cambridge University Press. ISBN
//!   978-0-521-86571-5. — see Chapter 3 for n-gram representations in
//!   information retrieval and their use in tolerant matching.

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
