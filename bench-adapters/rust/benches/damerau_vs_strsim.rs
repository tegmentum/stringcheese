//! Head-to-head: Comparand's OSA + full Damerau vs. `strsim` 0.11's
//! `osa_distance` + `damerau_levenshtein`.
//!
//! # Variant identity is load-bearing
//!
//! There are two commonly-named "Damerau" algorithms and they compute
//! different distances:
//!
//! * **Optimal String Alignment (OSA / "restricted Damerau")** — each
//!   substring can be edited at most once, does not satisfy the
//!   triangle inequality.
//!   * Comparand: `comparand_damerau::Osa`.
//!   * strsim:    `strsim::osa_distance`.
//! * **Full (unrestricted) Damerau-Levenshtein** — substrings can be
//!   edited unlimited times, is a true metric.
//!   * Comparand: `comparand_damerau::Damerau`.
//!   * strsim:    `strsim::damerau_levenshtein`.
//!
//! Pairing them the other way — Comparand's `Damerau` against
//! `strsim::osa_distance`, or Comparand's `Osa` against
//! `strsim::damerau_levenshtein` — would put two different algorithms
//! on the same axis and produce numbers that look meaningful but are
//! not. That is exactly the failure mode `docs/DESIGN.md` warns about
//! in the "Comparative Library Benchmarking" section, and the two
//! groups below are named `osa/…` and `damerau/…` to make the
//! variant explicit at the bench-name axis.
//!
//! # Representation caveat
//!
//! Same as the other `_vs_strsim.rs` files: Comparand consumes
//! `&[u8]`, strsim consumes `&str` and iterates chars. On ASCII this
//! is bit-for-bit equivalent semantics; strsim pays for UTF-8
//! iteration. If a strsim-generic-slice variant of these functions
//! existed the fair-fight column would appear here as well, but as of
//! 0.11 strsim exposes only `osa_distance(&str, &str)` and
//! `damerau_levenshtein(&str, &str)` — no `&[T]` overload.
//!
//! # Length cap for the full-Damerau group
//!
//! Comparand's full-Damerau *oracle* is O(m² · n) and
//! `comparand-bench` caps it at length 512 for that reason. The
//! *production* kernel — the one benchmarked here — is O(m · n) and
//! stays with the full sweep. `strsim::damerau_levenshtein` is
//! documented as O(m · n) too, so we sweep both at the full
//! `LENGTHS` set.

use std::hint::black_box;

use comparand_bench_adapters_rust::{LENGTHS, Pair, REGIMES, build_pair};
use comparand_core::DistanceMetric;
use comparand_damerau::{Damerau, Osa};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

const SALTS: (u64, u64, u64) = (0xE1, 0xE2, 0xE3);

fn bench_osa(c: &mut Criterion) {
    let mut group = c.benchmark_group("osa/vs_strsim");
    let alg = Osa;
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("comparand/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
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
                        black_box(strsim::osa_distance(
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

fn bench_damerau_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("damerau/vs_strsim");
    let alg = Damerau;
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in REGIMES {
            let pair = build_pair(len, kind, SALTS);
            group.bench_with_input(
                BenchmarkId::new(format!("comparand/{kind}"), len),
                &pair,
                |bencher, pair: &Pair| {
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
                        black_box(strsim::damerau_levenshtein(
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

criterion_group!(benches, bench_osa, bench_damerau_full);
criterion_main!(benches);
