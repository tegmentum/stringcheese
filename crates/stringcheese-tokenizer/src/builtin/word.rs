//! [`WordSegmenter`] — a thin wrapper over
//! `stringcheese_unicode::words` / `word_bounds`.
//!
//! A segmenter that yields one [`Segment`] per UAX #29 word. In the
//! default "words only" mode the segmenter matches
//! [`stringcheese_unicode::words()`] — apostrophes join adjacent
//! letters (`"don't"` is one word), numeric decimals stay intact
//! (`"3.14"` is one word), and whitespace / punctuation is dropped.
//! In "all boundaries" mode it matches
//! [`stringcheese_unicode::word_bounds`] and yields *every*
//! boundary-delimited piece so the caller can round-trip the input.
//!
//! The heavy lifting is done by `stringcheese_unicode`, which
//! delegates in turn to the well-tested `unicode-segmentation` crate;
//! this wrapper only re-shapes the iterator into the [`Segmenter`]
//! surface. The offset comes from the underlying
//! `word_indices` / `word_bound_indices` iterator — no pointer
//! arithmetic.
//!
//! [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/

use alloc::boxed::Box;

use stringcheese_unicode::SplitWordBoundsBehavior;

use crate::traits::{Segment, Segmenter};

/// Segments input into UAX #29 words.
///
/// Instantiate with [`WordSegmenter::new`] for the default
/// `WordsOnly` behavior (whitespace and punctuation dropped) or with
/// [`WordSegmenter::with_behavior`] for
/// [`SplitWordBoundsBehavior::AllBoundaries`], which preserves every
/// boundary-delimited piece.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::{Segmenter, WordSegmenter};
///
/// let seg = WordSegmenter::new();
/// let ws: Vec<_> = seg.segment("don't stop!").map(|s| s.text).collect();
/// assert_eq!(ws, ["don't", "stop"]);
/// ```
///
/// ```
/// use stringcheese_tokenizer::{Segmenter, WordSegmenter};
/// use stringcheese_unicode::SplitWordBoundsBehavior;
///
/// let seg = WordSegmenter::with_behavior(SplitWordBoundsBehavior::AllBoundaries);
/// let ps: Vec<_> = seg.segment("hi, you").map(|s| s.text).collect();
/// assert_eq!(ps, ["hi", ",", " ", "you"]);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct WordSegmenter {
    behavior: SplitWordBoundsBehavior,
}

impl WordSegmenter {
    /// Constructs a segmenter using the default `WordsOnly` behavior.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            behavior: SplitWordBoundsBehavior::WordsOnly,
        }
    }

    /// Constructs a segmenter with the requested boundary behavior.
    #[must_use]
    pub const fn with_behavior(behavior: SplitWordBoundsBehavior) -> Self {
        Self { behavior }
    }

    /// The behavior this segmenter applies.
    #[must_use]
    pub const fn behavior(&self) -> SplitWordBoundsBehavior {
        self.behavior
    }
}

/// Iterator returned by [`WordSegmenter::segment`].
///
/// Wraps the underlying `stringcheese_unicode::word_indices` /
/// `word_bound_indices` iterator so each yielded [`Segment`] carries
/// the byte offset the boundary iterator itself computed.
pub struct WordSegments<'a> {
    inner: Box<dyn Iterator<Item = (usize, &'a str)> + 'a>,
}

impl<'a> Iterator for WordSegments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let (offset, text) = self.inner.next()?;
        Some(Segment::new(offset, text))
    }
}

impl Segmenter for WordSegmenter {
    type Unit<'a>
        = Segment<'a>
    where
        Self: 'a;
    type Iter<'a>
        = WordSegments<'a>
    where
        Self: 'a;

    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a> {
        let inner: Box<dyn Iterator<Item = (usize, &'a str)> + 'a> = match self.behavior {
            SplitWordBoundsBehavior::WordsOnly => {
                Box::new(stringcheese_unicode::word_indices(text))
            }
            SplitWordBoundsBehavior::AllBoundaries => {
                Box::new(stringcheese_unicode::word_bound_indices(text))
            }
        };
        WordSegments { inner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn ascii_words() {
        let seg = WordSegmenter::new();
        let ws: Vec<_> = seg.segment("hello world").map(|s| s.text).collect();
        assert_eq!(ws, ["hello", "world"]);
    }

    #[test]
    fn apostrophe_joins_adjacent_letters() {
        let seg = WordSegmenter::new();
        let ws: Vec<_> = seg.segment("don't stop!").map(|s| s.text).collect();
        assert_eq!(ws, ["don't", "stop"]);
    }

    #[test]
    fn all_boundaries_preserves_input() {
        let seg = WordSegmenter::with_behavior(SplitWordBoundsBehavior::AllBoundaries);
        let s = "hi, you";
        let joined: alloc::string::String = seg.segment(s).map(|seg| seg.text).collect();
        assert_eq!(joined, s);
    }

    #[test]
    fn empty_input_yields_nothing() {
        let seg = WordSegmenter::new();
        let v: Vec<_> = seg.segment("").collect();
        assert!(v.is_empty());
    }

    #[test]
    fn words_only_offsets_locate_input_slices() {
        let seg = WordSegmenter::new();
        let s = "hello, world";
        let out: Vec<_> = seg.segment(s).collect();
        assert_eq!(out.len(), 2);
        assert_eq!(&s[out[0].range()], "hello");
        assert_eq!(&s[out[1].range()], "world");
        assert_eq!(out[0].offset, 0);
        assert_eq!(out[1].offset, 7);
    }

    #[test]
    fn all_boundaries_offsets_are_contiguous() {
        let seg = WordSegmenter::with_behavior(SplitWordBoundsBehavior::AllBoundaries);
        let s = "a, b";
        let out: Vec<_> = seg.segment(s).collect();
        let mut cursor = 0usize;
        for piece in &out {
            assert_eq!(piece.offset, cursor);
            cursor += piece.text.len();
        }
        assert_eq!(cursor, s.len());
    }

    #[test]
    fn multibyte_scalar_offsets_are_byte_indices() {
        let seg = WordSegmenter::new();
        let s = "héllo wörld";
        let out: Vec<_> = seg.segment(s).collect();
        assert_eq!(out.len(), 2);
        // "héllo" occupies bytes 0..6.
        assert_eq!(&s[out[0].range()], "héllo");
        assert_eq!(&s[out[1].range()], "wörld");
    }

    #[test]
    fn behavior_getter_reflects_construction() {
        assert_eq!(
            WordSegmenter::new().behavior(),
            SplitWordBoundsBehavior::WordsOnly
        );
        assert_eq!(
            WordSegmenter::with_behavior(SplitWordBoundsBehavior::AllBoundaries).behavior(),
            SplitWordBoundsBehavior::AllBoundaries
        );
    }
}
