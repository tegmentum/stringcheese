//! Ident — case conversion, slugify, sanitize.
//!
//! Three surfaces exercised at three input sizes:
//!
//! - **Case conversion** (`to_case`) wraps `heck`. Bench covers
//!   each target case so any per-form regression surfaces.
//! - **Case detection** (`Case::detect`) is an in-house classifier;
//!   should be O(N) with tiny constant.
//! - **Slugify** wraps `deunicode` for transliteration. Bench
//!   includes both plain-ASCII input (fast path, minimal
//!   transliteration) and accented Latin input (deunicode's
//!   table lookups dominate).
//! - **Sanitize** is an in-house filter with a closure-based
//!   allow-set. Should be close to memcpy on already-valid input.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::Rng;
use stringcheese_ident::{Case, Sanitizer, Slugger, to_case};

const LENGTHS: &[usize] = &[64, 512, 4096];

/// Camel-cased ASCII identifier: `wordOneWordTwo…`. Approximate
/// `target_len` bytes.
fn camel_id(target_len: usize, seed: u64) -> String {
    let mut rng = Rng::from_seed(seed);
    let mut out = String::with_capacity(target_len + 16);
    let mut first = true;
    while out.len() < target_len {
        let word_len = 3 + (rng.next_u64() % 6) as usize;
        for i in 0..word_len {
            let b = (rng.next_u64() % 26) as u8 + b'a';
            let c = if i == 0 && !first {
                (b - b'a' + b'A') as char
            } else {
                b as char
            };
            out.push(c);
        }
        first = false;
    }
    out.truncate(target_len);
    out
}

/// Human-readable text with mixed case + accented Latin,
/// suitable for slugification. Tile the same accented phrase.
fn accented_text(target_len: usize) -> String {
    let tile = "Café Résumé Naïve Word One Two Three ";
    let mut out = String::with_capacity(target_len + tile.len());
    while out.len() < target_len {
        out.push_str(tile);
    }
    out.truncate(target_len);
    while !out.is_char_boundary(out.len()) {
        out.pop();
    }
    out
}

/// Plain ASCII text — deunicode's fast passthrough path.
fn plain_ascii_text(target_len: usize, seed: u64) -> String {
    let mut rng = Rng::from_seed(seed);
    let mut out = String::with_capacity(target_len);
    for _ in 0..target_len {
        // Alternating letters and occasional spaces.
        let x = rng.next_u64() % 30;
        let c = if x < 26 {
            (x as u8 + b'a') as char
        } else {
            ' '
        };
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------
// Case conversion — one Case variant per bench for legibility.
// ---------------------------------------------------------------------

fn bench_to_case(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/to_case");
    for &len in LENGTHS {
        let text = camel_id(len, 0x1234 ^ len as u64);
        group.throughput(Throughput::Bytes(text.len() as u64));

        for target in [Case::Snake, Case::Kebab, Case::Pascal, Case::ScreamingSnake] {
            group.bench_with_input(
                BenchmarkId::new(format!("{target:?}"), len),
                &text,
                |b, t| {
                    b.iter(|| black_box(to_case(black_box(t), target)));
                },
            );
        }
    }
    group.finish();
}

fn bench_case_detect(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/case_detect");
    for &len in LENGTHS {
        let text = camel_id(len, 0x5678 ^ len as u64);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &text, |b, t| {
            b.iter(|| black_box(Case::detect(black_box(t))));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Slugify — plain ASCII fast path vs accented (deunicode work).
// ---------------------------------------------------------------------

fn bench_slugify(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/slugify");
    let slugger = Slugger::default();
    for &len in LENGTHS {
        let plain = plain_ascii_text(len, 0x9ABC ^ len as u64);
        let accented = accented_text(len);
        group.throughput(Throughput::Bytes(len as u64));

        group.bench_with_input(BenchmarkId::new("plain_ascii", len), &plain, |b, t| {
            b.iter(|| black_box(slugger.slugify(black_box(t))));
        });
        group.bench_with_input(BenchmarkId::new("accented", len), &accented, |b, t| {
            b.iter(|| black_box(slugger.slugify(black_box(t))));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Sanitize — in-house filter.
// ---------------------------------------------------------------------

fn bench_sanitize(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/sanitize");
    let sanitizer = Sanitizer::default();
    for &len in LENGTHS {
        let plain = plain_ascii_text(len, 0xDEF0 ^ len as u64);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &plain, |b, t| {
            b.iter(|| black_box(sanitizer.sanitize(black_box(t))));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_to_case,
    bench_case_detect,
    bench_slugify,
    bench_sanitize,
);
criterion_main!(benches);
