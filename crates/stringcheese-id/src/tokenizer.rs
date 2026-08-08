//! [`IndonesianTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Indonesian orthography is whitespace-and-punctuation delimited and
//! uses the modern 26-letter Latin alphabet with **no diacritics**.
//! Every scalar in an Indonesian word is ASCII-alphabetic, so the
//! default splitter needs no Indonesian-specific configuration.
//!
//! This module exposes [`IndonesianTokenizer`] as a **transparent
//! wrapper** around [`stringcheese_lang::SimpleTokenizer`] so the
//! pack's public surface still names an Indonesian-specific tokenizer
//! type (matching the pattern every other `stringcheese-<lang>` pack
//! follows).
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Indonesian is agglutinative in
//!   its affixation (`me-N-VERB-kan`, `pe-N-VERB-an`, `ke-VERB-an`,
//!   etc.), so a single orthographic word can carry a prefix and one
//!   or two suffixes. Splitting affixes off at the tokenizer would be
//!   wrong for IR indexing — the fused surface word is the token, and
//!   the Nazief-Adriani stemmer handles affix stripping. See
//!   [`crate::stemmer`].
//! - **Reduplication.** Indonesian plural marking is often expressed
//!   by full reduplication with a hyphen (`buku-buku` "books",
//!   `anak-anak` "children"). The tokenizer treats the hyphen as a
//!   token boundary and yields the two syllable halves as separate
//!   tokens; joining them back into a canonical plural is a
//!   post-processing step downstream applications can choose to add.
//!   This is consistent with how most Indonesian IR pipelines handle
//!   reduplication (the halves stem to the same base form anyway).
//! - **Compound splitting.** Indonesian compounds like `rumah sakit`
//!   ("hospital", literally "sick house") are written as two
//!   orthographic words separated by a space; the tokenizer emits
//!   them as two tokens. Joining semantically-related compounds needs
//!   a lexicon and is out of scope.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Indonesian tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Indonesian does not need a
/// bespoke splitter.
///
/// # Example
///
/// ```
/// use stringcheese_id::IndonesianTokenizer;
///
/// let toks: Vec<&str> = IndonesianTokenizer::new()
///     .tokenize("Saya membaca buku di rumah.")
///     .collect();
/// assert_eq!(toks, ["Saya", "membaca", "buku", "di", "rumah"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IndonesianTokenizer;

impl IndonesianTokenizer {
    /// Constructs a new [`IndonesianTokenizer`]. Zero-sized; free to
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
        IndonesianTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_simple_indonesian_sentence() {
        assert_eq!(collect("Saya suka kopi."), ["Saya", "suka", "kopi"]);
    }

    #[test]
    fn reduplication_hyphen_splits_the_halves() {
        // Indonesian plural reduplication is written with a hyphen;
        // the tokenizer treats it as a separator.
        assert_eq!(collect("buku-buku"), ["buku", "buku"]);
        assert_eq!(collect("anak-anak"), ["anak", "anak"]);
    }

    #[test]
    fn compound_forms_split_on_space() {
        // `rumah sakit` is a two-word compound meaning "hospital"; the
        // tokenizer emits the two orthographic words separately.
        assert_eq!(collect("rumah sakit"), ["rumah", "sakit"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("tahun 2026 lalu"), ["tahun", "2026", "lalu"]);
    }

    #[test]
    fn punctuation_is_dropped() {
        assert_eq!(
            collect("Halo, dunia! Selamat datang."),
            ["Halo", "dunia", "Selamat", "datang"]
        );
    }
}
