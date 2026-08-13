//! [`ItalianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Italian, like Spanish, does not use French-style clitic elision
//! (`l'`, `d'`, `qu'`) as a productive separator — the elided
//! article `l'` is written attached to the following noun (`l'anno`,
//! `l'uomo`) and is tokenised as a single orthographic word by the
//! default splitter. Italian also does not agglutinate compound
//! nouns the way German does. Its orthography is whitespace-and-
//! punctuation delimited; the default [`SimpleTokenizer`] is
//! correct on Italian text without further tuning.
//!
//! There is therefore no reason for the Italian pack to ship its
//! own tokenizer implementation — this module exposes
//! [`ItalianTokenizer`] as a **transparent wrapper** around
//! [`SimpleTokenizer`] so the pack's public surface still names an
//! Italian-specific tokenizer type (a courtesy for callers who
//! match the language-pack pattern) without duplicating any
//! splitting logic.
//!
//! # Non-goals
//!
//! - **Elision splitting.** `l'anno` splits at the apostrophe under
//!   [`SimpleTokenizer`] (the apostrophe is not
//!   [`char::is_alphanumeric`]) — the elided article becomes an
//!   `l` token and the noun a separate token. This matches the
//!   Spanish / Portuguese behaviour for parallel constructions; a
//!   future elision-aware tokenizer that keeps `l'anno` as
//!   `l'anno` (or expands it to `lo`/`la` + noun) is a follow-up
//!   that would ship alongside a full Italian IR pipeline.
//! - **Enclitic pronouns.** Italian, like Spanish, cliticises
//!   object pronouns onto infinitives, gerunds, and affirmative
//!   imperatives (`dirmi`, `dandogli`, `parlarne`). The surface
//!   token *is* the enclitic-fused form; splitting it is a
//!   stemmer-level concern, not a tokenizer concern.
//! - **Apostrophe-preserving reads.** Italian orthography treats
//!   the ASCII apostrophe and the U+2019 right single quotation
//!   mark as the same mark for the elision purpose. This
//!   tokenizer preserves neither — both are separators.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Italian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Italian does not need a
/// bespoke splitter.
///
/// # Example
///
/// ```
/// use stringcheese_it::ItalianTokenizer;
///
/// let toks: Vec<&str> = ItalianTokenizer::new()
///     .tokenize("Il gatto dorme sul tappeto.")
///     .collect();
/// assert_eq!(toks, ["Il", "gatto", "dorme", "sul", "tappeto"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ItalianTokenizer;

impl ItalianTokenizer {
    /// Constructs a new [`ItalianTokenizer`]. Zero-sized; free to
    /// call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of
    /// the input; no allocation is performed.
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
        ItalianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_an_italian_sentence() {
        assert_eq!(
            collect("Il gatto dorme sul tappeto."),
            ["Il", "gatto", "dorme", "sul", "tappeto"]
        );
    }

    #[test]
    fn accented_characters_stay_inside_tokens() {
        // Grave-accented final vowels are core Italian orthography;
        // `char::is_alphanumeric` classifies them as letters so they
        // stay inside their tokens.
        assert_eq!(collect("città perché caffè"), ["città", "perché", "caffè"]);
    }

    #[test]
    fn elided_article_splits_at_apostrophe() {
        // Under `SimpleTokenizer` the apostrophe is a separator, so
        // `l'anno` becomes two tokens `l` + `anno`. Documented in
        // the module-level docs as a Non-goal.
        assert_eq!(collect("l'anno scorso"), ["l", "anno", "scorso"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("nel 2026 arriva"), ["nel", "2026", "arriva"]);
    }

    #[test]
    fn punctuation_separates_tokens() {
        assert_eq!(
            collect("Ciao, mondo! Come stai?"),
            ["Ciao", "mondo", "Come", "stai"]
        );
    }
}
