//! Head-to-head: StringCheese's Hamming vs. `strsim` 0.11.
//!
//! # Representation caveat
//!
//! `strsim::hamming(a, b) -> Result<usize, StrSimError>` returns an
//! error when the two inputs have unequal `char` count; the timing
//! loop below unwraps that result inside the timed region so both
//! sides do the same amount of work. `strsim::hamming` iterates
//! `chars`; StringCheese's `hamming_distance` operates on `&[u8]`. On
//! ASCII input the two agree bit-for-bit.
//!
//! Hamming is only defined for equal-length inputs, so every corpus
//! is built through [`build_pair_equal_len`] — the "similar" regime
//! is substitutions only, matching StringCheese's own suite.
//!
//! # Cutoff variant
//!
//! `strsim` has no bounded/cutoff variant of Hamming, so
//! `hamming_distance_within` — StringCheese's early-terminating kernel —
//! has no strsim analogue to compare against. It is nonetheless
//! benched here at cutoff `k = 3` for parity with
//! `stringcheese-bench`'s own layout; the strsim column is left absent
//! from that group.
//!
//! # Matrix
//!
//! (length ∈ {8, 32, 128, 512, 2048}) × (regime ∈ {random, similar,
//! identical}) × (implementation ∈ {stringcheese, strsim}).

use std::hint::black_box;

use stringcheese_bench_adapters_rust::{LENGTHS, Pair, REGIMES, build_pair_equal_len};
use stringcheese_compare::hamming::{hamming_distance, hamming_distance_within};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SALTS: (u64, u64, u64) = (0xD1, 0xD2, 0xD3);

fn bench_exact(c: &mut Criterion) {
    let mut group = c.benchmark_group("hamming/vs_strsim");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair_equal_len(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("stringcheese/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        black_box(hamming_distance(
                            black_box(pair.a_bytes.as_slice()),
                            black_box(pair.b_bytes.as_slice()),
                        ))
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("strsim/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        // Unwrap in the timing loop; a benched call
                        // that panicked would abort criterion anyway.
                        // `build_pair_equal_len` guarantees equal
                        // length, so the unwrap is unreachable.
                        black_box(
                            strsim::hamming(
                                black_box(pair.a_string.as_str()),
                                black_box(pair.b_string.as_str()),
                            )
                            .expect("build_pair_equal_len guarantees equal char count"),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_within_k3(c: &mut Criterion) {
    // StringCheese-only group (strsim has no cutoff variant), kept for
    // parity with `stringcheese-bench`'s Hamming layout so a reader can
    // read across.
    let mut group = c.benchmark_group("hamming/within_k3_stringcheese_only");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair_equal_len(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("stringcheese/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
                    bencher.iter(|| {
                        black_box(hamming_distance_within(
                            black_box(pair.a_bytes.as_slice()),
                            black_box(pair.b_bytes.as_slice()),
                            black_box(3u32),
                        ))
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_exact, bench_within_k3);
criterion_main!(benches);
