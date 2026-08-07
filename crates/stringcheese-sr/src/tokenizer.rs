//! [`SerbianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Serbian orthography is whitespace-and-punctuation delimited in both
//! scripts. Every letter of the Vukovica (Cyrillic) alphabet and every
//! letter of Gaj's Latin alphabet — including the diacritic letters
//! `đ ž ć č š` and the digraph-letter Cyrillic scalars `љ њ џ ђ ћ` —
//! satisfies [`char::is_alphanumeric`] under Unicode classification.
//! [`SimpleTokenizer`] therefore
//! keeps every Serbian letter, in either script, inside its enclosing
//! token without any special-case machinery.
//!
//! This module exposes [`SerbianTokenizer`] as a transparent wrapper
//! around the default splitter so the pack's public surface names a
//! Serbian-specific tokenizer type without duplicating any splitting
//! logic — the same pattern the other language packs follow.
//!
//! # Dual-script tokenization
//!
//! A single input document may mix scripts (a Latin loan word inside
//! Cyrillic prose, a code block, a URL). The tokenizer emits tokens
//! in whatever script they appear in; downstream normalization to a
//! canonical script is a separate step (see [`crate::scripts`] for
//! the transliteration helpers).
//!
//! # Non-goals
//!
//! - **Digraph-aware tokenization.** The tokenizer treats every
//!   scalar as a single character; `lj` in Latin input remains two
//!   tokens' worth of characters within one token. Digraph collapse
//!   is a normalization concern handled by
//!   [`crate::scripts::to_cyrillic`], not the tokenizer.
//! - **Sentence segmentation.** The stopword / stemmer surface is
//!   token-level; sentence boundaries are out of scope.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Serbian tokenizer.
///
/// Zero-sized wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Serbian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_sr::SerbianTokenizer;
///
/// let toks: Vec<&str> = SerbianTokenizer::new()
///     .tokenize("Београд је главни град Србије.")
///     .collect();
/// assert_eq!(toks, ["Београд", "је", "главни", "град", "Србије"]);
///
/// let toks: Vec<&str> = SerbianTokenizer::new()
///     .tokenize("Beograd je glavni grad Srbije.")
///     .collect();
/// assert_eq!(toks, ["Beograd", "je", "glavni", "grad", "Srbije"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SerbianTokenizer;

impl SerbianTokenizer {
    /// Constructs a new [`SerbianTokenizer`]. Zero-sized; free to call.
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
        SerbianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_cyrillic_sentence() {
        assert_eq!(collect("Кућа је велика."), ["Кућа", "је", "велика"]);
    }

    #[test]
    fn splits_latin_sentence() {
        assert_eq!(collect("Kuća je velika."), ["Kuća", "je", "velika"]);
    }

    #[test]
    fn digraph_letters_stay_inside_cyrillic_tokens() {
        // Cyrillic digraph letters (single scalars) stay put.
        assert_eq!(collect("љубав"), ["љубав"]);
        assert_eq!(collect("његош"), ["његош"]);
        assert_eq!(collect("џем"), ["џем"]);
    }

    #[test]
    fn digraph_sequences_stay_inside_latin_tokens() {
        // Latin digraphs are two characters but both are alphabetic
        // and remain in the same token.
        assert_eq!(collect("ljubav"), ["ljubav"]);
        assert_eq!(collect("njegoš"), ["njegoš"]);
        assert_eq!(collect("džem"), ["džem"]);
    }

    #[test]
    fn diacritics_stay_inside_tokens() {
        assert_eq!(
            collect("čаša ćup đak žito šuma"),
            ["čаša", "ćup", "đak", "žito", "šuma"]
        );
    }

    #[test]
    fn mixed_script_input_preserves_scripts() {
        // A word in each script — no cross-script joining, no fold.
        assert_eq!(collect("Beograd Београд"), ["Beograd", "Београд"],);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("година 2026"), ["година", "2026"]);
    }
}
