//! [`IcelandicTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Icelandic, like its North Germanic siblings Swedish / Norwegian /
//! Danish, does not use French-style clitic elision (`l'`/`d'`/`qu'`)
//! and does not need attached-particle splitting. Its orthography is
//! whitespace-and-punctuation delimited, with the Icelandic-specific
//! letters `á`, `ð`, `é`, `í`, `ó`, `ú`, `ý`, `þ`, `æ`, `ö` all
//! ordinary letters that satisfy [`char::is_alphanumeric`], so the
//! shared [`SimpleTokenizer`] preserves them as word-internal
//! characters without further configuration.
//!
//! There is therefore no reason for the Icelandic pack to ship its
//! own tokenizer implementation — this module exposes
//! [`IcelandicTokenizer`] as a **transparent wrapper** around
//! [`SimpleTokenizer`] so the pack's public surface still names an
//! Icelandic-specific tokenizer type (a courtesy for callers who match
//! the language-pack pattern) without duplicating any splitting logic.
//!
//! # Non-goals
//!
//! - **Compound-noun splitting.** Icelandic productively compounds
//!   nouns (`bókasafn = bóka + safn` "library"). The tokenizer emits
//!   compounds as single tokens; splitting them requires a compound-
//!   noun dictionary and is out of scope for a starter pack.
//! - **Hyphenated compounds.** Icelandic sometimes hyphenates for
//!   clarity. The tokenizer treats the hyphen as a separator per
//!   [`SimpleTokenizer`]; each half becomes a token in its own right.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Icelandic tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Icelandic does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_is::IcelandicTokenizer;
///
/// let toks: Vec<&str> = IcelandicTokenizer::new()
///     .tokenize("Hún hefur farið í búðina.")
///     .collect();
/// assert_eq!(toks, ["Hún", "hefur", "farið", "í", "búðina"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IcelandicTokenizer;

impl IcelandicTokenizer {
    /// Constructs a new [`IcelandicTokenizer`]. Zero-sized; free to
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
        IcelandicTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_an_icelandic_sentence() {
        assert_eq!(collect("Hún fór heim."), ["Hún", "fór", "heim"]);
    }

    #[test]
    fn icelandic_letters_stay_inside_tokens() {
        // þ, ð, æ, ö, and the vowel accents are letters and satisfy
        // `char::is_alphanumeric`.
        assert_eq!(
            collect("þú ert góður maður"),
            ["þú", "ert", "góður", "maður"]
        );
        assert_eq!(collect("Ísland æfing öll"), ["Ísland", "æfing", "öll"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("árið 2026"), ["árið", "2026"]);
    }

    #[test]
    fn compound_nouns_stay_intact() {
        // Icelandic compound nouns are written as single orthographic
        // words.
        assert_eq!(collect("bókasafn járnbraut"), ["bókasafn", "járnbraut"]);
    }

    #[test]
    fn hyphen_splits_tokens() {
        // The simple tokenizer treats the hyphen as a separator.
        assert_eq!(collect("tölvu-póstur"), ["tölvu", "póstur"]);
    }
}
