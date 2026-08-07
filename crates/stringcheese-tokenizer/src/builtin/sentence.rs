//! [`SentenceSegmenter`] — a thin wrapper over
//! `stringcheese_unicode::sentences`.
//!
//! A segmenter that yields one [`Segment`] per UAX #29 sentence.
//! Boundaries are inferred from the `Sentence_Break` property —
//! mostly `.`, `!`, and `?` followed by whitespace, with carve-outs
//! for numeric decimals (`"3.14"` is not a sentence break) and
//! terminal punctuation followed by closing brackets or quotes.
//!
//! The heavy lifting is done by
//! [`stringcheese_unicode::sentence_indices`], which delegates in turn
//! to the well-tested `unicode-segmentation` crate; this wrapper only
//! re-shapes the iterator into the [`Segmenter`] surface.
//!
//! [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/

use alloc::boxed::Box;

use crate::traits::{Segment, Segmenter};

/// Segments input into UAX #29 sentences.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::{Segmenter, SentenceSegmenter};
///
/// let seg = SentenceSegmenter;
/// let ss: Vec<_> = seg.segment("Hello. World.").map(|s| s.text).collect();
/// assert_eq!(ss.len(), 2);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct SentenceSegmenter;

impl SentenceSegmenter {
    /// Constructs a new sentence segmenter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Iterator returned by [`SentenceSegmenter::segment`].
///
/// Wraps [`stringcheese_unicode::sentence_indices`] so each yielded
/// [`Segment`] carries the byte offset the boundary iterator itself
/// computed.
pub struct SentenceSegments<'a> {
    inner: Box<dyn Iterator<Item = (usize, &'a str)> + 'a>,
}

impl<'a> Iterator for SentenceSegments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, text) = self.inner.next()?;
        Some(Segment::new(offset, text))
    }
}

impl Segmenter for SentenceSegmenter {
    type Unit<'a>
        = Segment<'a>
    where
        Self: 'a;
    type Iter<'a>
        = SentenceSegments<'a>
    where
        Self: 'a;

    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a> {
        let inner: Box<dyn Iterator<Item = (usize, &'a str)> + 'a> =
            Box::new(stringcheese_unicode::sentence_indices(text));
        SentenceSegments { inner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn two_sentences() {
        let seg = SentenceSegmenter::new();
        let ss: Vec<_> = seg.segment("Hello. World.").map(|s| s.text).collect();
        assert_eq!(ss.len(), 2);
    }

    #[test]
    fn question_and_exclamation_break() {
        let seg = SentenceSegmenter::new();
        let ss: Vec<_> = seg.segment("Yes! No? Maybe.").map(|s| s.text).collect();
        assert_eq!(ss.len(), 3);
    }

    #[test]
    fn numeric_decimal_is_not_a_break() {
        let seg = SentenceSegmenter::new();
        let ss: Vec<_> = seg.segment("Pi is 3.14 today.").map(|s| s.text).collect();
        assert_eq!(ss.len(), 1);
    }

    #[test]
    fn empty_input_yields_nothing() {
        let seg = SentenceSegmenter::new();
        let v: Vec<_> = seg.segment("").collect();
        assert!(v.is_empty());
    }

    #[test]
    fn offsets_locate_input_slices() {
        let seg = SentenceSegmenter::new();
        let s = "Hi. Bye.";
        let out: Vec<_> = seg.segment(s).collect();
        assert_eq!(out.len(), 2);
        assert_eq!(&s[out[0].range()], out[0].text);
        assert_eq!(&s[out[1].range()], out[1].text);
        assert_eq!(out[0].offset, 0);
        assert!(out[1].offset > out[0].offset);
    }

    #[test]
    fn multibyte_scalar_offsets_are_byte_indices() {
        let seg = SentenceSegmenter::new();
        let s = "Café. Bière.";
        let out: Vec<_> = seg.segment(s).collect();
        assert_eq!(out.len(), 2);
        assert_eq!(&s[out[0].range()], out[0].text);
        assert_eq!(&s[out[1].range()], out[1].text);
    }
}
