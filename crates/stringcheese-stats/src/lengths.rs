//! Byte / code-point / grapheme length triple.
//!
//! The three counts a string has, all in one struct so callers can
//! ask any of them without three separate scans. Grapheme count is
//! `None` unless the `graphemes` feature is on — it's the only one
//! that requires ICU4X (via the `stringcheese-segment` crate).
//!
//! ## Why three counts
//!
//! Every count answers a different real question:
//!
//! - **Bytes** — buffer sizing, offset math, network payload
//!   budgeting. What Rust's `str::len` gives you.
//! - **Code points** — how many "characters" the standard library
//!   would name; what `chars().count()` gives you.
//! - **Graphemes** — how many "characters" a *human* would count.
//!   `"👨‍👩‍👧‍👦"` is one grapheme, seven scalars, twenty-five bytes.
//!
//! Silently reporting one when a caller means another is a bug
//! source in every language. Explicit fields defuse it.

/// Byte / code-point / grapheme lengths of a string.
///
/// [`Self::graphemes`] is `None` when the crate was built without
/// the `graphemes` feature; the other two are always present.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Lengths {
    /// UTF-8 byte length — what `str::len` returns.
    pub bytes: usize,
    /// Number of Unicode code points (scalars) —
    /// what `chars().count()` returns.
    pub code_points: usize,
    /// Number of extended grapheme clusters (UAX #29) —
    /// `None` when this build doesn't include ICU4X. Behind the
    /// `graphemes` feature.
    pub graphemes: Option<usize>,
}

impl Lengths {
    /// Compute every length for `text` in one pass.
    ///
    /// [`Self::graphemes`] is populated only when the crate's
    /// `graphemes` feature is enabled; otherwise it's `None`.
    #[must_use]
    pub fn of(text: &str) -> Self {
        let bytes = text.len();
        let code_points = text.chars().count();
        let graphemes = grapheme_count(text);
        Self {
            bytes,
            code_points,
            graphemes,
        }
    }
}

#[cfg(feature = "graphemes")]
#[allow(
    clippy::unnecessary_wraps,
    reason = "return type must match the feature-disabled variant, which returns None"
)]
fn grapheme_count(text: &str) -> Option<usize> {
    // Segment crate returns an iterator of `&str` grapheme slices;
    // count them without materialising a Vec.
    Some(stringcheese_segment::split(text, stringcheese_segment::SegmentUnit::Graphemes).count())
}

#[cfg(not(feature = "graphemes"))]
fn grapheme_count(_text: &str) -> Option<usize> {
    // Grapheme count needs ICU4X; not compiled in for this build.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_bytes_and_scalars_match() {
        let l = Lengths::of("hello");
        assert_eq!(l.bytes, 5);
        assert_eq!(l.code_points, 5);
    }

    #[test]
    fn multibyte_scalars_widen_bytes_not_code_points() {
        // 日本語 — 3 scalars, 9 bytes.
        let l = Lengths::of("日本語");
        assert_eq!(l.bytes, 9);
        assert_eq!(l.code_points, 3);
    }

    #[test]
    fn empty_string_all_zero() {
        let l = Lengths::of("");
        assert_eq!(l.bytes, 0);
        assert_eq!(l.code_points, 0);
    }

    #[cfg(not(feature = "graphemes"))]
    #[test]
    fn grapheme_field_is_none_without_feature() {
        let l = Lengths::of("hello");
        assert_eq!(l.graphemes, None);
    }

    #[cfg(feature = "graphemes")]
    #[test]
    fn grapheme_family_emoji_is_one_grapheme() {
        // ZWJ family emoji — one grapheme, multiple scalars.
        let l = Lengths::of("👨‍👩‍👧‍👦");
        assert_eq!(l.graphemes, Some(1));
        assert!(l.code_points > 1);
        assert!(l.bytes > l.code_points);
    }
}
