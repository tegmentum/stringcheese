//! [`SwedishTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Swedish, like German and Dutch, does not use clitic elision and does
//! not need attached-particle splitting. Its orthography is whitespace-
//! and-punctuation delimited, with compound nouns written as single
//! orthographic words (e.g. `glassbil`, `barnbok`) that belong intact in
//! a single token. The three Swedish-specific letters `å`, `ä`, and `ö`
//! are single Latin scalars and stay together as part of the surrounding
//! word.
//!
//! There is therefore no reason for the Swedish pack to ship its own
//! tokenizer implementation — this module exposes [`SwedishTokenizer`]
//! as a **transparent wrapper** around
//! [`SimpleTokenizer`] so the pack's public surface still names a
//! Swedish-specific tokenizer type (a courtesy for callers who match
//! the language-pack pattern) without duplicating any splitting logic.
//!
//! # Non-goals
//!
//! - **Compound-noun splitting.** Swedish productively compounds nouns
//!   (`glass + bil → glassbil`, `barn + bok → barnbok`). The tokenizer
//!   emits compounds as single tokens; splitting them requires a
//!   compound-noun dictionary and is out of scope for a starter pack.
//! - **Hyphenated compounds and clarifying hyphens.** Swedish sometimes
//!   hyphenates for clarity (`icke-våldsam`). The tokenizer treats the
//!   hyphen as a separator per [`SimpleTokenizer`]; each half becomes
//!   a token in its own right.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Swedish tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Swedish does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_sv::SwedishTokenizer;
///
/// let toks: Vec<&str> = SwedishTokenizer::new()
///     .tokenize("Katten sover på mattan.")
///     .collect();
/// assert_eq!(toks, ["Katten", "sover", "på", "mattan"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SwedishTokenizer;

impl SwedishTokenizer {
    /// Constructs a new [`SwedishTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> Tokens<'a> {
        SimpleTokenizer::new().tokenize(text)
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        SwedishTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_swedish_sentence() {
        assert_eq!(
            collect("Katten sover på mattan."),
            ["Katten", "sover", "på", "mattan"]
        );
    }

    #[test]
    fn accented_letters_stay_inside_tokens() {
        // The Swedish-specific letters `å`, `ä`, `ö` belong together
        // with the surrounding word.
        assert_eq!(
            collect("Är där någon över?"),
            ["Är", "där", "någon", "över"]
        );
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("år 2026"), ["år", "2026"]);
    }

    #[test]
    fn compound_nouns_stay_intact() {
        // Swedish compound nouns are written as single orthographic
        // words.
        assert_eq!(collect("glassbil barnbok"), ["glassbil", "barnbok"]);
    }

    #[test]
    fn hyphen_splits_tokens() {
        // Clarifying hyphens split into separate tokens per SimpleTokenizer.
        assert_eq!(collect("icke-våldsam"), ["icke", "våldsam"]);
    }
}
