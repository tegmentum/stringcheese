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
//! The naive implementation here is `O(n²)` per word (repeated linear
//! scans for the lowest-rank pair). The design doc's Phase 2 also
//! sketches an `O(n log n)` linked-list-plus-min-heap variant matching
//! tiktoken's throughput; the current focus is *correctness* on the
//! algorithmic surface, and the naive form is enough for that.
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

#[cfg(feature = "alloc")]
pub use bpe::{
    BpeMergeTable, BpeTokenizer, BpeVocabulary, PreTokenizerRegex, TokenId, VocabularyBuilderError,
};
