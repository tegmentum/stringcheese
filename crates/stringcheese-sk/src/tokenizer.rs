//! [`SlovakTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Slovak, like Czech, is whitespace-and-punctuation delimited and
//! requires no elision-splitting pass or apostrophe promotion. Every
//! letter of the Slovak alphabet (including the haček consonants
//! `č ď ľ ň š ť ž`, the syllabic long consonants `ĺ ŕ`, the long
//! vowels `á é í ó ú ý`, and the Slovak-specific `ä` and `ô`)
//! satisfies [`char::is_alphanumeric`] under Rust's default Unicode
//! classification, so [`SimpleTokenizer`]'s single rule
//! (`is_alphanumeric` → word, else → separator) does the right thing.
//!
//! This module exposes [`SlovakTokenizer`] as a **transparent wrapper**
//! around [`SimpleTokenizer`] so the pack's public surface still
//! names a Slovak-specific tokenizer type (a courtesy for callers who
//! match the language-pack pattern) without duplicating any splitting
//! logic.
//!
//! # Non-goals
//!
//! - **Hyphenated compounds.** Slovak uses hyphens sparingly (a few
//!   proper nouns and adjectival compounds like `česko-slovenský`).
//!   The tokenizer treats the hyphen as a separator per
//!   [`SimpleTokenizer`]; each half becomes a token in its own right.
//! - **Sentence-final punctuation.** Slovak uses the same punctuation
//!   as the Latin block (`.`, `,`, `!`, `?`, `;`, `:`); every one of
//!   these is a separator.
//! - **Digraph `ch`.** Slovak treats `ch` as a single letter for
//!   collation purposes (as Czech does), but it is spelled as two
//!   ASCII letters `c` and `h`; the tokenizer keeps them together as
//!   part of the surrounding word (the [`SimpleTokenizer`] rule sees
//!   them as two alphanumeric scalars, both of which stay in the same
//!   run).
//! - **Digraphs `dz` / `dž`.** Slovak treats these as single letters
//!   for collation; both are ASCII/Slovak-alphabetic and stay in the
//!   same run.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Slovak tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Slovak does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_sk::SlovakTokenizer;
///
/// let toks: Vec<&str> = SlovakTokenizer::new()
///     .tokenize("Mačka spí na koberci.")
///     .collect();
/// assert_eq!(toks, ["Mačka", "spí", "na", "koberci"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlovakTokenizer;

impl SlovakTokenizer {
    /// Constructs a new [`SlovakTokenizer`]. Zero-sized; free to call.
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
        SlovakTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_slovak_sentence() {
        assert_eq!(collect("Mačka spí."), ["Mačka", "spí"]);
    }

    #[test]
    fn slovak_specific_letters_stay_inside_tokens() {
        // Every Slovak-specific letter is alphabetic under Unicode's
        // classification and stays with its word.
        assert_eq!(collect("žltý kôň"), ["žltý", "kôň"]);
        assert_eq!(collect("späť"), ["späť"]);
        assert_eq!(collect("štyri"), ["štyri"]);
        assert_eq!(collect("ďakujem"), ["ďakujem"]);
    }

    #[test]
    fn palatal_l_and_syllabic_letters_stay_inside_tokens() {
        // `ľ`, `ĺ`, `ŕ` all stay inside their tokens.
        assert_eq!(collect("koľko"), ["koľko"]);
        assert_eq!(collect("stĺp"), ["stĺp"]);
        assert_eq!(collect("vŕba"), ["vŕba"]);
    }

    #[test]
    fn long_vowels_stay_inside_tokens() {
        assert_eq!(collect("dom úrad"), ["dom", "úrad"]);
        assert_eq!(collect("najlepší"), ["najlepší"]);
    }

    #[test]
    fn ch_digraph_stays_inside_tokens() {
        // The `ch` digraph is spelled as two ASCII letters, both
        // alphanumeric — they stay in the same token.
        assert_eq!(collect("chlieb"), ["chlieb"]);
        assert_eq!(collect("nechcem"), ["nechcem"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("rok 2026"), ["rok", "2026"]);
    }

    #[test]
    fn hyphenated_compounds_split() {
        // ASCII hyphens split (matching SimpleTokenizer's behaviour).
        assert_eq!(collect("česko-slovenský"), ["česko", "slovenský"]);
    }
}
