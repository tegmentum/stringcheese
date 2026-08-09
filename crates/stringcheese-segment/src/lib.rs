//! # Unicode-aware string segmentation
//!
//! Split `&str` at the caller-chosen boundary — bytes, code points,
//! graphemes, words, sentences, or lines. Explicit semantic units at
//! every call site: [`SegmentUnit`] names the boundary, [`split`]
//! returns an iterator of `&str` slices.
//!
//! ## Design
//!
//! - **Explicit at the call site.** No silent choice between "words"
//!   and "code points" — the caller names the unit; the answer is
//!   defined regardless of what StringCheese happens to default to.
//! - **Pay for what you ask for.** Bytes / code points / plain
//!   `\n`-lines are dependency-free. Graphemes / words / sentences
//!   need the `icu` feature (ICU4X segmenters, compiled data baked
//!   in). UAX #14 line breaking needs the `uax14-lines` feature.
//! - **Every unit returns `&str` slices of the input.** No copies,
//!   no owned strings. Iterators are lazy where the underlying
//!   segmenter allows it.
//!
//! ## Example
//!
//! ```
//! use stringcheese_segment::{split, SegmentUnit};
//!
//! let parts: Vec<&str> = split("aü日", SegmentUnit::CodePoints).collect();
//! assert_eq!(parts, vec!["a", "ü", "日"]);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

/// The segmentation boundary a call operates at.
///
/// Passed to [`split`]. Each variant determines how the input `&str`
/// is broken into `&str` slices.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SegmentUnit {
    /// Byte-level split — one slice per byte.
    ///
    /// Only produces valid single-scalar `&str` slices when every
    /// byte is ASCII. For mixed input the slices advance to the
    /// next character boundary so every returned `&str` remains a
    /// valid UTF-8 substring.
    Bytes,

    /// Code-point split — one slice per Unicode scalar value.
    CodePoints,

    /// Grapheme-cluster split via ICU4X. Requires the `icu` feature;
    /// a build without it panics on this variant.
    Graphemes,

    /// UAX #29 word segmentation via ICU4X. Requires `icu`. Without
    /// the feature, [`split`] falls back to ASCII-whitespace
    /// splitting.
    Words,

    /// UAX #29 sentence segmentation via ICU4X. Requires `icu`.
    /// Without the feature, [`split`] falls back to splitting on
    /// `. `, `? `, `! `, `\n\n`.
    Sentences,

    /// Line-level split at `\n`. Trailing `\n` is dropped from each
    /// line; a string ending in `\n` yields an empty final slice
    /// (mirrors `str::split('\n')` semantics).
    Lines,

    /// UAX #14 line breaking — breaks at every valid line-break
    /// opportunity, not just `\n`. Requires the `uax14-lines`
    /// feature.
    LinesUax14,
}

/// Split `text` into `&str` slices per `unit`. Returns a boxed
/// iterator so the return type is the same across every variant
/// and feature-gated implementation.
///
/// # Panics
///
/// Panics when a variant requires a Cargo feature that's disabled
/// in this build (e.g. [`SegmentUnit::Graphemes`] without `icu`).
/// This is a build-time misconfiguration, not a runtime input bug —
/// the caller and the crate features go together, so a panic is the
/// correct discipline.
#[cfg(feature = "alloc")]
#[must_use]
pub fn split(text: &str, unit: SegmentUnit) -> Box<dyn Iterator<Item = &str> + '_> {
    match unit {
        SegmentUnit::Bytes => Box::new(BytesIter::new(text)),
        SegmentUnit::CodePoints => Box::new(CodePointsIter::new(text)),
        SegmentUnit::Graphemes => split_graphemes(text),
        SegmentUnit::Words => split_words(text),
        SegmentUnit::Sentences => split_sentences(text),
        SegmentUnit::Lines => Box::new(text.split('\n')),
        SegmentUnit::LinesUax14 => split_lines_uax14(text),
    }
}

// ---------------------------------------------------------------------
// Byte and code-point iterators (dependency-free).
// ---------------------------------------------------------------------

#[cfg(feature = "alloc")]
struct BytesIter<'a> {
    text: &'a str,
    idx: usize,
}

#[cfg(feature = "alloc")]
impl<'a> BytesIter<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, idx: 0 }
    }
}

#[cfg(feature = "alloc")]
impl<'a> Iterator for BytesIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if self.idx >= self.text.len() {
            return None;
        }
        // Advance to the next character boundary so the returned
        // slice is a valid `&str` even for multibyte characters.
        // For ASCII, each iteration returns one byte; for multibyte
        // scalars, the whole scalar is returned as a single slice.
        let start = self.idx;
        let mut end = start + 1;
        while end < self.text.len() && !self.text.is_char_boundary(end) {
            end += 1;
        }
        self.idx = end;
        Some(&self.text[start..end])
    }
}

#[cfg(feature = "alloc")]
struct CodePointsIter<'a> {
    text: &'a str,
    idx: usize,
}

#[cfg(feature = "alloc")]
impl<'a> CodePointsIter<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, idx: 0 }
    }
}

#[cfg(feature = "alloc")]
impl<'a> Iterator for CodePointsIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if self.idx >= self.text.len() {
            return None;
        }
        let start = self.idx;
        let mut end = start + 1;
        while end < self.text.len() && !self.text.is_char_boundary(end) {
            end += 1;
        }
        self.idx = end;
        Some(&self.text[start..end])
    }
}

// ---------------------------------------------------------------------
// Graphemes / Words / Sentences — ICU4X-backed when the feature is on.
// ---------------------------------------------------------------------

#[cfg(all(feature = "alloc", feature = "icu"))]
fn split_graphemes(text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    let seg = icu_segmenter::GraphemeClusterSegmenter::new();
    Box::new(spans_between(
        seg.segment_str(text).collect::<alloc::vec::Vec<usize>>(),
        text,
    ))
}

#[cfg(all(feature = "alloc", not(feature = "icu")))]
fn split_graphemes(_text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    panic!("SegmentUnit::Graphemes requires the `icu` cargo feature")
}

#[cfg(all(feature = "alloc", feature = "icu"))]
fn split_words(text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    let seg = icu_segmenter::WordSegmenter::new_auto();
    Box::new(spans_between(
        seg.segment_str(text).collect::<alloc::vec::Vec<usize>>(),
        text,
    ))
}

/// Fallback: ASCII whitespace split — good enough for
/// dependency-free line/word diffs; for real UAX #29 word
/// boundaries, turn on `icu`.
#[cfg(all(feature = "alloc", not(feature = "icu")))]
fn split_words(text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    Box::new(text.split_ascii_whitespace())
}

#[cfg(all(feature = "alloc", feature = "icu"))]
fn split_sentences(text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    let seg = icu_segmenter::SentenceSegmenter::new();
    Box::new(spans_between(
        seg.segment_str(text).collect::<alloc::vec::Vec<usize>>(),
        text,
    ))
}

/// Fallback: naive split on `. ` / `? ` / `! ` / `\n\n`. For real
/// UAX #29 sentence boundaries, turn on `icu`.
#[cfg(all(feature = "alloc", not(feature = "icu")))]
fn split_sentences(text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    Box::new(NaiveSentenceIter::new(text))
}

#[cfg(all(feature = "alloc", not(feature = "icu")))]
struct NaiveSentenceIter<'a> {
    text: &'a str,
    pos: usize,
}

#[cfg(all(feature = "alloc", not(feature = "icu")))]
impl<'a> NaiveSentenceIter<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }
}

#[cfg(all(feature = "alloc", not(feature = "icu")))]
impl<'a> Iterator for NaiveSentenceIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if self.pos >= self.text.len() {
            return None;
        }
        let start = self.pos;
        let rest = &self.text[start..];
        let mut idx = 0;
        while idx < rest.len() {
            let byte = rest.as_bytes()[idx];
            let next = rest.as_bytes().get(idx + 1).copied();
            let terminator = matches!(
                (byte, next),
                (b'.' | b'?' | b'!', Some(b' ')) | (b'\n', Some(b'\n'))
            );
            if terminator {
                let end = start + idx + 2;
                self.pos = end;
                return Some(&self.text[start..end]);
            }
            idx += 1;
        }
        self.pos = self.text.len();
        Some(&self.text[start..])
    }
}

// ---------------------------------------------------------------------
// UAX #14 line breaking — optional.
// ---------------------------------------------------------------------

#[cfg(all(feature = "alloc", feature = "uax14-lines"))]
fn split_lines_uax14(text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    let boundaries: alloc::vec::Vec<usize> = unicode_linebreak::linebreaks(text)
        .map(|(idx, _)| idx)
        .collect();
    Box::new(spans_between_lines(boundaries, text))
}

#[cfg(all(feature = "alloc", not(feature = "uax14-lines")))]
fn split_lines_uax14(_text: &str) -> Box<dyn Iterator<Item = &str> + '_> {
    panic!("SegmentUnit::LinesUax14 requires the `uax14-lines` cargo feature")
}

#[cfg(all(feature = "alloc", feature = "uax14-lines"))]
fn spans_between_lines(
    mut boundaries: alloc::vec::Vec<usize>,
    text: &str,
) -> impl Iterator<Item = &str> + '_ {
    if boundaries.first() != Some(&0) {
        boundaries.insert(0, 0);
    }
    if boundaries.last() != Some(&text.len()) {
        boundaries.push(text.len());
    }
    boundaries
        .windows(2)
        .map(move |w| &text[w[0]..w[1]])
        .collect::<alloc::vec::Vec<_>>()
        .into_iter()
}

// ---------------------------------------------------------------------
// Helper — turn a Vec of byte offsets into `&str` spans.
// ---------------------------------------------------------------------

#[cfg(all(feature = "alloc", feature = "icu"))]
fn spans_between(
    mut boundaries: alloc::vec::Vec<usize>,
    text: &str,
) -> impl Iterator<Item = &str> + '_ {
    // ICU4X segmenters emit boundaries INCLUDING 0 and text.len();
    // guard against variants that don't.
    if boundaries.first() != Some(&0) {
        boundaries.insert(0, 0);
    }
    if boundaries.last() != Some(&text.len()) {
        boundaries.push(text.len());
    }
    boundaries
        .windows(2)
        .map(move |w| &text[w[0]..w[1]])
        .filter(|s| !s.is_empty())
        .collect::<alloc::vec::Vec<_>>()
        .into_iter()
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn bytes_split_ascii() {
        let parts: Vec<&str> = split("abc", SegmentUnit::Bytes).collect();
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn codepoints_split_multibyte() {
        let parts: Vec<&str> = split("aü日", SegmentUnit::CodePoints).collect();
        assert_eq!(parts, vec!["a", "ü", "日"]);
    }

    #[test]
    fn lines_split_drops_newline() {
        let parts: Vec<&str> = split("one\ntwo\nthree", SegmentUnit::Lines).collect();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn lines_trailing_newline_yields_empty_last() {
        let parts: Vec<&str> = split("one\n", SegmentUnit::Lines).collect();
        assert_eq!(parts, vec!["one", ""]);
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn words_ascii_fallback() {
        let parts: Vec<&str> = split("the  quick brown  fox", SegmentUnit::Words).collect();
        assert_eq!(parts, vec!["the", "quick", "brown", "fox"]);
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn sentences_naive_split() {
        let parts: Vec<&str> = split("Hi. Bye! What? Ok.", SegmentUnit::Sentences).collect();
        assert_eq!(parts.len(), 4);
    }

    #[cfg(feature = "icu")]
    #[test]
    fn graphemes_split_multibyte_via_icu() {
        // A grapheme cluster spans one or more code points; for
        // simple non-combining scalars, one grapheme = one scalar.
        let parts: Vec<&str> = split("aü日", SegmentUnit::Graphemes).collect();
        assert_eq!(parts, vec!["a", "ü", "日"]);
    }
}
