// Iterating with `cands.iter()` inside a bench closure spells out the
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// intent — "walk the same candidate slice on every iteration" — better
// than the auto-deref shorthand does. The `for c in *cands` form the
// pedantic lint suggests introduces a doubly-dereferenced expression
// that is materially harder to read at a glance in a criterion closure.
#![allow(clippy::explicit_iter_loop)]

//! Batch benchmarks — the workspace-reuse win.
//!
//! The design document's memory-philosophy section calls workspace reuse
//! "essential for entity resolution, databases, and WebAssembly". This
//! bench is where that claim shows up as a measurable number: one fixed
//! query compared against a corpus of candidates, with (a) a fresh
//! workspace allocation per pair and (b) a single reused workspace
//! across the batch.
//!
//! For Levenshtein and OSA the workspace-reuse win should be visible at
//! candidate count = 100 and pronounced at 1000. Full Damerau is included
//! because its DP matrix — grown to `(m+1) · (n+1)` cells — is the
//! largest per-call allocation of any algorithm in the suite; reusing
//! it should be the biggest single win in the whole bench.
//!
//! # Jaro deferral
//!
//! The Jaro crate does not yet expose a workspace-aware entry point —
//! the crate's module docs list this as future work — so there is no
//! `Jaro`-vs-`Jaro-with-workspace` measurement to make. That's still
//! useful to note: the delta between `Jaro`'s per-call `Vec<bool>`
//! allocations (visible as an absolute cost in the `jaro/*` groups) and
//! the workspace-free Hamming loop is the ceiling on the win a future
//! Jaro workspace could deliver.

use comparand_bench::inputs::{random_ascii, random_candidates};
use comparand_core::DistanceMetric;
use comparand_damerau::{Damerau, DamerauWorkspace, Osa, OsaWorkspace};
use comparand_levenshtein::{Levenshtein, LevenshteinWorkspace};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

/// Fixed query length across every batch group.
const QUERY_LEN: usize = 32;
/// Candidate counts. 100 shows the win exists; 1000 shows it grow.
const COUNTS: &[usize] = &[100, 1000];

fn query() -> Vec<u8> {
    random_ascii(QUERY_LEN, 0xDEAD_BEEF)
}

fn candidates(count: usize) -> Vec<Vec<u8>> {
    // Each candidate independently random; approximates the
    // "mostly-dissimilar" regime that dominates real record-linkage
    // corpora before any n-gram filtering is applied.
    random_candidates(count, QUERY_LEN, 0xC0FE_D00D_u64)
}

// ---- Levenshtein ----------------------------------------------------

fn bench_levenshtein_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch/levenshtein");
    let query = query();
    let alg = Levenshtein;

    for &count in COUNTS {
        let cands = candidates(count);
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("no_workspace", count),
            &(&query, &cands),
            |bencher, (query, cands)| {
                bencher.iter(|| {
                    // Fresh workspace allocated per call, via the trait
                    // impl. This is the "naïve batch" pattern.
                    let mut sink: u64 = 0;
                    for c in cands.iter() {
                        let d = alg.distance(black_box(query.as_slice()), black_box(c.as_slice()));
                        sink = sink.wrapping_add(u64::from(d.into_inner()));
                    }
                    black_box(sink);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("reused_workspace", count),
            &(&query, &cands),
            |bencher, (query, cands)| {
                bencher.iter(|| {
                    // Single workspace held for the entire batch.
                    // Pre-sized so the first candidate does not pay a
                    // grow cost; steady-state comparison.
                    let mut ws = LevenshteinWorkspace::with_capacity(QUERY_LEN + 1);
                    let mut sink: u64 = 0;
                    for c in cands.iter() {
                        let d = alg.distance_with_workspace(
                            black_box(query.as_slice()),
                            black_box(c.as_slice()),
                            &mut ws,
                        );
                        sink = sink.wrapping_add(u64::from(d.into_inner()));
                    }
                    black_box(sink);
                });
            },
        );
    }
    group.finish();
}

// ---- OSA ------------------------------------------------------------

fn bench_osa_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch/osa");
    let query = query();
    let alg = Osa;

    for &count in COUNTS {
        let cands = candidates(count);
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("no_workspace", count),
            &(&query, &cands),
            |bencher, (query, cands)| {
                bencher.iter(|| {
                    let mut sink: u64 = 0;
                    for c in cands.iter() {
                        let d = alg.distance(black_box(query.as_slice()), black_box(c.as_slice()));
                        sink = sink.wrapping_add(u64::from(d.into_inner()));
                    }
                    black_box(sink);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("reused_workspace", count),
            &(&query, &cands),
            |bencher, (query, cands)| {
                bencher.iter(|| {
                    // OSA needs 3 · (min(m, n) + 1) cells.
                    let mut ws = OsaWorkspace::with_capacity(3 * (QUERY_LEN + 1));
                    let mut sink: u64 = 0;
                    for c in cands.iter() {
                        let d = alg.distance_with_workspace(
                            black_box(query.as_slice()),
                            black_box(c.as_slice()),
                            &mut ws,
                        );
                        sink = sink.wrapping_add(u64::from(d.into_inner()));
                    }
                    black_box(sink);
                });
            },
        );
    }
    group.finish();
}

// ---- Full Damerau ---------------------------------------------------

fn bench_damerau_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch/damerau");
    let query = query();
    let alg = Damerau;

    for &count in COUNTS {
        let cands = candidates(count);
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("no_workspace", count),
            &(&query, &cands),
            |bencher, (query, cands)| {
                bencher.iter(|| {
                    let mut sink: u64 = 0;
                    for c in cands.iter() {
                        let d = alg.distance(black_box(query.as_slice()), black_box(c.as_slice()));
                        sink = sink.wrapping_add(u64::from(d.into_inner()));
                    }
                    black_box(sink);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("reused_workspace", count),
            &(&query, &cands),
            |bencher, (query, cands)| {
                bencher.iter(|| {
                    // Damerau needs the whole (m+1) · (n+1) DP matrix.
                    let mut ws = DamerauWorkspace::with_capacity((QUERY_LEN + 1) * (QUERY_LEN + 1));
                    let mut sink: u64 = 0;
                    for c in cands.iter() {
                        let d = alg.distance_with_workspace(
                            black_box(query.as_slice()),
                            black_box(c.as_slice()),
                            &mut ws,
                        );
                        sink = sink.wrapping_add(u64::from(d.into_inner()));
                    }
                    black_box(sink);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_levenshtein_batch,
    bench_osa_batch,
    bench_damerau_batch,
);
criterion_main!(benches);
