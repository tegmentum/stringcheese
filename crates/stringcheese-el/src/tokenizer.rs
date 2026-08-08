//! [`GreekTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Greek orthography is whitespace-and-punctuation delimited. Every
//! Greek letter in the modern Greek inventory (U+0370..=U+03FF) is
//! alphabetic under Unicode's `char::is_alphanumeric` classification
//! and therefore stays inside tokens naturally with the default
//! splitter. There is no reason for the Greek pack to ship its own
//! tokenizer implementation — this module exposes [`GreekTokenizer`]
//! as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names a Greek-specific tokenizer type (a courtesy for callers
//! who match the language-pack pattern) without duplicating any
//! splitting logic.
//!
//! # Byte-vs-char safety
//!
//! Every Greek scalar in the modern Greek block (U+0370..=U+03FF) is
//! encoded as **two UTF-8 bytes** (this range falls entirely inside
//! U+0080..=U+07FF, UTF-8's 2-byte window). The byte length of a Greek
//! word is roughly `2 * char_count`, and any code that mixes byte
//! offsets with character-boundary logic will silently corrupt token
//! boundaries. [`SimpleTokenizer`] itself uses [`str::char_indices`]
//! internally, so the boundaries it emits are always valid UTF-8 char
//! boundaries and the borrowed token slices are always well-formed
//! `&str` values. This wrapper adds no arithmetic of its own, so the
//! pack does not introduce a new opportunity to get byte / char math
//! wrong.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Greek has rich inflectional
//!   morphology — case, number, gender, verb aspect, and tense all
//!   show up as suffixes on the surface form. Splitting these at the
//!   tokenizer would be wrong for IR indexing; the fused surface word
//!   is the token, and the Greek stemmer handles suffix stripping. See
//!   [`crate::snowball`].
//! - **Greek question-mark handling.** Greek uses `;` (U+003B, the
//!   ASCII semicolon) as its question mark and `·` (U+0387, ANO
//!   TELEIA) as a mid-clause separator. Both are punctuation under
//!   Unicode's classification and split by the default splitter,
//!   matching the practice of every published Greek IR pipeline.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Greek tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Greek does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_el::GreekTokenizer;
///
/// let toks: Vec<&str> = GreekTokenizer::new()
///     .tokenize("Καλημέρα, κόσμε! Αθήνα — πρωτεύουσα.")
///     .collect();
/// assert_eq!(toks, ["Καλημέρα", "κόσμε", "Αθήνα", "πρωτεύουσα"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GreekTokenizer;

impl GreekTokenizer {
    /// Constructs a new [`GreekTokenizer`]. Zero-sized; free to call.
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
        GreekTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_greek_sentence() {
        assert_eq!(collect("Γάτα κοιμάται."), ["Γάτα", "κοιμάται"]);
    }

    #[test]
    fn greek_letters_stay_inside_tokens() {
        // Every letter in the modern Greek alphabet is alphabetic
        // under Unicode's classification and stays with its word.
        assert_eq!(collect("ελληνικά"), ["ελληνικά"]);
        assert_eq!(collect("Θεσσαλονίκη"), ["Θεσσαλονίκη"]);
        assert_eq!(collect("διεύθυνση"), ["διεύθυνση"]);
    }

    #[test]
    fn final_sigma_stays_inside_tokens() {
        // The final-sigma variant `ς` is still a letter and stays
        // with its word.
        assert_eq!(collect("κόσμος"), ["κόσμος"]);
        assert_eq!(collect("πόλης"), ["πόλης"]);
    }

    #[test]
    fn diaeresis_stays_inside_tokens() {
        // `ϊ` and `ϋ` (diaeresis-marked iota and upsilon) mark a
        // vowel as syllabically distinct from a preceding vowel that
        // would otherwise form a diphthong; both stay with their word.
        assert_eq!(collect("προϊόν"), ["προϊόν"]);
        assert_eq!(collect("ταΰγετος"), ["ταΰγετος"]);
    }

    #[test]
    fn greek_question_mark_is_a_separator() {
        // Greek uses `;` (ASCII semicolon) as its question mark; it is
        // classified as punctuation and splits.
        assert_eq!(collect("τι κάνεις;"), ["τι", "κάνεις"]);
    }

    #[test]
    fn ano_teleia_is_a_separator() {
        // `·` (U+0387 GREEK ANO TELEIA) is the mid-clause separator
        // and is classified as punctuation.
        assert_eq!(collect("ναι·όχι"), ["ναι", "όχι"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("έτος 2026 μήνας"), ["έτος", "2026", "μήνας"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "Καλημέρα κόσμε";
        let toks: Vec<&str> = GreekTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic. This is the key safety property of the
        // char_indices-based splitter — no byte offset can slice a
        // multi-byte Greek scalar apart.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
