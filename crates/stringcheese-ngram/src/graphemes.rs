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
/// ## ASCII fast-path
///
/// When the input is strict ASCII and contains no carriage return
/// (`\r`), every byte is a self-contained grapheme cluster — no
/// combining marks, no ZWJ sequences, no regional indicators — so a
/// byte-window is equivalent to a grapheme-window and the ICU4X
/// UAX #29 state machine is bypassed. The `\r` guard exists because
/// UAX #29 rule GB3 keeps `CR × LF` as a single cluster; excluding
/// bare `\r` too is a minor false-negative that keeps the check to a
/// single scan and is still correct.
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn grapheme_ngrams(text: &str, n: usize) -> impl Iterator<Item = String> + '_ {
    assert!(n > 0, "n must be > 0");
    // Fast path: pure ASCII with no CR (which would combine with a
    // following LF into a single grapheme per UAX #29 GB3). Each
    // byte is its own grapheme, so byte windows == grapheme windows.
    if text.is_ascii() && !text.as_bytes().contains(&b'\r') {
        let count = text.len().checked_sub(n).map_or(0, |k| k + 1);
        GraphemeNGrams::Ascii(AsciiFast {
            text,
            n,
            i: 0,
            end: count,
        })
    } else {
        let clusters: Vec<&str> = split(text, SegmentUnit::Graphemes).collect();
        let count = clusters.len().checked_sub(n).map_or(0, |k| k + 1);
        GraphemeNGrams::Icu(IcuSlow {
            clusters,
            n,
            i: 0,
            end: count,
        })
    }
}

/// Static-dispatch iterator that unifies the ASCII fast-path with the
/// ICU4X-backed slow-path. Enum-based rather than `Box<dyn Iterator>`
/// so the UTF-8 slow-path pays no per-`next()` virtual-call overhead.
enum GraphemeNGrams<'a> {
    Ascii(AsciiFast<'a>),
    Icu(IcuSlow<'a>),
}

struct AsciiFast<'a> {
    text: &'a str,
    n: usize,
    i: usize,
    end: usize,
}

struct IcuSlow<'a> {
    clusters: Vec<&'a str>,
    n: usize,
    i: usize,
    end: usize,
}

impl Iterator for GraphemeNGrams<'_> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        match self {
            GraphemeNGrams::Ascii(s) => {
                if s.i >= s.end {
                    return None;
                }
                // Byte indexing on pure ASCII always lands on a
                // UTF-8 code-point boundary, so this slice is valid.
                let gram = s.text[s.i..s.i + s.n].to_string();
                s.i += 1;
                Some(gram)
            }
            GraphemeNGrams::Icu(s) => {
                if s.i >= s.end {
                    return None;
                }
                let gram = s.clusters[s.i..s.i + s.n].concat();
                s.i += 1;
                Some(gram)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match self {
            GraphemeNGrams::Ascii(s) => s.end - s.i,
            GraphemeNGrams::Icu(s) => s.end - s.i,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for GraphemeNGrams<'_> {}

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

    // ------------------------------------------------------------------
    // ASCII fast-path differential tests: for pure-ASCII input the
    // grapheme n-gram output must equal the byte n-gram output, byte
    // for byte and offset for offset. The equivalence follows from the
    // UAX #29 rules — every ASCII byte outside CR/LF is a self-
    // contained grapheme cluster.
    // ------------------------------------------------------------------

    use crate::byte_ngrams;

    #[test]
    fn ascii_fast_path_matches_byte_ngrams_n3() {
        let text = "the quick brown fox jumps over the lazy dog. ";
        for n in 1..=6 {
            let g: Vec<String> = grapheme_ngrams(text, n).collect();
            let b: Vec<&[u8]> = byte_ngrams(text.as_bytes(), n).collect();
            assert_eq!(g.len(), b.len(), "n={n} count mismatch");
            for (i, (gi, bi)) in g.iter().zip(b.iter()).enumerate() {
                assert_eq!(gi.as_bytes(), *bi, "n={n} gram {i} bytes mismatch");
            }
        }
    }

    #[test]
    fn ascii_fast_path_short_input() {
        // n > len — no grams either way.
        let g: Vec<String> = grapheme_ngrams("hi", 5).collect();
        let b: Vec<&[u8]> = byte_ngrams(b"hi", 5).collect();
        assert!(g.is_empty());
        assert_eq!(g.len(), b.len());
    }

    #[test]
    fn ascii_fast_path_all_ascii_bytes() {
        // Sweep every ASCII byte except CR (which combines with LF into
        // one grapheme per UAX #29 GB3 and would break the fast-path
        // equivalence).
        let ascii: String = (0..=127u8)
            .filter(|&b| b != b'\r')
            .map(char::from)
            .collect();
        for n in [1, 2, 3, 5] {
            let g: Vec<String> = grapheme_ngrams(&ascii, n).collect();
            let b: Vec<&[u8]> = byte_ngrams(ascii.as_bytes(), n).collect();
            assert_eq!(g.len(), b.len(), "n={n}");
            for (i, (gi, bi)) in g.iter().zip(b.iter()).enumerate() {
                assert_eq!(gi.as_bytes(), *bi, "n={n} gram {i}");
            }
        }
    }

    #[test]
    fn crlf_input_takes_slow_path() {
        // The fast-path is gated off CR so CRLF still splits as a
        // single grapheme cluster. Confirms the qualifier is enforced.
        let text = "a\r\nb";
        let g: Vec<String> = grapheme_ngrams(text, 2).collect();
        // Grapheme clusters: "a", "\r\n", "b" -> 2 bigrams:
        //   ["a" + "\r\n", "\r\n" + "b"].
        assert_eq!(g, vec!["a\r\n", "\r\nb"]);
    }

    #[test]
    fn bare_cr_without_lf_still_takes_slow_path() {
        // Bare CR is its own grapheme (GB4/GB5), so byte-window and
        // grapheme-window happen to coincide here — but the guard
        // conservatively routes any `\r`-bearing input through the
        // slow path, and the result must still be correct.
        let text = "a\rb";
        let g: Vec<String> = grapheme_ngrams(text, 2).collect();
        let b: Vec<&[u8]> = byte_ngrams(text.as_bytes(), 2).collect();
        assert_eq!(g.len(), b.len());
        for (gi, bi) in g.iter().zip(b.iter()) {
            assert_eq!(gi.as_bytes(), *bi);
        }
    }
}
