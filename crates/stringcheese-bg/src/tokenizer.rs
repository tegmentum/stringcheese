//! [`BulgarianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Bulgarian orthography is whitespace-and-punctuation delimited.
//! Every Cyrillic letter in the modern Bulgarian inventory
//! (U+0410..=U+044F, dropping the Russian-only `ё`, `ы`, `э`) is
//! alphabetic under Unicode's `char::is_alphanumeric` classification
//! and therefore stays inside tokens naturally with the default
//! splitter. There is no reason for the Bulgarian pack to ship its own
//! tokenizer implementation — this module exposes
//! [`BulgarianTokenizer`] as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public
//! surface still names a Bulgarian-specific tokenizer type (a courtesy
//! for callers who match the language-pack pattern) without
//! duplicating any splitting logic.
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the modern Bulgarian block is encoded as
//! **two UTF-8 bytes** (U+0410..=U+044F falls in the 2-byte range
//! U+0080..=U+07FF). This means the byte length of a Bulgarian word is
//! roughly `2 * char_count`, and any code that mixes byte offsets with
//! character-boundary logic will silently corrupt token boundaries.
//! [`SimpleTokenizer`] itself uses [`str::char_indices`] internally, so
//! the boundaries it emits are always valid UTF-8 char boundaries and
//! the borrowed token slices are always well-formed `&str` values.
//! This wrapper adds no arithmetic of its own, so the pack does not
//! introduce a new opportunity to get byte / char math wrong.
//!
//! # Bulgarian does not need an apostrophe-aware tokenizer
//!
//! Unlike Ukrainian — which promotes the ASCII apostrophe `'` (U+0027)
//! to a word-internal character in words like `сім'я` — Bulgarian
//! orthography has no word-internal apostrophe. The hard consonant /
//! iotated vowel boundary that Ukrainian marks with `'` is not
//! marked in Bulgarian at all (Bulgarian's palatalization inventory
//! differs from Ukrainian's). The default splitter is therefore
//! adequate.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Bulgarian has rich inflectional
//!   morphology — the definite article, plural markers, and verb
//!   endings all show up as suffixes on the surface form. Splitting
//!   these at the tokenizer would be wrong for IR indexing; the fused
//!   surface word is the token, and the Snowball stemmer handles
//!   suffix stripping (including the signature Bulgarian definite
//!   article). See [`crate::snowball`].
//! - **Hyphenated compounds.** Some Bulgarian orthography uses hyphens
//!   as clitics (`по-голям` — "bigger", where `по-` is the
//!   comparative marker). The default splitter treats ASCII `-` as
//!   punctuation and splits on it; downstream systems that want to
//!   preserve the hyphenated form should apply a re-glue pass or feed
//!   the pack a pre-normalized input.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Bulgarian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Bulgarian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_bg::BulgarianTokenizer;
///
/// let toks: Vec<&str> = BulgarianTokenizer::new()
///     .tokenize("Здравей, свят! София — столица на България.")
///     .collect();
/// assert_eq!(toks, ["Здравей", "свят", "София", "столица", "на", "България"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BulgarianTokenizer;

impl BulgarianTokenizer {
    /// Constructs a new [`BulgarianTokenizer`]. Zero-sized; free to
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
        BulgarianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_bulgarian_sentence() {
        assert_eq!(collect("Котка спи."), ["Котка", "спи"]);
    }

    #[test]
    fn cyrillic_letters_stay_inside_tokens() {
        // Every letter in the modern Bulgarian alphabet is alphabetic
        // under Unicode's classification and stays with its word.
        assert_eq!(collect("български"), ["български"]);
        assert_eq!(collect("София"), ["София"]);
        // ъ is a vowel in Bulgarian — it stays with its word.
        assert_eq!(collect("България"), ["България"]);
        // щ carries just fine.
        assert_eq!(collect("щастие"), ["щастие"]);
    }

    #[test]
    fn dashes_and_em_dashes_are_separators() {
        // ASCII hyphens split — Bulgarian orthography uses hyphens
        // as clitics (`по-голям`) but the default splitter treats
        // them as separators.
        assert_eq!(collect("по-голям"), ["по", "голям"]);
        // U+2014 EM DASH splits, matching general Unicode punctuation.
        assert_eq!(collect("София — столица"), ["София", "столица"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("година 2026 месец"), ["година", "2026", "месец"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "Здравей свят";
        let toks: Vec<&str> = BulgarianTokenizer::new().tokenize(text).collect();
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
