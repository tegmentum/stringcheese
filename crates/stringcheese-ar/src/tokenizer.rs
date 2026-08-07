//! [`ArabicTokenizer`] — a thin wrapper over
//! [`SimpleTokenizer`] for Arabic
//! text.
//!
//! # Why the default tokenizer is enough
//!
//! Unlike the Japanese / Chinese case, Arabic *does* mark word
//! boundaries with whitespace. Modern Standard Arabic (as encoded in
//! newswire, web text, and every open Arabic corpus) uses ASCII space
//! U+0020 between orthographic words the same way English or French
//! does — the only difference is that the words themselves happen to
//! be written right-to-left. The default
//! [`SimpleTokenizer`] walks on
//! [`char::is_alphanumeric`], and every Arabic letter (U+0621..=U+064A
//! and beyond) satisfies that predicate. So the default splitter
//! handles Arabic word segmentation correctly out of the box.
//!
//! This module exists mostly to give callers an
//! [`ArabicTokenizer`] named type that mirrors the shape of
//! [`FrenchTokenizer`](../../stringcheese-fr/tokenizer/struct.FrenchTokenizer.html)
//! and [`JapaneseTokenizer`](../../stringcheese-ja/tokenizer/struct.JapaneseTokenizer.html)
//! — one named tokenizer per language pack — and to house the
//! discussion of when a caller might want something smarter.
//!
//! # When the default is *not* enough
//!
//! - **Diacritics carried through.** Fully-vocalized text keeps the
//!   harakat inside tokens (which is correct — they *are* alphanumeric
//!   under Unicode). If a downstream index wants the unvoweled surface
//!   form, run [`crate::normalize::normalize`] on the input *before*
//!   tokenizing. (Running it after tokenizing is also fine — it
//!   preserves the token count — but pre-normalizing lets the
//!   tokenizer avoid ever seeing the marks.)
//! - **Attached-particle segmentation.** Arabic writes several one-
//!   letter conjunctions and prepositions (`و` "and-", `ف` "so-",
//!   `ب` "with-", `ك` "like-", `ل` "for-", the definite `ال` "the-")
//!   as prefixes fused to the following word: `والكتاب` "and-the-book"
//!   is one orthographic word but three morphemes. The
//!   [`crate::stemmer::Light10`] stemmer strips these prefixes off
//!   surface forms; this tokenizer does not split them into separate
//!   tokens. If you need per-morpheme tokens, apply Light10 to each
//!   emitted token or wire in a dedicated segmenter (Farasa,
//!   MADAMIRA).
//! - **Tatweel (`ـ`, U+0640).** The purely-orthographic letter
//!   stretcher is `is_alphanumeric` under Unicode, so it stays
//!   attached to its word. Strip it before tokenizing if you don't
//!   want it in tokens.
//! - **Arabic punctuation.** The Arabic comma (`،` U+060C), Arabic
//!   semicolon (`؛` U+061B), and Arabic question mark (`؟` U+061F) are
//!   all Unicode-punctuation and are correctly treated as separators.
//!
//! # RTL note
//!
//! The tokenizer operates on **logical** (UTF-8 byte) order. Because
//! spaces are byte-neutral (U+0020 is one byte regardless of
//! surrounding script), the split points are the same in logical and
//! display order: what you see between two visible Arabic words is
//! the same space the tokenizer splits on. Emitted tokens are
//! borrowed slices of the input and preserve the input's byte order,
//! which is first-consonant-first (the RTL renderer displays them
//! right-to-left; the tokenizer neither knows nor cares).

use stringcheese_lang::SimpleTokenizer;
use stringcheese_lang::tokenizer::Tokens;

/// The Arabic word tokenizer.
///
/// A thin wrapper over
/// [`SimpleTokenizer`] with an
/// Arabic-specific docstring. See the [module-level docs](self) for
/// the discussion of when a caller might want something smarter.
///
/// A zero-sized value; construct as [`ArabicTokenizer`] and reuse
/// across threads and calls.
///
/// # Example
///
/// ```
/// use stringcheese_ar::ArabicTokenizer;
///
/// let toks: Vec<&str> = ArabicTokenizer::new()
///     .tokenize("محمد يحب القراءة، وهو طالب.")
///     .collect();
/// assert_eq!(
///     toks,
///     ["محمد", "يحب", "القراءة", "وهو", "طالب"]
/// );
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArabicTokenizer;

impl ArabicTokenizer {
    /// Constructs a new [`ArabicTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into words on whitespace and punctuation.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> Tokens<'a> {
        SimpleTokenizer::new().tokenize(text)
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        ArabicTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn whitespace_only_yields_no_tokens() {
        assert!(collect("   ").is_empty());
    }

    #[test]
    fn splits_arabic_sentence_on_ascii_space() {
        assert_eq!(collect("محمد يحب القراءة"), ["محمد", "يحب", "القراءة"]);
    }

    #[test]
    fn splits_on_arabic_comma_and_period() {
        // U+060C ARABIC COMMA and U+002E FULL STOP.
        assert_eq!(
            collect("محمد يحب القراءة، وهو طالب."),
            ["محمد", "يحب", "القراءة", "وهو", "طالب"]
        );
    }

    #[test]
    fn splits_on_arabic_question_mark() {
        // U+061F ARABIC QUESTION MARK.
        assert_eq!(collect("كيف حالك؟"), ["كيف", "حالك"]);
    }

    #[test]
    fn attached_particles_stay_with_their_word() {
        // والكتاب is one orthographic word ("and-the-book") and stays
        // as one token; morpheme splitting is the stemmer's job.
        assert_eq!(collect("والكتاب جيد"), ["والكتاب", "جيد"]);
    }

    #[test]
    fn tokens_borrow_from_input() {
        let text = "محمد وأحمد";
        let toks: Vec<&str> = ArabicTokenizer::new().tokenize(text).collect();
        // Every token slice is borrowed from `text`; verify by pointer
        // arithmetic.
        let base = text.as_ptr() as usize;
        for t in &toks {
            let offset = t.as_ptr() as usize - base;
            assert!(offset < text.len(), "token pointer outside input");
        }
    }
}
