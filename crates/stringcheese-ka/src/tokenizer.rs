//! [`GeorgianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Georgian orthography is whitespace-and-punctuation delimited (like
//! Greek and unlike Chinese / Japanese / Thai). Every letter in the
//! Mkhedruli block (U+10D0..=U+10FF), every letter in the Mtavruli
//! block (U+1C90..=U+1CBF), every letter in the historical Asomtavruli
//! (U+10A0..=U+10CF), and every letter in Nuskhuri (U+2D00..=U+2D2F)
//! is alphabetic under Unicode's [`char::is_alphanumeric`]
//! classification, so all four Georgian scripts stay inside tokens
//! naturally with the default splitter.
//!
//! Georgian also uses the paragraph separator `჻` U+10FB, which
//! Unicode classifies as punctuation — it splits under the default
//! rules. There is no reason for the Georgian pack to ship its own
//! tokenizer implementation; this module exposes [`GeorgianTokenizer`]
//! as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names a Georgian-specific tokenizer type (a courtesy for
//! callers who match the language-pack pattern) without duplicating
//! any splitting logic.
//!
//! # Byte-vs-char safety
//!
//! Every Mkhedruli scalar (U+10D0..=U+10FF) is encoded as **three
//! UTF-8 bytes** — the block falls entirely inside U+0800..=U+FFFF,
//! UTF-8's 3-byte window. The byte length of a Georgian word is
//! roughly `3 * char_count`; any code that mixed byte offsets with
//! character-boundary logic would silently corrupt token boundaries.
//! [`SimpleTokenizer`] itself uses [`str::char_indices`] internally,
//! so the boundaries it emits are always valid UTF-8 char boundaries
//! and the borrowed token slices are always well-formed `&str`
//! values. This wrapper adds no arithmetic of its own, so the pack
//! does not introduce a new opportunity to get byte / char math
//! wrong.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Georgian is agglutinative — case
//!   endings, plural markers, and postpositions all stack as suffixes
//!   on the surface form. Splitting these at the tokenizer would be
//!   wrong for IR indexing; the fused surface word is the token, and
//!   the Georgian stemmer handles suffix stripping. See
//!   [`crate::stemmer`].
//! - **Old Georgian abbreviation markers.** Old Georgian used
//!   contraction / abbreviation strokes over some words; those are
//!   not modelled here. Modern Georgian orthography is regular.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Georgian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Georgian does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_ka::GeorgianTokenizer;
///
/// let toks: Vec<&str> = GeorgianTokenizer::new()
///     .tokenize("გამარჯობა, მსოფლიო! თბილისი — დედაქალაქი.")
///     .collect();
/// assert_eq!(toks, ["გამარჯობა", "მსოფლიო", "თბილისი", "დედაქალაქი"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GeorgianTokenizer;

impl GeorgianTokenizer {
    /// Constructs a new [`GeorgianTokenizer`]. Zero-sized; free to call.
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
        GeorgianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_georgian_sentence() {
        assert_eq!(collect("კატა სძინავს."), ["კატა", "სძინავს"]);
    }

    #[test]
    fn georgian_letters_stay_inside_tokens() {
        // Every letter in the modern Mkhedruli inventory stays with its
        // word — Unicode classifies U+10D0..=U+10FA as alphabetic.
        assert_eq!(collect("ქართული"), ["ქართული"]);
        assert_eq!(collect("თბილისი"), ["თბილისი"]);
        assert_eq!(collect("საქართველო"), ["საქართველო"]);
    }

    #[test]
    fn mtavruli_letters_stay_inside_tokens() {
        // Mtavruli (U+1C90..=U+1CBF) is Unicode's capitalized-Mkhedruli
        // block added in Unicode 11; also alphabetic.
        assert_eq!(collect("ᲗᲑᲘᲚᲘᲡᲘ"), ["ᲗᲑᲘᲚᲘᲡᲘ"]);
    }

    #[test]
    fn georgian_paragraph_separator_is_a_separator() {
        // U+10FB (`჻`) is Georgian paragraph separator — punctuation
        // under Unicode's classification.
        assert_eq!(collect("დიახ჻არა"), ["დიახ", "არა"]);
    }

    #[test]
    fn ascii_punctuation_separates_tokens() {
        assert_eq!(collect("კარგი, ცუდი!"), ["კარგი", "ცუდი"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("წელი 2026 თვე"), ["წელი", "2026", "თვე"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "გამარჯობა მსოფლიო";
        let toks: Vec<&str> = GeorgianTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. This is the key safety property of the
        // char_indices-based splitter — no byte offset can slice a
        // multi-byte Georgian scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
