//! [`NorwegianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Norwegian, like German and Dutch, does not use French-style clitic
//! elision (`l'`/`d'`/`qu'`) and does not need attached-particle
//! splitting. Its orthography is whitespace-and-punctuation delimited,
//! with compound nouns written as single orthographic words (e.g.,
//! `fotballag = fotball + lag` "football team") that belong intact in a
//! single token. The Norwegian-specific letters `æ`, `ø`, `å` are
//! ordinary letters and satisfy [`char::is_alphanumeric`], so the
//! shared [`SimpleTokenizer`] preserves them as word-internal
//! characters without further configuration.
//!
//! There is therefore no reason for the Norwegian pack to ship its own
//! tokenizer implementation — this module exposes [`NorwegianTokenizer`]
//! as a **transparent wrapper** around [`SimpleTokenizer`] so the
//! pack's public surface still names a Norwegian-specific tokenizer
//! type (a courtesy for callers who match the language-pack pattern)
//! without duplicating any splitting logic.
//!
//! # Non-goals
//!
//! - **Compound-noun splitting.** Norwegian productively compounds
//!   nouns (`fotball + lag → fotballag`). The tokenizer emits
//!   compounds as single tokens; splitting them requires a
//!   compound-noun dictionary and is out of scope for a starter pack.
//! - **Hyphenated compounds.** Norwegian sometimes hyphenates for
//!   clarity (`e-post` "email"). The tokenizer treats the hyphen as a
//!   separator per [`SimpleTokenizer`]; each half becomes a token in
//!   its own right.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Norwegian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Norwegian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_no::NorwegianTokenizer;
///
/// let toks: Vec<&str> = NorwegianTokenizer::new()
///     .tokenize("Katten sover på matten.")
///     .collect();
/// assert_eq!(toks, ["Katten", "sover", "på", "matten"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct NorwegianTokenizer;

impl NorwegianTokenizer {
    /// Constructs a new [`NorwegianTokenizer`]. Zero-sized; free to
    /// call.
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
        NorwegianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_norwegian_sentence() {
        assert_eq!(collect("Katten sover."), ["Katten", "sover"]);
    }

    #[test]
    fn norwegian_letters_stay_inside_tokens() {
        // æ, ø, å are letters and satisfy `char::is_alphanumeric`.
        assert_eq!(
            collect("være ønske også hår"),
            ["være", "ønske", "også", "hår"]
        );
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("i året 2026"), ["i", "året", "2026"]);
    }

    #[test]
    fn compound_nouns_stay_intact() {
        // Norwegian compound nouns are written as single orthographic
        // words.
        assert_eq!(
            collect("fotballag jernbanestasjon"),
            ["fotballag", "jernbanestasjon"]
        );
    }

    #[test]
    fn hyphen_splits_tokens() {
        // Norwegian sometimes hyphenates (e-post "email"). The simple
        // tokenizer treats the hyphen as a separator.
        assert_eq!(collect("e-post"), ["e", "post"]);
    }
}
