//! OSA (Optimal String Alignment) and full Damerau-Levenshtein benchmarks.
//!
//! # Kernels covered
//!
//! * `osa/full_matrix` — the O(m·n) time / O(m·n) space OSA oracle. Bare
//!   `u32` return, no workspace.
//! * `osa/rolling_rows` — production OSA, three rolling rows, workspace.
//! * `osa/banded` — Ukkonen-style banded OSA, cutoff-aware, benched at
//!   permissive (`k = len`) and tight (`k = 3`) cutoffs.
//! * `damerau/full_matrix` — full Damerau oracle, Lowrance-Wagner with a
//!   linear scan for the "last position of symbol"; O(m² · n) time.
//!   Bare `u32` return, no workspace.
//! * `damerau/production` — full Damerau production kernel, `HashMap`
//!   auxiliary; `std`-gated in the algorithm crate. Workspace-backed.
//!
//! # Input-length caveat
//!
//! The full-Damerau *oracle* is O(m² · n). At n = 2048 that is ~17 billion
//! inner steps, well over criterion's "reasonable iteration" ceiling.
//! We cap the damerau-oracle group at 512.  The damerau-production
//! kernel is O(m · n) and stays with the rest of the sweep.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use comparand_bench::inputs::{identical_pair, random_ascii, similar_pair};
use comparand_core::DistanceMetric;
use comparand_damerau::{
    Damerau, DamerauWorkspace, Osa, OsaWorkspace,
    damerau::{
        full_matrix::distance_full_matrix as damerau_full_matrix,
        production::distance_production_with_workspace as damerau_production,
    },
    osa::{
        banded::distance_banded_with_workspace as osa_banded,
        full_matrix::distance_full_matrix as osa_full_matrix,
        rolling_rows::distance_rolling_rows_with_workspace as osa_rolling_rows,
    },
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

const LENGTHS: &[usize] = &[8, 32, 128, 512, 2048];
/// Sizes at which the Damerau oracle stays under a second per iteration.
/// O(m² · n) at n = 2048 would be tens of seconds per sample; skip it.
const LENGTHS_DAMERAU_ORACLE: &[usize] = &[8, 32, 128, 512];

#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn build_pair(len: usize, kind: &str) -> (Vec<u8>, Vec<u8>) {
    match kind {
        "random" => (
            random_ascii(len, seed_for(len, 0x31)),
            random_ascii(len, seed_for(len, 0x32)),
        ),
        "similar" => similar_pair(len, 0.05, seed_for(len, 0x33)),
        "identical" => identical_pair(len, seed_for(len, 0x34)),
        _ => unreachable!("unknown similarity regime"),
    }
}

// ---- OSA ------------------------------------------------------------

fn bench_osa_full_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("osa/full_matrix");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| osa_full_matrix(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

fn bench_osa_rolling_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("osa/rolling_rows");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                let mut ws = OsaWorkspace::new();
                bencher.iter(|| {
                    osa_rolling_rows(black_box(a.as_slice()), black_box(b.as_slice()), &mut ws)
                });
            });
        }
    }
    group.finish();
}

fn bench_osa_banded_permissive(c: &mut Criterion) {
    let mut group = c.benchmark_group("osa/banded_permissive");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            let cutoff = u32::try_from(len).unwrap_or(u32::MAX);
            group.bench_with_input(
                BenchmarkId::new(kind, len),
                &(a, b, cutoff),
                |bencher, (a, b, cutoff)| {
                    let mut ws = OsaWorkspace::new();
                    bencher.iter(|| {
                        osa_banded(
                            black_box(a.as_slice()),
                            black_box(b.as_slice()),
                            black_box(*cutoff),
                            &mut ws,
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_osa_banded_tight(c: &mut Criterion) {
    let mut group = c.benchmark_group("osa/banded_tight_k3");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                let mut ws = OsaWorkspace::new();
                bencher.iter(|| {
                    osa_banded(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        black_box(3u32),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_osa_handle_no_workspace(c: &mut Criterion) {
    let mut group = c.benchmark_group("osa/handle_no_workspace");
    let alg = Osa;
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| alg.distance(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

// ---- Full Damerau ---------------------------------------------------

fn bench_damerau_full_matrix(c: &mut Criterion) {
    // O(m² · n) — capped at 512.
    let mut group = c.benchmark_group("damerau/full_matrix");
    for &len in LENGTHS_DAMERAU_ORACLE {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher
                    .iter(|| damerau_full_matrix(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

fn bench_damerau_production(c: &mut Criterion) {
    let mut group = c.benchmark_group("damerau/production");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                let mut ws = DamerauWorkspace::new();
                bencher.iter(|| {
                    damerau_production(black_box(a.as_slice()), black_box(b.as_slice()), &mut ws)
                });
            });
        }
    }
    group.finish();
}

fn bench_damerau_handle_no_workspace(c: &mut Criterion) {
    let mut group = c.benchmark_group("damerau/handle_no_workspace");
    let alg = Damerau;
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| alg.distance(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_osa_full_matrix,
    bench_osa_rolling_rows,
    bench_osa_banded_permissive,
    bench_osa_banded_tight,
    bench_osa_handle_no_workspace,
    bench_damerau_full_matrix,
    bench_damerau_production,
    bench_damerau_handle_no_workspace,
);
criterion_main!(benches);
