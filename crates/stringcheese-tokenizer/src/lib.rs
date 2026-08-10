//! Tokenizer and segmenter abstractions for the StringCheese toolkit.
//!
//! This crate ships two families of traits and a small set of built-in
//! implementations. It is the foundation Phase 1 of the tokenizer
//! subsystem described in `docs/design/tokenizers.md` — later phases add
//! subword algorithm crates (`stringcheese-tokenizer-hf`, `-wordpiece`,
//! `-sentencepiece`) and pre-configured model packs
//! (`-tiktoken`, `-huggingface`) on top of these abstractions.
//!
//! # The two-trait taxonomy
//!
//! Tokenization splits into two capability classes along a single axis:
//! *round-trippability*.
//!
//! * [`Segmenter`] — a walker that yields spans of the input. There is no
//!   commitment that concatenating the yielded spans recovers the input;
//!   whitespace, discarded punctuation, and casing may be lost. Grapheme
//!   iteration, UAX #29 word segmentation, and n-gram windowing are all
//!   segmenters.
//! * [`Tokenizer`] — a bijection (up to documented lossy exceptions —
//!   normalization, unknown-character replacement, truncation) between
//!   text and a sequence of typed tokens. The invariant is
//!   `decode(encode(text)) == text` for a well-defined input class, which
//!   is what lets a caller reason about `count(text)` as a stable
//!   quantity and swap one tokenizer for another without changing what
//!   downstream algorithms see.
//!
//! Built-in [segmenters][Segmenter] shipped here — [`WhitespaceTokenizer`],
//! [`DelimiterTokenizer`], [`IdentifierTokenizer`], [`GraphemeSegmenter`],
//! [`NgramSegmenter`] — cover the "reach for it without pulling in a
//! model" cases. The [`Tokenizer`] trait's data-heavy implementations
//! (BPE, `WordPiece`, `SentencePiece`) live in sibling crates.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. The heap-using public surface is
//! gated behind the `alloc` feature; a build with neither `std` nor
//! `alloc` compiles as an empty surface (only the module scaffolding
//! survives). This mirrors every other StringCheese algorithm crate.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

pub mod error;
pub mod traits;

#[cfg(feature = "alloc")]
pub mod builtin;

pub use error::TokenizerError;
pub use traits::{Segment, Segmenter};

#[cfg(feature = "alloc")]
pub use traits::{Encoding, Tokenizer};

#[cfg(feature = "alloc")]
pub use builtin::{
    DelimiterTokenizer, GraphemeSegmenter, IdentifierMode, IdentifierTokenizer, NgramSegmenter,
    SentenceSegmenter, WhitespaceTokenizer, WordSegmenter,
};
