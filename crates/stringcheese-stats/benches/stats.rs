//! Baseline throughput benchmarks for the four public characterization
//! primitives in `stringcheese-stats`.
//!
//! Mirrors the shape of the sibling bench files (`stringcheese-compare`
//! / `stringcheese-manip` / `stringcheese-pattern`): one bench file,
//! criterion-owned `main` via `harness = false`, groups named
//! `<primitive>/<flavor>/<len>`. Regression trip-wire + last-measured
//! baseline table for a family whose current baselines used to live in
//! the intra-workspace `stringcheese-bench` file only.
//!
//! # Primitives
//!
//! Four groups, one per public entry point:
//!
//! * `entropy` — [`entropy`], Shannon entropy over Unicode code points.
//!   Only available under the `std` feature (uses `f64::log2`); this
//!   bench compiles under `default = ["std"]` so the group is always
//!   present.
//! * `histogram` — [`Histogram::of`], full Unicode general-category
//!   tally plus the coarse major-category roll-up.
//! * `ratios` — [`Ratios::of`], printable / control / whitespace /
//!   digit / alphabetic / punctuation ratios.
//! * `lengths` — [`Lengths::of`], byte + code-point (+ optional
//!   grapheme) count triple. The `graphemes` feature is off in this
//!   bench so the third field is `None` — the group measures the hot
//!   two-field path, which is what the vast majority of callers reach
//!   for.
//!
//! # Sizes
//!
//! Three input byte lengths per (primitive, flavor): 1 KiB / 10 KiB /
//! 100 KiB. Chosen to match the shape the crate's baseline table
//! already documents (which used 128 B / 1 KB / 8 KB), one order of
//! magnitude up so the character-scanning primitives spend most of
//! their time in the inner loop and not in per-call setup.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `ascii` — deterministic ASCII prose, drawn from a small English
//!   word list. Every byte is a scalar; the ASCII fast paths in
//!   [`Ratios::of`] and [`Histogram::of`] fire.
//! * `utf8` — a fixed diverse Unicode pool spanning Latin diacritics,
//!   Cyrillic, Greek, Arabic, Devanagari, CJK ideographs, and emoji.
//!   This is the general path in every primitive here — no ASCII fast
//!   path fires.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-stats
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-stats -- entropy
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-stats --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_stats::{Histogram, Lengths, Ratios, entropy};

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as
// `stringcheese-manip/benches/manip.rs`: fixed word / codepoint pool,
// cycled until the target byte length is met. No RNG, no time-based
// seed; successive `cargo bench` invocations feed byte-identical inputs.
// ---------------------------------------------------------------------------

/// Small English word list (same shape the manip bench uses).
const ASCII_WORDS: &[&str] = &[
    "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "and", "then", "runs",
    "back", "through", "the", "forest", "toward", "the", "little", "cabin", "on", "the", "hill",
    "where", "smoke", "curls", "from", "the", "chimney", "into", "the", "cold", "morning", "air",
];

/// Diverse UTF-8 pool selected to exercise every category the
/// histogram / ratios primitives make a policy decision about:
/// precomposed Latin, Cyrillic, Greek (letter-heavy), Arabic (RTL &
/// no case), Devanagari (combining), CJK ideographs (wide), and
/// supplementary-plane emoji.
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
    "über",
    "π",
    "ß",
];

/// Build an ASCII input of *at least* `bytes` bytes. The word pool is
/// ASCII-only, so byte length and scalar length coincide.
fn ascii_input(bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + 32);
    let mut idx = 0usize;
    while out.len() < bytes {
        out.push_str(ASCII_WORDS[idx % ASCII_WORDS.len()]);
        out.push(' ');
        idx += 1;
    }
    out
}

/// Build a UTF-8 input of at least `bytes` bytes. Character counts are
/// smaller than the byte count because most tokens carry multi-byte
/// scalars — that's the intended shape (the histogram / ratios paths
/// pay per-scalar cost, not per-byte).
fn utf8_input(bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + 32);
    let mut idx = 0usize;
    while out.len() < bytes {
        out.push_str(UTF8_TOKENS[idx % UTF8_TOKENS.len()]);
        out.push(' ');
        idx += 1;
    }
    out
}

/// Byte lengths swept by every (primitive, flavor) cell.
const SIZES: &[usize] = &[1024, 10 * 1024, 100 * 1024];

/// Builder function signature: `(bytes) -> String`. Kept as a
/// dedicated alias so the return type of [`flavors`] does not trip
/// `clippy::type_complexity`.
type InputBuilder = fn(usize) -> String;

/// The two flavors every group runs.
fn flavors() -> [(&'static str, InputBuilder); 2] {
    [("ascii", ascii_input), ("utf8", utf8_input)]
}

// ---------------------------------------------------------------------------
// Primitive groups. Throughput is reported over *input bytes* so the
// per-byte cost is directly comparable between flavors at the same
// character count (utf8 chars weigh 2-4 bytes each).
// ---------------------------------------------------------------------------

fn bench_entropy(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats/entropy");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |bencher, input| {
                bencher.iter(|| black_box(entropy(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_histogram(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats/histogram");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |bencher, input| {
                bencher.iter(|| black_box(Histogram::of(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats/ratios");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |bencher, input| {
                bencher.iter(|| black_box(Ratios::of(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_lengths(c: &mut Criterion) {
    // `Lengths::of` on this bench runs without the `graphemes` feature,
    // so the returned triple's `graphemes` field is `None`. That is
    // the hot path — grapheme counting adds an ICU4X call that a
    // future bench round could split into its own group.
    let mut group = c.benchmark_group("stats/lengths");
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |bencher, input| {
                bencher.iter(|| black_box(Lengths::of(black_box(input))));
            });
        }
    }
    group.finish();
}

criterion_group!(
    stats,
    bench_entropy,
    bench_histogram,
    bench_ratios,
    bench_lengths,
);
criterion_main!(stats);
