//! # Sliding-window n-gram generation
//!
//! Every n-gram producer in one place, with the [`NGramUnit`]
//! discipline every StringCheese boundary uses.
//!
//! ## What's here
//!
//! - [`char_ngrams`] — `n` consecutive Unicode code points as `&str`.
//! - [`byte_ngrams`] — `n` consecutive bytes as `&[u8]`.
//! - [`token_ngrams`] — `n` consecutive `&str` tokens as `&[&str]`.
//! - `grapheme_ngrams` — `n` consecutive grapheme clusters as
//!   `&str`, in the `graphemes` module behind the `graphemes`
//!   feature (only compiled in when that feature is enabled).
//!
//! Every function returns an iterator; nothing materialises a `Vec`
//! internally. Gram slices borrow the input for lifetime `'a` — no
//! copies.
//!
//! ## Padding
//!
//! Every constructor also has a `_padded` variant that prepends
//! `n - 1` sentinel units at the start and appends `n - 1` at the
//! end. Padding is what lets similarity metrics on short inputs
//! discriminate — a 5-char string produces only one 5-gram
//! unpadded, but eight 5-grams padded.
//!
//! Sentinel unit per representation:
//!
//! - characters — `'\u{FEFF}'` (BOM; never appears in normal text)
//! - bytes — `0x00`
//! - tokens — the empty string `""`
//!
//! ## Example
//!
//! ```
//! use stringcheese_ngram::char_ngrams;
//!
//! let grams: Vec<&str> = char_ngrams("hello", 3).collect();
//! assert_eq!(grams, vec!["hel", "ell", "llo"]);
//! ```
//!
//! ## Explicit unit
//!
//! No `.chars()` vs `.bytes()` silent default anywhere. The unit
//! is in the function name (`char_ngrams`, `byte_ngrams`,
//! `token_ngrams`, `grapheme_ngrams`) so a code review can point
//! at the wrong choice.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod bytes;
pub mod chars;
#[cfg(feature = "graphemes")]
pub mod graphemes;
pub mod tokens;

pub use bytes::{byte_ngrams, byte_ngrams_padded};
pub use chars::{char_ngrams, char_ngrams_padded};
#[cfg(feature = "graphemes")]
pub use graphemes::{grapheme_ngrams, grapheme_ngrams_padded};
pub use tokens::{token_ngrams, token_ngrams_padded};

/// The semantic unit an n-gram operates on.
///
/// Mostly for callers who want to select a producer at runtime; the
/// per-unit free functions are the primary API and are typed to
/// their return shape (bytes → `&[u8]`, everything else → `&str` or
/// `&[&str]`), which this enum can't collapse without erasing
/// them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NGramUnit {
    /// Sliding window over UTF-8 bytes. Grams are `&[u8]`.
    Bytes,
    /// Sliding window over Unicode code points. Grams are `&str`.
    Chars,
    /// Sliding window over extended grapheme clusters. Grams are
    /// `&str`. Requires the `graphemes` feature.
    Graphemes,
    /// Sliding window over caller-supplied tokens. Grams are
    /// `&[&str]`.
    Tokens,
}
