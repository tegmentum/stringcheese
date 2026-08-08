//! [`FinnishTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Finnish orthography is whitespace-and-punctuation delimited. Its
//! alphabet is the 26-letter Latin base plus three additional vowels:
//! `ä` (U+00E4), `ö` (U+00F6), and `å` (U+00E5, used almost exclusively
//! in Swedish loanwords and Swedish-origin names). All three are
//! alphabetic under `char::is_alphanumeric` and therefore stay inside
//! tokens naturally with the default splitter — the tokenizer has no
//! special knowledge of Finnish orthography beyond what Unicode's own
//! letter classification provides.
//!
//! There is therefore no reason for the Finnish pack to ship its own
//! tokenizer implementation — this module exposes [`FinnishTokenizer`]
//! as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names a Finnish-specific tokenizer type (a courtesy for
//! callers who match the language-pack pattern) without duplicating
//! any splitting logic.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Finnish is agglutinative — a
//!   single orthographic word can carry 5–10 morphemes (case,
//!   possessive, number, particle stack). Splitting these at the
//!   tokenizer would be wrong for IR indexing; the fused surface
//!   word is the token, and the Snowball stemmer handles suffix
//!   stripping. See [`crate::snowball`].
//! - **Compound splitting.** Finnish forms noun–noun compounds
//!   productively (`kirjakauppa` "bookshop" = `kirja` "book" +
//!   `kauppa` "shop"). Splitting these needs a lexicon; the shipped
//!   tokenizer treats compounds as single tokens.
//! - **Case folding.** The tokenizer preserves the surface case;
//!   case folding is a separate pipeline concern (Finnish's case
//!   fold is Unicode's default — there is no dotted / dotless-`I`
//!   distinction as in Turkish).

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Finnish tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Finnish does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_fi::FinnishTokenizer;
///
/// let toks: Vec<&str> = FinnishTokenizer::new()
///     .tokenize("Hei, maailma! Helsinki on kaunis.")
///     .collect();
/// assert_eq!(toks, ["Hei", "maailma", "Helsinki", "on", "kaunis"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FinnishTokenizer;

impl FinnishTokenizer {
    /// Constructs a new [`FinnishTokenizer`]. Zero-sized; free to call.
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
        FinnishTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_finnish_sentence() {
        assert_eq!(collect("Kissa nukkuu."), ["Kissa", "nukkuu"]);
    }

    #[test]
    fn finnish_special_letters_stay_inside_tokens() {
        // ä, ö, å are all alphabetic under Unicode.
        assert_eq!(collect("pöytä"), ["pöytä"]);
        assert_eq!(collect("sydän"), ["sydän"]);
        assert_eq!(collect("Åland"), ["Åland"]);
        assert_eq!(collect("Jyväskylä"), ["Jyväskylä"]);
        assert_eq!(collect("työ"), ["työ"]);
    }

    #[test]
    fn agglutinative_word_stays_one_token() {
        // "in my house also" — one orthographic word, one token.
        // The stemmer will handle suffix stripping.
        assert_eq!(collect("talossanikin"), ["talossanikin"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(
            collect("vuosi 2026 kuukausi"),
            ["vuosi", "2026", "kuukausi"]
        );
    }

    #[test]
    fn punctuation_splits_tokens() {
        assert_eq!(collect("Hei, maailma!"), ["Hei", "maailma"]);
    }
}
