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
//! # Jaro workspace reuse
//!
//! The Jaro crate now exposes a workspace-aware entry point
//! ([`JaroWorkspace`]), so the `batch/jaro` group below mirrors the
//! Levenshtein/OSA/Damerau shape: one "fresh workspace per call" bench
//! against the trait-based entry point, one "single workspace reused
//! across the batch" bench pre-sized to `QUERY_LEN + QUERY_LEN` cells.
//! The Jaro bitmap is `Vec<bool>`, considerably smaller than the DP
//! matrices the edit-distance families need, so the absolute win per
//! comparison is smaller — but the workload characteristic (allocate
//! twice per call vs. never) is the same.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use stringcheese_bench::inputs::{random_ascii, random_candidates};
use stringcheese_compare::damerau::{Damerau, DamerauWorkspace, Osa, OsaWorkspace};
use stringcheese_compare::jaro::{Jaro, JaroWorkspace};
use stringcheese_compare::levenshtein::{Levenshtein, LevenshteinWorkspace};
use stringcheese_core::{DistanceMetric, SimilarityMetric};

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
                    let mut ws: DamerauWorkspace<u8> =
                        DamerauWorkspace::with_capacity((QUERY_LEN + 1) * (QUERY_LEN + 1));
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

// ---- Jaro -----------------------------------------------------------

fn bench_jaro_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch/jaro");
    let query = query();
    let alg = Jaro;

    for &count in COUNTS {
        let cands = candidates(count);
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("no_workspace", count),
            &(&query, &cands),
            |bencher, (query, cands)| {
                bencher.iter(|| {
                    // Trait-based entry point allocates two fresh
                    // `Vec<bool>` per candidate — the pre-Item-2 shape.
                    let mut sink: f64 = 0.0;
                    for c in cands.iter() {
                        let s =
                            alg.similarity(black_box(query.as_slice()), black_box(c.as_slice()));
                        sink += s.into_inner();
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
                    // Single workspace held across the batch, pre-sized
                    // to hold both bitmaps for the fixed-length query.
                    let mut ws = JaroWorkspace::with_capacity(QUERY_LEN + QUERY_LEN);
                    let mut sink: f64 = 0.0;
                    for c in cands.iter() {
                        let s = alg.similarity_with_workspace(
                            black_box(query.as_slice()),
                            black_box(c.as_slice()),
                            &mut ws,
                        );
                        sink += s.into_inner();
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
    bench_jaro_batch,
);
criterion_main!(benches);
