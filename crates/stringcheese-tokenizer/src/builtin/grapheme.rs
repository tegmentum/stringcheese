//! [`GraphemeSegmenter`] — a thin wrapper over
//! `stringcheese_unicode::graphemes`.
//!
//! A segmenter that yields one [`Segment`] per extended grapheme cluster
//! per [Unicode Standard Annex #29]. This is the "human character"
//! boundary: precomposed vs. decomposed accented letters, emoji flags,
//! and ZWJ sequences all count as one grapheme even though their scalar
//! counts vary.
//!
//! The heavy lifting is done by `stringcheese_unicode::graphemes`, which
//! delegates in turn to the well-tested `unicode-segmentation` crate;
//! this wrapper only re-shapes the iterator into the [`Segmenter`]
//! surface with byte offsets attached.
//!
//! [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/

use alloc::boxed::Box;

use crate::traits::{Segment, Segmenter};

/// Segments input into extended grapheme clusters per UAX #29.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer::{GraphemeSegmenter, Segmenter};
///
/// let seg = GraphemeSegmenter;
/// let gs: Vec<_> = seg.segment("naïve").map(|s| s.text).collect();
/// assert_eq!(gs.len(), 5);
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct GraphemeSegmenter;

impl GraphemeSegmenter {
    /// Constructs a new grapheme segmenter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Iterator returned by [`GraphemeSegmenter::segment`].
///
/// Wraps the `stringcheese_unicode::graphemes` iterator and tracks a
/// running byte offset so each [`Segment`] carries its position in the
/// input.
pub struct GraphemeSegments<'a> {
    inner: Box<dyn Iterator<Item = &'a str> + 'a>,
    offset: usize,
}

impl<'a> Iterator for GraphemeSegments<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let g = self.inner.next()?;
        let seg = Segment::new(self.offset, g);
        self.offset += g.len();
        Some(seg)
    }
}

impl Segmenter for GraphemeSegmenter {
    type Unit<'a>
        = Segment<'a>
    where
        Self: 'a;
    type Iter<'a>
        = GraphemeSegments<'a>
    where
        Self: 'a;

    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a> {
        let inner: Box<dyn Iterator<Item = &'a str> + 'a> =
            Box::new(stringcheese_unicode::graphemes(text));
        GraphemeSegments { inner, offset: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn ascii_graphemes_one_per_byte() {
        let seg = GraphemeSegmenter::new();
        let gs: Vec<_> = seg.segment("hello").map(|s| s.text).collect();
        assert_eq!(gs, ["h", "e", "l", "l", "o"]);
    }

    #[test]
    fn combined_accent_is_one_grapheme() {
        let seg = GraphemeSegmenter::new();
        // e + U+0301 combining acute = one grapheme "é"
        let gs: Vec<_> = seg.segment("e\u{0301}").map(|s| s.text).collect();
        assert_eq!(gs.len(), 1);
        assert_eq!(gs[0], "e\u{0301}");
    }

    #[test]
    fn family_emoji_is_one_grapheme() {
        let seg = GraphemeSegmenter::new();
        let gs: Vec<_> = seg
            .segment("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}")
            .collect();
        assert_eq!(gs.len(), 1);
    }

    #[test]
    fn empty_input_yields_nothing() {
        let seg = GraphemeSegmenter::new();
        let v: Vec<_> = seg.segment("").collect();
        assert!(v.is_empty());
    }

    #[test]
    fn offsets_match_byte_positions() {
        let seg = GraphemeSegmenter::new();
        let s = "abc";
        let out: Vec<_> = seg.segment(s).collect();
        assert_eq!(out[0], Segment::new(0, "a"));
        assert_eq!(out[1], Segment::new(1, "b"));
        assert_eq!(out[2], Segment::new(2, "c"));
    }

    #[test]
    fn offsets_span_multibyte_scalars() {
        let seg = GraphemeSegmenter::new();
        let s = "aé";
        let out: Vec<_> = seg.segment(s).collect();
        assert_eq!(out[0].offset, 0);
        assert_eq!(out[0].text, "a");
        assert_eq!(out[1].offset, 1);
        assert_eq!(out[1].text, "é");
    }
}
