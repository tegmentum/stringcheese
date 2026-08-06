//! Grapheme-cluster segmentation.
//!
//! A **grapheme cluster** is what a user perceives as a single character
//! — the smallest unit of text a human would call "one letter" or "one
//! emoji". A grapheme may be composed of many Unicode scalar values:
//!
//! - `"é"` written as `e` + combining acute (U+0301) is *one* grapheme
//!   but *two* scalars.
//! - `"🇬🇧"` (the UK flag) is one grapheme but two scalars (regional
//!   indicator letters `G` and `B`).
//! - `"👨‍👩‍👧"` (family emoji) is one grapheme but five scalars (three emoji
//!   joined by zero-width joiners).
//!
//! Grapheme boundaries are specified in [Unicode Standard Annex #29];
//! this module wraps `unicode-segmentation`'s implementation.
//!
//! # Why this matters for comparison
//!
//! Before this module, StringCheese algorithms could compare over UTF-8
//! bytes or over Unicode scalar values (`&[char]`) — neither of which
//! corresponds to what a human counts as a "character". A Levenshtein
//! distance of 1 between `"café"` and `"cafe"` might mean the acute
//! accent was added or removed, but under a scalar-level comparison the
//! distance depends on whether the accented `e` was stored precomposed
//! (`é`, one scalar) or decomposed (`e` + combining acute, two scalars).
//!
//! With [`GraphemeSequence`], a distance kernel can compare over
//! *graphemes* and get the answer a human expects: replacing a
//! precomposed `é` with a bare `e` is one grapheme-level edit, and
//! replacing a decomposed `e + combining acute` with a bare `e` is also
//! one grapheme-level edit.
//!
//! # Extended vs. legacy graphemes
//!
//! [`graphemes`] and [`GraphemeSequence`] use the *extended*
//! grapheme-cluster definition. This is what all modern text
//! infrastructure means by "grapheme"; the legacy definition exists only
//! for backwards compatibility with pre-Unicode-10 text engines.
//!
//! [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/
//!
//! # References
//!
//! * Unicode Standard Annex #29. *Unicode Text Segmentation*. URL:
//!   <https://www.unicode.org/reports/tr29/> — the specification
//!   [`GraphemeSequence`] and [`graphemes`] implement (extended
//!   grapheme-cluster boundary rules).
//! * Unicode Consortium (2022). *The Unicode Standard, Version 15.0.0*.
//!   Mountain View, CA: The Unicode Consortium. ISBN 978-1-936213-32-0.

use alloc::vec::Vec;
use stringcheese_core::IndexableSequence;
use unicode_segmentation::UnicodeSegmentation;

/// Iterates the extended grapheme clusters of `input`.
///
/// The returned iterator yields subslices of `input`; no allocation is
/// performed. Where you need indexed access see [`GraphemeSequence`].
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::graphemes;
/// let gs: Vec<&str> = graphemes("naïve").collect();
/// assert_eq!(gs.len(), 5);
///
/// // A family emoji is *one* grapheme even though it is many scalars.
/// let family: Vec<&str> = graphemes("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}").collect();
/// assert_eq!(family.len(), 1);
/// ```
pub fn graphemes(input: &str) -> impl Iterator<Item = &str> {
    UnicodeSegmentation::graphemes(input, /* is_extended */ true)
}

/// A materialized sequence of grapheme clusters, indexable in `O(1)`.
///
/// [`graphemes`] yields grapheme subslices lazily and cannot answer
/// `get(i)` without walking the string from the start. This type does
/// that walk once, buffering the results, so downstream algorithms that
/// need indexed access (edit-distance kernels, alignment kernels) can
/// treat graphemes as a first-class sequence.
///
/// This is the type that finally lets a StringCheese distance kernel
/// compare "over graphemes":
///
/// ```
/// # use stringcheese_unicode::GraphemeSequence;
/// // Both sides are one grapheme each; the sequences have equal length.
/// let a = GraphemeSequence::new("café");
/// let b = GraphemeSequence::new("cafe");
/// assert_eq!(a.len(), 4);
/// assert_eq!(b.len(), 4);
///
/// // A downstream distance kernel would consume `a.as_slice()` and
/// // `b.as_slice()`. Both are `&[&str]` and their `IndexableSequence`
/// // impl is provided directly by [`GraphemeSequence`].
/// ```
///
/// # Lifetimes
///
/// `GraphemeSequence<'a>` borrows from the input string `'a`. The
/// buffered `Vec<&'a str>` holds subslice pointers into the source
/// string, not owned copies — segmentation therefore allocates once (the
/// `Vec` spine) and does not copy any text. As long as the input string
/// outlives the sequence, the sequence is valid.
///
/// If you need an owned form that can outlive the input, `collect`
/// into a `Vec<String>` explicitly.
#[derive(Debug, Clone)]
pub struct GraphemeSequence<'a> {
    graphemes: Vec<&'a str>,
}

impl<'a> GraphemeSequence<'a> {
    /// Segments `input` into its extended grapheme clusters and buffers
    /// the results for `O(1)` indexed access.
    ///
    /// Performs one allocation (the spine `Vec`).
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            graphemes: graphemes(input).collect(),
        }
    }

    /// The number of graphemes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.graphemes.len()
    }

    /// `true` if the sequence is empty (which happens exactly when the
    /// input string was empty).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.graphemes.is_empty()
    }

    /// Returns the grapheme at position `i`, or `None` if `i >= len()`.
    ///
    /// The returned `&&str` is a reference to the buffered pointer;
    /// dereference once (`**g`) or `.copied()` a slice iterator to get
    /// the underlying `&str`.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&&'a str> {
        self.graphemes.get(i)
    }

    /// The underlying buffered slice, suitable for handing to any
    /// algorithm that consumes `&[&str]` (which most StringCheese
    /// distance kernels do via [`IndexableSequence`]).
    #[must_use]
    pub fn as_slice(&self) -> &[&'a str] {
        &self.graphemes
    }
}

impl<'a> IndexableSequence for GraphemeSequence<'a> {
    type Item = &'a str;

    #[inline]
    fn len(&self) -> usize {
        self.graphemes.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.graphemes.is_empty()
    }

    #[inline]
    fn get(&self, index: usize) -> Option<&Self::Item> {
        self.graphemes.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn ascii_graphemes_match_chars() {
        let s = "hello";
        let gs: Vec<&str> = graphemes(s).collect();
        assert_eq!(gs.len(), 5);
        assert_eq!(gs, ["h", "e", "l", "l", "o"]);
    }

    #[test]
    fn precomposed_e_acute_is_one_grapheme() {
        let s = "\u{00E9}"; // "é" precomposed
        let gs: Vec<&str> = graphemes(s).collect();
        assert_eq!(gs.len(), 1);
    }

    #[test]
    fn decomposed_e_acute_is_one_grapheme() {
        let s = "e\u{0301}"; // "e" + combining acute
        let gs: Vec<&str> = graphemes(s).collect();
        assert_eq!(gs.len(), 1);
    }

    #[test]
    fn cafe_precomposed_is_four_graphemes() {
        let s = "caf\u{00E9}"; // "café" precomposed
        assert_eq!(GraphemeSequence::new(s).len(), 4);
    }

    #[test]
    fn cafe_decomposed_is_four_graphemes() {
        let s = "cafe\u{0301}"; // "cafe" + combining acute
        assert_eq!(GraphemeSequence::new(s).len(), 4);
    }

    #[test]
    fn uk_flag_is_one_grapheme() {
        // Regional indicator letters G + B.
        let s = "\u{1F1EC}\u{1F1E7}";
        assert_eq!(GraphemeSequence::new(s).len(), 1);
    }

    #[test]
    fn family_emoji_is_one_grapheme() {
        // Man + ZWJ + Woman + ZWJ + Girl.
        let s = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(GraphemeSequence::new(s).len(), 1);
    }

    #[test]
    fn get_returns_borrowed_str() {
        let s = "cafe";
        let seq = GraphemeSequence::new(s);
        assert_eq!(seq.get(0).copied(), Some("c"));
        assert_eq!(seq.get(3).copied(), Some("e"));
        assert_eq!(seq.get(4), None);
    }

    #[test]
    fn is_empty_matches_len_zero() {
        assert!(GraphemeSequence::new("").is_empty());
        assert!(!GraphemeSequence::new("x").is_empty());
    }

    #[test]
    fn indexable_sequence_impl_works() {
        let s = "abc";
        let seq = GraphemeSequence::new(s);
        // Fully-qualified call to disambiguate from inherent method.
        assert_eq!(<GraphemeSequence<'_> as IndexableSequence>::len(&seq), 3);
        assert_eq!(
            <GraphemeSequence<'_> as IndexableSequence>::get(&seq, 1).copied(),
            Some("b")
        );
        assert_eq!(
            <GraphemeSequence<'_> as IndexableSequence>::get(&seq, 3),
            None
        );
    }

    #[test]
    fn as_slice_yields_input_content() {
        let s = "abc";
        let seq = GraphemeSequence::new(s);
        let slice: &[&str] = seq.as_slice();
        let joined: alloc::string::String = slice.concat();
        assert_eq!(joined, s);
    }
}
