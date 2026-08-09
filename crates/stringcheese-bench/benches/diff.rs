//! Diff — Myers + Patience side-by-side.
//!
//! Every input is generated deterministically from
//! stringcheese-bench's seeded helpers. Both algorithms run
//! against the SAME `(old, new)` pair at every scale so the
//! output is a direct apples-to-apples comparison.
//!
//! ## Input shapes
//!
//! - **identical** — `old == new`. Best case for both algorithms;
//!   Myers's `d = 0` path exits immediately, Patience's LCS is
//!   the whole input.
//! - **single-insert** — one line inserted mid-way through
//!   `old`. Realistic single-commit shape.
//! - **medium-edits** — ~10 % of lines differ in a 100-line file.
//!   Realistic review-diff shape.
//! - **large-edits** — ~10 % of lines differ in a 1000-line
//!   file. The scale at which Myers's `O(ND)` and Patience's
//!   anchor recursion begin to diverge in wall clock.
//!
//! ## Why bench Myers vs Patience separately
//!
//! Myers guarantees a minimum edit script; Patience anchors on
//! unique elements and often produces a script that reads more
//! naturally to a human (moved-block-style). The wall-clock
//! tradeoff depends on the input: on identical / mostly-identical
//! inputs Myers wins (short `d`); on structurally-clustered
//! inputs Patience's anchor recursion can be competitive or
//! faster.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// Corpus generation casts u64 rng output to usize for indices and
// line lengths. The values are bounded by array sizes / small
// constants (never anywhere near 2^32), so cast truncation is a
// non-issue on any real target.
#![allow(clippy::cast_possible_truncation)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::Rng;
use stringcheese_diff::{Myers, Patience, diff};

// ---------------------------------------------------------------------
// Corpus construction.
// ---------------------------------------------------------------------

/// A synthetic "source file" — N lines, each ~30-60 chars of
/// deterministic pseudo-random ASCII. Every seed produces the
/// same corpus so benchmark runs stay comparable across machines.
fn lines(count: usize, seed: u64) -> Vec<String> {
    let mut rng = Rng::from_seed(seed);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let len = 30 + (i % 30);
        let mut line = String::with_capacity(len);
        for _ in 0..len {
            let b = (rng.next_u64() % 26) as u8 + b'a';
            line.push(b as char);
        }
        out.push(line);
    }
    out
}

/// Insert one line at a deterministic position; the returned
/// `Vec` is `old` with one extra line at index `count / 2`.
fn with_insertion(base: &[String], seed: u64) -> Vec<String> {
    let mut out = base.to_vec();
    let mut rng = Rng::from_seed(seed);
    let mid = base.len() / 2;
    let mut inserted = String::with_capacity(40);
    for _ in 0..40 {
        let b = (rng.next_u64() % 26) as u8 + b'a';
        inserted.push(b as char);
    }
    out.insert(mid, inserted);
    out
}

/// Randomly rewrite `edit_count` lines with fresh random content.
fn with_edits(base: &[String], edit_count: usize, seed: u64) -> Vec<String> {
    let mut out = base.to_vec();
    let mut rng = Rng::from_seed(seed);
    for _ in 0..edit_count {
        let idx = (rng.next_u64() as usize) % out.len();
        let len = 30 + (rng.next_u64() as usize % 30);
        let mut line = String::with_capacity(len);
        for _ in 0..len {
            let b = (rng.next_u64() % 26) as u8 + b'a';
            line.push(b as char);
        }
        out[idx] = line;
    }
    out
}

// ---------------------------------------------------------------------
// Benches.
// ---------------------------------------------------------------------

fn bench_identical(c: &mut Criterion) {
    // Best case — `old == new` — for both algorithms.
    let mut group = c.benchmark_group("diff/identical");
    for &n in &[100usize, 1000] {
        let old = lines(n, 0x11);
        let new = old.clone();
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("myers", n),
            &(&old, &new),
            |bencher, (old, new)| {
                bencher.iter(|| black_box(diff(black_box(old), black_box(new), Myers)));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("patience", n),
            &(&old, &new),
            |bencher, (old, new)| {
                bencher.iter(|| black_box(diff(black_box(old), black_box(new), Patience)));
            },
        );
    }
    group.finish();
}

fn bench_single_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff/single_insert");
    for &n in &[100usize, 1000] {
        let old = lines(n, 0x22);
        let new = with_insertion(&old, 0x22 ^ 0xFEED);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("myers", n),
            &(&old, &new),
            |bencher, (old, new)| {
                bencher.iter(|| black_box(diff(black_box(old), black_box(new), Myers)));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("patience", n),
            &(&old, &new),
            |bencher, (old, new)| {
                bencher.iter(|| black_box(diff(black_box(old), black_box(new), Patience)));
            },
        );
    }
    group.finish();
}

fn bench_ten_percent_edits(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff/ten_percent_edits");
    for &n in &[100usize, 1000] {
        let old = lines(n, 0x33);
        let new = with_edits(&old, n / 10, 0x33 ^ 0xCAFE);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("myers", n),
            &(&old, &new),
            |bencher, (old, new)| {
                bencher.iter(|| black_box(diff(black_box(old), black_box(new), Myers)));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("patience", n),
            &(&old, &new),
            |bencher, (old, new)| {
                bencher.iter(|| black_box(diff(black_box(old), black_box(new), Patience)));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_identical,
    bench_single_insert,
    bench_ten_percent_edits,
);
criterion_main!(benches);
