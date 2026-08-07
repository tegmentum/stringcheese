//! [`NgramSegmenter`] — character n-gram windows over an input string.
//!
//! Yields one [`Segment`] per length-`n` window of consecutive
//! *characters* (Unicode scalar values, not graphemes). Byte offsets on
//! the yielded segments align with the byte start of the window's first
//! character.
//!
//! This is the "quick way to get n-grams for Jaccard, Dice, or a
//! q-gram inverted index" surface. Callers who need the richer
//! [`stringcheese_compare::ngram`][ng] machinery — configurable padding
//! policies, byte grams, token grams, `GramSet` / `GramMultiSet` /
//! `GramVector` representations — reach for that crate directly; this
//! segmenter is intentionally the thin, zero-padding, zero-config path.
//!
//! [ng]: https://docs.rs/stringcheese-compare/latest/stringcheese_compare/ngram/

use alloc::vec::Vec;

use crate::traits::{Segment, Segmenter};

/// A character n-gram segmenter.
///
/// The `N` parameter is stored at runtime, not as a const generic, because
/// callers routinely need to compute `n` at runtime — from a config
/// value, a language pack's recommendation, or a policy input. This
/// avoids monomorphising the segmenter per arity.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::{NgramSegmenter, Segmenter};
///
/// let seg = NgramSegmenter::new(2).unwrap();
/// let grams: Vec<_> = seg.segment("abcd").map(|s| s.text).collect();
/// assert_eq!(grams, ["ab", "bc", "cd"]);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct NgramSegmenter {
    /// The arity of the n-grams. Guaranteed non-zero.
    n: usize,
}

impl NgramSegmenter {
    /// Constructs an n-gram segmenter of arity `n`. Returns `None` if
    /// `n == 0` (a zero-arity gram is not well-defined).
    #[must_use]
    pub const fn new(n: usize) -> Option<Self> {
        if n == 0 { None } else { Some(Self { n }) }
    }

    /// The arity of the n-grams this segmenter emits.
    #[must_use]
    pub const fn arity(&self) -> usize {
        self.n
    }
}

/// Iterator yielded by [`NgramSegmenter::segment`].
///
/// Pre-computes the byte positions of every character boundary so it can
/// index into the input with `O(1)` per window; the up-front cost is
/// `O(n_chars)` and one small `Vec`. This is the same tradeoff
/// `stringcheese_compare::ngram::CharacterGrams` makes.
pub struct NgramSegments<'a> {
    input: &'a str,
    // Byte offsets of every char boundary, plus one for the end-of-input.
    boundaries: Vec<usize>,
    n: usize,
    cursor: usize,
}

impl<'a> Iterator for NgramSegments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // We can emit a window starting at char position `cursor` iff
        // `cursor + n < boundaries.len()`.
        if self.cursor + self.n >= self.boundaries.len() {
            return None;
        }
        let start = self.boundaries[self.cursor];
        let end = self.boundaries[self.cursor + self.n];
        self.cursor += 1;
        Some(Segment::new(start, &self.input[start..end]))
    }
}

impl Segmenter for NgramSegmenter {
    type Unit<'a>
        = Segment<'a>
    where
        Self: 'a;
    type Iter<'a>
        = NgramSegments<'a>
    where
        Self: 'a;

    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a> {
        // Collect all character boundaries. `char_indices` yields (byte_pos, ch)
        // pairs; we take the byte position of each, then push `text.len()` as
        // the final boundary.
        let mut boundaries: Vec<usize> = text.char_indices().map(|(p, _)| p).collect();
        boundaries.push(text.len());

        NgramSegments {
            input: text,
            boundaries,
            n: self.n,
            cursor: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grams(n: usize, input: &str) -> Vec<alloc::string::String> {
        NgramSegmenter::new(n)
            .unwrap()
            .segment(input)
            .map(|s| alloc::string::ToString::to_string(s.text))
            .collect()
    }

    #[test]
    fn bigrams_basic() {
        assert_eq!(grams(2, "abcd"), ["ab", "bc", "cd"]);
    }

    #[test]
    fn trigrams_basic() {
        assert_eq!(grams(3, "abcde"), ["abc", "bcd", "cde"]);
    }

    #[test]
    fn unigrams() {
        assert_eq!(grams(1, "abc"), ["a", "b", "c"]);
    }

    #[test]
    fn n_equal_input_length_yields_one() {
        assert_eq!(grams(3, "abc"), ["abc"]);
    }

    #[test]
    fn n_greater_than_input_yields_nothing() {
        assert!(grams(5, "abc").is_empty());
    }

    #[test]
    fn empty_input_yields_nothing() {
        assert!(grams(2, "").is_empty());
        assert!(grams(1, "").is_empty());
    }

    #[test]
    fn zero_arity_rejected() {
        assert!(NgramSegmenter::new(0).is_none());
    }

    #[test]
    fn multibyte_content_grams_by_char() {
        // "héllo" has 5 chars, so 2-grams: hé, él, ll, lo (4 grams).
        assert_eq!(grams(2, "héllo"), ["hé", "él", "ll", "lo"]);
    }

    #[test]
    fn offsets_are_byte_indices() {
        let seg = NgramSegmenter::new(2).unwrap();
        let s = "héllo";
        let out: Vec<_> = seg.segment(s).collect();
        assert_eq!(out.len(), 4);
        // First gram "hé" starts at byte 0 (h is 1 byte, é is 2 bytes),
        // spans bytes 0..3.
        assert_eq!(out[0], Segment::new(0, "hé"));
        // Second gram "él" starts at byte 1.
        assert_eq!(out[1].offset, 1);
        assert_eq!(out[1].text, "él");
    }

    #[test]
    fn arity_accessor() {
        assert_eq!(NgramSegmenter::new(3).unwrap().arity(), 3);
    }
}
