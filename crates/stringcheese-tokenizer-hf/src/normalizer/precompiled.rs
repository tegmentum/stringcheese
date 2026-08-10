//! `SentencePiece`'s "Precompiled" charsmap normalizer.
//!
//! Every Llama, Mistral, T5, and XLM-`RoBERTa` `tokenizer.json` ships a
//! base64-encoded charsmap under `Precompiled.precompiled_charsmap`.
//! The charsmap is `SentencePiece`'s per-checkpoint character-remapping
//! table: a Darts / `DoubleArray` trie over UTF-8 byte prefixes plus a
//! flat table of replacement strings. Normalization walks the input
//! byte-for-byte, longest-prefix-matches against the trie at every
//! position, and emits the matched leaf's replacement string;
//! positions with no match consume one UTF-8 scalar verbatim.
//!
//! # Wire format
//!
//! ```text
//! [trie_size: u32 LE]                       // trie size in bytes
//! [trie: trie_size / 4 entries of u32 LE]   // Darts double array
//! [normalized: rest of buffer]              // NUL-separated UTF-8 strings
//! ```
//!
//! `normalized` is a concatenation of NUL-terminated UTF-8 replacement
//! strings addressed by byte offset from its start (the offset is what
//! the trie's leaves carry).
//!
//! # Darts double-array node encoding
//!
//! Each `u32` unit encodes either a transition or a leaf value. For a
//! transition unit:
//!
//! * bit  8       — `has_leaf` flag
//! * bit  9       — offset-length selector (0 => 8-bit shift, 1 => 16-bit shift)
//! * bits 10..31  — packed offset (shifted left per bit 9)
//! * bits  0..7   — label byte
//!
//! For a leaf value unit: `value = unit & 0x7FFF_FFFF` (the top bit is
//! the value marker set by the compiler).
//!
//! # Attribution
//!
//! The algorithm mirrors Google's
//! [`sentencepiece`](https://github.com/google/sentencepiece)
//! (`src/normalizer.cc`, Apache-2.0) and Hugging Face's
//! [`spm_precompiled`](https://github.com/huggingface/spm_precompiled)
//! plus the Rust normalizer at
//! `tokenizers/src/normalizers/precompiled.rs` (Apache-2.0). This
//! module was written from the same public spec — no source was
//! vendored.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::str;

// ---------------------------------------------------------------------
// Public surface.
// ---------------------------------------------------------------------

/// A parsed, ready-to-apply `SentencePiece` Precompiled normalizer.
///
/// Construct with [`Self::from_base64_charsmap`] then call
/// [`Self::normalize`] to apply the char-mapping table to arbitrary
/// UTF-8 input. Parsing decodes the base64 payload, splits it into a
/// Darts double-array trie and a NUL-separated replacement-string
/// table, and stores both. The struct is cheap to clone (both halves
/// are owned heap slices) and has no interior mutability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecompiledNormalizer {
    /// The Darts double-array trie, one `u32` per entry.
    trie: Vec<u32>,
    /// The concatenated NUL-terminated UTF-8 replacement strings.
    /// Leaf values are byte offsets into this buffer.
    normalized: Vec<u8>,
}

/// Error returned by [`PrecompiledNormalizer::from_base64_charsmap`]
/// when the base64 payload is malformed or the decoded blob does not
/// conform to the `SentencePiece` charsmap wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrecompiledError {
    /// The base64 string contained a byte outside the standard
    /// alphabet (`A-Z`, `a-z`, `0-9`, `+`, `/`, `=`) after whitespace
    /// stripping, or its length was not a multiple of 4 after
    /// padding.
    InvalidBase64,
    /// The decoded blob was too short to carry the 4-byte trie-size
    /// header.
    TruncatedHeader,
    /// The header's trie size was not a multiple of 4 (a `u32`
    /// double-array cannot be reconstructed).
    TrieSizeNotAligned {
        /// The trie size read from the header, in bytes.
        trie_bytes: u32,
    },
    /// The header's trie size ran past the end of the decoded blob.
    TrieOverrun {
        /// The trie size read from the header, in bytes.
        trie_bytes: u32,
        /// The total number of decoded bytes.
        blob_bytes: usize,
    },
}

impl fmt::Display for PrecompiledError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase64 => {
                f.write_str("Precompiled charsmap: base64 payload contains an invalid byte")
            }
            Self::TruncatedHeader => f.write_str(
                "Precompiled charsmap: decoded blob is shorter than the 4-byte trie-size header",
            ),
            Self::TrieSizeNotAligned { trie_bytes } => write!(
                f,
                "Precompiled charsmap: trie size {trie_bytes} is not a multiple of 4"
            ),
            Self::TrieOverrun {
                trie_bytes,
                blob_bytes,
            } => write!(
                f,
                "Precompiled charsmap: trie size {trie_bytes} overruns the {blob_bytes}-byte blob"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PrecompiledError {}

impl PrecompiledNormalizer {
    /// Decode a base64-encoded charsmap and parse it into a runnable
    /// normalizer.
    ///
    /// The payload is the exact string stored under
    /// `Precompiled.precompiled_charsmap` in a Hugging Face
    /// `tokenizer.json`. Standard base64 alphabet with optional `=`
    /// padding; whitespace is skipped.
    ///
    /// # Errors
    ///
    /// Returns [`PrecompiledError`] variants describing the failure
    /// mode — see each variant's doc for the exact trigger.
    pub fn from_base64_charsmap(charsmap_base64: &str) -> Result<Self, PrecompiledError> {
        let blob = decode_base64(charsmap_base64)?;
        Self::from_blob(&blob)
    }

    /// Parse a raw (already-base64-decoded) charsmap blob.
    ///
    /// Exposed for callers that store the decoded bytes directly.
    ///
    /// # Errors
    ///
    /// Same as [`Self::from_base64_charsmap`], minus
    /// [`PrecompiledError::InvalidBase64`].
    pub fn from_blob(blob: &[u8]) -> Result<Self, PrecompiledError> {
        if blob.len() < 4 {
            return Err(PrecompiledError::TruncatedHeader);
        }
        let trie_bytes = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]);
        if !trie_bytes.is_multiple_of(4) {
            return Err(PrecompiledError::TrieSizeNotAligned { trie_bytes });
        }
        let trie_end =
            4usize
                .checked_add(trie_bytes as usize)
                .ok_or(PrecompiledError::TrieOverrun {
                    trie_bytes,
                    blob_bytes: blob.len(),
                })?;
        if trie_end > blob.len() {
            return Err(PrecompiledError::TrieOverrun {
                trie_bytes,
                blob_bytes: blob.len(),
            });
        }
        let mut trie = Vec::with_capacity((trie_bytes / 4) as usize);
        for chunk in blob[4..trie_end].chunks_exact(4) {
            trie.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        let normalized = blob[trie_end..].to_vec();
        Ok(Self { trie, normalized })
    }

    /// Apply the charsmap to `input` and return the normalized string.
    ///
    /// The algorithm walks `input`'s UTF-8 bytes left to right; at
    /// each position it does a longest-prefix Darts lookup and emits
    /// the matched leaf's NUL-terminated replacement string (advancing
    /// past the matched bytes). Positions with no match emit the
    /// current UTF-8 scalar verbatim.
    ///
    /// If either the trie is empty or the input has no matches, the
    /// output is byte-for-byte the input (validated as UTF-8 by
    /// construction).
    #[must_use]
    pub fn normalize(&self, input: &str) -> String {
        // The average normalized-output length is very close to the
        // input's; pre-allocate the same capacity to avoid the first
        // few doublings.
        let mut out = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let mut pos = 0;
        while pos < bytes.len() {
            let remaining = &bytes[pos..];
            // Longest-match Darts lookup at `pos`.
            let matched = self.longest_prefix_match(remaining);
            if let Some((leaf_value, match_len)) = matched {
                if let Some(replacement) = self.leaf_string(leaf_value) {
                    out.push_str(replacement);
                    pos += match_len;
                    continue;
                }
                // Malformed leaf offset — fall through to the
                // pass-one-scalar branch so the input is preserved.
            }
            // No matching prefix (or a leaf that pointed at a
            // non-UTF-8 or out-of-range replacement). Consume one
            // UTF-8 scalar verbatim.
            let scalar_len = utf8_scalar_len(bytes[pos]);
            let end = (pos + scalar_len).min(bytes.len());
            match str::from_utf8(&bytes[pos..end]) {
                Ok(s) => out.push_str(s),
                Err(_) => {
                    // The input was validated as UTF-8 at the `&str`
                    // boundary; this arm is unreachable for well-formed
                    // callers. Emit U+FFFD to keep the loop finite and
                    // the output valid UTF-8 in case the invariant is
                    // ever violated by a lead-byte / continuation
                    // mismatch on the boundary.
                    out.push('\u{FFFD}');
                }
            }
            // Advance at least one byte so a pathological lead byte
            // cannot cause an infinite loop.
            pos = end.max(pos + 1);
        }
        out
    }

    /// Read the NUL-terminated replacement string at `offset` in the
    /// normalized table, or `None` if the offset is out of range or
    /// the string is not valid UTF-8.
    fn leaf_string(&self, offset: u32) -> Option<&str> {
        let start = offset as usize;
        if start >= self.normalized.len() {
            return None;
        }
        let mut end = start;
        while end < self.normalized.len() && self.normalized[end] != 0 {
            end += 1;
        }
        str::from_utf8(&self.normalized[start..end]).ok()
    }

    /// Longest-prefix Darts match starting at `key[0]`.
    ///
    /// Returns `Some((leaf_value, matched_byte_len))` for the deepest
    /// leaf reached, or `None` if no prefix matched.
    fn longest_prefix_match(&self, key: &[u8]) -> Option<(u32, usize)> {
        if self.trie.is_empty() {
            return None;
        }
        // Darts common_prefix_search. Iteration follows the Yata
        // double-array walk: at each byte, XOR-descend by the label
        // and by the packed offset; if `label` disagrees with the
        // input byte, the trie has no further match.
        let mut node_pos = 0usize;
        let mut unit = *self.trie.first()?;
        node_pos ^= darts_offset(unit) as usize;
        let mut best: Option<(u32, usize)> = None;
        for (i, &c) in key.iter().enumerate() {
            if c == 0 {
                // NUL terminates a Darts key; matches Google's C++
                // reference behaviour.
                break;
            }
            node_pos ^= c as usize;
            let Some(&next_unit) = self.trie.get(node_pos) else {
                break;
            };
            unit = next_unit;
            if darts_label(unit) != u32::from(c) {
                break;
            }
            node_pos ^= darts_offset(unit) as usize;
            if darts_has_leaf(unit) {
                let Some(&leaf_unit) = self.trie.get(node_pos) else {
                    break;
                };
                best = Some((darts_value(leaf_unit), i + 1));
            }
        }
        best
    }
}

// ---------------------------------------------------------------------
// Darts / `DoubleArray` bit helpers.
// ---------------------------------------------------------------------

/// `has_leaf` — bit 8. Set when the transition unit's associated
/// slot in the array carries a leaf value at `node_pos ^ offset`.
#[inline]
fn darts_has_leaf(unit: u32) -> bool {
    (unit >> 8) & 1 == 1
}

/// `value` — the low 31 bits of a leaf unit. The top bit is the
/// value marker set by the compiler and stripped before use.
#[inline]
fn darts_value(unit: u32) -> u32 {
    unit & 0x7FFF_FFFF
}

/// `label` — the low 8 bits of a transition unit combined with the
/// top-bit end-of-key marker. Compared against the input byte.
#[inline]
fn darts_label(unit: u32) -> u32 {
    unit & (0x8000_0000 | 0xFF)
}

/// `offset` — the packed transition offset. Bit 9 selects an
/// 8-bit or a 16-bit shift; the shifted value is XOR-ed into
/// `node_pos` to reach the child slot.
#[inline]
fn darts_offset(unit: u32) -> u32 {
    (unit >> 10) << ((unit & (1 << 9)) >> 6)
}

// ---------------------------------------------------------------------
// UTF-8 helpers.
// ---------------------------------------------------------------------

/// Number of bytes in the UTF-8 sequence starting with `lead`. Falls
/// back to 1 for invalid lead bytes so a malformed input still makes
/// progress.
#[inline]
fn utf8_scalar_len(lead: u8) -> usize {
    if lead < 0x80 {
        1
    } else if lead < 0xC0 {
        // Continuation byte on its own — advance by one so the outer
        // loop keeps progressing.
        1
    } else if lead < 0xE0 {
        2
    } else if lead < 0xF0 {
        3
    } else {
        4
    }
}

// ---------------------------------------------------------------------
// Minimal base64 decoder.
// ---------------------------------------------------------------------
//
// Standard alphabet, optional `=` padding, whitespace tolerant.
// Deliberately small — the alternative is pulling in the `base64`
// crate, which would only be used by this one code path.

fn decode_base64(input: &str) -> Result<Vec<u8>, PrecompiledError> {
    // Collect alphabet bytes, dropping ASCII whitespace. Non-ASCII
    // bytes are always invalid — the base64 alphabet is 7-bit.
    let mut bytes = Vec::with_capacity(input.len());
    for &b in input.as_bytes() {
        if !matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
            bytes.push(b);
        }
    }
    // Strip trailing '=' padding (0, 1, or 2 of them).
    let mut padding = 0usize;
    while bytes.last() == Some(&b'=') && padding < 2 {
        bytes.pop();
        padding += 1;
    }
    // After stripping padding, no further '=' may appear.
    if bytes.contains(&b'=') {
        return Err(PrecompiledError::InvalidBase64);
    }
    let mut out = Vec::with_capacity((bytes.len() * 3) / 4 + 3);
    let mut accum: u32 = 0;
    let mut nbits: u32 = 0;
    for &b in &bytes {
        let v = base64_value(b)?;
        accum = (accum << 6) | u32::from(v);
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push(((accum >> nbits) & 0xFF) as u8);
        }
    }
    // Any leftover bits must be zero — otherwise the input was
    // truncated mid-byte.
    if nbits > 0 && (accum & ((1u32 << nbits) - 1)) != 0 {
        return Err(PrecompiledError::InvalidBase64);
    }
    Ok(out)
}

#[inline]
fn base64_value(b: u8) -> Result<u8, PrecompiledError> {
    match b {
        b'A'..=b'Z' => Ok(b - b'A'),
        b'a'..=b'z' => Ok(b - b'a' + 26),
        b'0'..=b'9' => Ok(b - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(PrecompiledError::InvalidBase64),
    }
}

// ---------------------------------------------------------------------
// Charsmap builder — internal use only, but exposed to the module's
// tests so we can craft fixtures without checking in binary blobs.
// ---------------------------------------------------------------------

/// Build a base64-encoded charsmap from an in-memory Darts trie +
/// normalized-string table. This is the exact inverse of the
/// wire-format decode and is used to build test fixtures.
#[cfg(test)]
pub(crate) fn encode_charsmap_for_tests(trie: &[u32], normalized: &[u8]) -> String {
    let mut blob = Vec::with_capacity(4 + trie.len() * 4 + normalized.len());
    let trie_bytes = u32::try_from(trie.len() * 4).expect("trie fits in u32");
    blob.extend_from_slice(&trie_bytes.to_le_bytes());
    for &u in trie {
        blob.extend_from_slice(&u.to_le_bytes());
    }
    blob.extend_from_slice(normalized);
    encode_base64(&blob)
}

#[cfg(test)]
fn encode_base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let chunks = data.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() >= 2 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() >= 3 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ---------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    // ---------------------------------------------------------------
    // Base64.
    // ---------------------------------------------------------------

    #[test]
    fn base64_roundtrips_arbitrary_bytes() {
        for input in [
            &b""[..],
            b"a",
            b"ab",
            b"abc",
            b"abcd",
            b"abcde",
            b"\x00\x01\x02\x03\xff\xfe\xfd",
        ] {
            let encoded = encode_base64(input);
            let decoded = decode_base64(&encoded).unwrap();
            assert_eq!(decoded, input, "round-trip failed for {input:?}");
        }
    }

    #[test]
    fn base64_tolerates_whitespace_between_groups() {
        // Encoding of "Hello" — a real charsmap has no line breaks
        // but we tolerate them defensively.
        let encoded = "SGVs\nbG8=";
        let decoded = decode_base64(encoded).unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn base64_rejects_invalid_character() {
        assert!(matches!(
            decode_base64("!!!!"),
            Err(PrecompiledError::InvalidBase64)
        ));
    }

    #[test]
    fn base64_rejects_padding_in_middle() {
        assert!(matches!(
            decode_base64("AB=CD"),
            Err(PrecompiledError::InvalidBase64)
        ));
    }

    // ---------------------------------------------------------------
    // Wire format edge cases.
    // ---------------------------------------------------------------

    #[test]
    fn from_blob_rejects_truncated_header() {
        assert!(matches!(
            PrecompiledNormalizer::from_blob(&[0u8; 3]),
            Err(PrecompiledError::TruncatedHeader)
        ));
    }

    #[test]
    fn from_blob_rejects_unaligned_trie_size() {
        // Header says trie is 5 bytes — not a multiple of 4.
        let blob = [5u8, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(matches!(
            PrecompiledNormalizer::from_blob(&blob),
            Err(PrecompiledError::TrieSizeNotAligned { trie_bytes: 5 })
        ));
    }

    #[test]
    fn from_blob_rejects_trie_overrun() {
        // Header says trie is 16 bytes but only 4 bytes follow.
        let blob = [16u8, 0, 0, 0, 0, 0, 0, 0];
        assert!(matches!(
            PrecompiledNormalizer::from_blob(&blob),
            Err(PrecompiledError::TrieOverrun { .. })
        ));
    }

    #[test]
    fn from_blob_accepts_empty_trie() {
        // trie_size == 0; the normalized table can carry anything or
        // nothing. An empty trie has no leaves so normalize is a
        // no-op on any input.
        let blob = [0u8, 0, 0, 0];
        let n = PrecompiledNormalizer::from_blob(&blob).unwrap();
        assert!(n.trie.is_empty());
        assert!(n.normalized.is_empty());
        assert_eq!(n.normalize("hello"), "hello");
    }

    // ---------------------------------------------------------------
    // Hand-crafted trie: single-byte 'a' -> "AB".
    //
    // The Darts encoding used by SentencePiece is Yata's compact
    // double array. We can construct the smallest possible trie
    // that maps the key "a" (0x61) to leaf value 0 (offset 0 into
    // the normalized table) as follows:
    //
    //   trie[0]: root unit. label=0, offset=<X>, has_leaf=0.
    //   trie[X ^ 0x61]: transition on 'a'. label=0x61, offset=<Y>,
    //                    has_leaf=1.
    //   trie[X ^ 0x61 ^ Y]: leaf unit. value=0 (offset into normalized).
    //
    // We pick X=1 and Y=1 to keep everything on the first few slots:
    //   root = 0 (label=0, offset=1<<10 = ... actually let's compute)
    //
    // With bit 9 clear, offset shift = 0, so encoded_offset = offset << 10.
    // For offset=1, encoded_offset << 0 = 1 << 10.
    //
    // root unit = (1 << 10) | 0 | 0 = 0x400. offset=1, label=0, has_leaf=0.
    // node_pos = 0 ^ 1 = 1.
    // On 'a' (0x61): node_pos = 1 ^ 0x61 = 0x60 = 96.
    //
    // trie[96]: label=0x61, has_leaf=1, offset=1.
    //   unit = (1 << 10) | (1 << 8) | 0x61 = 0x400 | 0x100 | 0x61 = 0x561.
    // node_pos = 96 ^ 1 = 97.
    //
    // trie[97]: leaf value 0. Top bit is the value marker.
    //   unit = 0x8000_0000 | 0 = 0x8000_0000.
    //
    // normalized: "AB\0" — offset 0 gives "AB".
    // ---------------------------------------------------------------

    #[test]
    fn hand_crafted_single_byte_key_maps_to_replacement() {
        let mut trie = vec![0u32; 98];
        trie[0] = 0x400; // root: offset=1
        trie[96] = 0x561; // 'a' transition: label=0x61, has_leaf, offset=1
        trie[97] = 0x8000_0000; // leaf: value=0
        let normalized = b"AB\0".to_vec();
        let n = PrecompiledNormalizer {
            trie: trie.clone(),
            normalized: normalized.clone(),
        };
        assert_eq!(n.normalize("a"), "AB");
        // Passes through unmatched prefix, matches 'a', passes through
        // the trailing 'b'.
        assert_eq!(n.normalize("xay"), "xABy");
        // Multiple matches: "aa" -> "ABAB".
        assert_eq!(n.normalize("aa"), "ABAB");
    }

    #[test]
    fn hand_crafted_charsmap_survives_base64_round_trip() {
        // Same trie / normalized as above, but rebuilt from a
        // base64-encoded charsmap to exercise the parse path
        // end-to-end.
        let mut trie = vec![0u32; 98];
        trie[0] = 0x400;
        trie[96] = 0x561;
        trie[97] = 0x8000_0000;
        let normalized = b"AB\0";
        let b64 = encode_charsmap_for_tests(&trie, normalized);
        let n = PrecompiledNormalizer::from_base64_charsmap(&b64).unwrap();
        assert_eq!(n.normalize("banana"), "bABnABnAB");
    }

    #[test]
    fn empty_input_normalizes_to_empty_string() {
        let n = PrecompiledNormalizer::from_blob(&[0u8, 0, 0, 0]).unwrap();
        assert_eq!(n.normalize(""), "");
    }

    #[test]
    fn error_display_names_variant_context() {
        let err = PrecompiledError::TruncatedHeader.to_string();
        assert!(err.contains("Precompiled"));
        assert!(err.contains("header"));
    }

    #[test]
    fn utf8_scalar_len_matches_lead_byte_class() {
        assert_eq!(utf8_scalar_len(b'a'), 1);
        assert_eq!(utf8_scalar_len(0xC3), 2); // 2-byte lead
        assert_eq!(utf8_scalar_len(0xE2), 3); // 3-byte lead
        assert_eq!(utf8_scalar_len(0xF0), 4); // 4-byte lead
        assert_eq!(utf8_scalar_len(0xBF), 1); // continuation → advance one
    }
}
