//! SCUD-lite BPE data-pack loader.
//!
//! This module implements the runtime side of the SCUD-lite BPE format
//! sketched in `docs/design/tokenizers.md` § 5.2. Callers should not
//! reach for it directly; the [`crate::TiktokenPack`] type is the
//! ergonomic surface. The loader lives on its own so the build script
//! can share the format constants at review time.
//!
//! # Wire format
//!
//! ```text
//! +--------+---------+---------+----------+----------+----------+
//! | magic  | version | vocab_n | merges_n | vocab    | merges   |
//! | "SCUB" | u16     | u32     | u32      | var      | var      |
//! | 4 B    | 2 B     | 4 B     | 4 B      | ...      | ...      |
//! +--------+---------+---------+----------+----------+----------+
//! ```
//!
//! All integers are little-endian. Vocab entries: `(id: u32,
//! byte_length: u16, bytes: [u8; byte_length])`. Merge entries: `(left_id:
//! u32, right_id: u32, rank: u32)`, where the ids reference vocab
//! entries emitted earlier in the same file.
//!
//! The whole SCUD-lite blob is then deflate-compressed. The runtime
//! decompresses it in-place through [`miniz_oxide`], parses the header,
//! then walks the two regions.

use alloc::vec::Vec;

use stringcheese_tokenizer::TokenizerError;
use stringcheese_tokenizer_bpe::{BpeMergeTable, BpeTokenizer, BpeVocabulary};

/// The SCUD-lite magic bytes: `S`, `C`, `U`, `B` (BPE variant of the
/// SCUD envelope).
pub const SCUD_MAGIC: [u8; 4] = *b"SCUB";

/// The SCUD-lite version this loader knows how to parse.
pub const SCUD_VERSION: u16 = 1;

/// Errors surfaced by the loader. Every variant flattens into
/// [`TokenizerError::Other`] on the way out to the trait surface —
/// this internal enum keeps the diagnostic message narrow.
#[derive(Debug)]
enum LoadError {
    TooShort,
    BadMagic,
    UnsupportedVersion,
    TruncatedVocab,
    TruncatedMerges,
    MergeReferencesUnknownVocab,
    Decompress,
    Vocabulary,
}

impl LoadError {
    fn into_message(self) -> &'static str {
        match self {
            Self::TooShort => "SCUD-lite blob is shorter than its header",
            Self::BadMagic => "SCUD-lite blob does not start with SCUB magic",
            Self::UnsupportedVersion => "SCUD-lite blob has a version this loader does not support",
            Self::TruncatedVocab => "SCUD-lite vocab region is truncated",
            Self::TruncatedMerges => "SCUD-lite merges region is truncated",
            Self::MergeReferencesUnknownVocab => {
                "SCUD-lite merge references a vocab id not present in the pack"
            }
            Self::Decompress => "SCUD-lite blob failed to decompress",
            Self::Vocabulary => "SCUD-lite vocabulary contained a duplicate id or byte string",
        }
    }
}

/// Load a SCUD-lite pack — decompress, parse, and construct a
/// [`BpeTokenizer`].
///
/// # Errors
///
/// Returns [`TokenizerError::Other`] carrying a short static message
/// when the blob is malformed, references a vocab id that was not
/// declared, or fails to decompress. Returns [`TokenizerError::VocabularyMismatch`]
/// when the vocabulary has internally-inconsistent entries.
pub fn load_pack(compressed: &[u8]) -> Result<BpeTokenizer, TokenizerError> {
    let decompressed = miniz_oxide::inflate::decompress_to_vec(compressed)
        .map_err(|_| load_err(LoadError::Decompress))?;
    parse_scud_lite(&decompressed)
}

fn load_err(e: LoadError) -> TokenizerError {
    match e {
        LoadError::Vocabulary => TokenizerError::VocabularyMismatch,
        other => TokenizerError::Other(other.into_message()),
    }
}

fn parse_scud_lite(data: &[u8]) -> Result<BpeTokenizer, TokenizerError> {
    // Header: 4 (magic) + 2 (version) + 4 (vocab_n) + 4 (merges_n) = 14 B.
    if data.len() < 14 {
        return Err(load_err(LoadError::TooShort));
    }
    if data[..4] != SCUD_MAGIC {
        return Err(load_err(LoadError::BadMagic));
    }
    let version = u16::from_le_bytes([data[4], data[5]]);
    if version != SCUD_VERSION {
        return Err(load_err(LoadError::UnsupportedVersion));
    }
    let vocab_n = read_u32_le(data, 6);
    let merges_n = read_u32_le(data, 10);

    let mut cursor: usize = 14;
    let mut vocab = BpeVocabulary::new();
    // Parallel array so merge parsing can look up bytes by id in O(1)
    // rather than paying a `BpeVocabulary::bytes` lookup per merge.
    // The BpeMergeTable is keyed on byte strings, not ids.
    let mut id_to_bytes: Vec<Option<Vec<u8>>> = Vec::new();

    for _ in 0..vocab_n {
        if cursor + 4 + 2 > data.len() {
            return Err(load_err(LoadError::TruncatedVocab));
        }
        let id = read_u32_le(data, cursor);
        cursor += 4;
        let byte_len = usize::from(u16::from_le_bytes([data[cursor], data[cursor + 1]]));
        cursor += 2;
        if cursor + byte_len > data.len() {
            return Err(load_err(LoadError::TruncatedVocab));
        }
        let bytes = data[cursor..cursor + byte_len].to_vec();
        cursor += byte_len;

        let id_usize = id as usize;
        if id_usize >= id_to_bytes.len() {
            id_to_bytes.resize(id_usize + 1, None);
        }
        id_to_bytes[id_usize] = Some(bytes.clone());

        vocab
            .insert(id, bytes)
            .map_err(|_| load_err(LoadError::Vocabulary))?;
    }

    let mut merges = BpeMergeTable::new();
    for _ in 0..merges_n {
        if cursor + 12 > data.len() {
            return Err(load_err(LoadError::TruncatedMerges));
        }
        let left_id = read_u32_le(data, cursor);
        cursor += 4;
        let right_id = read_u32_le(data, cursor);
        cursor += 4;
        let rank = read_u32_le(data, cursor);
        cursor += 4;

        let left_bytes = id_to_bytes
            .get(left_id as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| load_err(LoadError::MergeReferencesUnknownVocab))?
            .clone();
        let right_bytes = id_to_bytes
            .get(right_id as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| load_err(LoadError::MergeReferencesUnknownVocab))?
            .clone();
        merges.insert(left_bytes, right_bytes, rank);
    }

    // Trailing bytes are tolerated for forward compat; the header
    // already committed to the exact vocab/merge counts.
    let _ = cursor;
    Ok(BpeTokenizer::from_parts(merges, vocab))
}

#[inline]
fn read_u32_le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use stringcheese_tokenizer::Tokenizer;

    /// Build a minimal in-memory SCUD-lite blob covering the byte
    /// alphabet plus one 2-byte token, with a single merge.
    fn synthesise_test_pack() -> Vec<u8> {
        let mut vocab: Vec<(u32, Vec<u8>)> = Vec::new();
        for b in 0u8..=255 {
            vocab.push((u32::from(b), vec![b]));
        }
        vocab.push((256, b"he".to_vec()));

        let merges: Vec<(u32, u32, u32)> = vec![(u32::from(b'h'), u32::from(b'e'), 0)];

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&SCUD_MAGIC);
        buf.extend_from_slice(&SCUD_VERSION.to_le_bytes());
        let vocab_n = u32::try_from(vocab.len()).unwrap();
        buf.extend_from_slice(&vocab_n.to_le_bytes());
        let merges_n = u32::try_from(merges.len()).unwrap();
        buf.extend_from_slice(&merges_n.to_le_bytes());
        for (id, bytes) in &vocab {
            buf.extend_from_slice(&id.to_le_bytes());
            let bl = u16::try_from(bytes.len()).unwrap();
            buf.extend_from_slice(&bl.to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        for (l, r, rank) in &merges {
            buf.extend_from_slice(&l.to_le_bytes());
            buf.extend_from_slice(&r.to_le_bytes());
            buf.extend_from_slice(&rank.to_le_bytes());
        }
        buf
    }

    fn compress_test(data: &[u8]) -> Vec<u8> {
        miniz_oxide::deflate::compress_to_vec(data, 6)
    }

    #[test]
    fn parses_hand_built_pack() {
        let raw = synthesise_test_pack();
        let compressed = compress_test(&raw);
        let tok = load_pack(&compressed).expect("pack loads");
        // The single merge "h" + "e" → "he" should fire on "he".
        let enc = tok.encode("he").unwrap();
        assert_eq!(enc.ids, vec![256]);
        let round = tok.decode(&enc.ids).unwrap();
        assert_eq!(round, "he");
    }

    #[test]
    fn parses_hand_built_pack_directly() {
        // Test the parser on uncompressed input via a private path — we
        // do this by decompressing our own compression, but the round
        // trip proves both directions.
        let raw = synthesise_test_pack();
        let compressed = compress_test(&raw);
        let tok = load_pack(&compressed).unwrap();
        // Byte-alphabet character encodes to its byte id.
        let enc = tok.encode("z").unwrap();
        assert_eq!(enc.ids, vec![u32::from(b'z')]);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut raw = synthesise_test_pack();
        raw[0] = b'X';
        let compressed = compress_test(&raw);
        let err = load_pack(&compressed).unwrap_err();
        assert!(matches!(err, TokenizerError::Other(_)));
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut raw = synthesise_test_pack();
        raw[4] = 99; // version LSB
        let compressed = compress_test(&raw);
        let err = load_pack(&compressed).unwrap_err();
        assert!(matches!(err, TokenizerError::Other(_)));
    }

    #[test]
    fn too_short_rejected() {
        let compressed = compress_test(b"SCUB\x01\x00\x00");
        let err = load_pack(&compressed).unwrap_err();
        assert!(matches!(err, TokenizerError::Other(_)));
    }

    #[test]
    fn decompress_failure_surfaces_as_other() {
        // Random noise unlikely to be valid deflate.
        let bogus = [0xffu8; 32];
        let err = load_pack(&bogus).unwrap_err();
        assert!(matches!(err, TokenizerError::Other(_)));
    }

    #[test]
    fn merge_referencing_unknown_id_rejected() {
        // Vocab: just byte 'a' at id 97. Merge references id 500
        // which isn't in the vocab.
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&SCUD_MAGIC);
        buf.extend_from_slice(&SCUD_VERSION.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes()); // vocab_n
        buf.extend_from_slice(&1u32.to_le_bytes()); // merges_n
        buf.extend_from_slice(&97u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.push(b'a');
        buf.extend_from_slice(&500u32.to_le_bytes());
        buf.extend_from_slice(&501u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        let compressed = compress_test(&buf);
        let err = load_pack(&compressed).unwrap_err();
        assert!(matches!(err, TokenizerError::Other(_)));
    }
}
