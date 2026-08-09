//! # Script-to-script transliteration
//!
//! One [`Transliterator`] trait; every implementation composes
//! cleanly.
//!
//! ## What ships
//!
//! - [`DeunicodeTransliterator`] — the general "any-script → ASCII
//!   approximation" path. Wraps [`deunicode`] because "romanise
//!   every script we've ever heard of" is exactly what deunicode
//!   solves.
//! - [`TableTransliterator`] — the classic char-to-string lookup
//!   pattern. Build one from a `(char, &str)` slice; every scalar
//!   the table doesn't cover passes through unchanged.
//! - [`Chained`] — apply two transliterators in sequence. `A ∘ B`
//!   means "first `A`, then `B` on the output."
//!
//! ## Built-in tables
//!
//! - [`tables::cyrillic_to_latin_iso9`] — Cyrillic → Latin under
//!   [ISO 9](https://en.wikipedia.org/wiki/ISO_9). Bijective;
//!   round-trips through the paired inverse table (not shipped
//!   yet — this crate seeds the pattern, not the coverage).
//!
//! Additional per-script tables (Greek → Latin, Georgian → Latin,
//! Devanagari → IAST, etc.) can land as follow-ups without
//! touching the trait.
//!
//! ## Where the language packs fit
//!
//! Per-language romanizers already live in language-pack crates —
//! `stringcheese-fa` for Persian, `stringcheese-ja` for Japanese
//! romaji, etc. Those crates can implement [`Transliterator`] on
//! their own types in follow-ups without needing this crate to
//! know about them. The trait is the coordination point.
//!
//! ## Example
//!
//! ```
//! use stringcheese_translit::{Transliterator, DeunicodeTransliterator};
//!
//! let t = DeunicodeTransliterator::new();
//! assert_eq!(t.transliterate("Café"), "Cafe");
//! // Japanese approximation via deunicode's per-scalar tables.
//! let s = t.transliterate("日本語");
//! assert!(!s.is_empty());
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod chained;
pub mod deunicode_impl;
pub mod table;
pub mod tables;

pub use chained::Chained;
pub use deunicode_impl::DeunicodeTransliterator;
pub use table::TableTransliterator;

use alloc::string::String;

/// The transliteration contract.
///
/// Object-safe by construction; `Box<dyn Transliterator>` works
/// when the implementation is picked at runtime.
pub trait Transliterator {
    /// Produce the transliterated form of `input`.
    fn transliterate(&self, input: &str) -> String;
}
