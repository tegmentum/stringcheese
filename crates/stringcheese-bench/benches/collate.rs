//! Collate — UCA / Natural / ASCII-CI side-by-side.
//!
//! Three [`Collator`] implementations behind one trait. This
//! bench measures both **single comparison** (the hot inner loop
//! of any `sort_by`) and **whole-slice sort** (the realistic
//! caller pattern).
//!
//! ## Expected shape
//!
//! - **UcaCollator** — Unicode Collation Algorithm via `feruca`.
//!   Correctness ceiling; the slowest of the three because every
//!   compare walks the Unicode weight tables.
//! - **NaturalCollator<AsciiCiCollator>** — numeric-run-aware
//!   over an ASCII-fast-path inner. Free vs raw ASCII-CI when
//!   inputs have no digits; noticeable overhead per digit-run.
//! - **AsciiCiCollator** — byte-level lowercase compare. The
//!   throughput floor a caller pays when they know their input
//!   is ASCII.
//!
//! The ratio between UCA and ASCII-CI on ASCII input is what
//! validates the "pick the right collator" note in
//! `stringcheese-collate/src/lib.rs`. If UCA is only 2× slower
//! the note over-sells; if it's 20× slower the note under-sells.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// UCA / ASCII-CI / Natural are proper nouns that clippy's
// doc_markdown flags — wrapping every acronym harms readability
// of a bench file whose whole purpose is the comparison story.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::Rng;
use stringcheese_collate::{AsciiCiCollator, Collator, NaturalCollator, UcaCollator};

// ---------------------------------------------------------------------
// Corpus construction.
// ---------------------------------------------------------------------

fn ascii_words(count: usize, seed: u64) -> Vec<String> {
    let mut rng = Rng::from_seed(seed);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let len = 4 + (rng.next_u64() % 12) as usize;
        let mut word = String::with_capacity(len);
        for _ in 0..len {
            // Mix upper + lowercase to give ASCII-CI actual work.
            let base = if rng.next_u64().is_multiple_of(2) { b'a' } else { b'A' };
            let b = base + (rng.next_u64() % 26) as u8;
            word.push(b as char);
        }
        out.push(word);
    }
    out
}

/// File-like names ending in a version number — the natural-sort
/// use case. `file{N}` with N ∈ [1, 1000] chosen deterministically.
fn versioned_names(count: usize, seed: u64) -> Vec<String> {
    let mut rng = Rng::from_seed(seed);
    (0..count)
        .map(|_| {
            let n = 1 + (rng.next_u64() % 1000);
            format!("file{n}")
        })
        .collect()
}

// ---------------------------------------------------------------------
// Pairwise compare — the hot inner loop of every sort_by.
// ---------------------------------------------------------------------

fn bench_pairwise_compare(c: &mut Criterion) {
    let mut group = c.benchmark_group("collate/compare");
    // A dozen medium-length ASCII words; every bench call runs
    // `compare` on every adjacent pair.
    let words = ascii_words(12, 0x101);
    let uca = UcaCollator::new();
    let ascii = AsciiCiCollator::new();
    let natural = NaturalCollator::new(AsciiCiCollator::new());

    group.throughput(Throughput::Elements(11));

    group.bench_function("uca", |bencher| {
        bencher.iter(|| {
            let mut sink: i8 = 0;
            for w in words.windows(2) {
                sink = sink.wrapping_add(uca.compare(black_box(&w[0]), black_box(&w[1])) as i8);
            }
            black_box(sink);
        });
    });

    group.bench_function("ascii_ci", |bencher| {
        bencher.iter(|| {
            let mut sink: i8 = 0;
            for w in words.windows(2) {
                sink = sink.wrapping_add(ascii.compare(black_box(&w[0]), black_box(&w[1])) as i8);
            }
            black_box(sink);
        });
    });

    group.bench_function("natural_over_ascii_ci", |bencher| {
        bencher.iter(|| {
            let mut sink: i8 = 0;
            for w in words.windows(2) {
                sink = sink.wrapping_add(natural.compare(black_box(&w[0]), black_box(&w[1])) as i8);
            }
            black_box(sink);
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------
// Full-slice sort — realistic caller pattern.
// ---------------------------------------------------------------------

fn bench_sort_ascii(c: &mut Criterion) {
    let mut group = c.benchmark_group("collate/sort_ascii");
    for &n in &[100usize, 1000] {
        let base = ascii_words(n, 0x202);
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("uca", n), &base, |bencher, base| {
            let uca = UcaCollator::new();
            bencher.iter(|| {
                let mut xs = base.clone();
                xs.sort_by(|a, b| uca.compare(a, b));
                black_box(xs);
            });
        });

        group.bench_with_input(BenchmarkId::new("ascii_ci", n), &base, |bencher, base| {
            let ascii = AsciiCiCollator::new();
            bencher.iter(|| {
                let mut xs = base.clone();
                xs.sort_by(|a, b| ascii.compare(a, b));
                black_box(xs);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("natural_over_ascii_ci", n),
            &base,
            |bencher, base| {
                let natural = NaturalCollator::new(AsciiCiCollator::new());
                bencher.iter(|| {
                    let mut xs = base.clone();
                    xs.sort_by(|a, b| natural.compare(a, b));
                    black_box(xs);
                });
            },
        );
    }
    group.finish();
}

/// Sort filenames with embedded version numbers — the natural-sort
/// canonical scenario. Only natural collation gives the humanly-
/// expected `file2 < file10` ordering; the ASCII-CI baseline is
/// included for the throughput ratio.
fn bench_sort_versioned(c: &mut Criterion) {
    let mut group = c.benchmark_group("collate/sort_versioned");
    for &n in &[100usize, 1000] {
        let base = versioned_names(n, 0x303);
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("ascii_ci", n), &base, |bencher, base| {
            let ascii = AsciiCiCollator::new();
            bencher.iter(|| {
                let mut xs = base.clone();
                xs.sort_by(|a, b| ascii.compare(a, b));
                black_box(xs);
            });
        });

        group.bench_with_input(
            BenchmarkId::new("natural_over_ascii_ci", n),
            &base,
            |bencher, base| {
                let natural = NaturalCollator::new(AsciiCiCollator::new());
                bencher.iter(|| {
                    let mut xs = base.clone();
                    xs.sort_by(|a, b| natural.compare(a, b));
                    black_box(xs);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_pairwise_compare,
    bench_sort_ascii,
    bench_sort_versioned,
);
criterion_main!(benches);
