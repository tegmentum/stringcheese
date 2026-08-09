//! Grapheme-cluster n-grams — sliding window over what a human
//! would count as one character.
//!
//! Requires the `graphemes` feature. Delegates to
//! [`stringcheese_segment`] for the UAX #29 grapheme segmentation,
//! then slides an `n`-cluster window across the result.
//!
//! Useful when a code-point-level window would over-count
//! multi-scalar clusters — the family-emoji `"👨‍👩‍👧‍👦"` is ONE
//! grapheme but SEVEN code points; a code-point 3-gram over
//! `"👨‍👩‍👧‍👦x"` would slice the emoji apart mid-sequence.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use stringcheese_segment::{SegmentUnit, split};

/// Iterator yielding every `n`-grapheme sliding window as an owned
/// `String`.
///
/// Grapheme-cluster boundaries can span multiple scalars, so the
/// concatenated gram doesn't share bytes with the original input in
/// a stable way when the input's grapheme structure isn't purely
/// scalar-aligned. Owned strings sidestep the lifetime headache.
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn grapheme_ngrams(text: &str, n: usize) -> impl Iterator<Item = String> + '_ {
    assert!(n > 0, "n must be > 0");
    let clusters: Vec<&str> = split(text, SegmentUnit::Graphemes).collect();
    let count = clusters.len().checked_sub(n).map_or(0, |k| k + 1);
    (0..count).map(move |i| clusters[i..i + n].concat())
}

/// Padded variant — prepends `n - 1` [`SENTINEL_STR`] sentinels at
/// the start and appends `n - 1` at the end.
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn grapheme_ngrams_padded(text: &str, n: usize) -> impl Iterator<Item = String> + '_ {
    assert!(n > 0, "n must be > 0");
    let clusters: Vec<&str> = split(text, SegmentUnit::Graphemes).collect();
    let pad = n - 1;
    let padded: Vec<&str> = core::iter::repeat_n(SENTINEL_STR, pad)
        .chain(clusters)
        .chain(core::iter::repeat_n(SENTINEL_STR, pad))
        .collect();
    let count = padded.len().checked_sub(n).map_or(0, |k| k + 1);
    (0..count).map(move |i| padded[i..i + n].concat())
}

/// Sentinel string used by [`grapheme_ngrams_padded`]. Uses U+FEFF
/// (BOM) — same choice as [`crate::chars::SENTINEL_CHAR`].
pub const SENTINEL_STR: &str = "\u{FEFF}";

/// The [`char`] form of [`SENTINEL_STR`].
pub fn sentinel_char() -> String {
    SENTINEL_STR.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_grapheme_grams_match_char_grams() {
        // No combining marks — one grapheme per code point.
        let g: Vec<String> = grapheme_ngrams("hello", 3).collect();
        assert_eq!(g, vec!["hel", "ell", "llo"]);
    }

    #[test]
    fn family_emoji_counts_as_one_grapheme() {
        // Family-emoji ZWJ sequence — one grapheme, several scalars.
        // Bigrams over "👨‍👩‍👧‍👦xy" should produce two grams:
        //   [family, x] and [x, y].
        let g: Vec<String> = grapheme_ngrams("👨\u{200D}👩\u{200D}👧\u{200D}👦xy", 2).collect();
        assert_eq!(g.len(), 2);
        // First gram carries the whole emoji plus 'x'.
        assert!(g[0].ends_with('x'));
        assert_eq!(g[1], "xy");
    }

    #[test]
    fn empty_when_short() {
        let g: Vec<String> = grapheme_ngrams("hi", 5).collect();
        assert!(g.is_empty());
    }

    #[test]
    #[should_panic(expected = "n must be > 0")]
    fn n_zero_panics() {
        let _ = grapheme_ngrams("hi", 0).count();
    }
}
