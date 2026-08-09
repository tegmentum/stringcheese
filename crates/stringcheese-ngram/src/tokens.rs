//! Token n-grams (shingles) — sliding window over a `&[&str]` token
//! list.
//!
//! The caller supplies the tokenisation (word split, [`str::split`],
//! `WordPiece`, whatever) — this crate slides the window. Grams are
//! `&[&str]` sub-slices of the input; no copies for the unpadded
//! variant.

use alloc::vec::Vec;

/// Iterator yielding every `n`-token sliding window as a
/// `&[&str]` sub-slice.
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn token_ngrams<'a>(
    tokens: &'a [&'a str],
    n: usize,
) -> impl Iterator<Item = &'a [&'a str]> + 'a {
    assert!(n > 0, "n must be > 0");
    tokens.windows(n)
}

/// Padded variant — prepends `n - 1` empty-string sentinels at the
/// start and appends `n - 1` at the end. Returns owned
/// `Vec<&str>` per gram (padding requires an owned vec at the
/// boundaries).
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn token_ngrams_padded<'a>(
    tokens: &'a [&'a str],
    n: usize,
) -> impl Iterator<Item = Vec<&'a str>> + 'a {
    assert!(n > 0, "n must be > 0");
    let pad = n - 1;
    let padded: Vec<&'a str> = core::iter::repeat_n(SENTINEL_TOKEN, pad)
        .chain(tokens.iter().copied())
        .chain(core::iter::repeat_n(SENTINEL_TOKEN, pad))
        .collect();
    let count = padded.len().checked_sub(n).map_or(0, |k| k + 1);
    (0..count).map(move |i| padded[i..i + n].to_vec())
}

/// Sentinel token used by [`token_ngrams_padded`]. The empty
/// string is a natural sentinel — it never appears in a well-formed
/// token stream.
pub const SENTINEL_TOKEN: &str = "";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_trigrams() {
        let toks = ["the", "quick", "brown", "fox", "jumps"];
        let g: Vec<&[&str]> = token_ngrams(&toks, 3).collect();
        assert_eq!(g.len(), 3);
        assert_eq!(g[0], &["the", "quick", "brown"]);
        assert_eq!(g[1], &["quick", "brown", "fox"]);
        assert_eq!(g[2], &["brown", "fox", "jumps"]);
    }

    #[test]
    fn empty_when_short() {
        let toks = ["a", "b"];
        let g: Vec<&[&str]> = token_ngrams(&toks, 5).collect();
        assert!(g.is_empty());
    }

    #[test]
    #[should_panic(expected = "n must be > 0")]
    fn n_zero_panics() {
        let toks = ["a"];
        let _ = token_ngrams(&toks, 0).count();
    }

    #[test]
    fn padded_boundary_carries_sentinels() {
        let toks = ["a", "b"];
        let g: Vec<Vec<&str>> = token_ngrams_padded(&toks, 3).collect();
        // 2 tokens + 2 sentinels each side = 6, 3-grams => 4 grams.
        assert_eq!(g.len(), 4);
        assert_eq!(g[0], vec![SENTINEL_TOKEN, SENTINEL_TOKEN, "a"]);
        assert_eq!(g[3], vec!["b", SENTINEL_TOKEN, SENTINEL_TOKEN]);
    }
}
