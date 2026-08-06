//! Hamming benchmarks.
//!
//! Two shapes: exact `hamming_distance` and bounded
//! `hamming_distance_within` with tight/permissive/no cutoffs. Hamming is
//! defined for equal-length inputs only, so every corpus is built from
//! [`similar_pair_equal_len`] (which perturbs a random string by
//! substitution only, keeping lengths equal).
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use stringcheese_bench::inputs::{identical_pair, random_ascii, similar_pair_equal_len};
use stringcheese_compare::hamming::{hamming_distance, hamming_distance_within};

/// Hamming is O(n); large n stays cheap, so we sweep the same set of
/// lengths the DP-based kernels use and extend up to 2048.
const LENGTHS: &[usize] = &[8, 32, 128, 512, 2048];

#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn build_equal_len_pair(len: usize, kind: &str) -> (Vec<u8>, Vec<u8>) {
    match kind {
        "random" => {
            // Two independent random strings of equal length. The
            // expected Hamming distance is 25/26 * len (all positions
            // mismatch except by chance) — the "mostly-dissimilar" case.
            (
                random_ascii(len, seed_for(len, 0x11)),
                random_ascii(len, seed_for(len, 0x12)),
            )
        }
        "similar" => similar_pair_equal_len(len, 0.05, seed_for(len, 0x13)),
        "identical" => identical_pair(len, seed_for(len, 0x14)),
        _ => unreachable!("unknown similarity regime"),
    }
}

fn bench_exact(c: &mut Criterion) {
    let mut group = c.benchmark_group("hamming/exact");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| hamming_distance(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

fn bench_within_no_cutoff(c: &mut Criterion) {
    // Cutoff = u32::MAX means "no early termination"; measures
    // `hamming_distance_within`'s per-iteration overhead vs.
    // `hamming_distance`.
    let mut group = c.benchmark_group("hamming/within_no_cutoff");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| {
                    hamming_distance_within(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        black_box(u32::MAX),
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_within_permissive(c: &mut Criterion) {
    // Cutoff = len is the "tight" bound in the sense that a mostly-
    // dissimilar random pair will still fit under it (expected distance
    // ≈ 0.96 · len). No early-out expected on random inputs; on similar
    // and identical, the loop runs to completion but never exceeds.
    let mut group = c.benchmark_group("hamming/within_permissive_k_len");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            let cutoff = u32::try_from(len).unwrap_or(u32::MAX);
            group.bench_with_input(
                BenchmarkId::new(kind, len),
                &(a, b, cutoff),
                |bencher, (a, b, cutoff)| {
                    bencher.iter(|| {
                        hamming_distance_within(
                            black_box(a.as_slice()),
                            black_box(b.as_slice()),
                            black_box(*cutoff),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_within_tight(c: &mut Criterion) {
    // Cutoff = 3: the spellcheck bound. Expected wins are:
    //   * random inputs: dramatic — early termination fires within ~3
    //     positions on average, so timing should be roughly constant in
    //     `len` rather than linear.
    //   * similar inputs (5% edit rate): mixed; at len=8 the whole
    //     string fits, at len=512 we're far above cutoff and terminate
    //     early.
    //   * identical inputs: still walks the whole string (count stays
    //     at 0), so no win.
    let mut group = c.benchmark_group("hamming/within_tight_k3");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| {
                    hamming_distance_within(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        black_box(3u32),
                    )
                });
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_exact,
    bench_within_no_cutoff,
    bench_within_permissive,
    bench_within_tight,
);
criterion_main!(benches);
