//! Head-to-head: Comparand's Levenshtein vs. `strsim` 0.11.
//!
//! # Representation caveat
//!
//! `strsim::levenshtein` operates on `&str` and iterates `chars`
//! internally; Comparand's `Levenshtein.distance` operates on `&[T:
//! Eq]` and is called here on `&[u8]`. For ASCII inputs — which is
//! what this bench uses — the two do the same DP on `u8` vs. `char`
//! cells; strsim's per-step cost is heavier because it materialises
//! chars from the string. To keep the comparison honest, we run two
//! groups:
//!
//! * `levenshtein/vs_strsim_str_ascii` — realistic usage. Comparand on
//!   bytes, strsim on `&str`. This is the "if you swap `strsim` for
//!   Comparand in your app" number, and it deliberately gives
//!   Comparand the fast-path advantage.
//! * `levenshtein/vs_strsim_generic_bytes` — algorithm-only. Comparand
//!   on `&[u8]`, `strsim::generic_levenshtein` on `&[u8]`. This is the
//!   "which DP kernel is faster" number and is the fair apples-to-apples
//!   comparison.
//!
//! The two together let a reader tease apart "how much of the win is
//! kernel quality" from "how much is the byte-slice API sidestepping
//! UTF-8 iteration".
//!
//! # Matrix
//!
//! (length ∈ {8, 32, 128, 512, 2048}) × (regime ∈ {random, similar,
//! identical}) × (implementation ∈ {comparand, strsim}). Matches
//! `comparand-bench`'s Levenshtein sweep.

use std::hint::black_box;

use comparand_bench_adapters_rust::{LENGTHS, Pair, REGIMES, build_pair};
use comparand_core::DistanceMetric;
use comparand_levenshtein::Levenshtein;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// Per-length salts. Distinct from `comparand-bench`'s own
/// Levenshtein salts (0x01, 0x02, 0x03/0x04) so a debugging session
/// that hits an unlikely coincidence in one corpus is very unlikely
/// to hit the same in the other.
const SALTS: (u64, u64, u64) = (0xA1, 0xA2, 0xA3);

fn bench_vs_strsim_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein/vs_strsim_str_ascii");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("comparand/{kind}"), len),
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
            group.bench_with_input(
                BenchmarkId::new(format!("strsim/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        // strsim::levenshtein returns `usize`. Wrapping
                        // the call in `black_box` prevents the return
                        // from being lifted out of the loop.
                        black_box(strsim::levenshtein(
                            black_box(pair.a_string.as_str()),
                            black_box(pair.b_string.as_str()),
                        ))
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_vs_strsim_generic_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("levenshtein/vs_strsim_generic_bytes");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("comparand/{kind}"), len),
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
            group.bench_with_input(
                BenchmarkId::new(format!("strsim_generic/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        // `generic_levenshtein` is strsim's own
                        // byte-slice DP kernel — identical algorithm,
                        // no `char` iteration; the fair fight.
                        //
                        // The signature is `fn(&Iter1, &Iter2)` with
                        // `Iter1: IntoIterator + Sized`, so we hand
                        // it `&Vec<u8>` rather than `&[u8]` — a plain
                        // slice is not `Sized`.
                        black_box(strsim::generic_levenshtein(
                            black_box(&pair.a_bytes),
                            black_box(&pair.b_bytes),
                        ))
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_vs_strsim_str, bench_vs_strsim_generic_bytes);
criterion_main!(benches);
