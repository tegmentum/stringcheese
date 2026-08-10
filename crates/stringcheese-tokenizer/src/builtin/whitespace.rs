//! [`WhitespaceTokenizer`] — split on Unicode `White_Space`.
//!
//! A segmenter that emits one [`Segment`] per maximal run of
//! non-whitespace characters, using Rust's built-in
//! [`char::is_whitespace`] predicate (which implements the Unicode
//! `White_Space` property). Runs of consecutive whitespace collapse,
//! leading and trailing whitespace are dropped, and no empty segments
//! are ever yielded.
//!
//! This is *not* a UAX #29 word segmenter: `"hello, world"` is two
//! segments (`"hello,"` and `"world"`), because the comma is not a
//! whitespace scalar. Callers who need UAX #29 semantics reach for
//! [`crate::WordSegmenter`], which wraps `stringcheese_unicode::words`
//! and drops the comma as a non-word boundary — so `"hello, world"`
//! yields the two word-only segments `"hello"` and `"world"`.

use crate::traits::{Segment, Segmenter};

/// Splits input at every run of Unicode `White_Space` scalars.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::{Segmenter, WhitespaceTokenizer};
///
/// let seg = WhitespaceTokenizer;
/// let words: Vec<_> = seg.segment("hello  world").map(|s| s.text).collect();
/// assert_eq!(words, ["hello", "world"]);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct WhitespaceTokenizer;

impl WhitespaceTokenizer {
    /// Constructs a new whitespace tokenizer. The type is zero-sized;
    /// this constructor exists so callers who prefer explicit
    /// construction over `WhitespaceTokenizer` have a symmetric option
    /// with the other segmenters.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Iterator yielded by [`WhitespaceTokenizer::segment`].
///
/// Walks the input byte-by-byte (well, char-by-char) looking for
/// `White_Space` transitions. Zero allocation, single-pass, `O(n)`.
#[derive(Debug)]
pub struct WhitespaceSegments<'a> {
    input: &'a str,
    cursor: usize,
}

impl<'a> Iterator for WhitespaceSegments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.input.as_bytes();

        // Skip leading whitespace.
        while self.cursor < self.input.len() {
            let ch = self.input[self.cursor..].chars().next()?;
            if ch.is_whitespace() {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }

        if self.cursor >= bytes.len() {
            return None;
        }

        let start = self.cursor;
        while self.cursor < self.input.len() {
            let Some(ch) = self.input[self.cursor..].chars().next() else {
                break;
            };
            if ch.is_whitespace() {
                break;
            }
            self.cursor += ch.len_utf8();
        }

        Some(Segment::new(start, &self.input[start..self.cursor]))
    }
}

impl Segmenter for WhitespaceTokenizer {
    type Unit<'a> = Segment<'a>;
    type Iter<'a> = WhitespaceSegments<'a>;

    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a> {
        WhitespaceSegments {
            input: text,
            cursor: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;

    fn texts(input: &str) -> Vec<String> {
        let seg = WhitespaceTokenizer::new();
        seg.segment(input).map(|s| s.text.to_string()).collect()
    }

    #[test]
    fn simple_ascii_words() {
        assert_eq!(texts("hello world"), ["hello", "world"]);
    }

    #[test]
    fn multiple_consecutive_delimiters_collapse() {
        assert_eq!(texts("a    b\t\tc"), ["a", "b", "c"]);
    }

    #[test]
    fn leading_and_trailing_whitespace_dropped() {
        assert_eq!(texts("   hello   world   "), ["hello", "world"]);
    }

    #[test]
    fn empty_input_yields_nothing() {
        let seg = WhitespaceTokenizer::new();
        let v: Vec<_> = seg.segment("").collect();
        assert!(v.is_empty());
    }

    #[test]
    fn only_whitespace_yields_nothing() {
        let seg = WhitespaceTokenizer::new();
        let v: Vec<_> = seg.segment("   \t\n").collect();
        assert!(v.is_empty());
    }

    #[test]
    fn unicode_whitespace_is_recognised() {
        // U+00A0 NO-BREAK SPACE, U+2028 LINE SEPARATOR
        assert_eq!(texts("a\u{00A0}b\u{2028}c"), ["a", "b", "c"]);
    }

    #[test]
    fn offsets_point_into_input() {
        let seg = WhitespaceTokenizer::new();
        let s = "hello world";
        let segs: Vec<_> = seg.segment(s).collect();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].offset, 0);
        assert_eq!(segs[0].text, "hello");
        assert_eq!(segs[1].offset, 6);
        assert_eq!(segs[1].text, "world");
        assert_eq!(&s[segs[1].range()], "world");
    }

    #[test]
    fn multibyte_scalar_offsets_are_byte_indices() {
        let seg = WhitespaceTokenizer::new();
        let s = "héllo wörld";
        let segs: Vec<_> = seg.segment(s).collect();
        assert_eq!(segs.len(), 2);
        // "héllo" occupies bytes 0..6 (h + é (2) + l + l + o = 6)
        assert_eq!(segs[0].offset, 0);
        assert_eq!(&s[segs[0].range()], "héllo");
        assert_eq!(&s[segs[1].range()], "wörld");
    }

    #[test]
    fn single_word_no_whitespace() {
        assert_eq!(texts("hello"), ["hello"]);
    }

    #[test]
    fn punctuation_stays_attached() {
        // Whitespace only splits on whitespace, not on punctuation.
        assert_eq!(texts("don't stop!"), ["don't", "stop!"]);
    }
}
