//! Comparative encode-throughput bench: `stringcheese-tokenizer-hf`
//! vs upstream `tokenizers-rs` (Hugging Face) and `tiktoken-rs`
//! (`OpenAI`) on real vocabularies at three input scales.
//!
//! Three bench groups, mirroring the three families the audit called
//! out as the load-bearing shapes for a comparative bar:
//!
//! * `tokenizer_hf/encode/gpt2` — byte-level BPE (GPT-2 family) vs
//!   `tokenizers-rs`.
//! * `tokenizer_hf/encode/cl100k_base` — tiktoken-shape BPE vs
//!   `tiktoken-rs`. Uses `stringcheese-tokenizer-tiktoken`'s
//!   `builder::build_scud_from_tiktoken` on our side so both engines
//!   consume the same real `mergeable_ranks` bytes.
//! * `tokenizer_hf/encode/llama_2_7b` — SentencePiece-shape character
//!   BPE with `byte_fallback` and a Metaspace pre-tokenizer vs
//!   `tokenizers-rs`. Exercises the byte-fallback path that landed
//!   for Llama-2 / Mistral / Qwen support.
//!
//! Each group runs three input sizes: 1 KiB, 10 KiB, 100 KiB of
//! deterministic English prose. Criterion reports throughput in
//! MiB/s per sample; wall-clock ns/iter appears in the raw output.
//!
//! # Feature gate
//!
//! Only compiled with `--features parity-real-vocab` (see
//! `required-features` on the bench declaration in `Cargo.toml`). The
//! feature pulls in the two oracles plus
//! `stringcheese-tokenizer-tiktoken` for the tiktoken plaintext
//! parser.
//!
//! # Vocab lookup
//!
//! Real upstream vocabularies (tens of MB) are not committed to this
//! repository. The bench looks for each checkpoint's
//! `tokenizer.json` under two roots in priority order:
//!
//! 1. `$STRINGCHEESE_REAL_VOCABS_DIR/<checkpoint>/tokenizer.json` —
//!    honoured when the env-var is set. Recommended for CI where a
//!    setup step materialises vocabs into a per-job cache directory.
//! 2. `<hf-crate>/tests/conformance/vocabs/<checkpoint>/tokenizer.json` —
//!    the same directory the `stringcheese-tokenizer-hf` conformance
//!    runner reads.
//!
//! For the `cl100k_base` group, the tiktoken plaintext blob is looked
//! up under two roots that follow the same convention as the
//! `stringcheese-tokenizer-tiktoken-conformance` fetch layer:
//!
//! 1. `$TIKTOKEN_PARITY_DATA_DIR/cl100k_base.tiktoken` (env-var).
//! 2. `~/.cache/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken`.
//!
//! When a lookup fails, the group *soft-skips* via `eprintln!` (see
//! `cargo bench -- --nocapture` for skip messages) rather than
//! failing the bench — this is what keeps the bench build itself
//! green in CI when no vocabs are provisioned.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-bench --features parity-real-vocab \
//!     --bench tokenizer_hf
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-bench --features parity-real-vocab \
//!     --bench tokenizer_hf -- gpt2
//! ```
//!
//! # Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--measurement-time 8 --warm-up-time 2 --sample-size 20`).
//! Wall-clock samples vary ±10-15 % on a laptop under load; treat
//! the ratios as informative, the absolutes as illustrative.
//! Throughput reported as MiB/s of *input bytes*. Higher is better.
//!
//! ```text
//! group                        1 KiB          10 KiB         100 KiB
//! ----------------------------------------------------------------------
//! gpt2 / stringcheese-hf       3.0 MiB/s      2.9 MiB/s      6.9 MiB/s
//! gpt2 / tokenizers-rs         2.9 MiB/s      3.5 MiB/s      6.2 MiB/s
//! cl100k / stringcheese-hf     6.2 MiB/s      6.5 MiB/s      6.6 MiB/s
//! cl100k / tiktoken-rs        13.7 MiB/s     14.0 MiB/s     13.9 MiB/s
//! llama-2 / stringcheese-hf    6.8 MiB/s      4.2 MiB/s      3.9 MiB/s
//! llama-2 / tokenizers-rs      8.7 MiB/s      6.5 MiB/s      5.8 MiB/s
//! ```
//!
//! Read:
//!
//! * **`cl100k_base`** — `tiktoken-rs` is ~2.1× faster than us across
//!   every input size (previously 3-3.5×). The gap-halving is the
//!   result of a Wave-14 hot-path pass that stopped allocating one
//!   `Vec<u8>` per byte-piece in the BPE merge loop: pieces are now
//!   `(start, len)` byte ranges into the enclosing word's byte
//!   buffer and rank lookups on the merge table read through the
//!   shared slice directly (`hashbrown::HashMap<Vec<u8>, u32>` via
//!   `Vec<u8>: Borrow<[u8]>`). Remaining gap versus tiktoken-rs is
//!   attributable to their tighter merge-loop shape (a plain
//!   `Vec<Rank>` walk without linked-list bookkeeping) and, at 1 KiB,
//!   the residual per-word overhead of `Vec<MergeNode>` /
//!   `BinaryHeap` allocations.
//! * **gpt2** — now at approximate parity with tokenizers-rs across
//!   input sizes (was 1.3-1.7× behind at small sizes). The
//!   allocation-removal pass helps more here than for tiktoken-rs
//!   because tokenizers-rs's own merge-loop shape is closer to ours
//!   (linked-list + heap).
//! * **`llama_2_7b`** — tokenizers-rs is ~1.4-1.5× faster (was 2×).
//!   Exercises the `byte_fallback` + Metaspace path; per-char seeding
//!   still allocates a piece for every input codepoint even after the
//!   Wave-14 refactor because the seed step is per-char, so this
//!   shape gains less than the byte-per-piece cl100k / gpt2 shapes.
//!
//! Summary: allocation removal in the merge-loop hot path closed
//! most of the gap. The cl100k gap dropped from 3-3.5× to ~2.1×;
//! gpt2 reached parity; llama-2 narrowed from 2× to ~1.5×.
//! Flat-throughput signature at 1/10/100 KiB is gone on cl100k
//! (numbers now scale linearly with input size).
//!
//! Update this table whenever a perf change lands so future readers
//! don't have to re-run the bench to see whether the delta improved
//! or regressed the baseline.

#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use std::env;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_hf::hf::{HfTokenizer, parse_tokenizer_json, to_tokenizer};
use stringcheese_tokenizer_hf::{
    BpeMergeTable, BpeTokenizer, BpeVocabulary, PreTokenizerRegex, RegexPreTokenizer,
    TIKTOKEN_CANONICAL_PATTERN,
};
use stringcheese_tokenizer_tiktoken::builder;

// ---------------------------------------------------------------------
// Input.
// ---------------------------------------------------------------------

/// Deterministic English prose. Wraps a fixed word list so criterion
/// samples over byte-identical bytes across every run; no RNG, no OS
/// entropy, no time-based seed.
fn deterministic_prose(bytes: usize) -> String {
    let words: &[&str] = &[
        "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "and", "then",
        "runs", "back", "through", "the", "forest", "toward", "the", "little", "cabin", "on",
        "the", "hill", "where", "smoke", "curls", "from", "the", "chimney", "into", "the", "cold",
        "morning", "air", "while", "the", "sun", "climbs", "above", "the", "eastern", "ridge",
        "and", "birds", "call", "from", "the", "pines", "beyond", "the", "meadow",
    ];
    let mut out = String::with_capacity(bytes + 64);
    let mut idx = 0usize;
    while out.len() < bytes {
        out.push_str(words[idx % words.len()]);
        out.push(' ');
        idx += 1;
    }
    out
}

const INPUT_SIZES: &[usize] = &[1024, 10 * 1024, 100 * 1024];

// ---------------------------------------------------------------------
// Vocab lookup — matches the pattern used by
// `stringcheese-tokenizer-hf`'s conformance runner (two-root
// priority: env-var first, then the in-tree fixture cache).
// ---------------------------------------------------------------------

const VOCAB_DIR_ENV: &str = "STRINGCHEESE_REAL_VOCABS_DIR";
const TIKTOKEN_DIR_ENV: &str = "TIKTOKEN_PARITY_DATA_DIR";

/// Locate a `tokenizer.json` for `checkpoint` under the two lookup
/// roots (see module docs).
fn find_tokenizer_json(checkpoint: &str) -> Option<PathBuf> {
    if let Some(root) = env::var_os(VOCAB_DIR_ENV) {
        let p = PathBuf::from(root).join(checkpoint).join("tokenizer.json");
        if p.is_file() {
            return Some(p);
        }
    }
    // Walk from CARGO_MANIFEST_DIR (= crates/stringcheese-bench) up
    // one level to `crates/`, then down into the hf crate's fixture
    // vocabs directory so the bench reuses whichever real vocabs a
    // contributor already provisioned for the conformance runner.
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("bench crate lives inside crates/")
        .join("stringcheese-tokenizer-hf")
        .join("tests")
        .join("conformance")
        .join("vocabs")
        .join(checkpoint)
        .join("tokenizer.json");
    if p.is_file() {
        return Some(p);
    }
    None
}

/// Locate the tiktoken plaintext `mergeable_ranks` blob for `variant`.
/// Two roots, priority order:
///
/// 1. `$TIKTOKEN_PARITY_DATA_DIR/<variant>.tiktoken`.
/// 2. `~/.cache/stringcheese-tokenizer-tiktoken/<variant>.tiktoken`
///    (or `$XDG_CACHE_HOME/stringcheese-tokenizer-tiktoken/…` when
///    set). Matches the cache path the
///    `stringcheese-tokenizer-tiktoken-conformance` fetch layer
///    writes to.
fn find_tiktoken_plaintext(variant: &str) -> Option<PathBuf> {
    let filename = format!("{variant}.tiktoken");
    if let Some(root) = env::var_os(TIKTOKEN_DIR_ENV) {
        let p = PathBuf::from(root).join(&filename);
        if p.is_file() {
            return Some(p);
        }
    }
    let cache_root = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let p = cache_root
        .join("stringcheese-tokenizer-tiktoken")
        .join(&filename);
    if p.is_file() { Some(p) } else { None }
}

// ---------------------------------------------------------------------
// Load helpers — one per engine.
// ---------------------------------------------------------------------

/// Build the `stringcheese-tokenizer-hf` side from a real HF
/// `tokenizer.json`, panicking on any load failure (bench only ever
/// reaches this after a soft-skip guard so a failure here is a real
/// bug).
fn load_stringcheese_hf(path: &Path) -> HfTokenizer {
    let json = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let config =
        parse_tokenizer_json(&json).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    to_tokenizer(&config).unwrap_or_else(|e| panic!("materialise {}: {e:?}", path.display()))
}

/// Build the `tokenizers-rs` side from the same on-disk
/// `tokenizer.json`. The two engines share the exact same bytes so
/// the comparison is apples-to-apples.
fn load_tokenizers_rs(path: &Path) -> tokenizers::Tokenizer {
    tokenizers::Tokenizer::from_file(path)
        .unwrap_or_else(|e| panic!("tokenizers-rs failed to load {}: {e}", path.display()))
}

/// Build the `stringcheese-tokenizer-hf` `cl100k_base` side from a real
/// tiktoken plaintext blob. Mirrors
/// `stringcheese_tokenizer_tiktoken_conformance::parity::build_stringcheese_tokenizer`
/// — we duplicate the shape here because that crate is deliberately
/// workspace-excluded and cannot be depended on from a workspace
/// member.
fn load_stringcheese_cl100k(plaintext: &[u8]) -> BpeTokenizer {
    let (vocab_entries, merge_entries) =
        builder::build_scud_from_tiktoken(plaintext).expect("cl100k tiktoken plaintext must parse");

    let mut vocab = BpeVocabulary::new();
    let mut sorted = vocab_entries;
    sorted.sort_by_key(|e| e.id);
    for entry in sorted {
        vocab
            .insert(entry.id, entry.bytes)
            .expect("cl100k vocab insert");
    }

    let mut merges = BpeMergeTable::new();
    for m in merge_entries {
        let left_bytes = vocab
            .bytes(m.left_id)
            .expect("merge left id in vocab")
            .to_vec();
        let right_bytes = vocab
            .bytes(m.right_id)
            .expect("merge right id in vocab")
            .to_vec();
        merges.insert(left_bytes, right_bytes, m.rank);
    }

    let pre = RegexPreTokenizer::new(TIKTOKEN_CANONICAL_PATTERN)
        .expect("cl100k pre-tokenizer regex must compile");

    BpeTokenizer::from_parts(merges, vocab).with_pre_tokenizer(PreTokenizerRegex::regex(pre))
}

// ---------------------------------------------------------------------
// Encode helpers — one per engine. Route through the trait so the
// bench measures the full normalize → pre-tokenize → model →
// post-process pipeline rather than any inherent shortcut method.
// ---------------------------------------------------------------------

fn encode_stringcheese_hf(tok: &HfTokenizer, input: &str) -> Vec<u32> {
    match tok {
        HfTokenizer::Bpe(bpe) => {
            Tokenizer::encode(bpe.as_ref(), input)
                .expect("stringcheese-hf BPE encode")
                .ids
        }
        HfTokenizer::WordPiece(wp) => {
            Tokenizer::encode(wp, input)
                .expect("stringcheese-hf WordPiece encode")
                .ids
        }
        HfTokenizer::Unigram(uni) => {
            Tokenizer::encode(uni, input)
                .expect("stringcheese-hf Unigram encode")
                .ids
        }
        other => panic!("tokenizer_hf bench: unrecognised HfTokenizer variant {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Bench group runners.
// ---------------------------------------------------------------------

/// Skip-with-message when a required vocab is missing. Keeps the
/// bench binary green in CI even without provisioned vocabs.
fn eprintln_skip(group: &str, why: &str) {
    eprintln!("SKIP tokenizer_hf/encode/{group}: {why}");
}

fn bench_gpt2(c: &mut Criterion) {
    let Some(path) = find_tokenizer_json("gpt2") else {
        eprintln_skip(
            "gpt2",
            &format!(
                "no tokenizer.json. Provide one at ${VOCAB_DIR_ENV}/gpt2/tokenizer.json or \
                 crates/stringcheese-tokenizer-hf/tests/conformance/vocabs/gpt2/tokenizer.json."
            ),
        );
        return;
    };

    let ours = load_stringcheese_hf(&path);
    let theirs = load_tokenizers_rs(&path);

    let mut group = c.benchmark_group("tokenizer_hf/encode/gpt2");
    for &bytes in INPUT_SIZES {
        let input = deterministic_prose(bytes);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("stringcheese_hf", bytes),
            &input,
            |b, input| {
                b.iter(|| {
                    let ids = encode_stringcheese_hf(&ours, black_box(input));
                    black_box(ids);
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("tokenizers_rs", bytes),
            &input,
            |b, input| {
                b.iter(|| {
                    let enc = theirs
                        .encode(black_box(input.as_str()), false)
                        .expect("tokenizers-rs encode");
                    black_box(enc);
                });
            },
        );
    }
    group.finish();
}

fn bench_cl100k_base(c: &mut Criterion) {
    let Some(blob_path) = find_tiktoken_plaintext("cl100k_base") else {
        eprintln_skip(
            "cl100k_base",
            &format!(
                "no cl100k_base.tiktoken plaintext blob. Provide one at \
                 ${TIKTOKEN_DIR_ENV}/cl100k_base.tiktoken or \
                 ~/.cache/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken. The \
                 `stringcheese-tokenizer-tiktoken-conformance` crate populates the latter \
                 automatically when run with `--features parity-real-vocab`."
            ),
        );
        return;
    };
    let plaintext =
        fs::read(&blob_path).unwrap_or_else(|e| panic!("read {}: {e}", blob_path.display()));
    let ours = load_stringcheese_cl100k(&plaintext);
    let theirs = tiktoken_rs::cl100k_base().expect("tiktoken-rs cl100k_base load");

    let mut group = c.benchmark_group("tokenizer_hf/encode/cl100k_base");
    for &bytes in INPUT_SIZES {
        let input = deterministic_prose(bytes);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("stringcheese_hf", bytes),
            &input,
            |b, input| {
                b.iter(|| {
                    let enc = ours.encode(black_box(input)).expect("cl100k encode");
                    black_box(enc);
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("tiktoken_rs", bytes),
            &input,
            |b, input| {
                b.iter(|| {
                    let ids = theirs.encode_ordinary(black_box(input));
                    black_box(ids);
                });
            },
        );
    }
    group.finish();
}

fn bench_llama_2_7b(c: &mut Criterion) {
    let Some(path) = find_tokenizer_json("llama-2-7b-hf") else {
        eprintln_skip(
            "llama_2_7b",
            &format!(
                "no tokenizer.json. Provide one at ${VOCAB_DIR_ENV}/llama-2-7b-hf/tokenizer.json \
                 or crates/stringcheese-tokenizer-hf/tests/conformance/vocabs/llama-2-7b-hf/tokenizer.json. \
                 Exercises the SentencePiece byte_fallback + Metaspace pre-tokenizer path."
            ),
        );
        return;
    };

    let ours = load_stringcheese_hf(&path);
    let theirs = load_tokenizers_rs(&path);

    let mut group = c.benchmark_group("tokenizer_hf/encode/llama_2_7b");
    for &bytes in INPUT_SIZES {
        let input = deterministic_prose(bytes);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("stringcheese_hf", bytes),
            &input,
            |b, input| {
                b.iter(|| {
                    let ids = encode_stringcheese_hf(&ours, black_box(input));
                    black_box(ids);
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("tokenizers_rs", bytes),
            &input,
            |b, input| {
                b.iter(|| {
                    let enc = theirs
                        .encode(black_box(input.as_str()), false)
                        .expect("tokenizers-rs encode");
                    black_box(enc);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    tokenizer_hf,
    bench_gpt2,
    bench_cl100k_base,
    bench_llama_2_7b
);
criterion_main!(tokenizer_hf);
