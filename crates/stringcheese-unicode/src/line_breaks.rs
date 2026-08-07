//! UAX #14 line-break opportunities.
//!
//! A **line-break opportunity** is a position in text where a
//! word-wrapper is *allowed* (or, in the case of a hard terminator,
//! *required*) to end one line and start another. The rules are
//! specified in [Unicode Standard Annex #14][UAX14] and are computed
//! from the `Line_Break` property of adjacent scalars — they know
//! that a break may fall after a space, after a hyphen, between two
//! CJK ideographs, and around ZWSP; they know that a break may *not*
//! fall inside a non-breaking-space run, immediately before closing
//! punctuation, or between the components of a numeric expression
//! such as `3.14`.
//!
//! Two flavors of opportunity exist:
//!
//! - [`LineBreak::Mandatory`] — a hard break is *required*. Sources
//!   include `\n` (`LF`), `\r\n` (`CR LF`), `\r`, the paragraph
//!   separator `U+2029`, the line separator `U+2028`, and the run
//!   of `NEXT LINE` / `FORM FEED` codepoints. The end of text is
//!   also reported as `Mandatory` — every input yields at least one
//!   break so a downstream layout engine sees the final line closed.
//! - [`LineBreak::Allowed`] — a *soft* break is permitted; a
//!   word-wrapper picks or skips it based on the target column width.
//!   Sources include after `SPACE`, after `ZWSP` (U+200B), between
//!   ideographs, and after some (but not all) punctuation.
//!
//! # Byte-offset convention
//!
//! Each yielded `(offset, kind)` pair reports the byte offset of the
//! character **immediately after** the break opportunity — i.e. the
//! byte position at which the *next* line would start if the wrapper
//! chose this break. This matches the convention of the underlying
//! `unicode-linebreak` crate and is the same shape a downstream
//! `wrap_at_width` helper wants: slicing `text[..offset]` yields the
//! candidate line, slicing `text[offset..]` yields the remainder.
//!
//! # Relationship to word / sentence segmentation
//!
//! UAX #14 answers a different question than UAX #29:
//!
//! - UAX #29 [`words`](mod@super::words) asks *"where do words end?"*
//!   and is used for tokenization.
//! - UAX #29 [`sentences`](mod@super::sentences) asks *"where do
//!   sentences end?"* and is used for sentence-level segmentation.
//! - UAX #14 [`line_breaks`] asks *"where may a line end?"* and is
//!   used for text layout. A word boundary is *often*, but not
//!   always, a valid line-break opportunity — punctuation rules,
//!   non-breaking spaces, and East Asian ideograph handling all
//!   diverge from word segmentation. Callers doing text layout
//!   (word-wrap, justification, terminal reflow) should reach for
//!   this module, not [`words`](mod@super::words).
//!
//! # Deferred
//!
//! * Locale-tailored break rules (e.g. Japanese `line-break: strict`
//!   vs. `normal` vs. `loose` CSS profiles) are not exposed today —
//!   the default UAX #14 algorithm applies uniformly.
//! * Southeast Asian dictionary-based break detection for Thai, Lao,
//!   Khmer, and Myanmar (the "SA" break class): `unicode-linebreak`
//!   resolves SA characters to Ordinary Alphabetic (AL), the UAX #14
//!   default tailoring. Callers who need dictionary-based breaks for
//!   these scripts should compose this iterator with a downstream
//!   dictionary pass.
//! * A CSS `word-break: break-all`-style forced-break iterator that
//!   inserts a break opportunity between every grapheme. That
//!   behavior is a wrapper concern rather than a segmentation
//!   concern; a future `stringcheese-manip::wrap` helper can layer
//!   it on top.
//!
//! [UAX14]: https://www.unicode.org/reports/tr14/
//!
//! # References
//!
//! * Unicode Standard Annex #14. *Unicode Line Breaking Algorithm*.
//!   URL: <https://www.unicode.org/reports/tr14/> — the specification
//!   the underlying `unicode-linebreak` crate implements.
//! * Unicode Consortium (2022). *The Unicode Standard, Version 15.0.0*.
//!   Mountain View, CA: The Unicode Consortium. ISBN 978-1-936213-32-0.

use alloc::vec::Vec;
use stringcheese_core::IndexableSequence;
use unicode_linebreak::{BreakOpportunity, linebreaks};

/// Kind of line-break opportunity reported by [`line_breaks`] and
/// [`LineBreakSequence`].
///
/// This is a one-to-one restatement of `unicode-linebreak`'s
/// `BreakOpportunity` enum: the wrapper exists so this crate's public
/// surface does not leak a third-party type name and so the variant
/// docstrings can be tailored to StringCheese's conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineBreak {
    /// A line *must* break at this position. Sources include `\n`,
    /// `\r\n`, `\r`, `U+0085 NEXT LINE`, `U+2028 LINE SEPARATOR`,
    /// `U+2029 PARAGRAPH SEPARATOR`, `U+000B VERTICAL TAB`, `U+000C
    /// FORM FEED`, and the end of input (the terminal mandatory break
    /// guarantees every input yields at least one opportunity so a
    /// downstream layout engine sees the final line closed).
    Mandatory,
    /// A line *may* break at this position. A word-wrapper picks or
    /// skips it based on target column width. Sources include after
    /// space, after `U+200B ZERO WIDTH SPACE`, between ideographs,
    /// and after some (but not all) punctuation.
    Allowed,
}

impl From<BreakOpportunity> for LineBreak {
    #[inline]
    fn from(op: BreakOpportunity) -> Self {
        match op {
            BreakOpportunity::Mandatory => LineBreak::Mandatory,
            BreakOpportunity::Allowed => LineBreak::Allowed,
        }
    }
}

/// Iterates the UAX #14 line-break opportunities of `input`.
///
/// Each yielded `(offset, kind)` pair reports the byte offset of the
/// character immediately *after* the break opportunity — the position
/// at which the next line would start if the wrapper chose this break
/// — together with a [`LineBreak`] describing whether the break is
/// [`LineBreak::Mandatory`] (a hard terminator such as `\n`) or
/// [`LineBreak::Allowed`] (a soft-wrap opportunity a wrapper may
/// take or skip).
///
/// Every non-empty input ends with a [`LineBreak::Mandatory`] at
/// `input.len()`, so a downstream layout engine always sees the final
/// line explicitly closed.
///
/// The underlying iterator is lazy and zero-allocation. For indexed
/// access see [`LineBreakSequence`].
///
/// # Examples
///
/// Extracting soft-wrap points from a paragraph and wrapping at a
/// character budget:
///
/// ```
/// # use stringcheese_unicode::{LineBreak, line_breaks};
/// let paragraph = "The quick brown fox jumps over the lazy dog.";
///
/// // Every soft break the layout engine may take.
/// let soft: Vec<usize> = line_breaks(paragraph)
///     .filter_map(|(off, kind)| (kind == LineBreak::Allowed).then_some(off))
///     .collect();
/// assert!(soft.contains(&4));  // "The " → position of 'q'
/// assert!(soft.contains(&10)); // "quick "
///
/// // The terminal break is always Mandatory.
/// let last = line_breaks(paragraph).last().unwrap();
/// assert_eq!(last, (paragraph.len(), LineBreak::Mandatory));
///
/// // Greedy wrap: take the largest break offset that fits the budget.
/// let budget = 20;
/// let mut wrap_at = 0usize;
/// for (off, _kind) in line_breaks(paragraph) {
///     if off <= budget { wrap_at = off; } else { break; }
/// }
/// // "The quick brown fox " fits in 20 columns.
/// assert_eq!(&paragraph[..wrap_at], "The quick brown fox ");
/// ```
///
/// Hard line terminators produce [`LineBreak::Mandatory`]:
///
/// ```
/// # use stringcheese_unicode::{LineBreak, line_breaks};
/// let text = "one\ntwo\r\nthree";
/// let hard: Vec<usize> = line_breaks(text)
///     .filter_map(|(off, kind)| (kind == LineBreak::Mandatory).then_some(off))
///     .collect();
/// // `\n` closes "one\n" at byte 4; `\r\n` closes "...two\r\n" at
/// // byte 9; end-of-input closes the final line at byte 14.
/// assert_eq!(hard, vec![4, 9, 14]);
/// ```
pub fn line_breaks(input: &str) -> impl Iterator<Item = (usize, LineBreak)> + '_ {
    linebreaks(input).map(|(off, op)| (off, LineBreak::from(op)))
}

/// A materialized sequence of UAX #14 line-break opportunities,
/// indexable in `O(1)`.
///
/// [`line_breaks`] yields opportunities lazily and cannot answer
/// `get(i)` without walking the string from the start. This type
/// does that walk once, buffering the results, so downstream layout
/// engines that need indexed access (a bidirectional wrapper that
/// backtracks over candidate break points, a paginator that binary-
/// searches for the largest break under a byte budget) can treat
/// line-break opportunities as a first-class sequence.
///
/// ```
/// # use stringcheese_unicode::{LineBreak, LineBreakSequence};
/// let seq = LineBreakSequence::new("hello world");
/// // "hello " (allowed after space) + terminal mandatory.
/// assert_eq!(seq.len(), 2);
/// assert_eq!(seq.get(0).copied(), Some((6, LineBreak::Allowed)));
/// assert_eq!(seq.get(1).copied(), Some((11, LineBreak::Mandatory)));
/// ```
///
/// # Lifetimes
///
/// `LineBreakSequence<'a>` is parameterized over the input lifetime
/// so it composes cleanly with iterators that also borrow from the
/// same source string, but the buffered `Vec<(usize, LineBreak)>`
/// holds only `Copy` values — no borrow of the source is retained
/// through the buffer. The lifetime parameter is nevertheless kept
/// on the type to preserve the same shape as [`WordSequence`] and
/// [`SentenceSequence`] so a generic `MaterializedSegmentation<'a>`
/// bound at a future comparator layer applies uniformly.
///
/// [`WordSequence`]: super::WordSequence
/// [`SentenceSequence`]: super::SentenceSequence
#[derive(Debug, Clone)]
pub struct LineBreakSequence<'a> {
    breaks: Vec<(usize, LineBreak)>,
    // Retain the input lifetime marker so this type is unifiable with
    // other segmentation-buffered wrappers at a generic bound.
    _marker: core::marker::PhantomData<&'a str>,
}

impl<'a> LineBreakSequence<'a> {
    /// Segments `input` into its UAX #14 line-break opportunities and
    /// buffers the results for `O(1)` indexed access.
    ///
    /// Performs one allocation (the spine `Vec`).
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            breaks: line_breaks(input).collect(),
            _marker: core::marker::PhantomData,
        }
    }

    /// The number of break opportunities. Every non-empty input has
    /// at least one (the terminal [`LineBreak::Mandatory`]).
    #[must_use]
    pub fn len(&self) -> usize {
        self.breaks.len()
    }

    /// `true` if the sequence is empty. This happens exactly when the
    /// input string was empty — a non-empty input always produces at
    /// least the terminal mandatory break.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.breaks.is_empty()
    }

    /// Returns the break opportunity at position `i`, or `None` if
    /// `i >= len()`.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&(usize, LineBreak)> {
        self.breaks.get(i)
    }

    /// The underlying buffered slice, suitable for algorithms that
    /// want to binary-search for the largest break offset under a
    /// column budget or iterate opportunities in reverse.
    #[must_use]
    pub fn as_slice(&self) -> &[(usize, LineBreak)] {
        &self.breaks
    }
}

impl IndexableSequence for LineBreakSequence<'_> {
    type Item = (usize, LineBreak);

    #[inline]
    fn len(&self) -> usize {
        self.breaks.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.breaks.is_empty()
    }

    #[inline]
    fn get(&self, index: usize) -> Option<&Self::Item> {
        self.breaks.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn empty_input_yields_no_opportunities() {
        assert!(line_breaks("").next().is_none());
        assert!(LineBreakSequence::new("").is_empty());
    }

    #[test]
    fn single_word_yields_only_terminal_mandatory() {
        let bs: Vec<(usize, LineBreak)> = line_breaks("hello").collect();
        assert_eq!(bs, vec![(5, LineBreak::Mandatory)]);
    }

    #[test]
    fn space_is_a_soft_break_opportunity() {
        let bs: Vec<(usize, LineBreak)> = line_breaks("hello world").collect();
        // After "hello " comes 'w' at byte 6; end of text at 11.
        assert_eq!(
            bs,
            vec![(6, LineBreak::Allowed), (11, LineBreak::Mandatory)]
        );
    }

    #[test]
    fn newline_is_mandatory() {
        let bs: Vec<(usize, LineBreak)> = line_breaks("a\nb").collect();
        assert_eq!(
            bs,
            vec![(2, LineBreak::Mandatory), (3, LineBreak::Mandatory)]
        );
    }

    #[test]
    fn crlf_is_a_single_mandatory_break() {
        // `\r\n` counts as one hard terminator, not two.
        let bs: Vec<(usize, LineBreak)> = line_breaks("a\r\nb").collect();
        assert_eq!(
            bs,
            vec![(3, LineBreak::Mandatory), (4, LineBreak::Mandatory)]
        );
    }

    #[test]
    fn paragraph_separator_is_mandatory() {
        // U+2029 (PARAGRAPH SEPARATOR) is 3 UTF-8 bytes.
        let text = "a\u{2029}b";
        let bs: Vec<(usize, LineBreak)> = line_breaks(text).collect();
        // Break after PS (byte 4, position of 'b'); terminal at end (5).
        assert_eq!(
            bs,
            vec![(4, LineBreak::Mandatory), (5, LineBreak::Mandatory)]
        );
    }

    #[test]
    fn line_separator_is_mandatory() {
        // U+2028 (LINE SEPARATOR) is 3 UTF-8 bytes.
        let text = "a\u{2028}b";
        let bs: Vec<(usize, LineBreak)> = line_breaks(text).collect();
        assert_eq!(
            bs,
            vec![(4, LineBreak::Mandatory), (5, LineBreak::Mandatory)]
        );
    }

    #[test]
    fn terminal_break_offset_matches_input_len() {
        for input in ["one", "one two", "one\ntwo"] {
            let last = line_breaks(input).last().unwrap();
            assert_eq!(last.0, input.len(), "input={input:?}");
        }
    }

    #[test]
    fn no_break_inside_word() {
        // No opportunity may fall between the letters of "hello".
        let bs: Vec<(usize, LineBreak)> = line_breaks("hello").collect();
        for &(off, _) in &bs {
            assert!(off == 0 || off == 5, "unexpected break at {off}");
        }
    }

    #[test]
    fn sequence_get_returns_opportunities() {
        let seq = LineBreakSequence::new("hi there");
        assert_eq!(seq.len(), 2);
        assert_eq!(seq.get(0).copied(), Some((3, LineBreak::Allowed)));
        assert_eq!(seq.get(1).copied(), Some((8, LineBreak::Mandatory)));
        assert_eq!(seq.get(2), None);
    }

    #[test]
    fn indexable_sequence_impl_works() {
        let seq = LineBreakSequence::new("hi there");
        assert_eq!(<LineBreakSequence<'_> as IndexableSequence>::len(&seq), 2);
        assert_eq!(
            <LineBreakSequence<'_> as IndexableSequence>::get(&seq, 0).copied(),
            Some((3, LineBreak::Allowed))
        );
        assert_eq!(
            <LineBreakSequence<'_> as IndexableSequence>::get(&seq, 2),
            None
        );
    }

    #[test]
    fn as_slice_exposes_underlying_pairs() {
        let seq = LineBreakSequence::new("a b");
        let s = seq.as_slice();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0], (2, LineBreak::Allowed));
        assert_eq!(s[1], (3, LineBreak::Mandatory));
    }

    #[test]
    fn greedy_wrap_over_line_breaks() {
        // A tiny word-wrapper: given a byte budget, pick the largest
        // break offset that fits. Confirms the offset convention
        // (byte-after-break) is what a real caller wants.
        let text = "The quick brown fox jumps over the lazy dog.";
        let mut lines: Vec<&str> = Vec::new();
        let mut start = 0usize;
        let budget = 15usize;
        loop {
            let mut wrap_at = start;
            for (off, kind) in line_breaks(&text[start..]) {
                let abs = start + off;
                if abs - start <= budget {
                    wrap_at = abs;
                }
                if kind == LineBreak::Mandatory && abs - start > budget {
                    break;
                }
                if abs - start > budget {
                    break;
                }
            }
            if wrap_at <= start {
                // No break fits — emit the remainder to avoid an
                // infinite loop; a real wrapper would fall back to
                // force-break-at-grapheme here.
                lines.push(&text[start..]);
                break;
            }
            lines.push(&text[start..wrap_at]);
            if wrap_at >= text.len() {
                break;
            }
            start = wrap_at;
        }
        // Every line was closed at a UAX #14 opportunity and every
        // slice is within the byte budget except the possibly-shorter
        // final line.
        for (i, l) in lines.iter().enumerate() {
            let is_last = i + 1 == lines.len();
            assert!(
                l.len() <= budget || is_last,
                "line {i} of {l:?} exceeds budget {budget}"
            );
        }
        // Concatenation reconstructs the input.
        let joined: alloc::string::String = lines.concat();
        assert_eq!(joined, text);
    }

    #[test]
    fn nonascii_offsets_are_byte_offsets_not_scalar_offsets() {
        // "café world" — the é is two bytes (U+00E9 → 0xC3 0xA9).
        let text = "café world";
        let bs: Vec<(usize, LineBreak)> = line_breaks(text).collect();
        // After "café " (5 bytes: c a f é_2 space) comes 'w' at byte 6.
        assert!(
            bs.iter()
                .any(|&(off, k)| off == 6 && k == LineBreak::Allowed)
        );
        // Terminal Mandatory at byte length.
        assert_eq!(bs.last().unwrap().0, text.len());
    }

    #[test]
    fn hyphen_permits_soft_break_after() {
        // UAX #14 permits a break after a hyphen (`BA` / `HY` classes).
        let bs: Vec<(usize, LineBreak)> = line_breaks("state-of-the-art").collect();
        // At least one Allowed after a hyphen (byte offset 6 = position after "state-").
        assert!(bs.iter().any(|&(_, k)| k == LineBreak::Allowed));
    }

    #[test]
    fn from_break_opportunity_round_trip() {
        assert_eq!(
            LineBreak::from(BreakOpportunity::Mandatory),
            LineBreak::Mandatory
        );
        assert_eq!(
            LineBreak::from(BreakOpportunity::Allowed),
            LineBreak::Allowed
        );
    }
}
