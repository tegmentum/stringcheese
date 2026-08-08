//! [`MacedonianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Macedonian orthography is whitespace-and-punctuation delimited.
//! Every Cyrillic letter in the modern Macedonian inventory is
//! alphabetic under Unicode's `char::is_alphanumeric` classification and
//! therefore stays inside tokens naturally with the default splitter.
//! There is no reason for the Macedonian pack to ship its own tokenizer
//! implementation — this module exposes [`MacedonianTokenizer`] as a
//! **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names a Macedonian-specific tokenizer type without duplicating
//! any splitting logic.
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the modern Macedonian block is encoded as
//! **two UTF-8 bytes** (U+0400..=U+045F falls in the 2-byte range
//! U+0080..=U+07FF). The byte length of a Macedonian word is roughly
//! `2 * char_count`, and any code that mixes byte offsets with
//! character-boundary logic will silently corrupt token boundaries.
//! [`SimpleTokenizer`] itself uses [`str::char_indices`] internally, so
//! the boundaries it emits are always valid UTF-8 char boundaries and
//! the borrowed token slices are always well-formed `&str` values.
//! This wrapper adds no arithmetic of its own, so the pack does not
//! introduce a new opportunity to get byte / char math wrong.
//!
//! # Macedonian does not need an apostrophe-aware tokenizer
//!
//! Unlike Ukrainian — which promotes the ASCII apostrophe `'` (U+0027)
//! to a word-internal character — Macedonian orthography does not use a
//! word-internal apostrophe. The default splitter is adequate.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Macedonian's rich inflectional
//!   morphology — the three-way postposed definite article, plural
//!   markers, and verb endings — all shows up as suffixes on the
//!   surface form. Splitting these at the tokenizer would be wrong for
//!   IR indexing; the fused surface word is the token, and the
//!   [`crate::stemmer`] rule-based stemmer handles suffix stripping
//!   (including the signature three-way definite article).

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Macedonian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Macedonian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_mk::MacedonianTokenizer;
///
/// let toks: Vec<&str> = MacedonianTokenizer::new()
///     .tokenize("Здраво, свет! Скопје — главен град.")
///     .collect();
/// assert_eq!(toks, ["Здраво", "свет", "Скопје", "главен", "град"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MacedonianTokenizer;

impl MacedonianTokenizer {
    /// Constructs a new [`MacedonianTokenizer`]. Zero-sized; free to
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
        MacedonianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_macedonian_sentence() {
        assert_eq!(collect("Мачка спие."), ["Мачка", "спие"]);
    }

    #[test]
    fn cyrillic_letters_stay_inside_tokens() {
        // Every letter in the modern Macedonian alphabet is alphabetic
        // under Unicode's classification and stays with its word.
        assert_eq!(collect("македонски"), ["македонски"]);
        assert_eq!(collect("Скопје"), ["Скопје"]);
        // Macedonian-specific letters carry through: ѓ, ќ, љ, њ, џ, ѕ, ј.
        assert_eq!(collect("ѓавол"), ["ѓавол"]);
        assert_eq!(collect("куќа"), ["куќа"]);
        assert_eq!(collect("љубов"), ["љубов"]);
        assert_eq!(collect("њива"), ["њива"]);
        assert_eq!(collect("џез"), ["џез"]);
        assert_eq!(collect("ѕвезда"), ["ѕвезда"]);
    }

    #[test]
    fn em_dashes_are_separators() {
        // U+2014 EM DASH splits, matching general Unicode punctuation.
        assert_eq!(
            collect("Скопје — главен град"),
            ["Скопје", "главен", "град"]
        );
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("година 2026 месец"), ["година", "2026", "месец"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "Здраво свет";
        let toks: Vec<&str> = MacedonianTokenizer::new().tokenize(text).collect();
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
