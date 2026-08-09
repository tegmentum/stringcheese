//! Stats — entropy / histogram / ratios / lengths.
//!
//! All-in-house crate; no wrap dispatch. Bench measures the
//! per-character-iteration overhead directly across three input
//! sizes so the constant setup cost + per-char cost split shows.
//!
//! ## Expected shape
//!
//! - **lengths** — cheapest (byte length is O(1) from `str::len`;
//!   code-point count is a `chars().count()`).
//! - **ratios** — six counters incremented per char; no map
//!   overhead. Should be close to lengths in per-byte cost.
//! - **entropy** — inserts every char into a `BTreeMap`; the
//!   map ops dominate on inputs with high scalar diversity.
//! - **histogram** — inserts into a `hashbrown::HashMap` keyed
//!   by `GeneralCategory`. Fixed key set (~30 categories) means
//!   the map stays tiny; per-char cost is dominated by the
//!   `get_general_category` table lookup.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::random_ascii;
use stringcheese_stats::{Histogram, Lengths, Ratios, entropy};

const LENGTHS: &[usize] = &[128, 1024, 8192];

fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn text_for(len: usize, salt: u64) -> String {
    let bytes = random_ascii(len, seed_for(len, salt));
    String::from_utf8(bytes).expect("random_ascii returns valid UTF-8")
}

// ---------------------------------------------------------------------
// Lengths — the cheapest primitive.
// ---------------------------------------------------------------------

fn bench_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats/lengths");
    for &len in LENGTHS {
        let text = text_for(len, 0xE1);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &text, |bencher, t| {
            bencher.iter(|| black_box(Lengths::of(black_box(t))));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Ratios — six counters in one pass.
// ---------------------------------------------------------------------

fn bench_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats/ratios");
    for &len in LENGTHS {
        let text = text_for(len, 0xE2);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &text, |bencher, t| {
            bencher.iter(|| black_box(Ratios::of(black_box(t))));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Entropy — BTreeMap frequency table.
// ---------------------------------------------------------------------

fn bench_entropy(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats/entropy");
    for &len in LENGTHS {
        let text = text_for(len, 0xE3);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &text, |bencher, t| {
            bencher.iter(|| black_box(entropy(black_box(t))));
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Histogram — HashMap keyed by GeneralCategory.
// ---------------------------------------------------------------------

fn bench_histogram(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats/histogram");
    for &len in LENGTHS {
        let text = text_for(len, 0xE4);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &text, |bencher, t| {
            bencher.iter(|| black_box(Histogram::of(black_box(t))));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_lengths,
    bench_ratios,
    bench_entropy,
    bench_histogram,
);
criterion_main!(benches);
