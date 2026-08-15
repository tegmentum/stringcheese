//! Baseline throughput benchmarks for SimHash fingerprint build +
//! Hamming-distance / similarity comparison in `stringcheese-simhash`.
//!
//! Mirrors the shape of the sibling bench files
//! (`stringcheese-compare` / `stringcheese-manip` /
//! `stringcheese-textsplit`): one bench file, criterion-owned `main`
//! via `harness = false`, groups named `<op>/<flavor>/<n>`.
//! Regression trip-wire + last-measured baseline table for the three
//! phases SimHash pipelines actually spend time in.
//!
//! # Groups
//!
//! Three groups — one per operation a real SimHash pipeline runs:
//!
//! * `hash` — `Sketcher::add_all(...).finalize_64()`, the full
//!   O(n_features) build cost. Every feature is hashed to 128
//!   bits (two `ahash` calls per feature) and streams into the
//!   128-way accumulator. This is where every SimHash pipeline
//!   actually spends its time.
//! * `hamming` — `Sketch64::hamming_distance`, one xor + one
//!   `popcount`. Cheapest surface in the crate; measured mostly to
//!   confirm the comparison stays comfortably in the "essentially
//!   free" band across the two widths.
//! * `similar` — `Sketch64::similarity`, one `hamming_distance`
//!   plus one division. Same shape as `hamming` with an extra
//!   `f64` op — the two together mirror the "candidate scan +
//!   threshold decision" pass an LSH candidate consumer runs on
//!   every incoming pair.
//!
//! # Sizes swept
//!
//! Three input sizes per (op, flavor): 100 / 1000 / 10000 features.
//! Bracket the typical feature-bag count for a small paragraph
//! (weighted-term vector), a document, and a large document /
//! merged shard.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `short` — fixed-width 8-char feature strings (`feat-0000`).
//!   Every feature costs the same to hash; this is the reference
//!   shape and the fastest arm.
//! * `long` — 64-char feature strings (`feature-...`). Same feature
//!   count as `short`, but per-hash cost is larger because `ahash`
//!   processes more bytes per call. Numbers on this flavor drop
//!   ~1.5-2× per feature — that's the load-bearing per-byte hash
//!   cost `ahash` charges.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-simhash
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-simhash -- hash
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-simhash --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]
// `SimHash` / `Hamming` / `LSH` trip `doc_markdown` — same allow
// shape the crate-level `src/lib.rs` uses; the docs are for humans.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_simhash::Sketcher;

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG; every `cargo bench` invocation feeds byte-identical
// inputs so criterion's noise floor stays meaningful across runs.
// ---------------------------------------------------------------------------

/// Build a vector of `n` short (8-byte-ish) feature strings.
fn short_features(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("feat-{i:04x}")).collect()
}

/// Build a vector of `n` long (64-byte) feature strings. Same
/// feature count as `short_features`, but ~8× the per-hash byte
/// cost — the "long strings" arm exercises `ahash`'s per-byte
/// throughput.
fn long_features(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("feature-with-a-longer-payload-to-hash-{i:016x}-padded-tail"))
        .collect()
}

/// Feature counts swept by the `hash` group.
const N_FEATURES: &[usize] = &[100, 1_000, 10_000];

/// Builder alias to keep `flavors()`' return type off
/// `clippy::type_complexity`.
type FeatureBuilder = fn(usize) -> Vec<String>;

fn flavors() -> [(&'static str, FeatureBuilder); 2] {
    [("short", short_features), ("long", long_features)]
}

// ---------------------------------------------------------------------------
// Bench groups. Throughput reported over feature count for `hash`
// (the O(n_features) inner loop), and left off for `hamming` /
// `similar` (fixed-cost single-call surfaces).
// ---------------------------------------------------------------------------

fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("simhash/hash");
    for (flavor, build) in flavors() {
        for &n in N_FEATURES {
            let features = build(n);
            group.throughput(Throughput::Elements(n as u64));
            let id = BenchmarkId::new(flavor, n);
            group.bench_with_input(id, &features, |bencher, features| {
                bencher.iter(|| {
                    let sketcher = Sketcher::new();
                    black_box(
                        sketcher
                            .add_all(black_box(features).iter().map(String::as_str))
                            .finalize_64(),
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_hamming(c: &mut Criterion) {
    let mut group = c.benchmark_group("simhash/hamming");
    // `hamming_distance` cost is fixed regardless of what features
    // built the sketches — one xor + one `popcount`. Sweep the two
    // flavors for shape parity with the `hash` group; both should
    // land at the same nanosecond count.
    for (flavor, build) in flavors() {
        for &n in N_FEATURES {
            let features = build(n);
            let s1 = Sketcher::new()
                .add_all(features.iter().map(String::as_str))
                .finalize_64();
            let s2 = Sketcher::new()
                .add_all(features.iter().skip(1).map(String::as_str))
                .finalize_64();
            group.throughput(Throughput::Elements(1));
            let id = BenchmarkId::new(flavor, n);
            group.bench_with_input(id, &(s1, s2), |bencher, (s1, s2)| {
                bencher.iter(|| black_box(black_box(s1).hamming_distance(black_box(s2))));
            });
        }
    }
    group.finish();
}

fn bench_similar(c: &mut Criterion) {
    let mut group = c.benchmark_group("simhash/similar");
    // `similarity` = `hamming_distance` + one division; same shape
    // as the hamming group, one f64 op heavier per call.
    for (flavor, build) in flavors() {
        for &n in N_FEATURES {
            let features = build(n);
            let s1 = Sketcher::new()
                .add_all(features.iter().map(String::as_str))
                .finalize_64();
            let s2 = Sketcher::new()
                .add_all(features.iter().skip(1).map(String::as_str))
                .finalize_64();
            group.throughput(Throughput::Elements(1));
            let id = BenchmarkId::new(flavor, n);
            group.bench_with_input(id, &(s1, s2), |bencher, (s1, s2)| {
                bencher.iter(|| black_box(black_box(s1).similarity(black_box(s2))));
            });
        }
    }
    group.finish();
}

criterion_group!(simhash, bench_hash, bench_hamming, bench_similar);
criterion_main!(simhash);
