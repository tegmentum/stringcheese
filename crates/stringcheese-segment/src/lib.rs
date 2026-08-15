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
//!
//! ## Relationship to `stringcheese-icu-segment`
//!
//! This crate is the **in-process convenience splitter**: one
//! [`SegmentUnit`] enum, one [`split`] entry point, returns
//! `Box<dyn Iterator<Item = &str>>` over the caller's own input
//! slice. Basic units (bytes, code points, `\n`-lines) are
//! zero-dependency; grapheme / word / sentence units delegate to
//! ICU4X's `icu_segmenter` under the `icu` feature. Reach for it
//! when the caller wants string slices and is happy with a Rust-side
//! API.
//!
//! [`stringcheese-icu-segment`] is the Phase-5 WIT-i18n crate that
//! ships an independent UAX #29 rule-engine (`BreakEngine` over
//! built-in classification tables), returns **byte-offset lists**
//! rather than slices, consumes optional SCUD `BreakPack` data
//! (including CJK dictionary word-break tailoring), and ships behind
//! a WASM component boundary. Reach for it when the caller needs
//! WIT-shaped access, boundary offsets rather than slices, or
//! dictionary-based CJK word segmentation from a SCUD pack.
//!
//! The two implementations do not share code — this crate delegates
//! to ICU4X; the icu-segment crate walks UAX #29 rules directly. Pick
//! by shape (slices vs offsets, in-process vs WIT), not by
//! correctness of segmentation.
//!
//! [`stringcheese-icu-segment`]: https://docs.rs/stringcheese-icu-segment

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

    // -----------------------------------------------------------------
    // SegmentUnit enum: derive shape
    // -----------------------------------------------------------------

    #[test]
    fn segment_unit_is_copy_and_eq() {
        // Copy/Eq/Hash are required because SegmentUnit is a small
        // config-shaped enum consumers pass by value and stash in
        // config records. Regression-guard the derives.
        let a = SegmentUnit::Bytes;
        let b = a; // implicit Copy
        assert_eq!(a, b);
        assert_ne!(SegmentUnit::Bytes, SegmentUnit::CodePoints);
        assert_ne!(SegmentUnit::Lines, SegmentUnit::LinesUax14);
    }

    #[test]
    fn segment_unit_debug_is_non_empty() {
        // Debug format is used in error messages and traces.
        let s = alloc::format!("{:?}", SegmentUnit::Graphemes);
        assert!(!s.is_empty());
        assert!(s.contains("Graphemes"));
    }

    #[test]
    fn segment_unit_is_hashable() {
        // Hash bound is enforced by the derive; verify by using it as
        // a HashMap key. Trips a compile error if the derive slips.
        use std::collections::HashMap;
        let mut m: HashMap<SegmentUnit, u32> = HashMap::new();
        m.insert(SegmentUnit::Words, 1);
        m.insert(SegmentUnit::Sentences, 2);
        assert_eq!(m.get(&SegmentUnit::Words), Some(&1));
    }

    // -----------------------------------------------------------------
    // Bytes variant
    // -----------------------------------------------------------------

    #[test]
    fn bytes_empty_input_yields_no_slices() {
        let parts: Vec<&str> = split("", SegmentUnit::Bytes).collect();
        assert!(parts.is_empty());
    }

    #[test]
    fn bytes_single_ascii_char_yields_one_slice() {
        let parts: Vec<&str> = split("x", SegmentUnit::Bytes).collect();
        assert_eq!(parts, vec!["x"]);
    }

    #[test]
    fn bytes_multibyte_char_stays_one_slice() {
        // "ü" is 2 bytes; despite the name "Bytes", the iterator
        // advances to the next char boundary so returned &str is
        // valid. This is the documented behavior.
        let parts: Vec<&str> = split("aüb", SegmentUnit::Bytes).collect();
        assert_eq!(parts, vec!["a", "ü", "b"]);
    }

    #[test]
    fn bytes_four_byte_char_stays_one_slice() {
        // Emoji "🦀" is a 4-byte scalar (U+1F980); still one slice.
        let parts: Vec<&str> = split("🦀", SegmentUnit::Bytes).collect();
        assert_eq!(parts, vec!["🦀"]);
    }

    #[test]
    fn bytes_roundtrips_via_concat() {
        // The concatenation of every yielded slice equals the input —
        // the fundamental round-trip invariant.
        let input = "hello, 世界! 🦀";
        let parts: Vec<&str> = split(input, SegmentUnit::Bytes).collect();
        let round: alloc::string::String = parts.concat();
        assert_eq!(round, input);
    }

    // -----------------------------------------------------------------
    // CodePoints variant
    // -----------------------------------------------------------------

    #[test]
    fn codepoints_empty_input_yields_no_slices() {
        let parts: Vec<&str> = split("", SegmentUnit::CodePoints).collect();
        assert!(parts.is_empty());
    }

    #[test]
    fn codepoints_single_char_yields_one_slice() {
        let parts: Vec<&str> = split("ü", SegmentUnit::CodePoints).collect();
        assert_eq!(parts, vec!["ü"]);
    }

    #[test]
    fn codepoints_yields_one_slice_per_scalar() {
        // "🦀" is one scalar. So is "é" as NFC-precomposed U+00E9,
        // vs two scalars for the NFD form "e\u{301}".
        let precomposed: Vec<&str> = split("é", SegmentUnit::CodePoints).collect();
        assert_eq!(precomposed.len(), 1);
        let decomposed: Vec<&str> = split("e\u{301}", SegmentUnit::CodePoints).collect();
        assert_eq!(decomposed.len(), 2);
    }

    #[test]
    fn codepoints_roundtrips_via_concat() {
        let input = "café — 日本語 🇯🇵";
        let parts: Vec<&str> = split(input, SegmentUnit::CodePoints).collect();
        let round: alloc::string::String = parts.concat();
        assert_eq!(round, input);
    }

    #[test]
    fn codepoints_count_matches_char_count() {
        // The number of yielded slices matches str::chars().count().
        let input = "aé日🦀";
        let parts: Vec<&str> = split(input, SegmentUnit::CodePoints).collect();
        assert_eq!(parts.len(), input.chars().count());
    }

    // -----------------------------------------------------------------
    // Lines variant (\n split)
    // -----------------------------------------------------------------

    #[test]
    fn lines_empty_input_yields_one_empty_slice() {
        // str::split('\n') on "" yields one empty slice; preserving
        // that saves a special-case for callers.
        let parts: Vec<&str> = split("", SegmentUnit::Lines).collect();
        assert_eq!(parts, vec![""]);
    }

    #[test]
    fn lines_single_line_yields_that_line() {
        let parts: Vec<&str> = split("just one", SegmentUnit::Lines).collect();
        assert_eq!(parts, vec!["just one"]);
    }

    #[test]
    fn lines_consecutive_newlines_yield_empty_middles() {
        let parts: Vec<&str> = split("a\n\nb", SegmentUnit::Lines).collect();
        assert_eq!(parts, vec!["a", "", "b"]);
    }

    #[test]
    fn lines_crlf_preserves_cr_on_line() {
        // \n split keeps \r on the preceding line. This is what
        // str::split('\n') does; test guards the pass-through.
        let parts: Vec<&str> = split("one\r\ntwo\r\nthree", SegmentUnit::Lines).collect();
        assert_eq!(parts, vec!["one\r", "two\r", "three"]);
    }

    #[test]
    fn lines_only_newlines_yields_all_empty() {
        let parts: Vec<&str> = split("\n\n\n", SegmentUnit::Lines).collect();
        assert_eq!(parts, vec!["", "", "", ""]);
    }

    // -----------------------------------------------------------------
    // Words variant (fallback path — no icu)
    // -----------------------------------------------------------------

    #[cfg(not(feature = "icu"))]
    #[test]
    fn words_fallback_empty_input_yields_nothing() {
        let parts: Vec<&str> = split("", SegmentUnit::Words).collect();
        assert!(parts.is_empty());
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn words_fallback_skips_all_whitespace() {
        let parts: Vec<&str> = split("  a  b   c  ", SegmentUnit::Words).collect();
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn words_fallback_treats_tab_and_newline_as_separator() {
        let parts: Vec<&str> = split("a\tb\nc", SegmentUnit::Words).collect();
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn words_fallback_only_whitespace_yields_nothing() {
        let parts: Vec<&str> = split("   \t\n   ", SegmentUnit::Words).collect();
        assert!(parts.is_empty());
    }

    // -----------------------------------------------------------------
    // Sentences variant (fallback path — no icu)
    // -----------------------------------------------------------------

    #[cfg(not(feature = "icu"))]
    #[test]
    fn sentences_fallback_empty_input_yields_nothing() {
        let parts: Vec<&str> = split("", SegmentUnit::Sentences).collect();
        assert!(parts.is_empty());
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn sentences_fallback_no_terminator_yields_one_slice() {
        // No `. ` / `? ` / `! ` / `\n\n` → the whole input is one
        // sentence.
        let parts: Vec<&str> = split("no terminator here", SegmentUnit::Sentences).collect();
        assert_eq!(parts, vec!["no terminator here"]);
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn sentences_fallback_splits_on_period_space() {
        let parts: Vec<&str> = split("Hi there. How are you.", SegmentUnit::Sentences).collect();
        assert_eq!(parts.len(), 2);
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn sentences_fallback_splits_on_double_newline() {
        let parts: Vec<&str> = split("one\n\ntwo", SegmentUnit::Sentences).collect();
        assert_eq!(parts.len(), 2);
    }

    #[cfg(not(feature = "icu"))]
    #[test]
    fn sentences_fallback_period_without_space_stays_together() {
        // `.` without a following space is not a terminator in the
        // naive splitter (protects filenames, decimals).
        let parts: Vec<&str> =
            split("foo.bar and 3.14 stays whole", SegmentUnit::Sentences).collect();
        assert_eq!(parts.len(), 1);
    }

    // -----------------------------------------------------------------
    // Graphemes / Words / Sentences (icu-backed paths)
    // -----------------------------------------------------------------

    #[cfg(feature = "icu")]
    #[test]
    fn graphemes_empty_input_yields_no_slices() {
        // ICU4X returns [0, 0]; the filter drops empty slices, so
        // the iterator ends up empty.
        let parts: Vec<&str> = split("", SegmentUnit::Graphemes).collect();
        assert!(parts.is_empty());
    }

    #[cfg(feature = "icu")]
    #[test]
    fn graphemes_combining_mark_stays_with_base() {
        // "e" + U+0301 combining acute is one grapheme cluster even
        // though it's two code points.
        let parts: Vec<&str> = split("e\u{301}", SegmentUnit::Graphemes).collect();
        assert_eq!(parts, vec!["e\u{301}"]);
    }

    #[cfg(feature = "icu")]
    #[test]
    fn graphemes_zwj_sequence_stays_one_cluster() {
        // Family emoji: man + ZWJ + woman + ZWJ + girl.
        // Should collapse to a single grapheme cluster.
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let parts: Vec<&str> = split(family, SegmentUnit::Graphemes).collect();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], family);
    }

    #[cfg(feature = "icu")]
    #[test]
    fn graphemes_flag_emoji_stays_one_cluster() {
        // Regional-indicator flag emoji: U+1F1EF U+1F1F5 → 🇯🇵.
        let flag = "\u{1F1EF}\u{1F1F5}";
        let parts: Vec<&str> = split(flag, SegmentUnit::Graphemes).collect();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], flag);
    }

    #[cfg(feature = "icu")]
    #[test]
    fn graphemes_roundtrip_via_concat() {
        let input = "café 🇯🇵 e\u{301}f 🦀";
        let parts: Vec<&str> = split(input, SegmentUnit::Graphemes).collect();
        let round: alloc::string::String = parts.concat();
        assert_eq!(round, input);
    }

    #[cfg(feature = "icu")]
    #[test]
    fn words_icu_empty_input_yields_no_slices() {
        let parts: Vec<&str> = split("", SegmentUnit::Words).collect();
        assert!(parts.is_empty());
    }

    #[cfg(feature = "icu")]
    #[test]
    fn words_icu_roundtrip_via_concat() {
        // ICU4X word segmenter includes whitespace/punctuation as
        // their own segments; concatenation reproduces the input.
        let input = "The quick brown fox.";
        let parts: Vec<&str> = split(input, SegmentUnit::Words).collect();
        let round: alloc::string::String = parts.concat();
        assert_eq!(round, input);
    }

    #[cfg(feature = "icu")]
    #[test]
    fn words_icu_finds_word_boundaries_in_english() {
        // At minimum, "hello" and "world" appear as segments.
        let parts: Vec<&str> = split("hello world", SegmentUnit::Words).collect();
        assert!(parts.contains(&"hello"));
        assert!(parts.contains(&"world"));
    }

    #[cfg(feature = "icu")]
    #[test]
    fn sentences_icu_empty_input_yields_no_slices() {
        let parts: Vec<&str> = split("", SegmentUnit::Sentences).collect();
        assert!(parts.is_empty());
    }

    #[cfg(feature = "icu")]
    #[test]
    fn sentences_icu_roundtrip_via_concat() {
        let input = "Hi. Bye! What? Ok.";
        let parts: Vec<&str> = split(input, SegmentUnit::Sentences).collect();
        let round: alloc::string::String = parts.concat();
        assert_eq!(round, input);
    }

    #[cfg(feature = "icu")]
    #[test]
    fn sentences_icu_finds_multiple_sentences() {
        let parts: Vec<&str> = split("Hi. Bye! What? Ok.", SegmentUnit::Sentences).collect();
        // ICU4X's SentenceSegmenter finds boundaries after each
        // terminating punctuation-plus-space. Four sentences.
        assert!(parts.len() >= 2);
    }

    // -----------------------------------------------------------------
    // LinesUax14 (feature-gated)
    // -----------------------------------------------------------------

    #[cfg(feature = "uax14-lines")]
    #[test]
    fn uax14_lines_empty_input_yields_no_slices() {
        // The empty input has one line-break at 0 (which we insert)
        // and one at text.len() = 0 (the end marker). The windows(2)
        // pair is empty → no slices.
        let parts: Vec<&str> = split("", SegmentUnit::LinesUax14).collect();
        assert!(parts.is_empty());
    }

    #[cfg(feature = "uax14-lines")]
    #[test]
    fn uax14_lines_roundtrips_via_concat() {
        // The union of the emitted slices reconstructs the input —
        // UAX #14 breaks at opportunities, not just \n.
        let input = "Hello world.\nA second sentence, with a comma.";
        let parts: Vec<&str> = split(input, SegmentUnit::LinesUax14).collect();
        let round: alloc::string::String = parts.concat();
        assert_eq!(round, input);
    }

    #[cfg(feature = "uax14-lines")]
    #[test]
    fn uax14_lines_breaks_at_hard_newline() {
        // A hard newline is a mandatory break opportunity — the
        // preceding line ends with the newline character on it.
        let parts: Vec<&str> = split("a\nb", SegmentUnit::LinesUax14).collect();
        // The reassembly is exact; every slice is non-empty.
        let round: alloc::string::String = parts.concat();
        assert_eq!(round, "a\nb");
        for p in &parts {
            assert!(!p.is_empty());
        }
    }

    // -----------------------------------------------------------------
    // Feature-gate panic paths
    // -----------------------------------------------------------------

    #[cfg(not(feature = "icu"))]
    #[test]
    #[should_panic(expected = "SegmentUnit::Graphemes requires the `icu` cargo feature")]
    fn graphemes_without_icu_panics() {
        let _ = split("hi", SegmentUnit::Graphemes).next();
    }

    #[cfg(not(feature = "uax14-lines"))]
    #[test]
    #[should_panic(expected = "SegmentUnit::LinesUax14 requires the `uax14-lines` cargo feature")]
    fn lines_uax14_without_feature_panics() {
        let _ = split("hi", SegmentUnit::LinesUax14).next();
    }

    // -----------------------------------------------------------------
    // Property tests — cross-variant invariants
    // -----------------------------------------------------------------

    #[cfg(not(target_family = "wasm"))]
    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Concatenating Bytes-split slices reproduces the input.
            #[test]
            fn bytes_roundtrip(s in ".{0,64}") {
                let parts: Vec<&str> = split(&s, SegmentUnit::Bytes).collect();
                let round: alloc::string::String = parts.concat();
                prop_assert_eq!(round, s);
            }

            /// Concatenating CodePoints-split slices reproduces the input.
            #[test]
            fn codepoints_roundtrip(s in ".{0,64}") {
                let parts: Vec<&str> = split(&s, SegmentUnit::CodePoints).collect();
                let round: alloc::string::String = parts.concat();
                prop_assert_eq!(round, s);
            }

            /// The number of CodePoints slices equals the number of
            /// scalars in the input (`str::chars().count()`).
            #[test]
            fn codepoints_count_equals_chars_count(s in ".{0,64}") {
                let parts: Vec<&str> = split(&s, SegmentUnit::CodePoints).collect();
                prop_assert_eq!(parts.len(), s.chars().count());
            }

            /// Every CodePoints slice contains exactly one `char`.
            #[test]
            fn codepoints_each_slice_is_one_scalar(s in ".{0,64}") {
                for part in split(&s, SegmentUnit::CodePoints) {
                    prop_assert_eq!(part.chars().count(), 1);
                }
            }

            /// Lines split reassembles the input by joining with `\n`.
            #[test]
            fn lines_reassemble_with_newline(s in "[^\r]{0,64}") {
                let parts: Vec<&str> = split(&s, SegmentUnit::Lines).collect();
                let joined = parts.join("\n");
                prop_assert_eq!(joined, s);
            }

            /// `split` never panics on arbitrary text for the
            /// dependency-free variants.
            #[test]
            fn dependency_free_variants_never_panic(s in ".{0,64}") {
                for _ in split(&s, SegmentUnit::Bytes) {}
                for _ in split(&s, SegmentUnit::CodePoints) {}
                for _ in split(&s, SegmentUnit::Lines) {}
            }

            /// Every Bytes slice is a valid UTF-8 substring (guaranteed
            /// by Rust `&str` slicing, but round-trip-check here to
            /// catch a hypothetical `unsafe`-block regression).
            #[test]
            fn bytes_slices_are_valid_utf8(s in ".{0,64}") {
                for part in split(&s, SegmentUnit::Bytes) {
                    // Coerce through the standard library to prove it.
                    prop_assert!(core::str::from_utf8(part.as_bytes()).is_ok());
                }
            }
        }

        // ICU-only property tests: grapheme and word round-trip via concat.
        #[cfg(feature = "icu")]
        proptest! {
            #[test]
            fn graphemes_roundtrip(s in ".{0,64}") {
                let parts: Vec<&str> = split(&s, SegmentUnit::Graphemes).collect();
                let round: alloc::string::String = parts.concat();
                prop_assert_eq!(round, s);
            }

            #[test]
            fn words_roundtrip(s in "[a-zA-Z0-9 .,!?]{0,64}") {
                let parts: Vec<&str> = split(&s, SegmentUnit::Words).collect();
                let round: alloc::string::String = parts.concat();
                prop_assert_eq!(round, s);
            }

            #[test]
            fn sentences_roundtrip(s in "[a-zA-Z0-9 .,!?\n]{0,64}") {
                let parts: Vec<&str> = split(&s, SegmentUnit::Sentences).collect();
                let round: alloc::string::String = parts.concat();
                prop_assert_eq!(round, s);
            }
        }
    }
}
