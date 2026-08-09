//! Character n-grams — sliding window over Unicode code points.
//!
//! Grams are `&str` slices of the input; no copies. Both variants
//! yield `n`-code-point windows.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Iterator yielding every `n`-code-point sliding window of `text`
/// as a `&str` slice.
///
/// # Panics
///
/// Panics on `n == 0` — a zero-width gram has no useful semantics.
///
/// Empty when `text` contains fewer than `n` code points.
///
/// # Example
///
/// ```
/// use stringcheese_ngram::char_ngrams;
///
/// let g: Vec<&str> = char_ngrams("héllo", 2).collect();
/// // Each gram is 2 code points (variable byte-width).
/// assert_eq!(g, vec!["hé", "él", "ll", "lo"]);
/// ```
pub fn char_ngrams(text: &str, n: usize) -> CharNgrams<'_> {
    assert!(n > 0, "n must be > 0");
    // Collect char boundaries once. `text.char_indices()` yields the
    // byte offset of each code point; append `text.len()` so
    // window[i..i+n] slices cleanly.
    let mut boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    boundaries.push(text.len());
    CharNgrams {
        text,
        boundaries,
        i: 0,
        n,
    }
}

/// Padded variant of [`char_ngrams`] — prepends `n - 1` U+FEFF
/// sentinels at the start and appends `n - 1` at the end.
///
/// Returns an owned `String` per gram (padding forces allocation
/// once at the boundary; the middle of the sequence still borrows
/// from `text` internally).
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn char_ngrams_padded(text: &str, n: usize) -> impl Iterator<Item = String> + '_ {
    assert!(n > 0, "n must be > 0");
    // Build the padded string once; iterate its char-grams.
    let pad = core::iter::repeat_n(SENTINEL_CHAR, n - 1).collect::<String>();
    let padded: String = alloc::format!("{pad}{text}{pad}");
    let boundaries: Vec<usize> = padded
        .char_indices()
        .map(|(i, _)| i)
        .chain(core::iter::once(padded.len()))
        .collect();
    // (num_chars + 1) boundary entries — a valid n-gram uses n+1
    // consecutive boundary entries, so the count of grams is
    // max(boundaries.len() - n, 0).
    let count = boundaries.len().saturating_sub(n);
    (0..count).map(move |i| padded[boundaries[i]..boundaries[i + n]].to_string())
}

/// Sentinel code point used by [`char_ngrams_padded`]. U+FEFF
/// (BOM) is unassigned in normal text bodies and won't clash with
/// real content.
pub const SENTINEL_CHAR: char = '\u{FEFF}';

/// Iterator returned by [`char_ngrams`].
#[derive(Clone, Debug)]
pub struct CharNgrams<'a> {
    text: &'a str,
    boundaries: Vec<usize>,
    i: usize,
    n: usize,
}

impl<'a> Iterator for CharNgrams<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        // boundaries has (num_chars + 1) entries; a window of n
        // characters uses boundaries[i]..boundaries[i + n].
        if self.i + self.n >= self.boundaries.len() {
            return None;
        }
        let start = self.boundaries[self.i];
        let end = self.boundaries[self.i + self.n];
        self.i += 1;
        Some(&self.text[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_trigrams() {
        let g: Vec<&str> = char_ngrams("hello", 3).collect();
        assert_eq!(g, vec!["hel", "ell", "llo"]);
    }

    #[test]
    fn multibyte_grams_slice_correctly() {
        // "héllo" — é is 2 bytes. Bigrams should be 2-scalar
        // windows regardless of byte width.
        let g: Vec<&str> = char_ngrams("héllo", 2).collect();
        assert_eq!(g, vec!["hé", "él", "ll", "lo"]);
    }

    #[test]
    fn empty_when_input_shorter_than_n() {
        let g: Vec<&str> = char_ngrams("hi", 5).collect();
        assert!(g.is_empty());
    }

    #[test]
    fn n_equals_input_length_yields_one_gram() {
        let g: Vec<&str> = char_ngrams("hi", 2).collect();
        assert_eq!(g, vec!["hi"]);
    }

    #[test]
    fn empty_string_yields_nothing() {
        let g: Vec<&str> = char_ngrams("", 1).collect();
        assert!(g.is_empty());
    }

    #[test]
    #[should_panic(expected = "n must be > 0")]
    fn n_zero_panics() {
        let _ = char_ngrams("hi", 0);
    }

    #[test]
    fn padded_boundary_count() {
        // "hi" with n=3 padded: prepend 2 sentinels, append 2.
        // Padded string is `SEN SEN h i SEN SEN` = 6 chars.
        // 3-grams from 6-char string: 6 - 3 + 1 = 4 grams.
        let g: Vec<String> = char_ngrams_padded("hi", 3).collect();
        assert_eq!(g.len(), 4);
    }
}
