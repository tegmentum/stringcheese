//! Pattern-matching throughput — Literal / Wildcard / Glob.
//!
//! All three implement the same `Pattern` trait but sit on very
//! different backends:
//!
//! - **Literal** wraps `memchr` (SIMD-accelerated substring
//!   search on `x86_64` / aarch64). Byte-throughput ceiling.
//! - **Wildcard** wraps `globset` (escape brackets first) →
//!   compile to a `regex::bytes::Regex` → match. Regex-engine
//!   throughput once compiled.
//! - **Glob** — same pipeline as Wildcard but keeps character
//!   classes.
//!
//! The bench measures **matching** throughput on already-compiled
//! patterns; construction cost is a one-time hit that most
//! callers amortise across a corpus. Compilation is measured in a
//! separate group to catch regressions on that side too.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::random_ascii;
use stringcheese_pattern::{Glob, Literal, MatchUnit, Pattern, Wildcard};

const LENGTHS: &[usize] = &[128, 1024, 8192];

fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn text_for(len: usize, salt: u64) -> String {
    let bytes = random_ascii(len, seed_for(len, salt));
    String::from_utf8(bytes).expect("random_ascii returns valid UTF-8")
}

// ---------------------------------------------------------------------
// Matching — the hot path.
// ---------------------------------------------------------------------

fn bench_literal_find_all(c: &mut Criterion) {
    // A short 3-byte needle that's likely to hit many times in a
    // random ASCII stream — measures memchr-throughput on the
    // find_iter loop.
    let mut group = c.benchmark_group("pattern/literal/find_iter");
    let pat = Literal::new("aBc", MatchUnit::Bytes);
    for &len in LENGTHS {
        let haystack = text_for(len, 0xD1);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &haystack, |bencher, h| {
            bencher.iter(|| {
                let count = pat.find_iter(black_box(h)).count();
                black_box(count);
            });
        });
    }
    group.finish();
}

fn bench_wildcard_find_anywhere(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern/wildcard/find_anywhere");
    // "a?b" — any 3-char run with a and b at the ends. Realistic
    // shape for a search-glob-style filter.
    let pat = Wildcard::anywhere("a?b", MatchUnit::Bytes);
    for &len in LENGTHS {
        let haystack = text_for(len, 0xD2);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &haystack, |bencher, h| {
            bencher.iter(|| {
                let count = pat.find_iter(black_box(h)).count();
                black_box(count);
            });
        });
    }
    group.finish();
}

fn bench_glob_find_anywhere(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern/glob/find_anywhere");
    // Character class + wildcard — the shape a caller writes for
    // "find any alphanumeric run flanked by 'a' and 'b'".
    let pat = Glob::anywhere("a[a-z0-9]b", MatchUnit::Bytes);
    for &len in LENGTHS {
        let haystack = text_for(len, 0xD3);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &haystack, |bencher, h| {
            bencher.iter(|| {
                let count = pat.find_iter(black_box(h)).count();
                black_box(count);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Compilation — one-time cost, but worth tracking.
// ---------------------------------------------------------------------

fn bench_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern/construct");

    group.bench_function("literal", |bencher| {
        bencher.iter(|| black_box(Literal::new(black_box("needle"), MatchUnit::Bytes)));
    });

    group.bench_function("wildcard", |bencher| {
        bencher.iter(|| black_box(Wildcard::new(black_box("a?b*c"), MatchUnit::Bytes)));
    });

    group.bench_function("glob", |bencher| {
        bencher.iter(|| black_box(Glob::new(black_box("a[a-z0-9]b*c"), MatchUnit::Bytes)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_literal_find_all,
    bench_wildcard_find_anywhere,
    bench_glob_find_anywhere,
    bench_construction,
);
criterion_main!(benches);
