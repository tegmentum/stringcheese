//! [`PolishTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Polish orthography is whitespace-and-punctuation delimited and
//! requires no elision-splitting pass (unlike French `l'`, `d'`,
//! `qu'`). Every Polish letter, including the nine diacritic-carrying
//! ones (`ą ć ę ł ń ó ś ź ż` and their uppercase forms), is
//! alphabetic under Unicode's classification and stays with its
//! surrounding word under
//! [`SimpleTokenizer`]'s
//! `char::is_alphanumeric` splitter.
//!
//! The Polish digraphs `sz`, `cz`, `rz`, `ch`, `dz`, `dź`, `dż` are
//! written as separate Latin letters and belong intact inside a
//! single token; no digraph-decomposition pass is applied at the
//! tokenizer level (the phonetic encoder handles the digraphs; the
//! stemmer operates on `char`s so a digraph is two adjacent
//! characters in the input).
//!
//! There is therefore no reason for the Polish pack to ship its own
//! tokenizer implementation — this module exposes [`PolishTokenizer`]
//! as a **transparent wrapper** around
//! [`SimpleTokenizer`] so the pack's public surface still names a
//! Polish-specific tokenizer type (a courtesy for callers who match
//! the language-pack pattern) without duplicating any splitting logic.
//!
//! # Non-goals
//!
//! - **Hyphenated compounds.** Polish sometimes hyphenates for clarity
//!   (`biało-czerwony`, `czarno-biały`). The tokenizer treats the
//!   hyphen as a separator per
//!   [`SimpleTokenizer`]; each
//!   half becomes a token in its own right.
//! - **Clitic detection.** Polish reflexive `się` and modal `by` are
//!   free-standing orthographic words and already tokenize as their
//!   own tokens; there is no `-ing`/`-ll` style clitic to detect at
//!   the tokenizer level.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Polish tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Polish does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_pl::PolishTokenizer;
///
/// let toks: Vec<&str> = PolishTokenizer::new()
///     .tokenize("Kot śpi na macie.")
///     .collect();
/// assert_eq!(toks, ["Kot", "śpi", "na", "macie"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PolishTokenizer;

impl PolishTokenizer {
    /// Constructs a new [`PolishTokenizer`]. Zero-sized; free to call.
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
        PolishTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_polish_sentence() {
        assert_eq!(collect("Kot śpi na macie."), ["Kot", "śpi", "na", "macie"]);
    }

    #[test]
    fn every_polish_diacritic_letter_stays_inside_tokens() {
        // Each of the nine Polish diacritic-carrying letters
        // (ą ć ę ł ń ó ś ź ż) is alphabetic under Unicode and belongs
        // with its surrounding word.
        assert_eq!(collect("mąż"), ["mąż"]);
        assert_eq!(collect("ćwiczenie"), ["ćwiczenie"]);
        assert_eq!(collect("gęś"), ["gęś"]);
        assert_eq!(collect("łóżko"), ["łóżko"]);
        assert_eq!(collect("koń"), ["koń"]);
        assert_eq!(collect("Kraków"), ["Kraków"]);
        assert_eq!(collect("źle"), ["źle"]);
        assert_eq!(collect("żaba"), ["żaba"]);
    }

    #[test]
    fn digraphs_stay_intact() {
        // The Polish digraphs sz, cz, rz, ch, dz, dź, dż are written
        // as sequences of Latin letters and belong together in the
        // surrounding word.
        assert_eq!(collect("szkoła czas"), ["szkoła", "czas"]);
        assert_eq!(collect("rzeka chleb"), ["rzeka", "chleb"]);
        assert_eq!(collect("dzwon dźwięk dżem"), ["dzwon", "dźwięk", "dżem"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("rok 2026 miesiąc"), ["rok", "2026", "miesiąc"]);
    }

    #[test]
    fn uppercase_diacritics_stay_inside_tokens() {
        // The uppercase Polish diacritic letters are also alphabetic.
        assert_eq!(collect("ŻABA KOŃ MĄŻ"), ["ŻABA", "KOŃ", "MĄŻ"]);
    }

    #[test]
    fn hyphenated_compounds_split_at_the_hyphen() {
        // ASCII hyphens are separators — each half becomes a token.
        assert_eq!(collect("biało-czerwony"), ["biało", "czerwony"]);
    }
}
