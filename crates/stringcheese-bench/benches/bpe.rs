//! BPE encoder throughput benchmarks.
//!
//! The dominant cost inside [`BpeTokenizer::encode`] is the merge-table
//! rank lookup — called O(n) times per word inside the O(n log n) merge
//! loop (once per heap-pop-validation and once per new-adjacency check).
//! This bench measures encode throughput on a realistic-shape input
//! (10 KiB of ASCII prose seeded from a fixed RNG, with a batch of
//! registered special tokens for the extraction pass to exercise) so
//! refactors to `BpeMergeTable::rank`'s underlying map representation
//! (`BTreeMap` ↔ `HashMap` ↔ raw-entry ↔ …) can be tracked directly.
//!
//! Two shapes:
//!
//! * `encode_throughput` — end-to-end `encode` throughput, i.e. the
//!   full pipeline including special-token extraction, pre-tokenizer
//!   fallback (whitespace), the O(n log n) merge loop, and vocabulary
//!   lookup. This is what a caller sees.
//! * `merge_table_lookup` — a targeted microbench that hits
//!   `BpeMergeTable::rank` directly, isolating the map lookup so the
//!   BTreeMap→HashMap delta is legible without pipeline noise.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::collections::BTreeMap;
use std::hint::black_box;
use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_bpe::{BpeMergeTable, BpeTokenizer, BpeVocabulary};

/// Deterministic LCG — the bench crate's `inputs::random_ascii` is
/// shape-compatible but not re-exported for foreign callers, and we
/// want zero cross-crate coupling here.
fn lcg(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    (*state >> 33) as u32
}

/// Roughly 10 KiB of English-ish ASCII prose seeded from `seed`. Made
/// deterministic so criterion samples over the *same* input every run.
fn synthetic_prose(bytes: usize, seed: u64) -> String {
    // A pool of common English words. Sampling uniformly from a fixed
    // pool gives us a Zipf-ish frequency distribution (words repeat)
    // that stresses the merge-table's hot subset — the shape that
    // dominates real-world BPE encode cost.
    let words: &[&str] = &[
        "the", "of", "and", "to", "a", "in", "that", "is", "was", "for", "with", "on", "as", "by",
        "at", "from", "or", "an", "be", "this", "which", "have", "not", "are", "but", "had", "has",
        "his", "her", "they", "we", "you", "he", "she", "it", "one", "all", "would", "there",
        "their", "what", "so", "up", "out", "if", "about", "who", "get", "go", "me", "when",
        "make", "can", "like", "time", "no", "just", "know", "take", "into", "year", "your",
        "some", "could", "them", "see", "other", "than", "then", "now", "look", "only", "come",
        "its", "over", "think", "also", "back", "after", "use", "two", "how", "our", "work",
        "first", "well", "way", "even", "new", "want", "because", "any", "these", "give", "day",
        "most", "us",
    ];
    let mut state = seed.wrapping_add(1);
    let mut out = String::with_capacity(bytes + 16);
    while out.len() < bytes {
        let idx = (lcg(&mut state) as usize) % words.len();
        out.push_str(words[idx]);
        out.push(' ');
    }
    out
}

/// Build a tokenizer with a byte-alphabet vocab, a set of realistic
/// merges (all two-letter and two-letter-plus-common-suffix ASCII pairs
/// — a few thousand merges total, comparable in order-of-magnitude
/// footprint to the small-vocab tiktoken variants after byte-alphabet),
/// and a handful of registered special tokens so the special extraction
/// pass has real work to do.
fn build_bench_tokenizer() -> BpeTokenizer {
    let mut vocab = BpeVocabulary::new();
    let mut next = vocab.ensure_byte_alphabet(0).unwrap();
    let mut merges = BpeMergeTable::new();
    let mut rank: u32 = 0;

    // Every 2-letter lowercase ASCII pair (26*26 = 676 merges).
    for a in b'a'..=b'z' {
        for b in b'a'..=b'z' {
            let left = vec![a];
            let right = vec![b];
            let mut concat = left.clone();
            concat.extend_from_slice(&right);
            if vocab.id(&concat).is_none() {
                vocab.insert(next, concat.clone()).unwrap();
                next += 1;
            }
            merges.insert(left, right, rank);
            rank += 1;
        }
    }
    // Every 2-letter piece + one of a handful of common suffixes
    // (~676 * 6 = ~4000 more merges). Simulates the "second-generation"
    // merges a BPE trainer emits after saturating the byte-pair layer.
    let suffixes: &[&[u8]] = &[b"s", b"e", b"d", b"r", b"n", b"t"];
    for a in b'a'..=b'z' {
        for b in b'a'..=b'z' {
            for suf in suffixes {
                let left = vec![a, b];
                let right = suf.to_vec();
                let mut concat = left.clone();
                concat.extend_from_slice(&right);
                if vocab.id(&concat).is_none() {
                    vocab.insert(next, concat.clone()).unwrap();
                    next += 1;
                }
                merges.insert(left, right, rank);
                rank += 1;
            }
        }
    }

    // 20 registered special tokens exercised by the extraction pass.
    let mut specials: BTreeMap<String, u32> = BTreeMap::new();
    for i in 0..20u32 {
        specials.insert(format!("<|special_{i}|>"), 100_000 + i);
    }

    BpeTokenizer::from_parts(merges, vocab).with_special_tokens(specials)
}

fn bench_encode_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("bpe/encode_throughput");
    // 10 KiB is the audit's suggested working point; also bench 1 KiB
    // and 40 KiB to catch scaling regressions that a single input size
    // would miss.
    for &bytes in &[1024usize, 10 * 1024, 40 * 1024] {
        let input = synthetic_prose(bytes, 0xDEAD_BEEF);
        let tok = build_bench_tokenizer();
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("bytes", bytes),
            &(tok, input),
            |b, (tok, input)| {
                b.iter(|| {
                    let enc = tok.encode(black_box(input)).unwrap();
                    black_box(enc);
                });
            },
        );
    }
    group.finish();
}

fn bench_merge_table_lookup(c: &mut Criterion) {
    // Targeted microbench: hit `BpeMergeTable::rank` directly with a
    // deterministic sequence of two-byte keys covering the full lowercase
    // ASCII pair space. Isolates the map lookup cost from every other
    // step of the pipeline.
    let mut merges = BpeMergeTable::new();
    let mut rank = 0u32;
    for a in b'a'..=b'z' {
        for b in b'a'..=b'z' {
            merges.insert(vec![a], vec![b], rank);
            rank += 1;
        }
    }
    // Same-order deterministic query pattern; both hits and misses so
    // the negative-lookup path is exercised too.
    let queries: Vec<([u8; 1], [u8; 1])> = {
        let mut v = Vec::with_capacity(26 * 26);
        for a in b'a'..=b'z' {
            for b in b'a'..=b'z' {
                v.push(([a], [b]));
            }
        }
        v
    };

    let mut group = c.benchmark_group("bpe/merge_table_lookup");
    group.throughput(Throughput::Elements(queries.len() as u64));
    group.bench_function("all_pairs_ascii_lower", |b| {
        b.iter(|| {
            let mut acc: u64 = 0;
            for (l, r) in &queries {
                if let Some(rk) = merges.rank(black_box(l), black_box(r)) {
                    acc = acc.wrapping_add(u64::from(rk));
                }
            }
            black_box(acc);
        });
    });
    group.finish();
}

criterion_group!(bpe, bench_encode_throughput, bench_merge_table_lookup);
criterion_main!(bpe);
