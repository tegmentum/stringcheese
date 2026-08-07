//! Levenshtein benchmarks.
//!
//! Each bench group crosses **input length × input-pair similarity × kernel
//! variant**. The three kernels are the ones the algorithm crate ships:
//!
//! * `full_matrix` — the O(m·n) time / O(m·n) space oracle.
//! * `rolling_rows` — the production kernel: O(m·n) time, O(min(m,n)) space,
//!   caller-owned [`LevenshteinWorkspace`].
//! * `banded` — Ukkonen-style, cutoff-aware; benched at a permissive cutoff
//!   (`k = len`) and a tight cutoff (`k = 3`).
//!
//! Input lengths are 8, 32, 128, 512, and 2048. 2048 stays included for
//! Levenshtein because O(n²) at n = 2048 is roughly 4M cell updates —
//! still comfortably sub-second on a modern desktop. The full-matrix
//! oracle at 2048 approaches 16 MiB of DP matrix and gets slow but
//! remains a useful reference; if this becomes a criterion-pain in
//! practice, drop that one row.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use stringcheese_bench::inputs::{identical_pair, random_ascii, similar_pair};
use stringcheese_compare::levenshtein::{
    Levenshtein, LevenshteinWorkspace, distance_banded_with_workspace, distance_full_matrix,
    distance_rolling_rows_with_workspace,
    simd::{self as levenshtein_simd, myers_scalar},
};
use stringcheese_core::DistanceMetric;

/// Input lengths swept across every group. Bytes.
const LENGTHS: &[usize] = &[8, 32, 128, 512, 2048];

/// Deterministic per-length seed. Uses the length itself as an entropy
/// mixer so that changing `LENGTHS` doesn't reshuffle the corpora at
/// unrelated lengths.
#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

/// The three canonical similarity regimes: random (mostly-dissimilar),
/// similar (5% edit rate), and identical. Every kernel is measured on
/// each; kernels with early-termination should show a clean win on the
/// identical/similar side, none on the random side.
fn build_pair(len: usize, kind: &str) -> (Vec<u8>, Vec<u8>) {
    match kind {
        "random" => (
            random_ascii(len, seed_for(len, 0x01)),
            random_ascii(len, seed_for(len, 0x02)),
        ),
        "similar" => similar_pair(len, 0.05, seed_for(len, 0x03)),
        "identical" => identical_pair(len, seed_for(len, 0x04)),
        _ => unreachable!("unknown similarity regime"),
    }
}

fn bench_full_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein/full_matrix");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| {
                    distance_full_matrix(black_box(a.as_slice()), black_box(b.as_slice()))
                });
            });
        }
    }
    group.finish();
}

fn bench_rolling_rows(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein/rolling_rows");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                // Workspace lives outside the timing loop so we're
                // measuring steady-state; the first iteration's growth
                // is amortized away by criterion's warmup phase.
                let mut ws = LevenshteinWorkspace::new();
                bencher.iter(|| {
                    distance_rolling_rows_with_workspace(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_banded_permissive(c: &mut Criterion) {
    // Cutoff = len means "never trigger early termination"; this is the
    // baseline for the banded kernel's overhead vs. rolling_rows.
    let mut group = c.benchmark_group("levenshtein/banded_permissive");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            let cutoff = u32::try_from(len).unwrap_or(u32::MAX);
            group.bench_with_input(
                BenchmarkId::new(kind, len),
                &(a, b, cutoff),
                |bencher, (a, b, cutoff)| {
                    let mut ws = LevenshteinWorkspace::new();
                    bencher.iter(|| {
                        distance_banded_with_workspace(
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

fn bench_banded_tight(c: &mut Criterion) {
    // Cutoff = 3 is the classical "typo cutoff" used in spellcheck; on
    // random inputs of any nontrivial length this should short-circuit
    // almost immediately, so we expect a large win over rolling_rows and
    // banded_permissive on the random-kind samples.
    let mut group = c.benchmark_group("levenshtein/banded_tight_k3");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                let mut ws = LevenshteinWorkspace::new();
                bencher.iter(|| {
                    distance_banded_with_workspace(
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

fn bench_handle_no_workspace(c: &mut Criterion) {
    // Trait-level entry point: `Levenshtein.distance(a, b)`. This
    // allocates a fresh workspace on every call; the delta between this
    // group and `rolling_rows` is exactly the allocation cost, which is
    // the argument for exposing workspace-aware APIs at all.
    let mut group = c.benchmark_group("levenshtein/handle_no_workspace");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            let alg = Levenshtein;
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| alg.distance(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

fn bench_myers_scalar(c: &mut Criterion) {
    // Scalar Myers 1999 bit-parallel kernel. For patterns of length
    // <= 64 this is the algorithmic-win path; for longer patterns it
    // falls through to an inline rolling-rows DP whose numbers should
    // look identical to `bench_rolling_rows`.
    let mut group = c.benchmark_group("levenshtein/myers_scalar");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| {
                    myers_scalar::distance(black_box(a.as_slice()), black_box(b.as_slice()))
                });
            });
        }
    }
    group.finish();
}

fn bench_myers_dispatched(c: &mut Criterion) {
    // Runtime-dispatched entry point. The overhead of the CPU-feature
    // check is what this group measures vs. `myers_scalar` — it should
    // be negligible for any input above the MYERS_MIN_LEN threshold.
    let mut group = c.benchmark_group("levenshtein/myers_dispatched");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| {
                    levenshtein_simd::distance(black_box(a.as_slice()), black_box(b.as_slice()))
                });
            });
        }
    }
    group.finish();
}

/// Wide-block sweep length list. Densely samples across the SSE2/NEON
/// break (128) and the AVX2 break (256) so each backend's bit-parallel
/// range and its rolling-rows fallback are both represented.
///
/// * 64 is the last length the scalar single-word path handles — the
///   floor for measuring the wide-block win.
/// * 96, 128 sit in the SSE2/NEON 128-bit lane.
/// * 160, 192, 224, 256 sit in the AVX2 256-bit lane and are where the
///   biggest wide-block-vs-rolling-rows gap should appear.
const WIDE_BLOCK_LENGTHS: &[usize] = &[64, 96, 128, 160, 192, 224, 256];

fn bench_myers_wide_block_scalar(c: &mut Criterion) {
    // Scalar Myers at the wide-block-relevant lengths. For m > 64 this
    // is the rolling-rows fallback embedded in `myers_scalar`, and it
    // is the baseline the SIMD wide-block wins should be measured
    // against. Random-kind only — the wide-block algorithm has no
    // early termination, so the "similar" and "identical" regimes only
    // matter for banded/full_matrix comparisons.
    let mut group = c.benchmark_group("levenshtein/myers_wide_block_scalar");
    for &len in WIDE_BLOCK_LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        let (a, b) = build_pair(len, "random");
        group.bench_with_input(
            BenchmarkId::from_parameter(len),
            &(a, b),
            |bencher, (a, b)| {
                bencher.iter(|| {
                    myers_scalar::distance(black_box(a.as_slice()), black_box(b.as_slice()))
                });
            },
        );
    }
    group.finish();
}

fn bench_myers_wide_block_dispatched(c: &mut Criterion) {
    // Runtime-dispatched Myers at the wide-block-relevant lengths.
    // The dispatcher picks the widest available backend for the host,
    // so on an AVX2 host every length in this sweep runs through the
    // AVX2 backend (which internally delegates to SSE2 for m ≤ 128 and
    // to scalar for m ≤ 64). Speedup vs. `myers_wide_block_scalar` is
    // the headline number for this landing.
    let mut group = c.benchmark_group("levenshtein/myers_wide_block_dispatched");
    for &len in WIDE_BLOCK_LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        let (a, b) = build_pair(len, "random");
        group.bench_with_input(
            BenchmarkId::from_parameter(len),
            &(a, b),
            |bencher, (a, b)| {
                bencher.iter(|| {
                    levenshtein_simd::distance(black_box(a.as_slice()), black_box(b.as_slice()))
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_full_matrix,
    bench_rolling_rows,
    bench_banded_permissive,
    bench_banded_tight,
    bench_handle_no_workspace,
    bench_myers_scalar,
    bench_myers_dispatched,
    bench_myers_wide_block_scalar,
    bench_myers_wide_block_dispatched,
);
criterion_main!(benches);
