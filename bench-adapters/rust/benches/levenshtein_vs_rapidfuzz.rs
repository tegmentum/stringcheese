//! Head-to-head: StringCheese's Levenshtein vs. `rapidfuzz` 0.5.
//!
//! # Representation caveat
//!
//! `rapidfuzz::distance::levenshtein::distance` accepts any
//! `IntoIterator` whose item is `Hash + Eq`. We feed it
//! `pair.a_bytes.iter().copied()` — i.e. an iterator of `u8` — which
//! matches StringCheese's `&[u8]` call and keeps both sides on the same
//! representation. This is the fairest possible framing for a StringCheese
//! vs. rapidfuzz Levenshtein comparison; the iterator materialisation
//! cost on rapidfuzz's side is a per-call constant that shows up in
//! the small-length rows and vanishes at length 512 and up.
//!
//! # Cutoff variant
//!
//! `rapidfuzz` exposes `distance_with_args` for score-cutoff bounded
//! variants. We include one cutoff group at `k = 3` (the "spellcheck"
//! bound `stringcheese-bench` uses for its banded Levenshtein). The
//! StringCheese analogue is `banded::distance_banded_with_workspace(_, _,
//! 3, ws)` — measured directly in `stringcheese-bench`, so a reader
//! interested in a bounded-vs-bounded comparison should read this
//! bench's rapidfuzz-cutoff row against `stringcheese-bench`'s
//! `levenshtein/banded_tight_k3` row.
//!
//! # Matrix
//!
//! (length ∈ {8, 32, 128, 512, 2048}) × (regime ∈ {random, similar,
//! identical}) × (implementation ∈ {stringcheese, rapidfuzz,
//! `rapidfuzz_k3`}).

use std::hint::black_box;

use stringcheese_bench_adapters_rust::{LENGTHS, Pair, REGIMES, build_pair};
use stringcheese_core::DistanceMetric;
use stringcheese_levenshtein::{
    Levenshtein, LevenshteinWorkspace, distance_banded_with_workspace,
    distance_rolling_rows_with_workspace,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rapidfuzz::distance::levenshtein;

const SALTS: (u64, u64, u64) = (0xB1, 0xB2, 0xB3);

fn bench_unbounded(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein/vs_rapidfuzz_bytes");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("stringcheese/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    let alg = Levenshtein;
                    bencher.iter(|| {
                        alg.distance(
                            black_box(pair.a_bytes.as_slice()),
                            black_box(pair.b_bytes.as_slice()),
                        )
                    });
                },
            );
            // Workspace-backed rolling-rows kernel; measured separately
            // because StringCheese's own suite treats it as a distinct
            // data point (workspace allocation is amortised out) and
            // rapidfuzz's `distance` has no workspace-vs-fresh
            // distinction to compare against.
            group.bench_with_input(
                BenchmarkId::new(format!("stringcheese_ws/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    let mut ws = LevenshteinWorkspace::new();
                    bencher.iter(|| {
                        distance_rolling_rows_with_workspace(
                            black_box(pair.a_bytes.as_slice()),
                            black_box(pair.b_bytes.as_slice()),
                            &mut ws,
                        )
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("rapidfuzz/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        black_box(levenshtein::distance(
                            black_box(pair.a_bytes.iter().copied()),
                            black_box(pair.b_bytes.iter().copied()),
                        ))
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_cutoff_k3(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein/vs_rapidfuzz_bytes_k3");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("stringcheese_banded/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    let mut ws = LevenshteinWorkspace::new();
                    bencher.iter(|| {
                        distance_banded_with_workspace(
                            black_box(pair.a_bytes.as_slice()),
                            black_box(pair.b_bytes.as_slice()),
                            black_box(3u32),
                            &mut ws,
                        )
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("rapidfuzz_k3/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    // `Args::default().score_cutoff(3)` mirrors
                    // StringCheese's `banded_with_workspace(_, _, 3, _)`.
                    // rapidfuzz returns `Option<usize>` — `None` when
                    // the true distance exceeds the cutoff.
                    let args = levenshtein::Args::default().score_cutoff(3);
                    bencher.iter(|| {
                        black_box(levenshtein::distance_with_args(
                            black_box(pair.a_bytes.iter().copied()),
                            black_box(pair.b_bytes.iter().copied()),
                            &args,
                        ))
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_unbounded, bench_cutoff_k3);
criterion_main!(benches);
