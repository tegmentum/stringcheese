//! Head-to-head: Comparand's Jaro / Jaro-Winkler vs. `strsim` 0.11.
//!
//! # Representation caveat
//!
//! `strsim::jaro` and `strsim::jaro_winkler` accept `&str` and
//! iterate `chars` internally; Comparand's `Jaro.similarity` accepts
//! `&[T: Eq]` and is called here on `&[u8]`. The inputs are ASCII so
//! the two are semantically equivalent, but strsim pays the cost of
//! UTF-8 iteration on every call. This is exactly the representation
//! choice discussion the Comparand toolkit is designed to make
//! visible — the byte-slice path is genuinely faster on ASCII input;
//! the `&[char]` path is what a caller with non-ASCII data would
//! actually use.
//!
//! # Return-type extraction
//!
//! `strsim::jaro` returns bare `f64`; Comparand's `Jaro.similarity`
//! returns `Similarity<f64>`. We call `into_inner()` inside the
//! timing loop for both sides so the two implementations are timed
//! doing the same amount of work — the newtype-unwrap is a
//! zero-cost inline the optimiser folds away, but keeping it visible
//! in the timing loop matches the toolkit's own claim that its
//! result types cost nothing at runtime.
//!
//! # Matrix
//!
//! (length ∈ {8, 32, 128, 512, 2048}) × (regime ∈ {random, similar,
//! identical}) × (implementation ∈ {`comparand_jaro`, `strsim_jaro`,
//! `comparand_jw`, `strsim_jw`}).

use std::hint::black_box;

use comparand_bench_adapters_rust::{LENGTHS, Pair, REGIMES, build_pair};
use comparand_core::SimilarityMetric;
use comparand_jaro::{Jaro, JaroWinkler};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SALTS: (u64, u64, u64) = (0xC1, 0xC2, 0xC3);

fn bench_jaro(c: &mut Criterion) {
    let mut group = c.benchmark_group("jaro/vs_strsim");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("comparand/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    let alg = Jaro;
                    bencher.iter(|| {
                        black_box(
                            alg.similarity(
                                black_box(pair.a_bytes.as_slice()),
                                black_box(pair.b_bytes.as_slice()),
                            )
                            .into_inner(),
                        )
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("strsim/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        black_box(strsim::jaro(
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

fn bench_jaro_winkler(c: &mut Criterion) {
    // Comparand's `JaroWinkler::classic()` = Winkler-1990: prefix
    // limit 4, scaling 0.1, always-apply boost. `strsim::jaro_winkler`
    // is documented as the classic variant with the same parameters,
    // so this is a same-algorithm head-to-head.
    let mut group = c.benchmark_group("jaro_winkler/vs_strsim");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("comparand/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    let alg = JaroWinkler::classic();
                    bencher.iter(|| {
                        black_box(
                            alg.similarity(
                                black_box(pair.a_bytes.as_slice()),
                                black_box(pair.b_bytes.as_slice()),
                            )
                            .into_inner(),
                        )
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("strsim/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        black_box(strsim::jaro_winkler(
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

criterion_group!(benches, bench_jaro, bench_jaro_winkler);
criterion_main!(benches);
