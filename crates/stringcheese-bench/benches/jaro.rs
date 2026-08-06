//! Jaro and Jaro-Winkler-classic benchmarks.
//!
//! Jaro's baseline complexity is O(|a| · w) where `w = max(|a|,|b|)/2 - 1`,
//! plus O(|a| + |b|) auxiliary space for the matched-position bitmaps.
//! Every call allocates two `Vec<bool>` internally — the jaro crate
//! documents this and defers a workspace-aware variant to future work,
//! so no `*_with_workspace` entry point exists to benchmark. The batch
//! bench (`benches/batch.rs`) therefore does *not* include a
//! workspace-vs-no-workspace comparison for Jaro; if a workspace API
//! lands later, this bench and the batch bench should grow a paired
//! group for it.
//!
//! Both `Jaro` and `JaroWinkler::classic()` are exercised so that the
//! prefix-boost overhead is visible.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use stringcheese_bench::inputs::{identical_pair, random_ascii, similar_pair};
use stringcheese_core::SimilarityMetric;
use stringcheese_jaro::{Jaro, JaroWinkler};

const LENGTHS: &[usize] = &[8, 32, 128, 512, 2048];

#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn build_pair(len: usize, kind: &str) -> (Vec<u8>, Vec<u8>) {
    match kind {
        "random" => (
            random_ascii(len, seed_for(len, 0x21)),
            random_ascii(len, seed_for(len, 0x22)),
        ),
        "similar" => similar_pair(len, 0.05, seed_for(len, 0x23)),
        "identical" => identical_pair(len, seed_for(len, 0x24)),
        _ => unreachable!("unknown similarity regime"),
    }
}

fn bench_jaro(c: &mut Criterion) {
    let mut group = c.benchmark_group("jaro/base");
    let alg = Jaro;
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| alg.similarity(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

fn bench_jaro_winkler_classic(c: &mut Criterion) {
    // Winkler-1990 classic: prefix limit 4, scaling 0.1, always-apply
    // boost (threshold 0.0). Measures Jaro + a short prefix scan of at
    // most 4 symbols, so the overhead over base Jaro should be tiny.
    let mut group = c.benchmark_group("jaro/winkler_classic");
    let alg = JaroWinkler::classic();
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| alg.similarity(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

fn bench_jaro_winkler_with_threshold(c: &mut Criterion) {
    // Winkler's later 1999 formulation: don't boost below Jaro = 0.7.
    // On random inputs the boost is skipped (fast path); on similar and
    // identical inputs it fires. Useful for measuring the threshold
    // check's cost.
    let mut group = c.benchmark_group("jaro/winkler_with_threshold");
    let alg = JaroWinkler::with_threshold();
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| alg.similarity(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_jaro,
    bench_jaro_winkler_classic,
    bench_jaro_winkler_with_threshold,
);
criterion_main!(benches);
