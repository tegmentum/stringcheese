//! [`RussianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Russian orthography is whitespace-and-punctuation delimited. Every
//! Cyrillic letter in the modern Russian inventory (U+0410..=U+044F
//! plus `Ё` U+0401 / `ё` U+0451) is alphabetic under Unicode's
//! `char::is_alphanumeric` classification and therefore stays inside
//! tokens naturally with the default splitter. There is no reason for
//! the Russian pack to ship its own tokenizer implementation — this
//! module exposes [`RussianTokenizer`] as a **transparent wrapper**
//! around [`stringcheese_lang::SimpleTokenizer`] so the pack's public
//! surface still names a Russian-specific tokenizer type (a courtesy
//! for callers who match the language-pack pattern) without
//! duplicating any splitting logic.
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the modern Russian block is encoded as
//! **two UTF-8 bytes** (U+0400..=U+04FF fall in the 2-byte range
//! U+0080..=U+07FF). This means the byte length of a Russian word is
//! roughly `2 * char_count`, and any code that mixes byte offsets with
//! character-boundary logic will silently corrupt token boundaries.
//! [`SimpleTokenizer`] itself uses [`str::char_indices`] internally, so
//! the boundaries it emits are always valid UTF-8 char boundaries and
//! the borrowed token slices are always well-formed `&str` values.
//! This wrapper adds no arithmetic of its own, so the pack does not
//! introduce a new opportunity to get byte / char math wrong.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Russian has rich inflectional
//!   morphology — case, number, gender, and verb aspect all show up as
//!   suffixes on the surface form. Splitting these at the tokenizer
//!   would be wrong for IR indexing; the fused surface word is the
//!   token, and the Snowball stemmer handles suffix stripping. See
//!   [`crate::snowball`].
//! - **Hyphenated compounds.** Some Russian orthography (`по-русски`,
//!   `кто-то`) uses hyphens as clitics that link the pieces into a
//!   single lexeme. The default splitter treats ASCII `-` as
//!   punctuation and splits on it, matching Snowball's Russian
//!   pipeline; downstream systems that want to preserve the hyphenated
//!   form should apply a re-glue pass or feed the pack a
//!   pre-normalized input.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Russian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Russian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_ru::RussianTokenizer;
///
/// let toks: Vec<&str> = RussianTokenizer::new()
///     .tokenize("Привет, мир! Москва — столица России.")
///     .collect();
/// assert_eq!(toks, ["Привет", "мир", "Москва", "столица", "России"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RussianTokenizer;

impl RussianTokenizer {
    /// Constructs a new [`RussianTokenizer`]. Zero-sized; free to call.
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
        RussianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_russian_sentence() {
        assert_eq!(collect("Кот спит."), ["Кот", "спит"]);
    }

    #[test]
    fn cyrillic_letters_stay_inside_tokens() {
        // Every letter in the modern Russian alphabet is alphabetic
        // under Unicode's classification and stays with its word.
        assert_eq!(collect("русский"), ["русский"]);
        assert_eq!(collect("Ёлка"), ["Ёлка"]);
        assert_eq!(collect("щётка"), ["щётка"]);
        assert_eq!(collect("подъезд"), ["подъезд"]);
    }

    #[test]
    fn dashes_and_em_dashes_are_separators() {
        // ASCII hyphens split — Russian orthography uses hyphens as
        // clitics (`кто-то`) but the default splitter matches Snowball's
        // Russian pipeline, which treats them as separators.
        assert_eq!(collect("кто-то"), ["кто", "то"]);
        // U+2014 EM DASH splits, matching general Unicode punctuation.
        assert_eq!(collect("Москва — столица"), ["Москва", "столица"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("год 2026 месяц"), ["год", "2026", "месяц"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "Привет мир";
        let toks: Vec<&str> = RussianTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. This is the key safety property of the
        // char_indices-based splitter — no byte offset can slice a
        // multi-byte Cyrillic scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
