//! Byte n-grams — sliding window over raw `u8` slices.
//!
//! Byte-level grams are the right choice for language-agnostic
//! near-duplicate detection, binary content fingerprinting, and
//! any pipeline that already works in bytes (memchr, aho-corasick,
//! rolling-hash CDC). No UTF-8 assumption; input can be arbitrary
//! bytes.

use alloc::vec::Vec;

/// Iterator yielding every `n`-byte sliding window of `bytes` as
/// a `&[u8]` slice.
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn byte_ngrams(bytes: &[u8], n: usize) -> impl Iterator<Item = &[u8]> + '_ {
    assert!(n > 0, "n must be > 0");
    bytes.windows(n)
}

/// Padded variant — prepends `n - 1` [`SENTINEL_BYTE`] values at
/// the start and appends `n - 1` at the end. Returns owned
/// `Vec<u8>` per gram (padding forces allocation once at the
/// boundary).
///
/// # Panics
///
/// Panics on `n == 0`.
pub fn byte_ngrams_padded(bytes: &[u8], n: usize) -> impl Iterator<Item = Vec<u8>> + '_ {
    assert!(n > 0, "n must be > 0");
    let pad = n - 1;
    let padded: Vec<u8> = core::iter::repeat_n(SENTINEL_BYTE, pad)
        .chain(bytes.iter().copied())
        .chain(core::iter::repeat_n(SENTINEL_BYTE, pad))
        .collect();
    // Windows over the padded vec. Count is (len - n + 1) when
    // len >= n; 0 otherwise.
    let count = padded.len().checked_sub(n).map_or(0, |k| k + 1);
    (0..count).map(move |i| padded[i..i + n].to_vec())
}

/// Sentinel byte used by [`byte_ngrams_padded`]. NUL is the
/// conventional sentinel for byte-level grams.
pub const SENTINEL_BYTE: u8 = 0x00;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_byte_bigrams() {
        let g: Vec<&[u8]> = byte_ngrams(b"abcd", 2).collect();
        assert_eq!(g, vec![b"ab" as &[u8], b"bc", b"cd"]);
    }

    #[test]
    fn empty_when_short() {
        let g: Vec<&[u8]> = byte_ngrams(b"ab", 5).collect();
        assert!(g.is_empty());
    }

    #[test]
    fn n_equals_length() {
        let g: Vec<&[u8]> = byte_ngrams(b"ab", 2).collect();
        assert_eq!(g, vec![b"ab" as &[u8]]);
    }

    #[test]
    #[should_panic(expected = "n must be > 0")]
    fn n_zero_panics() {
        let _ = byte_ngrams(b"abc", 0).count();
    }

    #[test]
    fn padded_count_matches_pad_math() {
        // "ab" padded for n=3: 2 sentinels + 2 bytes + 2 sentinels = 6 bytes.
        // 3-grams: 6 - 3 + 1 = 4 grams.
        let g: Vec<Vec<u8>> = byte_ngrams_padded(b"ab", 3).collect();
        assert_eq!(g.len(), 4);
        // First gram is all sentinels + first real byte.
        assert_eq!(g[0], vec![SENTINEL_BYTE, SENTINEL_BYTE, b'a']);
        // Last gram is last real byte + sentinels.
        assert_eq!(g[3], vec![b'b', SENTINEL_BYTE, SENTINEL_BYTE]);
    }
}
