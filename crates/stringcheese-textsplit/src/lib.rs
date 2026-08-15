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
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/textsplit.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-textsplit
//! ```
//!
//! Three groups drive every shipped splitter (RecursiveSplitter,
//! ParagraphSplitter, SentenceSplitter). Each group runs three input
//! byte lengths (1 KiB / 10 KiB / 100 KiB) crossed with two flavors
//! (`prose` — the shape the splitters were designed for — and `code`
//! — no sentence-ending punctuation, few paragraph breaks, forces
//! every splitter off its fast path). 18 measurement points total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`) at
//! `chunk_size = 1000, overlap = 0`. Wall-clock samples vary
//! ±10-20 % on a laptop under load; treat the order-of-magnitude
//! column as the load-bearing part, not the last digit. Throughput
//! reported over *input bytes* — higher is better.
//!
//! ```text
//! splitter    / flavor    1 KiB         10 KiB        100 KiB
//! ----------------------------------------------------------------
//! recursive   / prose     1.45 GiB/s    1.42 GiB/s    1.47 GiB/s
//! recursive   / code       395 MiB/s     431 MiB/s     404 MiB/s
//! paragraph   / prose     1.73 GiB/s    1.93 GiB/s    2.01 GiB/s
//! paragraph   / code       329 MiB/s     367 MiB/s     354 MiB/s
//! sentence    / prose     1.02 GiB/s    1.16 GiB/s    1.21 GiB/s
//! sentence    / code      1.19 GiB/s    1.28 GiB/s    1.30 GiB/s
//! ```
//!
//! Read:
//!
//! * **ParagraphSplitter is the fastest on prose** — one `\n\n` scan
//!   plus the merge, and no recursion when paragraphs fit under the
//!   chunk_size. RecursiveSplitter sits ~20-30 % behind on the same
//!   input because it makes an extra pass down the separator list
//!   before landing on `. ` / space.
//! * **The `code` flavor collapses paragraph + recursive to
//!   ~350-430 MiB/s**. No sentence-ending punctuation and few
//!   paragraph breaks force both splitters into their space-split
//!   fallback arm; the ~3-4× slowdown vs prose is the load-bearing
//!   caveat callers on non-prose inputs should be aware of.
//! * **SentenceSplitter is actually *faster* on `code`** than on
//!   prose. The naive `. `/`? `/`! `/`\n\n` fallback in
//!   `stringcheese-segment` finds no sentence markers in the code
//!   pool, so `.split()` returns one long "sentence" and the merge
//!   loop runs a small handful of times; on prose it splits per
//!   clause and runs many more merge iterations. This is a naive-
//!   splitter artifact — under the `sentences-icu` feature (ICU4X
//!   detection) the numbers should reverse.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression or
//!   a measurement environment change (thermal throttling, background
//!   load); rerun with `--sample-size 30` and a quiet host before
//!   filing a fix.
//!
//! ## Prior baseline (2026-08-09, `stringcheese-bench/benches/textsplit.rs`)
//!
//! Retained for context — the earlier prose-only rows against which
//! the current suite's numbers should be compared:
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
