//! [`DelimiterTokenizer`] — split on a caller-supplied set of characters.
//!
//! A segmenter that emits one [`Segment`] per maximal run of characters
//! that are *not* in a caller-supplied delimiter set. This is the tool
//! for parsing formats whose split rules are not "whitespace" but "any
//! of `,;|`" or similar — CSV rows, delimited-value strings, log-line
//! fields.
//!
//! Runs of consecutive delimiters collapse, leading and trailing
//! delimiters are dropped, and no empty segments are ever yielded.
//! (Callers who want per-delimiter positions — including empty tails —
//! reach for `stringcheese_manip::split::split` instead.)

use alloc::vec::Vec;

use crate::traits::{Segment, Segmenter};

/// Splits input at every run of characters in a caller-supplied set.
///
/// The delimiter set is stored as a small `Vec<char>` because the common
/// case is a handful of ASCII punctuation characters; a linear scan is
/// faster than any hash-based lookup for that size, and it avoids any
/// dependence on a hashing infrastructure the crate could not otherwise
/// support in `no_std`.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::{DelimiterTokenizer, Segmenter};
///
/// let seg = DelimiterTokenizer::new(&[',', ';']);
/// let fields: Vec<_> = seg.segment("a,b;c").map(|s| s.text).collect();
/// assert_eq!(fields, ["a", "b", "c"]);
/// ```
#[derive(Debug, Clone)]
pub struct DelimiterTokenizer {
    /// The delimiter set. Order is not significant to correctness.
    pub chars: Vec<char>,
}

impl DelimiterTokenizer {
    /// Constructs a tokenizer whose delimiters are the characters in
    /// `chars`. Duplicates are tolerated (they cost only a lookup slot).
    #[must_use]
    pub fn new(chars: &[char]) -> Self {
        Self {
            chars: chars.to_vec(),
        }
    }

    /// Returns `true` if `ch` is one of the configured delimiters.
    #[inline]
    fn is_delim(&self, ch: char) -> bool {
        self.chars.contains(&ch)
    }
}

/// Iterator yielded by [`DelimiterTokenizer::segment`].
#[derive(Debug)]
pub struct DelimiterSegments<'a, 'r> {
    tokenizer: &'r DelimiterTokenizer,
    input: &'a str,
    cursor: usize,
}

impl<'a> Iterator for DelimiterSegments<'a, '_> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // Skip leading delimiters.
        while self.cursor < self.input.len() {
            let ch = self.input[self.cursor..].chars().next()?;
            if self.tokenizer.is_delim(ch) {
                self.cursor += ch.len_utf8();
            } else {
                break;
            }
        }

        if self.cursor >= self.input.len() {
            return None;
        }

        let start = self.cursor;
        while self.cursor < self.input.len() {
            let Some(ch) = self.input[self.cursor..].chars().next() else {
                break;
            };
            if self.tokenizer.is_delim(ch) {
                break;
            }
            self.cursor += ch.len_utf8();
        }

        Some(Segment::new(start, &self.input[start..self.cursor]))
    }
}

impl Segmenter for DelimiterTokenizer {
    type Unit<'a>
        = Segment<'a>
    where
        Self: 'a;
    type Iter<'a>
        = DelimiterSegments<'a, 'a>
    where
        Self: 'a;

    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a> {
        DelimiterSegments {
            tokenizer: self,
            input: text,
            cursor: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split_on(chars: &[char], input: &str) -> Vec<alloc::string::String> {
        DelimiterTokenizer::new(chars)
            .segment(input)
            .map(|s| alloc::string::ToString::to_string(s.text))
            .collect()
    }

    #[test]
    fn basic_csv_shape() {
        assert_eq!(split_on(&[','], "a,b,c"), ["a", "b", "c"]);
    }

    #[test]
    fn multiple_delimiters_all_split() {
        assert_eq!(split_on(&[',', ';'], "a,b;c,d"), ["a", "b", "c", "d"]);
    }

    #[test]
    fn consecutive_delimiters_collapse() {
        assert_eq!(split_on(&[','], "a,,b,,,c"), ["a", "b", "c"]);
    }

    #[test]
    fn leading_and_trailing_delimiters_dropped() {
        assert_eq!(split_on(&[','], ",,a,b,,"), ["a", "b"]);
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(split_on(&[','], "").is_empty());
    }

    #[test]
    fn only_delimiters_yields_nothing() {
        assert!(split_on(&[',', ';'], ",;,;").is_empty());
    }

    #[test]
    fn no_delimiter_in_input_yields_whole_input() {
        assert_eq!(split_on(&[','], "hello world"), ["hello world"]);
    }

    #[test]
    fn unicode_delimiters() {
        assert_eq!(split_on(&['·'], "a·b·c"), ["a", "b", "c"]);
    }

    #[test]
    fn multibyte_content_preserved() {
        assert_eq!(split_on(&[','], "café,olé"), ["café", "olé"]);
    }

    #[test]
    fn offsets_align_with_byte_positions() {
        let seg = DelimiterTokenizer::new(&[',']);
        let out: alloc::vec::Vec<_> = seg.segment("a,bb,ccc").collect();
        assert_eq!(out[0], Segment::new(0, "a"));
        assert_eq!(out[1], Segment::new(2, "bb"));
        assert_eq!(out[2], Segment::new(5, "ccc"));
    }

    #[test]
    fn empty_delimiter_set_is_whole_input() {
        assert_eq!(split_on(&[], "hello"), ["hello"]);
    }
}
