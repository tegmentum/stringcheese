//! Public writer for the SCUD-lite BPE format.
//!
//! This module is the *runtime* counterpart to the transcoder in
//! `build.rs`. It exists so contributors who want to regenerate packs
//! outside of a `cargo build` (say, converting a locally-cached
//! `mergeable_ranks.tiktoken` file into a SCUD-lite blob for shipping)
//! do not have to reimplement the format from scratch.
//!
//! The build script does **not** call into this module — a `build.rs`
//! cannot depend on the crate it is building — so the two copies of the
//! writer are duplicated. If you change the wire format here, update
//! `build.rs` in lock-step. See `docs/design/tokenizers.md` § 5.2 for
//! the source-of-truth definition.

use alloc::string::String;
use alloc::vec::Vec;

use crate::scud::{SCUD_MAGIC, SCUD_VERSION};

/// One vocabulary entry: a numeric id paired with the raw bytes it
/// stands for.
#[derive(Debug, Clone)]
pub struct VocabEntry {
    /// The token id.
    pub id: u32,
    /// The bytes the id encodes to.
    pub bytes: Vec<u8>,
}

/// One merge entry: the two vocab ids that combine, plus the rank
/// (lower = higher priority — same convention as tiktoken and Hugging
/// Face).
#[derive(Debug, Clone, Copy)]
pub struct MergeEntry {
    /// The vocab id of the left half.
    pub left_id: u32,
    /// The vocab id of the right half.
    pub right_id: u32,
    /// The merge priority; lower fires first.
    pub rank: u32,
}

/// Serialise a `(vocab, merges)` pair into the uncompressed SCUD-lite
/// wire format.
///
/// # Panics
///
/// Panics if any vocab entry has more than `u16::MAX` bytes (`65_535`)
/// or if the vocab / merges arrays overflow `u32`. Neither bound is
/// reachable with any published tiktoken variant; the largest single
/// vocab entry in `o200k_base` is a handful of bytes long.
#[must_use]
pub fn encode_scud(vocab: &[VocabEntry], merges: &[MergeEntry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(header_size() + estimate_body(vocab, merges));
    out.extend_from_slice(&SCUD_MAGIC);
    out.extend_from_slice(&SCUD_VERSION.to_le_bytes());
    let vocab_n = u32::try_from(vocab.len()).expect("vocab length fits in u32");
    let merges_n = u32::try_from(merges.len()).expect("merges length fits in u32");
    out.extend_from_slice(&vocab_n.to_le_bytes());
    out.extend_from_slice(&merges_n.to_le_bytes());
    for entry in vocab {
        out.extend_from_slice(&entry.id.to_le_bytes());
        let len = u16::try_from(entry.bytes.len())
            .expect("vocab entry byte length fits in u16 (max 65535)");
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&entry.bytes);
    }
    for merge in merges {
        out.extend_from_slice(&merge.left_id.to_le_bytes());
        out.extend_from_slice(&merge.right_id.to_le_bytes());
        out.extend_from_slice(&merge.rank.to_le_bytes());
    }
    out
}

/// Deflate-compress a SCUD-lite blob at maximum compression. Callers
/// who want to trade compression ratio for build-time speed can call
/// [`miniz_oxide::deflate::compress_to_vec`] directly with a lower
/// level.
#[must_use]
pub fn compress(scud: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(scud, 10)
}

const fn header_size() -> usize {
    4 + 2 + 4 + 4
}

fn estimate_body(vocab: &[VocabEntry], merges: &[MergeEntry]) -> usize {
    let vocab_bytes: usize = vocab.iter().map(|v| 4 + 2 + v.bytes.len()).sum();
    let merge_bytes = merges.len() * 12;
    vocab_bytes + merge_bytes
}

/// Parse the upstream tiktoken `mergeable_ranks` plaintext format
/// (`<base64(bytes)> <rank>` per line) into a `(vocab, merges)` pair
/// suitable for [`encode_scud`].
///
/// The tiktoken plaintext carries only byte-string → rank pairs. This
/// function synthesises merges by finding, for each non-byte-alphabet
/// entry, the split whose two halves are both already vocab entries
/// and both have a strictly lower rank than the entry being merged
/// into. Ties are broken by preferring the split whose two halves
/// have the lowest `(left_rank, right_rank)` lexicographic key —
/// which matches the choice a trainer would make when picking a merge.
///
/// # Errors
///
/// Returns `Err(message)` when a line does not match the expected
/// `<base64> <rank>` shape, when base64 decoding fails, or when a
/// rank does not parse as a `u32`.
pub fn build_scud_from_tiktoken(
    input: &[u8],
) -> Result<(Vec<VocabEntry>, Vec<MergeEntry>), String> {
    let text = core::str::from_utf8(input)
        .map_err(|_| String::from("tiktoken plaintext is not valid UTF-8"))?;
    parse_tiktoken_plaintext(text)
}

fn parse_tiktoken_plaintext(input: &str) -> Result<(Vec<VocabEntry>, Vec<MergeEntry>), String> {
    use alloc::collections::BTreeMap;
    let mut ranks: Vec<(Vec<u8>, u32)> = Vec::new();
    for (lineno, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let b64 = parts
            .next()
            .ok_or_else(|| format_lineno(lineno, "missing base64 token"))?;
        let rank_str = parts
            .next()
            .ok_or_else(|| format_lineno(lineno, "missing rank"))?;
        let bytes = base64_decode(b64).map_err(|e| format_lineno(lineno, e))?;
        let rank: u32 = rank_str
            .parse()
            .map_err(|_| format_lineno(lineno, "rank did not parse as u32"))?;
        ranks.push((bytes, rank));
    }

    let vocab: Vec<VocabEntry> = ranks
        .iter()
        .map(|(b, r)| VocabEntry {
            id: *r,
            bytes: b.clone(),
        })
        .collect();

    let bytes_to_rank: BTreeMap<Vec<u8>, u32> =
        ranks.iter().map(|(b, r)| (b.clone(), *r)).collect();

    let mut merges: Vec<MergeEntry> = Vec::new();
    for (bytes, rank) in &ranks {
        if bytes.len() < 2 {
            continue;
        }
        let mut best: Option<(u32, u32)> = None;
        let mut best_key: Option<(u32, u32)> = None;
        for split in 1..bytes.len() {
            let left = &bytes[..split];
            let right = &bytes[split..];
            let (Some(&lr), Some(&rr)) = (bytes_to_rank.get(left), bytes_to_rank.get(right)) else {
                continue;
            };
            if lr >= *rank || rr >= *rank {
                continue;
            }
            let key = (lr, rr);
            if best_key.is_none_or(|k| key < k) {
                best_key = Some(key);
                best = Some((lr, rr));
            }
        }
        if let Some((lid, rid)) = best {
            merges.push(MergeEntry {
                left_id: lid,
                right_id: rid,
                rank: *rank,
            });
        }
    }

    Ok((vocab, merges))
}

fn format_lineno(lineno: usize, msg: &str) -> String {
    let mut s = String::from("tiktoken plaintext line ");
    s.push_str(&itoa(lineno + 1));
    s.push_str(": ");
    s.push_str(msg);
    s
}

// Tiny no-alloc-friendly usize -> decimal string; keeps this module
// free of `format!` calls in `no_std` builds if ever ported.
fn itoa(mut n: usize) -> String {
    if n == 0 {
        return String::from("0");
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + u8::try_from(n % 10).expect("modulo 10 fits in u8");
        n /= 10;
    }
    String::from_utf8(buf[i..].to_vec()).expect("digits are ASCII")
}

/// Minimal RFC 4648 §4 base64 decoder. Whitespace inside the input is
/// tolerated; `=` padding terminates the stream.
fn base64_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    let s = s.trim();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for ch in s.chars() {
        let v: u32 = match ch {
            'A'..='Z' => u32::from(ch as u8 - b'A'),
            'a'..='z' => u32::from(ch as u8 - b'a') + 26,
            '0'..='9' => u32::from(ch as u8 - b'0') + 52,
            '+' => 62,
            '/' => 63,
            '=' => break,
            c if c.is_whitespace() => continue,
            _ => return Err("invalid base64 character"),
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((buf >> bits) & 0xff).expect("6-bit mask fits in u8"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scud::load_pack;
    use alloc::vec;
    use stringcheese_tokenizer::Tokenizer;

    #[test]
    fn round_trip_encode_then_load() {
        let mut vocab = Vec::new();
        for b in 0u32..=255 {
            vocab.push(VocabEntry {
                id: b,
                bytes: vec![u8::try_from(b).unwrap()],
            });
        }
        vocab.push(VocabEntry {
            id: 256,
            bytes: b"lo".to_vec(),
        });
        let merges = vec![MergeEntry {
            left_id: u32::from(b'l'),
            right_id: u32::from(b'o'),
            rank: 0,
        }];
        let scud = encode_scud(&vocab, &merges);
        let compressed = compress(&scud);
        let tok = load_pack(&compressed).unwrap();
        let enc = tok.encode("lo").unwrap();
        assert_eq!(enc.ids, vec![256]);
    }

    #[test]
    fn tiktoken_plaintext_parses_minimal_input() {
        // "a" (byte 0x61) at rank 97, "b" at 98, "ab" at 300.
        let plaintext = "YQ== 97\nYg== 98\nYWI= 300\n";
        let (vocab, merges) = build_scud_from_tiktoken(plaintext.as_bytes()).unwrap();
        assert_eq!(vocab.len(), 3);
        // The "ab" entry should have a merge referencing 97 + 98.
        assert_eq!(merges.len(), 1);
        assert_eq!(merges[0].left_id, 97);
        assert_eq!(merges[0].right_id, 98);
        assert_eq!(merges[0].rank, 300);
    }

    #[test]
    fn tiktoken_plaintext_rejects_bad_base64() {
        let plaintext = "!!!! 42\n";
        let err = build_scud_from_tiktoken(plaintext.as_bytes()).unwrap_err();
        assert!(err.contains("line 1"));
    }

    #[test]
    fn tiktoken_plaintext_rejects_missing_rank() {
        let plaintext = "YQ==\n";
        let err = build_scud_from_tiktoken(plaintext.as_bytes()).unwrap_err();
        assert!(err.contains("missing rank"));
    }

    #[test]
    fn base64_decoder_matches_reference() {
        assert_eq!(base64_decode("YWJj").unwrap(), b"abc");
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("").unwrap(), b"");
    }
}
