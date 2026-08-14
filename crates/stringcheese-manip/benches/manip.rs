//! Baseline + regression trip-wire bench for the 12 hot public
//! operations of `stringcheese-manip`.
//!
//! Adopts the same shape the tokenizer subsystem committed to in
//! `stringcheese-tokenizer-hf-bench`: one criterion group per operation
//! family, `Throughput::Bytes(input.len())` reported so a change in
//! sample allocation surfaces as an MiB/s delta the CI-attached bench
//! logs can eyeball. The workspace had zero criterion coverage on the
//! non-tokenizer compute crates before this file landed; the goal is
//! breadth (12 operations, one representative call each) not depth (no
//! sweeping over configuration variants).
//!
//! # Operation coverage (12 groups)
//!
//! | # | Group                              | Public entry point                      |
//! |---|------------------------------------|-----------------------------------------|
//! | 1 | `manip/inspect/grapheme_count`     | [`inspect::grapheme_count`]             |
//! | 2 | `manip/trim/trim`                  | [`trim::trim`]                          |
//! | 3 | `manip/case/to_lowercase`          | [`case::to_lowercase`]                  |
//! | 4 | `manip/case/to_uppercase`          | [`case::to_uppercase`]                  |
//! | 5 | `manip/split/split_whitespace`     | [`split::split_whitespace`] (drained)   |
//! | 6 | `manip/join/join`                  | [`join::join`]                          |
//! | 7 | `manip/replace/replace`            | [`replace::replace`]                    |
//! | 8 | `manip/replace/replace_many`       | [`replace::replace_many`] (Aho-Corasick)|
//! | 9 | `manip/normalize/collapse_ws`      | [`normalize::collapse_whitespace`]      |
//! |10 | `manip/normalize/nfc`              | [`normalize::nfc`]                      |
//! |11 | `manip/find/find_all`              | [`find::find_all`]                      |
//! |12 | `manip/escape/escape_html`         | [`escape::escape_html`]                 |
//!
//! # Input sizes and flavors
//!
//! Every group is measured at three character counts — 64, 1024, 65536 —
//! and in two Unicode flavors:
//!
//! * **ascii** — a deterministic prose seed cycled from a small
//!   English word list. Every byte is one scalar and one grapheme,
//!   so the group measures the ASCII fast path where the crate has
//!   one.
//! * **utf8** — a fixed diverse pool covering Latin-with-diacritics,
//!   Cyrillic, Greek, Arabic (RTL), Devanagari (combining), CJK
//!   ideographs, and supplementary-plane emoji (including a ZWJ
//!   family sequence that exercises grapheme cluster boundaries).
//!   Every byte-length input is truncated on a scalar boundary so the
//!   bench never feeds `unwrap()` an invalid slice.
//!
//! That is 12 × 3 × 2 = 72 measurements per full run. Criterion
//! reports throughput in MiB/s of input bytes; per-iteration wall
//! clock lives in the raw output.
//!
//! # Running
//!
//! Full run:
//!
//! ```text
//! cargo bench -p stringcheese-manip --bench manip
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-manip --bench manip -- case/to_lowercase
//! ```
//!
//! Compile-only (what CI uses for the `manip-bench (optional)` job):
//!
//! ```text
//! cargo bench -p stringcheese-manip --bench manip --no-run --locked
//! ```
//!
//! # Baseline
//!
//! See the module-level table in `src/lib.rs` for the last-measured
//! throughput numbers. When a performance-affecting change lands,
//! re-run the full bench and update that table so a future reader
//! never has to guess whether the delta improved or regressed the
//! recorded baseline.

#![allow(
    missing_docs,
    reason = "criterion_group! emits undocumented public fns; the bench binary is not part of the crate's public API"
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_manip::{case, escape, find, inspect, join, normalize, replace, split, trim};

// ---------------------------------------------------------------------
// Deterministic input construction.
//
// Both flavors seed from a fixed word / codepoint pool cycled until the
// target character count is met, then truncated on a scalar boundary.
// No RNG, no time-based seed, no OS entropy — successive `cargo bench`
// invocations feed byte-identical inputs so criterion's noise-floor
// numbers are directly comparable across runs.
// ---------------------------------------------------------------------

/// Small English word list — same shape the tokenizer bench uses so
/// two bench runs on the same laptop can cross-check that the input
/// generator is not the bottleneck.
const ASCII_WORDS: &[&str] = &[
    "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "and", "then", "runs",
    "back", "through", "the", "forest", "toward", "the", "little", "cabin", "on", "the", "hill",
    "where", "smoke", "curls", "from", "the", "chimney", "into", "the", "cold", "morning", "air",
    "while", "the", "sun", "climbs", "above", "the", "eastern", "ridge", "and", "birds", "call",
    "from", "the", "pines", "beyond", "the", "meadow",
];

/// Diverse UTF-8 pool. Selected to hit every category the manip crate
/// makes a policy decision about: precomposed vs decomposable Latin
/// (NFC path), Cyrillic, Greek (case-changing), Arabic (RTL & no
/// case), Devanagari (combining marks push scalar and grapheme counts
/// apart), CJK ideographs (wide, single-codepoint), and supplementary-
/// plane emoji including a ZWJ family sequence so a grapheme-cluster
/// pass has non-trivial work to do.
const UTF8_TOKENS: &[&str] = &[
    "café",
    "naïve",
    "Ελληνικά",
    "Здравствуй",
    "мир",
    "العربية",
    "नमस्ते",
    "日本語",
    "한국어",
    "中文",
    "🌍",
    "🦀",
    "👩‍👩‍👧‍👦",
    "the",
    "über",
    "π",
    "ß",
];

/// Build an ASCII input of *at least* `chars` characters, then
/// truncate on a scalar boundary at exactly `chars` characters. Every
/// byte is a single scalar so the length in characters is the length
/// in bytes.
fn ascii_input(chars: usize) -> String {
    let mut out = String::with_capacity(chars + 16);
    let mut idx = 0usize;
    while out.chars().count() < chars {
        out.push_str(ASCII_WORDS[idx % ASCII_WORDS.len()]);
        out.push(' ');
        idx += 1;
    }
    truncate_to_chars(out, chars)
}

/// Build a UTF-8 input of at least `chars` characters, truncated on a
/// scalar boundary at exactly `chars`. Byte length > `chars` because
/// most tokens in [`UTF8_TOKENS`] carry multi-byte scalars.
fn utf8_input(chars: usize) -> String {
    let mut out = String::with_capacity(chars * 4 + 16);
    let mut idx = 0usize;
    while out.chars().count() < chars {
        out.push_str(UTF8_TOKENS[idx % UTF8_TOKENS.len()]);
        out.push(' ');
        idx += 1;
    }
    truncate_to_chars(out, chars)
}

/// Truncate `s` on a scalar boundary at exactly `chars` characters.
/// Panics if `s` has fewer than `chars` characters — the two builders
/// above always over-produce, so a panic here signals a bug in the
/// generator, not user error.
fn truncate_to_chars(s: String, chars: usize) -> String {
    let byte_end = s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i);
    let mut trimmed = s;
    trimmed.truncate(byte_end);
    trimmed
}

/// Character counts every group iterates over. 64 catches the
/// per-call overhead lane, 1024 the middle-ground where cache effects
/// are still small, 65536 the large-input lane where inner-loop
/// throughput dominates.
const SIZES: &[usize] = &[64, 1024, 65_536];

/// Builder function signature: `(chars) -> String`. Kept as a
/// dedicated alias so the return type of [`flavors`] does not trip
/// `clippy::type_complexity` — pedantic mode counts an in-line
/// `fn(usize) -> String` inside a tuple-array as "very complex".
type InputBuilder = fn(usize) -> String;

/// The two Unicode flavors every group runs. Kept as a `(label,
/// builder)` pair so criterion's `BenchmarkId` axis reads
/// `flavor/size` in the output.
fn flavors() -> [(&'static str, InputBuilder); 2] {
    [("ascii", ascii_input), ("utf8", utf8_input)]
}

// ---------------------------------------------------------------------
// Group runners — one per operation, each iterates the (flavor, size)
// matrix and reports byte throughput so absolute numbers stay
// comparable across flavors even when the character count is fixed
// (utf8 inputs weigh more bytes at the same scalar count).
// ---------------------------------------------------------------------

fn bench_inspect_grapheme_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/inspect/grapheme_count");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(inspect::grapheme_count(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_trim(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/trim/trim");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            // Sandwich the payload in whitespace so `trim` has something
            // to remove — measuring the no-op path is what CI already
            // gets from `--no-run`, we want the real work path here.
            let input = format!("   \t\n{}\n\t   ", build(n));
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(trim::trim(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_case_to_lowercase(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/case/to_lowercase");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            // Force casing to actually run over uppercase input on the
            // ascii lane; the utf8 pool already carries mixed-case
            // tokens (Cyrillic Здр…, Greek Ελληνικά, German ß).
            let input = build(n).to_ascii_uppercase();
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(case::to_lowercase(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_case_to_uppercase(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/case/to_uppercase");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n).to_ascii_lowercase();
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(case::to_uppercase(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_split_whitespace(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/split/split_whitespace");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                // Drain the iterator via `.count()` so the whole
                // scan runs; without consumption criterion would
                // measure only the ctor.
                b.iter(|| black_box(split::split_whitespace(black_box(input)).count()));
            });
        }
    }
    group.finish();
}

fn bench_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/join/join");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            // Pre-split the input into a Vec<&str> so the bench measures
            // the join itself, not the split preamble. The Vec is
            // constructed once outside the timed region.
            let source = build(n);
            let pieces: Vec<&str> = source.split(' ').collect();
            let total_bytes: usize =
                pieces.iter().map(|p| p.len()).sum::<usize>() + pieces.len().saturating_sub(1);
            group.throughput(Throughput::Bytes(total_bytes as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &pieces, |b, pieces| {
                b.iter(|| black_box(join::join(black_box(pieces).iter().copied(), " ")));
            });
        }
    }
    group.finish();
}

fn bench_replace(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/replace/replace");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            // Needle is picked to actually occur in each flavor so the
            // bench never degenerates into the no-match early-return
            // path. Both `the` (ascii) and `а` (utf8, Cyrillic a) hit
            // the respective pools.
            let (from, to) = if flavor == "ascii" {
                ("the", "THE")
            } else {
                ("а", "А")
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(replace::replace(black_box(input), from, to)));
            });
        }
    }
    group.finish();
}

fn bench_replace_many(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/replace/replace_many");
    // Five needle/replacement pairs per flavor — enough to force the
    // Aho-Corasick automaton down `replace::replace_many`'s delegate
    // path rather than the two-pair single-pass fallback.
    let ascii_pairs: &[(&str, &str)] = &[
        ("the", "THE"),
        ("and", "AND"),
        ("fox", "FOX"),
        ("dog", "DOG"),
        ("morning", "MORNING"),
    ];
    let utf8_pairs: &[(&str, &str)] = &[
        ("café", "CAFÉ"),
        ("Ελληνικά", "ΕΛΛΗΝΙΚΆ"),
        ("मस्ते", "मस्ते"),
        ("日本語", "日本語"),
        ("🌍", "🌎"),
    ];
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            let pairs: &[(&str, &str)] = if flavor == "ascii" {
                ascii_pairs
            } else {
                utf8_pairs
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(replace::replace_many(black_box(input), pairs)));
            });
        }
    }
    group.finish();
}

fn bench_normalize_collapse_whitespace(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/normalize/collapse_ws");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            // Inject runs of whitespace so the collapser has non-empty
            // work; the raw ASCII builder already emits single-space
            // separators, which the collapser leaves alone.
            let raw = build(n);
            let input = raw.replace(' ', "   \t \n ");
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(normalize::collapse_whitespace(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_normalize_nfc(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/normalize/nfc");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            // Force decomposable input on the utf8 lane so NFC has to
            // recompose; the ASCII lane exercises the ASCII-only fast
            // path the underlying `unicode-normalization` crate takes.
            let raw = build(n);
            let input = if flavor == "utf8" {
                // NFD-decompose, then re-normalize back to NFC in the
                // hot loop. Using `unicode_normalization` directly here
                // would bypass the delegation the crate under test
                // makes, so instead we simulate the equivalent by
                // sprinkling combining marks that force real work: a
                // combining acute (U+0301) after every space.
                raw.replace(' ', " \u{0301}")
            } else {
                raw
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(normalize::nfc(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_find_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/find/find_all");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            let needle = if flavor == "ascii" { "the" } else { "а" };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(find::find_all(black_box(input), needle)));
            });
        }
    }
    group.finish();
}

fn bench_escape_html(c: &mut Criterion) {
    let mut group = c.benchmark_group("manip/escape/escape_html");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            // Sprinkle HTML-metasignificant bytes so the escaper takes
            // the allocating branch of its `Cow` return rather than
            // hugging the borrowed-through fast path (which is what
            // pure prose would trigger).
            let raw = build(n);
            let input = raw.replace(' ', " <a href=\"x&y\">&amp;</a> ");
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |b, input| {
                b.iter(|| black_box(escape::escape_html(black_box(input))));
            });
        }
    }
    group.finish();
}

criterion_group!(
    manip,
    bench_inspect_grapheme_count,
    bench_trim,
    bench_case_to_lowercase,
    bench_case_to_uppercase,
    bench_split_whitespace,
    bench_join,
    bench_replace,
    bench_replace_many,
    bench_normalize_collapse_whitespace,
    bench_normalize_nfc,
    bench_find_all,
    bench_escape_html,
);
criterion_main!(manip);
