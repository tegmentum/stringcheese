//! [`TurkishTokenizer`] — a thin wrapper around [`SimpleTokenizer`].
//!
//! Turkish orthography is whitespace-and-punctuation delimited. It
//! uses ASCII letters plus six letters outside the ASCII range —
//! `ç` (U+00E7), `ğ` (U+011F), `ı` (U+0131), `İ` (U+0130), `ö`
//! (U+00F6), `ş` (U+015F), `ü` (U+00FC). All of these are alphabetic
//! under `char::is_alphanumeric` and therefore stay inside tokens
//! naturally with the default splitter — the tokenizer has no special
//! knowledge of Turkish orthography beyond what Unicode's own letter
//! classification provides.
//!
//! There is therefore no reason for the Turkish pack to ship its own
//! tokenizer implementation — this module exposes [`TurkishTokenizer`]
//! as a **transparent wrapper** around
//! [`stringcheese_lang::SimpleTokenizer`] so the pack's public surface
//! still names a Turkish-specific tokenizer type (a courtesy for
//! callers who match the language-pack pattern) without duplicating
//! any splitting logic.
//!
//! # Non-goals
//!
//! - **Morphological segmentation.** Turkish is agglutinative — a
//!   single orthographic word can carry a dozen morphemes. Splitting
//!   these at the tokenizer would be wrong for IR indexing; the fused
//!   surface word is the token, and the Snowball stemmer handles
//!   suffix stripping. See [`crate::snowball`].
//! - **Enclitic particles.** Turkish writes the copular question
//!   particle `mi` / `mı` / `mu` / `mü` as a separate word by
//!   convention (`Ne yapıyorsun?` — `mı` as an interrogative is a
//!   distinct orthographic word). No special glue-back logic here.
//! - **Case folding.** Turkish has a special case-fold mapping for the
//!   dotted / dotless `İ` pair (see [`crate::case_fold`]). The
//!   tokenizer preserves the surface case; case folding is a separate
//!   pipeline concern.

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Turkish tokenizer.
///
/// A zero-sized value; a transparent wrapper around
/// [`stringcheese_lang::SimpleTokenizer`]. See the
/// [module-level docs](self) for why Turkish does not need a bespoke
/// splitter.
///
/// # Example
///
/// ```
/// use stringcheese_tr::TurkishTokenizer;
///
/// let toks: Vec<&str> = TurkishTokenizer::new()
///     .tokenize("Merhaba, dünya! İstanbul çok güzel.")
///     .collect();
/// assert_eq!(toks, ["Merhaba", "dünya", "İstanbul", "çok", "güzel"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TurkishTokenizer;

impl TurkishTokenizer {
    /// Constructs a new [`TurkishTokenizer`]. Zero-sized; free to call.
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
        TurkishTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn splits_a_turkish_sentence() {
        assert_eq!(collect("Kedi uyuyor."), ["Kedi", "uyuyor"]);
    }

    #[test]
    fn turkish_special_letters_stay_inside_tokens() {
        // ç, ğ, ı, İ, ö, ş, ü are all alphabetic under Unicode.
        assert_eq!(collect("çocuk"), ["çocuk"]);
        assert_eq!(collect("ağaç"), ["ağaç"]);
        assert_eq!(collect("ışık"), ["ışık"]);
        assert_eq!(collect("İstanbul"), ["İstanbul"]);
        assert_eq!(collect("Öğrenci"), ["Öğrenci"]);
        assert_eq!(collect("şeker"), ["şeker"]);
        assert_eq!(collect("üçüncü"), ["üçüncü"]);
    }

    #[test]
    fn dotless_and_dotted_i_are_preserved() {
        // The tokenizer does not case-fold — `ı` and `i` stay distinct
        // (as do `I` and `İ`).
        assert_eq!(collect("ışık İtalya"), ["ışık", "İtalya"]);
    }

    #[test]
    fn interrogative_particle_stays_separate_by_orthography() {
        // Turkish writes `mı`, `mi`, `mu`, `mü` (question particles)
        // as separate orthographic words.
        assert_eq!(collect("Geldi mi?"), ["Geldi", "mi"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("yıl 2026 ay"), ["yıl", "2026", "ay"]);
    }
}
