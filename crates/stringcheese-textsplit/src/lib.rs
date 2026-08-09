//! # Text splitters and semantic chunkers
//!
//! Complements the byte-oriented content-defined chunking in
//! `stringcheese-cdc` with the text-oriented splitters LLM
//! pipelines actually reach for.
//!
//! ## What ships
//!
//! - [`TextSplitter`] — the common trait: `split(&str) -> Vec<Chunk>`.
//! - [`Chunk`] — one output chunk, carrying both the text and its
//!   byte range in the original input (so downstream can point
//!   back at what was chunked).
//! - [`RecursiveSplitter`] — the classic LangChain-style splitter.
//!   Given a separator list `["\n\n", "\n", " ", ""]` and a target
//!   chunk size, split by the first separator that yields pieces
//!   under target, and recurse on pieces that are still too big.
//!   Optional overlap between adjacent chunks.
//! - [`ParagraphSplitter`] — group runs of text separated by blank
//!   lines. One chunk per paragraph unless a paragraph exceeds the
//!   size limit, in which case it falls through to the recursive
//!   splitter.
//! - [`SentenceSplitter`] — collect sentences into chunks until
//!   the size threshold is reached; each chunk is a run of one or
//!   more whole sentences. Uses [`stringcheese_segment`] for the
//!   sentence-boundary detection.
//!
//! ## Sizing model
//!
//! Every splitter measures chunk size in **UTF-8 bytes**, not
//! code points or graphemes. Byte-count is what LLM APIs bill on
//! (rough proxy for tokens; use the tokenizer when the exact
//! count matters) and what every downstream string buffer sizes
//! against.
//!
//! ## Overlap
//!
//! Overlap is measured in bytes too, and always taken from the
//! **end** of the previous chunk. When two adjacent chunks would
//! overlap by more bytes than either one contains, the shorter
//! chunk sets the overlap bound.
//!
//! ## Baseline (2026-08-09)
//!
//! Throughput on synthetic prose, from
//! `stringcheese-bench/benches/textsplit.rs` (chunk_size 1000
//! where applicable):
//!
//! | Splitter                      | 1 KB      | 8 KB      | 32 KB    |
//! |-------------------------------|-----------|-----------|----------|
//! | `ParagraphSplitter`           | 1.63 GiB/s | 1.59 GiB/s | 1.68 GiB/s |
//! | `RecursiveSplitter` (no overlap) | 1.32 GiB/s | 1.24 GiB/s | 1.23 GiB/s |
//! | `RecursiveSplitter` (overlap 200) | 1.09 GiB/s | 1.08 GiB/s | 1.12 GiB/s |
//! | `SentenceSplitter`            | 1.06 GiB/s |   947 MiB/s |   767 MiB/s |
//!
//! `ParagraphSplitter` is the fastest — one `\n\n` scan and no
//! recursion when paragraphs fit under the chunk_size.
//! `RecursiveSplitter` sits ~20 % behind (extra pass for the
//! greedy merge). Overlap adds a ~15 % constant cost per chunk.
//! `SentenceSplitter` used to be 14× slower than the others —
//! `input[cursor..].find(s)` per sentence was O(N²) overall;
//! the current implementation tracks cursor directly for O(N).
//! At small chunk_size (`RecursiveSplitter::new(200, 0)`)
//! throughput drops to ~550 MiB/s at 32 KB because merge
//! iteration count grows.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
// Baseline table mentions `ParagraphSplitter` / `RecursiveSplitter`
// / `SentenceSplitter` in table cells; doc_markdown flags them
// for backticks and the tables get harder to read with more
// noise. The names themselves are already legibly formatted.
#![allow(clippy::doc_markdown)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod paragraph;
pub mod recursive;
pub mod sentence;

pub use paragraph::ParagraphSplitter;
pub use recursive::RecursiveSplitter;
pub use sentence::SentenceSplitter;

use alloc::string::String;
use alloc::vec::Vec;

/// One output chunk from a [`TextSplitter`].
///
/// Carries both the chunk text and its byte range in the original
/// input so downstream can highlight, cite, or reconstruct the
/// source location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Chunk text. Always a valid `String`.
    pub text: String,
    /// Byte offset in the original input where this chunk starts.
    pub start: usize,
    /// Byte offset (exclusive) in the original input where this
    /// chunk ends. `end - start == text.len()` when there's no
    /// overlap; overlapping chunks may see `end - start > text.len()`
    /// on the first chunk only if the source and text diverge
    /// (they never do in the shipped splitters — `text` is always
    /// a substring of the input).
    pub end: usize,
}

impl Chunk {
    /// Length of the chunk in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// True when the chunk carries no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// The common text-splitter contract.
pub trait TextSplitter {
    /// Split `input` into chunks per the splitter's rules.
    fn split(&self, input: &str) -> Vec<Chunk>;
}
