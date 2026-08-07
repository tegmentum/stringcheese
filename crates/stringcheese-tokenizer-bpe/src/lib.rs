//! Data-neutral Byte-Pair Encoding tokenizer for the StringCheese toolkit.
//!
//! This crate implements the BPE *algorithm* — Sennrich, Haddow, and
//! Birch (2016), extending Gage (1994) — over caller-supplied merge
//! tables and vocabularies. It does not ship any specific model's data;
//! the pre-configured `OpenAI` variants (`cl100k_base`, `o200k_base`,
//! `r50k_base`, ...) live in the downstream
//! `stringcheese-tokenizer-tiktoken` crate (Phase 3, not yet
//! implemented), and Hugging Face `tokenizer.json` parsing lives in
//! `stringcheese-tokenizer-huggingface` (Phase 5).
//!
//! # Algorithm
//!
//! Given a **merge table** (ordered pair merges, each with a rank; lower
//! rank = higher priority) and a **vocabulary** (byte string ↔ token id
//! bijection), encoding proceeds as:
//!
//! 1. Extract special tokens: any registered special-token surface
//!    string is matched literally (longest match first) and emitted as
//!    its reserved id without participating in the merge loop.
//! 2. Pre-tokenize the surrounding text into "words" via an optional
//!    string literal (Phase 2b will add full regex support).
//!    Whitespace is the default fallback.
//! 3. For each word: convert to UTF-8 bytes; seed a `pieces` sequence
//!    with the byte-level ids for those bytes.
//! 4. Repeatedly locate the adjacent pair whose merge rank is lowest and
//!    replace it with the merged token; stop when no pair remains in
//!    the merge table.
//! 5. Look up each final piece in the vocabulary to obtain the emitted
//!    [`TokenId`]s.
//!
//! Decoding: concatenate the byte string for each token id and interpret
//! the result as UTF-8.
//!
//! # Complexity
//!
//! Encoding runs an `O(n log n)` merge loop per word: a doubly-linked
//! list of pieces backed by an arena `Vec`, plus a `BinaryHeap` (used as
//! a min-heap via `Reverse`) of pending merge candidates keyed by
//! `(rank, left_idx)`. Stale heap entries are handled by *lazy
//! deletion* — we don't purge them when a merge happens; we skip them
//! when they surface at the top and no longer satisfy the adjacency
//! and rank invariants. This is the Sennrich, Haddow, & Birch (2016)
//! formulation and matches tiktoken's throughput shape. The naive
//! `O(n²)` variant is retained inside `#[cfg(test)]` as the oracle
//! demanded by the design doc's Phase 2 acceptance criterion.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. BPE inherently allocates —
//! per-word piece vectors, vocabulary maps, and merge-rank lookups all
//! need heap — so the entire public surface is behind `alloc`; a build
//! with neither `std` nor `alloc` compiles as an empty surface.
//!
//! # References
//!
//! * Sennrich, R., Haddow, B., & Birch, A. (2016). "Neural Machine
//!   Translation of Rare Words with Subword Units." *ACL 2016*.
//!   arXiv:1508.07909, <https://arxiv.org/abs/1508.07909>.
//! * Gage, P. (1994). "A New Algorithm for Data Compression." *The C
//!   Users Journal*, 12(2), 23–38. The original byte-pair-encoding
//!   compression algorithm.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

#[cfg(feature = "alloc")]
mod bpe;

// The Phase 2b regex-based pre-tokenizer lives in its own module so
// the (heavier) `fancy-regex` backend only compiles when `std` is
// enabled — the `no_std` build of this crate keeps its footprint at
// the `PreTokenizerRegex::Literal` fallback.
#[cfg(feature = "std")]
mod pre_tokenizer;

#[cfg(feature = "alloc")]
pub use bpe::{
    BpeMergeTable, BpeTokenizer, BpeVocabulary, PreTokenizerRegex, TokenId, VocabularyBuilderError,
};

#[cfg(feature = "std")]
pub use pre_tokenizer::{
    GPT2_PATTERN, PreTokenizerCompileError, RegexPreTokenizer, TIKTOKEN_CANONICAL_PATTERN,
};
