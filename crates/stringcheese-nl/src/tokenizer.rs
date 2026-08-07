//! [`DutchTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Dutch, like German, does not use French-style clitic elision
//! (`l'`/`d'`/`qu'`) and does not need attached-particle splitting.
//! Its orthography is whitespace-and-punctuation delimited, with
//! compound nouns written as single orthographic words (e.g.
//! `boekwinkel`) that belong intact in a single token. The `ij`
//! digraph (as in `IJsselmeer`, `mijn`, `zij`) is spelled as two
//! Latin letters `i` and `j`; the tokenizer keeps them together as
//! part of the surrounding word.
//!
//! There is therefore no reason for the Dutch pack to ship its own
//! tokenizer implementation — this module exposes [`DutchTokenizer`]
//! as a **transparent wrapper** around
//! [`SimpleTokenizer`] so the pack's public surface still names a
//! Dutch-specific tokenizer type (a courtesy for callers who match the
//! language-pack pattern) without duplicating any splitting logic.
//!
//! # Non-goals
//!
//! - **Compound-noun splitting.** Dutch productively compounds nouns
//!   (`boek + winkel → boekwinkel`, `stad + huis → stadhuis`). The
//!   tokenizer emits compounds as single tokens; splitting them
//!   requires a compound-noun dictionary and is out of scope for
//!   a starter pack.
//! - **`IJ` capitalisation.** Standard Dutch capitalises the `ij`
//!   digraph as `IJ` (both letters) at the start of a word. The
//!   tokenizer preserves the surface bytes; downstream case-folding
//!   is a separate concern.
//! - **Hyphenated compounds and `-`-prefixed word parts.** Dutch
//!   sometimes hyphenates for clarity (`na-oorlogs`). The tokenizer
//!   treats the hyphen as a separator per [`SimpleTokenizer`]; each
//!   half becomes a token in its own right.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Dutch tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Dutch does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_nl::DutchTokenizer;
///
/// let toks: Vec<&str> = DutchTokenizer::new()
///     .tokenize("De kat slaapt op de mat.")
///     .collect();
/// assert_eq!(toks, ["De", "kat", "slaapt", "op", "de", "mat"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DutchTokenizer;

impl DutchTokenizer {
    /// Constructs a new [`DutchTokenizer`]. Zero-sized; free to call.
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
        DutchTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_dutch_sentence() {
        assert_eq!(collect("De kat slaapt."), ["De", "kat", "slaapt"]);
    }

    #[test]
    fn ij_digraph_stays_inside_tokens() {
        // The Dutch digraph `ij` is two Latin letters and belongs
        // together as part of the surrounding word.
        assert_eq!(collect("zij mijn wij zijn"), ["zij", "mijn", "wij", "zijn"]);
    }

    #[test]
    fn diacritics_stay_inside_tokens() {
        assert_eq!(collect("één keer"), ["één", "keer"]);
        assert_eq!(collect("coöperatie ideeën"), ["coöperatie", "ideeën"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("het jaar 2026"), ["het", "jaar", "2026"]);
    }

    #[test]
    fn compound_nouns_stay_intact() {
        // Dutch compound nouns are written as single orthographic words.
        assert_eq!(collect("boekwinkel stadhuis"), ["boekwinkel", "stadhuis"]);
    }
}
