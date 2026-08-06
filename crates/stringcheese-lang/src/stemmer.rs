//! [`Stemmer`] — the plugin trait for word-stemming algorithms.
//!
//! A stemmer reduces a word to its stem: an equivalence-class
//! representative that collapses inflectional variants (`running`,
//! `runs`, `ran` → `run`) into one form. Different language pack
//! authors will pick different algorithms — Porter (1980) or Porter2
//! (Snowball) for English, Kraaij-Pohlmann for Dutch, an Arabic light
//! stemmer for MSA — and the trait is deliberately narrow so any of
//! them fits.
//!
//! # Contract
//!
//! Implementations should be:
//!
//! - **Deterministic.** The same input yields the same output every
//!   call.
//! - **Idempotent.** `stem(stem(w))` equals `stem(w)`. A stemmer that
//!   produces a non-fixed-point representative violates the equivalence
//!   class the trait promises.
//! - **Non-panicking.** Any `&str` input is valid; behaviour on
//!   words the algorithm was not designed for (empty, non-alphabetic,
//!   non-Latin) is implementation-defined but must not panic.
//!
//! # Cow return type
//!
//! [`Stemmer::stem`] returns `Cow<'s, str>` so an identity stem (a word
//! the algorithm chooses not to modify) can borrow the input rather
//! than allocate a new owned [`String`]. Algorithms that always
//! allocate (they build the stem into a fresh buffer rather than
//! truncating the input) simply return `Cow::Owned(_)` unconditionally.

use alloc::borrow::Cow;

/// A word-stemming algorithm.
///
/// Every language pack that supports stemming carries a [`Stemmer`]
/// implementation as a member and delegates
/// [`Language::stem`](crate::Language::stem) to it. Standalone
/// stemmers can be used directly without going through
/// [`Language`](crate::Language) at all.
///
/// See the [module-level docs](self) for the contract.
pub trait Stemmer: Send + Sync {
    /// Returns the stem of `word`.
    ///
    /// See [`Stemmer::stem`]'s trait-level contract for the promises
    /// the return value must satisfy.
    fn stem<'s>(&self, word: &'s str) -> Cow<'s, str>;
}
