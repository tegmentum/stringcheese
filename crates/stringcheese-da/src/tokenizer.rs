//! [`DanishTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Danish, like its North Germanic siblings Swedish and Norwegian, does
//! not use French-style clitic elision (`l'`/`d'`/`qu'`) and does not
//! need attached-particle splitting. Its orthography is whitespace-and-
//! punctuation delimited, with compound nouns written as single
//! orthographic words (e.g., `børnehave = børne + have` "kindergarten")
//! that belong intact in a single token. The Danish-specific letters
//! `æ`, `ø`, `å` are ordinary letters and satisfy
//! [`char::is_alphanumeric`], so the shared [`SimpleTokenizer`]
//! preserves them as word-internal characters without further
//! configuration.
//!
//! There is therefore no reason for the Danish pack to ship its own
//! tokenizer implementation — this module exposes [`DanishTokenizer`]
//! as a **transparent wrapper** around [`SimpleTokenizer`] so the
//! pack's public surface still names a Danish-specific tokenizer type
//! (a courtesy for callers who match the language-pack pattern) without
//! duplicating any splitting logic.
//!
//! # Non-goals
//!
//! - **Compound-noun splitting.** Danish productively compounds nouns
//!   (`børne + have → børnehave`). The tokenizer emits compounds as
//!   single tokens; splitting them requires a compound-noun dictionary
//!   and is out of scope for a starter pack.
//! - **Hyphenated compounds.** Danish sometimes hyphenates for clarity
//!   (`e-mail` "email"). The tokenizer treats the hyphen as a separator
//!   per [`SimpleTokenizer`]; each half becomes a token in its own
//!   right.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Danish tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Danish does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_da::DanishTokenizer;
///
/// let toks: Vec<&str> = DanishTokenizer::new()
///     .tokenize("Katten sover på måtten.")
///     .collect();
/// assert_eq!(toks, ["Katten", "sover", "på", "måtten"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DanishTokenizer;

impl DanishTokenizer {
    /// Constructs a new [`DanishTokenizer`]. Zero-sized; free to call.
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
        DanishTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_danish_sentence() {
        assert_eq!(collect("Katten sover."), ["Katten", "sover"]);
    }

    #[test]
    fn danish_letters_stay_inside_tokens() {
        // æ, ø, å are letters and satisfy `char::is_alphanumeric`.
        assert_eq!(
            collect("være ønske også år"),
            ["være", "ønske", "også", "år"]
        );
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("i året 2026"), ["i", "året", "2026"]);
    }

    #[test]
    fn compound_nouns_stay_intact() {
        // Danish compound nouns are written as single orthographic
        // words.
        assert_eq!(collect("børnehave togstation"), ["børnehave", "togstation"]);
    }

    #[test]
    fn hyphen_splits_tokens() {
        // Danish sometimes hyphenates (e-mail "email"). The simple
        // tokenizer treats the hyphen as a separator.
        assert_eq!(collect("e-mail"), ["e", "mail"]);
    }
}
