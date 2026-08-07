//! [`VietnameseTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Vietnamese orthography is whitespace-and-punctuation delimited.
//! Unlike Chinese, Japanese, or Thai, Vietnamese writes every syllable
//! as a **whitespace-separated word** — the compound noun `học sinh`
//! ("student") is *two* orthographic words, not one, even though it
//! is a single semantic unit. This is a deliberate historical choice
//! by the chữ Quốc ngữ (national script) reformers who adapted the
//! Latin alphabet to Vietnamese in the 17th century.
//!
//! Every Vietnamese letter, including the seven letter-modifier
//! variants (`ă â đ ê ô ơ ư` and their uppercase forms) and every
//! precomposed vowel-plus-tone-mark scalar (e.g. `ạ ả ã á à ằ ẳ ẵ ắ
//! ầ …`), is alphabetic under Unicode's classification and stays
//! with its surrounding word under
//! [`SimpleTokenizer`]'s `char::is_alphanumeric` splitter. Vietnamese
//! text delivered in NFC form — the web overwhelmingly does — is
//! therefore correctly split by the default tokenizer with no
//! per-character special-casing.
//!
//! **NFD input warning.** Vietnamese text in **NFD form** encodes
//! diacritics as a base letter followed by one or two combining
//! marks (U+0300..=U+036F). Combining marks are classified as
//! `char::is_alphanumeric() == false` — they are `Mark_Nonspacing`
//! (`Mn`), not Letter — so the default tokenizer *does* split on
//! them, producing wrong-looking token boundaries in the middle of a
//! Vietnamese syllable. Callers whose input might be in NFD form
//! should run [`crate::normalize::VietnameseNormalizer`] (defaults
//! to `nfc = true`) *before* tokenizing. The tokenizer does not
//! perform the NFC pass on its behalf because the normalizer is the
//! documented site for that decision, and running NFC twice is
//! wasteful.
//!
//! There is therefore no reason for the Vietnamese pack to ship its
//! own tokenizer implementation — this module exposes
//! [`VietnameseTokenizer`] as a **transparent wrapper** around
//! [`SimpleTokenizer`] so the pack's public surface still names a
//! Vietnamese-specific tokenizer type (a courtesy for callers who
//! match the language-pack pattern) without duplicating any
//! splitting logic.
//!
//! # Non-goals
//!
//! - **Multi-syllable compound joining.** `học sinh` "student",
//!   `máy tính` "computer", `nước ngoài` "foreign country" all
//!   remain two tokens. Joining them into one lemma requires a
//!   Vietnamese lexicon (`VnCoreNLP`, `underthesea`, `PyVI`) that is out
//!   of scope for the offline pack.
//! - **Hyphenated compounds.** Vietnamese occasionally hyphenates for
//!   clarity (`Việt-Nam`, older orthography). The tokenizer treats
//!   the hyphen as a separator per
//!   [`SimpleTokenizer`]; each half becomes a token in its own
//!   right.
//! - **Vietnamese-specific punctuation.** Vietnamese uses standard
//!   Latin punctuation (`,`, `.`, `?`, `!`, `;`, `:`, guillemets
//!   `« »`, curly quotes `“ ”`); every one of these is a Unicode
//!   punctuation scalar and is treated as a separator by
//!   [`SimpleTokenizer`] with no extra rules needed. Standard latin
//!   digit sequences and Vietnamese-locale-formatted numbers (e.g.
//!   `1.000.000`) split on the decimal separator like every other
//!   pack — the tokenizer does not carry a locale-aware
//!   number-parser.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Vietnamese tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Vietnamese does not need a
/// bespoke splitter.
///
/// # Example
///
/// ```
/// use stringcheese_vi::VietnameseTokenizer;
///
/// let toks: Vec<&str> = VietnameseTokenizer::new()
///     .tokenize("Học sinh đọc sách.")
///     .collect();
/// assert_eq!(toks, ["Học", "sinh", "đọc", "sách"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct VietnameseTokenizer;

impl VietnameseTokenizer {
    /// Constructs a new [`VietnameseTokenizer`]. Zero-sized; free to
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
        VietnameseTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_vietnamese_sentence() {
        assert_eq!(
            collect("Học sinh đọc sách."),
            ["Học", "sinh", "đọc", "sách"]
        );
    }

    #[test]
    fn every_letter_modifier_stays_inside_tokens() {
        // Each of the seven Vietnamese letter-modifier characters
        // (ă â đ ê ô ơ ư) is alphabetic under Unicode and belongs
        // with its surrounding word.
        assert_eq!(collect("ăn"), ["ăn"]);
        assert_eq!(collect("cầu"), ["cầu"]);
        assert_eq!(collect("đường"), ["đường"]);
        assert_eq!(collect("bến"), ["bến"]);
        assert_eq!(collect("cột"), ["cột"]);
        assert_eq!(collect("cơm"), ["cơm"]);
        assert_eq!(collect("nước"), ["nước"]);
    }

    #[test]
    fn every_tone_mark_stays_inside_tokens() {
        // Each of the five Vietnamese tone marks (grave, acute,
        // hook-above, tilde, dot-below) on precomposed vowels is
        // alphabetic under Unicode in NFC form.
        assert_eq!(collect("và"), ["và"]);
        assert_eq!(collect("cá"), ["cá"]);
        assert_eq!(collect("hỏi"), ["hỏi"]);
        assert_eq!(collect("mã"), ["mã"]);
        assert_eq!(collect("nặng"), ["nặng"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("năm 2026 tháng"), ["năm", "2026", "tháng"]);
    }

    #[test]
    fn uppercase_diacritics_stay_inside_tokens() {
        // The uppercase Vietnamese diacritic letters are also
        // alphabetic under Unicode.
        assert_eq!(collect("VÀ ĐƯỢC NHỮNG"), ["VÀ", "ĐƯỢC", "NHỮNG"]);
    }

    #[test]
    fn multi_syllable_compounds_split_at_the_space() {
        // Vietnamese multi-syllable compounds are two orthographic
        // words in the standard spelling; the tokenizer follows the
        // orthography.
        assert_eq!(collect("học sinh"), ["học", "sinh"]);
        assert_eq!(collect("máy tính"), ["máy", "tính"]);
        assert_eq!(collect("nước ngoài"), ["nước", "ngoài"]);
    }

    #[test]
    fn hyphenated_compounds_split_at_the_hyphen() {
        // ASCII hyphens are separators — each half becomes a token.
        assert_eq!(collect("Việt-Nam"), ["Việt", "Nam"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "học sinh";
        let toks: Vec<&str> = VietnameseTokenizer::new().tokenize(text).collect();
        assert_eq!(toks[0].as_ptr(), text.as_ptr());
    }
}
