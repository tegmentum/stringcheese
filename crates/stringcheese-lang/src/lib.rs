//! Language-pack infrastructure for the StringCheese toolkit.
//!
//! This crate is the umbrella's plug point for per-language behaviour —
//! stemming, stopword lists, tokenization, phonetic encoding, and
//! locale-aware collation. It defines the [`Language`] trait every
//! `stringcheese-<lang>` pack implements, the smaller [`Stemmer`],
//! [`Collator`], and [`LanguagePhoneticEncoder`] plugin traits a pack
//! can compose with, and two ready-made helper types
//! ([`Stopwords`] and [`SimpleTokenizer`]) so a pack that only needs
//! defaults can drop them in without hand-rolling either.
//!
//! # Design commitment
//!
//! Language packs are **opt-in**. This crate carries no built-in
//! language data — no stopword lists, no stemming tables, no
//! tokenizers. Each supported language ships its own crate
//! (`stringcheese-en`, `stringcheese-de`, `stringcheese-fr`, …) that
//! implements [`Language`]. Callers who need English pull in
//! `stringcheese-en` explicitly; callers who don't pay nothing (not a
//! byte of stopword list, not an entry in the code-page tables) at
//! either compile or runtime.
//!
//! The [`Language`] trait is also **data-driven, not
//! algorithm-driven**. The trait describes *what a language rule does*
//! (return the stopwords, stem this word, tokenize this text) rather
//! than *how it does it*. A pack can compose whatever internal
//! machinery it wants — a hand-written Porter stemmer, a delegated
//! Snowball step chain, a lookup-table lemmatizer — behind the same
//! trait surface.
//!
//! # Non-goals
//!
//! - No language detection. Deciding *which* [`Language`] to feed a
//!   piece of text is a caller responsibility; language detection is a
//!   downstream library.
//! - No morphological analysis or POS tagging. The trait's stemming
//!   contract is intentionally weak: return an equivalence-class
//!   representative for the word, nothing more.
//! - No lexica or wordlists. This crate never embeds vocabulary; the
//!   only text it ships is the [`Stopwords`] wrapper's iteration over
//!   the caller-supplied slice.
//!
//! # Quick-start
//!
//! Use [`SimpleTokenizer`] to split text into whitespace-and-punctuation
//! delimited tokens without pulling in a language pack:
//!
//! ```
//! use stringcheese_lang::SimpleTokenizer;
//!
//! let tokens: Vec<&str> = SimpleTokenizer::new().tokenize("hello, world!").collect();
//! assert_eq!(tokens, ["hello", "world"]);
//! ```
//!
//! Or plug in a language pack — say `stringcheese-en` — and use the
//! full [`Language`] trait surface:
//!
//! ```ignore
//! use stringcheese_en::ENGLISH;
//! use stringcheese_lang::Language;
//!
//! assert!(ENGLISH.is_stopword("the"));
//! assert_eq!(ENGLISH.stem("running"), "run");
//! ```
//!
//! # Module map
//!
//! - [`language`] — the [`Language`] trait itself, plus the
//!   [`LanguageProvider`] discovery trait for callers who want to look
//!   a language up by BCP-47 code at runtime.
//! - [`stemmer`] — the [`Stemmer`] plugin trait a pack can compose.
//! - [`collator`] — the [`Collator`] plugin trait for locale-aware
//!   sort orders.
//! - [`phonetic`] — the object-safe [`LanguagePhoneticEncoder`]
//!   trait, plus a [`SoundexAdapter`](phonetic::SoundexAdapter)-style
//!   wrapper packs use to expose the phonetic crate's typed encoders
//!   through the trait's normalized `(primary, alternate)` return
//!   type.
//! - [`stopwords`] — the [`Stopwords`] wrapper with a case-insensitive
//!   `contains` check.
//! - [`tokenizer`] — the [`SimpleTokenizer`] whitespace-and-punctuation
//!   default tokenizer.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod collator;
#[cfg(feature = "alloc")]
pub mod language;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

pub use collator::Collator;
#[cfg(feature = "alloc")]
pub use language::{Language, LanguageProvider};
#[cfg(feature = "alloc")]
pub use phonetic::LanguagePhoneticEncoder;
#[cfg(feature = "alloc")]
pub use stemmer::Stemmer;
pub use stopwords::Stopwords;
pub use tokenizer::SimpleTokenizer;

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-lang` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
