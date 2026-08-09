//! [`DiffUnit`] and the `&str → Iterator<&str>` splitter.
//!
//! The unit choice is explicit at the call site — same convention
//! as everywhere else in StringCheese. Bytes / code points / lines
//! never pull ICU; graphemes / words / sentences need the
//! `segment-icu` feature; UAX #14 line breaking needs
//! `segment-lines-uax14`.
//!
//! ## Implementation
//!
//! Every variant maps 1:1 to a [`stringcheese_segment::SegmentUnit`]
//! and [`split`] delegates to [`stringcheese_segment::split`]. The
//! diff crate keeps its own `DiffUnit` name — call-site ergonomics
//! read better as `DiffUnit::Lines` than `SegmentUnit::Lines` when
//! passed to `diff_at` — but the semantics live in one place.

use alloc::boxed::Box;

use stringcheese_segment::SegmentUnit;

/// The segmentation boundary a text-diff call operates at.
///
/// Passed to [`crate::diff_at`] and to [`split`]. Each variant
/// determines how the input string is split into `&str` slices
/// before the algorithm runs.
///
/// Mirror of [`stringcheese_segment::SegmentUnit`] — kept as a
/// distinct type so the diff-crate's public API doesn't force
/// downstreams to name-import from the segmentation crate.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiffUnit {
    /// Byte-level split — one slice per byte (advances past
    /// character boundaries so every returned `&str` stays valid).
    Bytes,

    /// Code-point split — one slice per Unicode scalar value.
    CodePoints,

    /// Grapheme-cluster split via ICU4X. Requires the `segment-icu`
    /// feature; a build without it panics on this variant.
    Graphemes,

    /// UAX #29 word segmentation via ICU4X. Requires `segment-icu`.
    /// Without the feature, [`split`] falls back to ASCII-whitespace
    /// splitting.
    Words,

    /// UAX #29 sentence segmentation via ICU4X. Requires
    /// `segment-icu`. Without the feature, [`split`] falls back to
    /// splitting on `. `, `? `, `! `, `\n\n`.
    Sentences,

    /// Line-level split at `\n`. The trailing `\n` on each line is
    /// dropped — the unified-diff writer re-adds it when emitting.
    /// Empty trailing lines (from strings ending in `\n`) are
    /// yielded; wrap in [`DiffUnit::LinesUax14`] for UAX #14
    /// semantics.
    Lines,

    /// UAX #14 line breaking (allows breaks at every valid line-
    /// break opportunity, not just `\n`). Requires the
    /// `segment-lines-uax14` feature.
    LinesUax14,
}

impl From<DiffUnit> for SegmentUnit {
    fn from(unit: DiffUnit) -> Self {
        match unit {
            DiffUnit::Bytes => Self::Bytes,
            DiffUnit::CodePoints => Self::CodePoints,
            DiffUnit::Graphemes => Self::Graphemes,
            DiffUnit::Words => Self::Words,
            DiffUnit::Sentences => Self::Sentences,
            DiffUnit::Lines => Self::Lines,
            DiffUnit::LinesUax14 => Self::LinesUax14,
        }
    }
}

/// Split `text` into `&str` slices per `unit`. Returns a boxed
/// iterator so the return type is the same across every variant
/// and feature-gated iterator implementation.
///
/// # Panics
///
/// Panics when a variant requires a Cargo feature that's disabled
/// in this build (e.g. [`DiffUnit::Graphemes`] without
/// `segment-icu`). This is a build-time error, not a runtime input
/// bug — the caller and the crate features go together, so a panic
/// is the correct discipline.
#[must_use]
pub fn split(text: &str, unit: DiffUnit) -> Box<dyn Iterator<Item = &str> + '_> {
    stringcheese_segment::split(text, unit.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn bytes_split_ascii() {
        let parts: Vec<&str> = split("abc", DiffUnit::Bytes).collect();
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn codepoints_split_multibyte() {
        // ü is 2 bytes, 日 is 3 bytes — one slice per scalar.
        let parts: Vec<&str> = split("aü日", DiffUnit::CodePoints).collect();
        assert_eq!(parts, vec!["a", "ü", "日"]);
    }

    #[test]
    fn lines_split_drops_newline() {
        let parts: Vec<&str> = split("one\ntwo\nthree", DiffUnit::Lines).collect();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn lines_trailing_newline_yields_empty_last() {
        // `split('\n')` semantics — a trailing `\n` produces an
        // empty final slice, which callers see and handle.
        let parts: Vec<&str> = split("one\n", DiffUnit::Lines).collect();
        assert_eq!(parts, vec!["one", ""]);
    }

    #[cfg(not(feature = "segment-icu"))]
    #[test]
    fn words_ascii_fallback() {
        let parts: Vec<&str> = split("the  quick brown  fox", DiffUnit::Words).collect();
        assert_eq!(parts, vec!["the", "quick", "brown", "fox"]);
    }

    #[cfg(not(feature = "segment-icu"))]
    #[test]
    fn sentences_naive_split() {
        let parts: Vec<&str> = split("Hi. Bye! What? Ok.", DiffUnit::Sentences).collect();
        assert_eq!(parts.len(), 4);
    }
}
