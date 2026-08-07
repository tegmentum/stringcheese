//! Build script: emit one SCUD-lite pack per enabled variant into
//! `OUT_DIR`.
//!
//! The runtime crate `include_bytes!`-embeds these files. For each
//! enabled feature this script either:
//!
//! 1. Transcodes `data/<variant>.tiktoken` (the upstream `mergeable_ranks`
//!    plaintext format) into SCUD-lite + deflate when the file exists.
//! 2. Falls back to a small deterministic *synthetic* tokenizer keyed
//!    on the variant name so tests don't need `OpenAI`'s blob in-tree.
//!
//! See `data/README.md` for how to drop real tiktoken data into the
//! crate.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------
// The SCUD-lite writer and the tiktoken-plaintext parser are duplicated
// here in the build script because a build script cannot depend on the
// crate it is building. The runtime copy lives in `src/scud.rs` and
// `src/builder.rs`; keep the two in sync.
// ---------------------------------------------------------------------

const SCUD_MAGIC: &[u8; 4] = b"SCUB";
const SCUD_VERSION: u16 = 1;

#[derive(Clone)]
struct VocabEntry {
    id: u32,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct MergeEntry {
    left_id: u32,
    right_id: u32,
    rank: u32,
}

/// Write a SCUD-lite blob into `out` given a vocab + merges pair.
fn write_scud(vocab: &[VocabEntry], merges: &[MergeEntry], out: &mut Vec<u8>) {
    out.extend_from_slice(SCUD_MAGIC);
    out.extend_from_slice(&SCUD_VERSION.to_le_bytes());
    let vocab_n = u32::try_from(vocab.len()).expect("vocab fits in u32");
    let merges_n = u32::try_from(merges.len()).expect("merges fits in u32");
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
}

/// Parse the upstream tiktoken `mergeable_ranks` plaintext format:
/// one `<base64(bytes)> <rank>` entry per line. Comments and blank
/// lines are ignored.
///
/// The parser returns a paired `(vocab, merges)` — the tiktoken blob
/// carries only "byte string → rank" pairs, but for SCUD-lite we also
/// synthesise a vocabulary entry per unique byte string (the rank
/// becomes the token id, which matches tiktoken's own numbering
/// convention: ids are dense and equal to the rank of the merge that
/// produced them, with the byte alphabet occupying the low ids by
/// construction).
fn parse_tiktoken_plaintext(input: &str) -> Result<(Vec<VocabEntry>, Vec<MergeEntry>), String> {
    let mut ranks: Vec<(Vec<u8>, u32)> = Vec::new();
    for (lineno, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let b64 = parts
            .next()
            .ok_or_else(|| format!("line {}: missing base64 token", lineno + 1))?;
        let rank_str = parts
            .next()
            .ok_or_else(|| format!("line {}: missing rank", lineno + 1))?;
        let bytes = base64_decode(b64)
            .map_err(|e| format!("line {}: base64 decode failed: {e}", lineno + 1))?;
        let rank: u32 = rank_str
            .parse()
            .map_err(|e| format!("line {}: rank parse failed: {e}", lineno + 1))?;
        ranks.push((bytes, rank));
    }

    // Vocab: one entry per token, keyed by rank (which becomes the id).
    let vocab: Vec<VocabEntry> = ranks
        .iter()
        .map(|(b, r)| VocabEntry {
            id: *r,
            bytes: b.clone(),
        })
        .collect();

    // Merges: for every non-byte-alphabet entry, greedily search for a
    // (prefix, suffix) split whose two halves are BOTH in the vocab and
    // both have a lower rank than this entry. Ties broken by preferring
    // the split whose left half has the lowest rank (matches how a
    // trainer produces the merges).
    let mut bytes_to_rank: std::collections::HashMap<Vec<u8>, u32> =
        std::collections::HashMap::new();
    for (b, r) in &ranks {
        bytes_to_rank.insert(b.clone(), *r);
    }

    let mut merges: Vec<MergeEntry> = Vec::new();
    for (bytes, rank) in &ranks {
        if bytes.len() < 2 {
            continue; // byte-alphabet entry
        }
        // Find the best split.
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
        // No valid split → this vocab entry has no merge that produces
        // it. That is fine (it means it is a byte-alphabet token or a
        // singleton) and we simply do not emit a merge.
    }

    Ok((vocab, merges))
}

/// RFC 4648 §4 base64 decoder (accepts `=` padding). Small, no
/// dependencies — the tiktoken plaintext is short-string base64 and
/// perf doesn't matter here.
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
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

/// Deflate-compress with `miniz_oxide` at level 10 (best).
fn compress(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(data, 10)
}

/// Build a small deterministic synthetic tokenizer. The vocabulary
/// covers the full byte alphabet plus a variant-specific set of
/// meaningful multi-byte merges seeded from the variant name; the
/// merges compose realistically so BPE tests exercise the pipeline.
///
/// The vocabulary is intentionally *tiny* (~byte alphabet + a few
/// hundred merges). Real tiktoken packs have 50k–200k entries; the
/// synthetic stand-in is enough to prove the SCUD-lite loader and the
/// integration with `stringcheese-tokenizer-bpe` work end-to-end.
fn build_synthetic(variant: &str) -> (Vec<VocabEntry>, Vec<MergeEntry>) {
    let mut vocab: Vec<VocabEntry> = Vec::new();
    // Byte alphabet 0..=255.
    for b in 0u8..=255 {
        vocab.push(VocabEntry {
            id: u32::from(b),
            bytes: vec![b],
        });
    }
    let mut next_id: u32 = 256;
    let mut merges: Vec<MergeEntry> = Vec::new();
    let mut rank: u32 = 0;

    // A shared corpus of common English digraphs/trigraphs plus a
    // couple of variant-specific tokens. These are enough to make BPE
    // fire on realistic inputs like "Hello, world!".
    let seed: &[&[u8]] = &[
        b"he", b"ll", b"llo", b"hello", b"wo", b"or", b"rl", b"ld", b"world", b"lo", b" w",
        b" world", b", ", b"th", b"the", b" the", b"in", b"ing", b"er", b"an", b"and", b" and",
        b"is", b" is", b"it", b"to", b"of", b"on", b"at", b"as", b"be", b"ha", b"have", b"no",
        b"not", b"ca", b"cat", b"cats", b"do", b"dog", b"dogs", b"fo", b"for", b"you", b"yo",
        // Whitespace-prefixed variants that mirror tiktoken's real merges.
        b" a", b" b", b" c", b" d", b" e", b" f", b" g", b" h", b" i", b" o", b" s", b" t",
        // A punctuation-heavy pair so the "Hello, world!" test lands on
        // a non-trivial encoding regardless of the variant.
        b", ", b"! ",
    ];

    // Insert every seed as a vocab entry paired with a rank-0..N merge
    // rule that composes it out of two existing vocab entries. We use
    // a longest-prefix decomposition: find the longest existing prefix
    // that is already a vocab entry, split there.
    let mut byte_index: std::collections::HashMap<Vec<u8>, u32> = std::collections::HashMap::new();
    for entry in &vocab {
        byte_index.insert(entry.bytes.clone(), entry.id);
    }
    for token in seed {
        if token.len() < 2 || byte_index.contains_key(*token) {
            continue;
        }
        // Find a split whose two halves are already in the vocab.
        // Prefer the split with the largest left half so merges compose
        // hierarchically ("hello" = "he" + "llo").
        let mut split: Option<usize> = None;
        for k in (1..token.len()).rev() {
            let left = &token[..k];
            let right = &token[k..];
            if byte_index.contains_key(left) && byte_index.contains_key(right) {
                split = Some(k);
                break;
            }
        }
        // If no split works, decompose byte-by-byte via cascading
        // partial merges. We inject each successively-longer prefix as
        // a vocab entry so a later split finds one.
        if split.is_none() {
            for k in 2..token.len() {
                let prefix = &token[..k];
                if !byte_index.contains_key(prefix) {
                    let id = next_id;
                    next_id += 1;
                    let left = &prefix[..k - 1];
                    let right = &prefix[k - 1..];
                    if let (Some(&lid), Some(&rid)) = (byte_index.get(left), byte_index.get(right))
                    {
                        vocab.push(VocabEntry {
                            id,
                            bytes: prefix.to_vec(),
                        });
                        merges.push(MergeEntry {
                            left_id: lid,
                            right_id: rid,
                            rank,
                        });
                        rank += 1;
                        byte_index.insert(prefix.to_vec(), id);
                    }
                }
            }
            // Now retry the split for the full token.
            for k in (1..token.len()).rev() {
                let left = &token[..k];
                let right = &token[k..];
                if byte_index.contains_key(left) && byte_index.contains_key(right) {
                    split = Some(k);
                    break;
                }
            }
        }
        if let Some(k) = split {
            let left = &token[..k];
            let right = &token[k..];
            let lid = byte_index[left];
            let rid = byte_index[right];
            let id = next_id;
            next_id += 1;
            vocab.push(VocabEntry {
                id,
                bytes: token.to_vec(),
            });
            merges.push(MergeEntry {
                left_id: lid,
                right_id: rid,
                rank,
            });
            rank += 1;
            byte_index.insert(token.to_vec(), id);
        }
    }

    // Tag the variant so a bytewise-equal pack does not accidentally
    // shadow a different variant's file at cache hit time.
    let tag = format!("<|synthetic-{variant}|>").into_bytes();
    vocab.push(VocabEntry {
        id: next_id,
        bytes: tag,
    });

    (vocab, merges)
}

fn generate_variant(variant: &str, out_dir: &Path, data_dir: &Path) {
    let plaintext_path = data_dir.join(format!("{variant}.tiktoken"));
    let (vocab, merges) = if plaintext_path.exists() {
        let text = fs::read_to_string(&plaintext_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", plaintext_path.display()));
        match parse_tiktoken_plaintext(&text) {
            Ok(pair) => pair,
            Err(e) => panic!("failed to parse {}: {e}", plaintext_path.display()),
        }
    } else {
        build_synthetic(variant)
    };

    let mut scud = Vec::new();
    write_scud(&vocab, &merges, &mut scud);
    let compressed = compress(&scud);
    let out_path = out_dir.join(format!("{variant}.scud.mz"));
    let mut f = fs::File::create(&out_path)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", out_path.display()));
    f.write_all(&compressed)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", out_path.display()));
}

fn main() {
    // Re-run when the data directory changes so contributor-supplied
    // real packs are picked up on the next build.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=data");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"));
    let data_dir = manifest_dir.join("data");

    // Iterate the compile-time feature flags. `cargo` sets
    // `CARGO_FEATURE_<UPPER>` for every enabled feature.
    let variants: &[&str] = &["cl100k_base", "p50k_base", "r50k_base", "o200k_base"];
    for variant in variants {
        let env_key = format!("CARGO_FEATURE_{}", variant.to_uppercase());
        if env::var_os(&env_key).is_some() {
            generate_variant(variant, &out_dir, &data_dir);
        }
    }
}
