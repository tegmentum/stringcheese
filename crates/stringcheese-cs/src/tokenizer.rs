//! [`CzechTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Czech, like German and Dutch, is whitespace-and-punctuation
//! delimited and requires no elision-splitting pass or apostrophe
//! promotion. Every letter of the Czech alphabet (including the
//! haček consonants `č ď ň ř š ť ž`, the e-caron `ě`, and the long
//! vowels `á é í ó ú ů ý`) satisfies [`char::is_alphanumeric`] under
//! Rust's default Unicode classification, so
//! [`SimpleTokenizer`]'s single rule (`is_alphanumeric` → word,
//! else → separator) does the right thing.
//!
//! This module exposes [`CzechTokenizer`] as a **transparent wrapper**
//! around [`SimpleTokenizer`] so the pack's public surface still
//! names a Czech-specific tokenizer type (a courtesy for callers who
//! match the language-pack pattern) without duplicating any splitting
//! logic.
//!
//! # Non-goals
//!
//! - **Hyphenated compounds.** Czech uses hyphens sparingly (a few
//!   proper nouns like `Frýdek-Místek`, and clitic combinations like
//!   `česko-slovenský`). The tokenizer treats the hyphen as a
//!   separator per [`SimpleTokenizer`]; each half becomes a token in
//!   its own right.
//! - **Sentence-final punctuation.** Czech uses the same punctuation
//!   as the Latin block (`.`, `,`, `!`, `?`, `;`, `:`); every one of
//!   these is a separator.
//! - **Digraph `ch`.** Czech treats `ch` as a single letter for
//!   collation purposes, but it is spelled as two ASCII letters `c`
//!   and `h`; the tokenizer keeps them together as part of the
//!   surrounding word (the [`SimpleTokenizer`] rule sees them as two
//!   alphanumeric scalars, both of which stay in the same run).

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Czech tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Czech does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_cs::CzechTokenizer;
///
/// let toks: Vec<&str> = CzechTokenizer::new()
///     .tokenize("Kočka spí na koberci.")
///     .collect();
/// assert_eq!(toks, ["Kočka", "spí", "na", "koberci"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CzechTokenizer;

impl CzechTokenizer {
    /// Constructs a new [`CzechTokenizer`]. Zero-sized; free to call.
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
        CzechTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_czech_sentence() {
        assert_eq!(collect("Kočka spí."), ["Kočka", "spí"]);
    }

    #[test]
    fn czech_specific_letters_stay_inside_tokens() {
        // Every Czech-specific letter is alphabetic under Unicode's
        // classification and stays with its word.
        assert_eq!(collect("žluťoučký kůň"), ["žluťoučký", "kůň"]);
        assert_eq!(collect("řeřicha"), ["řeřicha"]);
        assert_eq!(collect("čtyři"), ["čtyři"]);
        assert_eq!(collect("děkuji"), ["děkuji"]);
    }

    #[test]
    fn long_vowels_stay_inside_tokens() {
        assert_eq!(collect("dům úřad"), ["dům", "úřad"]);
        assert_eq!(collect("nejlepší"), ["nejlepší"]);
    }

    #[test]
    fn ch_digraph_stays_inside_tokens() {
        // The `ch` digraph is spelled as two ASCII letters, both
        // alphanumeric — they stay in the same token.
        assert_eq!(collect("chleba"), ["chleba"]);
        assert_eq!(collect("nechci"), ["nechci"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("rok 2026"), ["rok", "2026"]);
    }

    #[test]
    fn hyphenated_compounds_split() {
        // ASCII hyphens split (matching SimpleTokenizer's behaviour).
        assert_eq!(collect("Frýdek-Místek"), ["Frýdek", "Místek"]);
    }
}
