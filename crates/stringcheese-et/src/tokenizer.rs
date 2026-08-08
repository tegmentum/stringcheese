//! [`EstonianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Estonian orthography is whitespace-and-punctuation delimited. Its
//! alphabet is the 26-letter Latin base plus four additional native
//! vowels — `ä` (U+00E4), `ö` (U+00F6), `ü` (U+00FC), `õ` (U+00F5) —
//! and two loanword consonants — `š` (U+0161), `ž` (U+017E). All six
//! are alphabetic under `char::is_alphanumeric` and therefore stay
//! inside tokens naturally with the default splitter — the tokenizer
//! has no special knowledge of Estonian orthography beyond what
//! Unicode's own letter classification provides.
//!
//! There is therefore no reason for the Estonian pack to ship its own
//! tokenizer implementation — this module exposes
//! [`EstonianTokenizer`] as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names an Estonian-specific tokenizer type (a courtesy for
//! callers who match the language-pack pattern) without duplicating
//! any splitting logic.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Estonian is agglutinative — a
//!   single orthographic word carries case and number suffixes stacked
//!   on the stem. Splitting these at the tokenizer would be wrong for
//!   IR indexing; the fused surface word is the token, and the stemmer
//!   handles suffix stripping. See [`crate::stemmer`].
//! - **Compound splitting.** Estonian forms noun–noun compounds
//!   productively (`raamatukogu` "library" = `raamatu` "book" +
//!   `kogu` "collection"). Splitting these needs a lexicon; the
//!   shipped tokenizer treats compounds as single tokens.
//! - **Case folding.** The tokenizer preserves the surface case;
//!   case folding is a separate pipeline concern (Estonian's case
//!   fold is Unicode's default — there is no dotted / dotless-`I`
//!   distinction as in Turkish).

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Estonian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Estonian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_et::EstonianTokenizer;
///
/// let toks: Vec<&str> = EstonianTokenizer::new()
///     .tokenize("Tere, maailm! Tallinn on ilus.")
///     .collect();
/// assert_eq!(toks, ["Tere", "maailm", "Tallinn", "on", "ilus"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct EstonianTokenizer;

impl EstonianTokenizer {
    /// Constructs a new [`EstonianTokenizer`]. Zero-sized; free to call.
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
        EstonianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_an_estonian_sentence() {
        assert_eq!(collect("Kass magab."), ["Kass", "magab"]);
    }

    #[test]
    fn estonian_special_letters_stay_inside_tokens() {
        // ä, ö, ü, õ are all alphabetic under Unicode.
        assert_eq!(collect("õnnetu"), ["õnnetu"]);
        assert_eq!(collect("küla"), ["küla"]);
        assert_eq!(collect("ära"), ["ära"]);
        assert_eq!(collect("öö"), ["öö"]);
        assert_eq!(collect("mõõt"), ["mõõt"]);
        // š / ž from loanwords.
        assert_eq!(collect("šokolaad"), ["šokolaad"]);
        assert_eq!(collect("žanr"), ["žanr"]);
    }

    #[test]
    fn agglutinative_word_stays_one_token() {
        // "in my house" — a single orthographic word, one token.
        // The stemmer will handle suffix stripping.
        assert_eq!(collect("majas"), ["majas"]);
        assert_eq!(collect("raamatukogus"), ["raamatukogus"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("aasta 2026 kuu"), ["aasta", "2026", "kuu"]);
    }

    #[test]
    fn punctuation_splits_tokens() {
        assert_eq!(collect("Tere, maailm!"), ["Tere", "maailm"]);
    }
}
