//! [`SimpleTokenizer`] — a whitespace-and-punctuation word splitter.
//!
//! A token, under [`SimpleTokenizer`], is a maximal contiguous run of
//! characters for which [`char::is_alphanumeric`] returns `true`. Every
//! other character (whitespace, ASCII punctuation, Unicode punctuation,
//! symbols, control characters, …) is a *separator* that ends the
//! current token and never appears in any output slice.
//!
//! # Why alphanumeric
//!
//! This matches the default choice made by scikit-learn's default
//! word tokenizer and NLTK's `WhitespaceTokenizer` variant, and it
//! gives sensible behaviour on a wide range of scripts (Latin, Cyrillic,
//! Greek, CJK ideographs — all classified as alphabetic by Unicode).
//! Scripts that rely on combining marks (Devanagari virama, Arabic /
//! Hebrew vowel points) will be *over-split* by this baseline — the
//! [`char::is_alphanumeric`] classifier treats combining marks as
//! separators — so language packs targeting those scripts must
//! implement their own tokenizer. Decimals stay attached to their
//! digits (`"3.14"` yields
//! `["3", "14"]` — the tokenizer does not preserve punctuation as part
//! of a numeric token).
//!
//! Language packs whose tokenization needs are more sophisticated
//! (English contractions like `"don't"` → `["do", "n't"]`, German
//! compound splitting, Japanese boundary detection, …) should
//! implement their own tokenizer and override
//! [`Language::tokenize`](crate::Language::tokenize).
//!
//! # Zero allocation
//!
//! [`SimpleTokenizer::tokenize`] returns an iterator over borrowed
//! `&str` slices of the input string; no allocation is performed. The
//! tokenizer is a zero-sized unit-like value, safe to construct on
//! every call.

/// A whitespace-and-punctuation word tokenizer.
///
/// See the [module-level docs](self) for the tokenization rule and its
/// rationale.
///
/// # Example
///
/// ```
/// use stringcheese_lang::SimpleTokenizer;
///
/// let toks: Vec<&str> = SimpleTokenizer::new()
///     .tokenize("Hello, world! It's 2026.")
///     .collect();
/// assert_eq!(toks, ["Hello", "world", "It", "s", "2026"]);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SimpleTokenizer;

impl SimpleTokenizer {
    /// Constructs a new [`SimpleTokenizer`]. Zero-sized; free to call.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Splits `text` into whitespace-and-punctuation-delimited tokens.
    ///
    /// The returned iterator yields borrowed `&'a str` slices of the
    /// input; no allocation is performed.
    #[inline]
    pub fn tokenize<'a>(&self, text: &'a str) -> Tokens<'a> {
        Tokens { text, offset: 0 }
    }
}

/// Iterator over the tokens of a string, produced by
/// [`SimpleTokenizer::tokenize`].
///
/// The iterator borrows the input string and yields borrowed slices of
/// it — no allocation is performed. Held state is a single `usize`
/// byte offset.
#[derive(Clone, Debug)]
pub struct Tokens<'a> {
    text: &'a str,
    offset: usize,
}

impl<'a> Iterator for Tokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.text.as_bytes();

        // Skip any leading separators.
        while self.offset < bytes.len() {
            // Peek at the next scalar starting at `self.offset`. Because
            // `self.offset` is always on a UTF-8 boundary (we only ever
            // advance by whole scalars), `self.text[self.offset..]` is
            // safe and its first char is the scalar we want.
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if ch.is_alphanumeric() {
                break;
            }
            self.offset += ch.len_utf8();
        }

        if self.offset >= bytes.len() {
            return None;
        }

        // Consume the run of alphanumeric scalars.
        let start = self.offset;
        while self.offset < bytes.len() {
            let rest = &self.text[self.offset..];
            let ch = rest.chars().next()?;
            if !ch.is_alphanumeric() {
                break;
            }
            self.offset += ch.len_utf8();
        }

        Some(&self.text[start..self.offset])
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::vec::Vec;

    fn collect(input: &str) -> Vec<&str> {
        SimpleTokenizer::new().tokenize(input).collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(collect("").is_empty());
    }

    #[test]
    fn whitespace_only_yields_no_tokens() {
        assert!(collect("   ").is_empty());
        assert!(collect("\t\n\r ").is_empty());
    }

    #[test]
    fn punctuation_only_yields_no_tokens() {
        assert!(collect("!!!").is_empty());
        assert!(collect("---,,,;;;").is_empty());
    }

    #[test]
    fn splits_on_whitespace() {
        assert_eq!(collect("hello world"), ["hello", "world"]);
        assert_eq!(collect("a  b   c"), ["a", "b", "c"]);
    }

    #[test]
    fn splits_on_punctuation() {
        assert_eq!(collect("hello,world!"), ["hello", "world"]);
        assert_eq!(collect("a.b.c"), ["a", "b", "c"]);
    }

    #[test]
    fn leading_and_trailing_separators_stripped() {
        assert_eq!(collect("  hello  "), ["hello"]);
        assert_eq!(collect("!!hello!!"), ["hello"]);
    }

    #[test]
    fn digits_are_tokens() {
        assert_eq!(collect("hello 2026 world"), ["hello", "2026", "world"]);
        // Decimal point is a separator — the tokenizer doesn't grow a
        // number-parsing brain here.
        assert_eq!(collect("3.14"), ["3", "14"]);
    }

    #[test]
    fn unicode_alphabetic_is_a_token_character() {
        // Cyrillic and Greek both classify as alphabetic and belong
        // inside tokens. Scripts with combining marks (Devanagari,
        // Arabic, Hebrew) are intentionally out of scope for this
        // baseline tokenizer — the `is_alphanumeric` classifier splits
        // on combining marks like the Devanagari virama, so a language
        // pack targeting those scripts must ship its own tokenizer.
        assert_eq!(collect("Привет мир"), ["Привет", "мир"]);
        assert_eq!(collect("γειά σου"), ["γειά", "σου"]);
    }

    #[test]
    fn unicode_punctuation_is_a_separator() {
        // U+2014 EM DASH, U+201C/U+201D typographic quotes.
        assert_eq!(collect("hello\u{2014}world"), ["hello", "world"]);
        assert_eq!(
            collect("\u{201C}hello\u{201D} \u{201C}world\u{201D}"),
            ["hello", "world"]
        );
    }

    #[test]
    fn borrowed_slices_are_from_the_input() {
        let text = "hello world";
        let toks: Vec<&str> = SimpleTokenizer::new().tokenize(text).collect();
        assert_eq!(toks[0].as_ptr(), text.as_ptr());
    }
}
