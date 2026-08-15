//! Baseline throughput benchmarks for the sliding-window n-gram
//! producers in `stringcheese-ngram`.
//!
//! Mirrors the shape of the sibling bench files
//! (`stringcheese-compare` / `stringcheese-manip` /
//! `stringcheese-textsplit`): one bench file, criterion-owned `main`
//! via `harness = false`, groups named `<unit>/<flavor>/<n>/<len>`.
//! Regression trip-wire + last-measured baseline table for the three
//! unit-explicit n-gram families this crate ships.
//!
//! # Units
//!
//! Three groups, one per shipped n-gram unit:
//!
//! * `char` — [`char_ngrams`], sliding window over Unicode code
//!   points. Boundaries are collected once up-front (a `char_indices`
//!   walk), then the iterator hands back `&str` slices. ASCII input
//!   is the fast path (byte-count == code-point count); non-ASCII
//!   input widens boundaries and shrinks per-scalar throughput.
//! * `byte` — [`byte_ngrams`], a thin wrapper over
//!   [`slice::windows`]. Zero allocation, zero UTF-8 assumption; the
//!   fastest arm and the reference for what a bare-metal sliding
//!   window looks like.
//! * `grapheme` — [`grapheme_ngrams`], only compiled when the
//!   `graphemes` feature is on. Grapheme clusters via ICU4X;
//!   throughput sits behind `char`/`byte` because the boundary
//!   detection is the load-bearing cost. This bench compiles the
//!   arm only when `stringcheese-ngram` was built with `--features
//!   graphemes`, which the default `cargo bench --all-features`
//!   invocation turns on.
//!
//! # Sizes
//!
//! Two byte lengths per (unit, flavor, n): 256 B / 4 KiB. Chosen to
//! match the "cache-resident vs L1/L2 pressure" split the sibling
//! benches document; wider sweeps didn't buy much extra signal
//! given the algorithms are all O(N) with a small constant.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `ascii` — deterministic English prose. Every scalar is
//!   one-byte; boundary collection and byte iteration coincide,
//!   which is what `char_ngrams` is optimised for.
//! * `utf8` — Cyrillic prose. Every scalar is two bytes;
//!   `char_ngrams`' boundary collection now scans twice as many
//!   bytes to gather the same number of scalars. `byte_ngrams` is
//!   unaffected — bytes are bytes.
//!
//! # `n`-values
//!
//! Two per cell: `n = 3` (character trigrams — the standard shingle
//! width for near-duplicate / Jaccard pipelines) and `n = 5` (the
//! wider window Broder recommends when the corpus is homogeneous
//! and the base rate of shared 3-grams is too high). No allocation
//! difference between the two; the difference is per-gram slice
//! width.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-ngram
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-ngram -- char
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-ngram --no-run
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

use stringcheese_ngram::{byte_ngrams, char_ngrams};

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG; every `cargo bench` invocation feeds byte-identical
// inputs so criterion's noise floor stays meaningful across runs.
// ---------------------------------------------------------------------------

/// ASCII prose seed. Cycled to build inputs of arbitrary size.
const ASCII_SEED: &str = "the quick brown fox jumps over the lazy dog. ";

/// Cyrillic prose seed. Every scalar is two bytes in UTF-8, so a
/// N-byte input carries N/2 code points — the doubled boundary
/// walk `char_ngrams` has to do is what the `utf8` flavor
/// measures.
const CYRILLIC_SEED: &str = "быстрая коричневая лиса прыгает через ленивую собаку. ";

/// Build a string of *at least* `bytes` bytes by cycling `seed`.
fn build_input(seed: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + seed.len());
    while out.len() < bytes {
        out.push_str(seed);
    }
    out
}

/// Byte lengths swept by every (unit, flavor, n) cell.
const SIZES: &[usize] = &[256, 4 * 1024];

/// `n` values swept per cell — the two windows Jaccard-style
/// pipelines actually reach for.
const NS: &[usize] = &[3, 5];

/// The two flavors every group runs.
fn flavors() -> [(&'static str, &'static str); 2] {
    [("ascii", ASCII_SEED), ("utf8", CYRILLIC_SEED)]
}

// ---------------------------------------------------------------------------
// n-gram groups. Every iterator is fully consumed (`.count()`) so
// nothing gets optimised away. Throughput reported over *input
// bytes* — higher is better.
// ---------------------------------------------------------------------------

fn bench_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("ngram/char");
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            for &n in NS {
                group.throughput(Throughput::Bytes(input.len() as u64));
                let id = BenchmarkId::new(format!("{flavor}/n{n}"), input.len());
                group.bench_with_input(id, &input, |bencher, input| {
                    bencher.iter(|| black_box(char_ngrams(black_box(input), n).count()));
                });
            }
        }
    }
    group.finish();
}

fn bench_byte(c: &mut Criterion) {
    let mut group = c.benchmark_group("ngram/byte");
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            for &n in NS {
                let bytes = input.as_bytes();
                group.throughput(Throughput::Bytes(bytes.len() as u64));
                let id = BenchmarkId::new(format!("{flavor}/n{n}"), bytes.len());
                group.bench_with_input(id, bytes, |bencher, bytes| {
                    bencher.iter(|| black_box(byte_ngrams(black_box(bytes), n).count()));
                });
            }
        }
    }
    group.finish();
}

#[cfg(feature = "graphemes")]
fn bench_grapheme(c: &mut Criterion) {
    use stringcheese_ngram::grapheme_ngrams;
    let mut group = c.benchmark_group("ngram/grapheme");
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            for &n in NS {
                group.throughput(Throughput::Bytes(input.len() as u64));
                let id = BenchmarkId::new(format!("{flavor}/n{n}"), input.len());
                group.bench_with_input(id, &input, |bencher, input| {
                    bencher.iter(|| black_box(grapheme_ngrams(black_box(input), n).count()));
                });
            }
        }
    }
    group.finish();
}

#[cfg(not(feature = "graphemes"))]
fn bench_grapheme(_c: &mut Criterion) {
    // Grapheme arm compiled out when the `graphemes` feature is
    // off. `cargo bench --all-features` (the shape the CI job uses)
    // turns it on; a bare `cargo bench` skips it and still emits
    // valid output for the other two units.
}

criterion_group!(ngram, bench_char, bench_byte, bench_grapheme);
criterion_main!(ngram);
