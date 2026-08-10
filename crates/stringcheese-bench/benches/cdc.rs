//! Content-defined chunking + rolling-hash throughput benchmarks.
//!
//! Two families of measurement share one binary because they share
//! their input generator (`random_ascii` here stands in for arbitrary
//! byte content — the CDC and rolling-hash kernels are byte-generic
//! and see the whole 0..=255 range identically in expectation):
//!
//! * **`cdc/fastcdc/*`** — end-to-end chunk-boundary throughput for
//!   `FastCDC` at the two paper-derived configs (`default_8k` = 2 KiB
//!   / 8 KiB / 64 KiB and `default_16k` = 4 KiB / 16 KiB / 128 KiB).
//!   Both configs skip roughly 25% of every chunk (the sub-min
//!   window) and hash + mask-check the rest — throughput should
//!   converge on the same MiB/s regardless of the target average.
//! * **`cdc/fingerprint/*/roll_per_byte`** — the trait-driven
//!   `RollingHash::roll(byte)` throughput for each fingerprint. This
//!   is the primitive the `FastCdcStream` state machine drives per
//!   byte, so the ratio of FastCDC throughput to gear roll_per_byte
//!   throughput isolates the amortised cost of the mask check and
//!   per-cut reset (typically ~0.9× on a `default_8k` config, i.e.
//!   the mask/reset machinery costs a few tens of percent on top of
//!   the underlying gear roll).
//! * **`cdc/fingerprint/gear/digest_of_slice`** — the slice-oriented
//!   entry point exposed by the crate's `simd` feature, which
//!   dispatches at run time to AVX2/SSE2 on `x86_64`, NEON on
//!   `aarch64`, and SIMD128 on `wasm32+simd128`. Only gear is
//!   benched here because the other three (buzhash, polynomial,
//!   rabin) carry a semantic `window` that truncates the effective
//!   input to `bytes[..window]` (polynomial, rabin) or requires the
//!   full stream to correctly evict old bytes at every position
//!   (buzhash), neither of which is a fair
//!   apples-to-apples throughput comparison against the streaming
//!   `roll(byte)` loop above.
//!
//! # Input size
//!
//! Sized to give the CDC pass tens of chunks per iteration at both
//! configs — at 1 MiB the 8 KiB config sees ~128 chunks, at 16 MiB
//! ~2048 chunks. That is enough to average out per-chunk boundary-
//! detection variance without inflating iteration time to the point
//! criterion's sample budget shrinks.
//!
//! # Wall-clock only
//!
//! No allocation tracking here — the CDC iterator collects into a
//! `Vec<ChunkBoundary>` per iteration, and dhat would treat that as
//! signal rather than the measurement overhead it is. The
//! `alloc-tracking` feature is left off for this bench and the
//! per-chunk allocation cost is amortised across the thousands of
//! chunks each iteration produces.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// FastCDC is a proper noun that clippy::doc_markdown flags on every
// occurrence; wrapping it in backticks everywhere makes the header
// less readable than leaving it alone.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::random_ascii;
use stringcheese_cdc::cdc::ChunkBoundary;
use stringcheese_cdc::fingerprint::{
    Buzhash, GearHash, PolynomialHash, RabinFingerprint, RollingHash, gear::simd as gear_simd,
};
use stringcheese_cdc::{FastCdc, FastCdcConfig};

/// Input sizes in bytes. Chosen so 1 MiB spans ~128 default-8k
/// chunks and 16 MiB spans ~2048 — enough that per-chunk fixed
/// overhead does not dominate the throughput signal.
const INPUT_SIZES: &[usize] = &[1024 * 1024, 4 * 1024 * 1024, 16 * 1024 * 1024];

/// Effective window for the streaming rolling-hash `roll(byte)`
/// benches. 64 matches Gear's fixed effective window (one bit per
/// byte in a `u64` state) and is a reasonable common baseline for
/// buzhash / polynomial / rabin so all four are compared under the
/// same conditions.
const ROLL_WINDOW: usize = 64;

#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

// -----------------------------------------------------------------------
// FastCDC — chunk-boundary throughput.
// -----------------------------------------------------------------------

fn bench_fastcdc_8k(c: &mut Criterion) {
    // 2 KiB / 8 KiB / 64 KiB — the paper's default and the config
    // typical production callers (restic, ronomon/deduplication)
    // start from. Free-skip fraction per chunk averages ~25% at
    // this config (2 KiB min / 8 KiB avg), leaving ~75% of bytes
    // going through the hash + mask check.
    let mut group = c.benchmark_group("cdc/fastcdc/default_8k");
    let cdc = FastCdc::new(FastCdcConfig::default_8k());
    for &size in INPUT_SIZES {
        let input = random_ascii(size, seed_for(size, 0xD1));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| {
                // Collect into a Vec so the iterator is drained —
                // a `for _ in` loop over the iterator alone would
                // let LLVM elide the boundary construction.
                let boundaries: Vec<ChunkBoundary> = cdc.chunk_boundaries(input).collect();
                black_box(boundaries);
            });
        });
    }
    group.finish();
}

fn bench_fastcdc_16k(c: &mut Criterion) {
    // 4 KiB / 16 KiB / 128 KiB — the paper's second published
    // config. Free-skip fraction averages the same ~25% (4 KiB min
    // / 16 KiB avg), but with half as many chunks per MiB the
    // per-cut fixed overhead is amortised further. Expect
    // throughput within a few percent of the 8k config on the same
    // input; a big divergence would point at per-cut overhead
    // dominating on one branch.
    let mut group = c.benchmark_group("cdc/fastcdc/default_16k");
    let cdc = FastCdc::new(FastCdcConfig::default_16k());
    for &size in INPUT_SIZES {
        let input = random_ascii(size, seed_for(size, 0xD2));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| {
                let boundaries: Vec<ChunkBoundary> = cdc.chunk_boundaries(input).collect();
                black_box(boundaries);
            });
        });
    }
    group.finish();
}

// -----------------------------------------------------------------------
// Gear slice-batch (SIMD-dispatched) throughput.
//
// Gear is the only fingerprint whose `digest_of_slice` processes
// every byte of the input (no `window`-based truncation). Bench it
// alongside the FastCDC group so the gap between the pure-hash
// slice-batch throughput and the FastCDC boundary loop throughput
// exposes the mask-check + per-cut-reset overhead.
// -----------------------------------------------------------------------

fn bench_gear_digest_of_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdc/fingerprint/gear/digest_of_slice");
    for &size in INPUT_SIZES {
        let input = random_ascii(size, seed_for(size, 0xE1));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| black_box(gear_simd::digest_of_slice(black_box(input))));
        });
    }
    group.finish();
}

// -----------------------------------------------------------------------
// Rolling-hash `roll(byte)` — per-byte trait-call throughput.
//
// The trait-driven `roll(byte)` API is the one CDC state machines
// use internally (`FastCdcStream::feed` calls it on each byte past
// `min_size`). Bench all four fingerprints under the same window so
// the CDC bench above can be directly compared against gear's per-
// byte roll cost (the FastCDC path adds a mask check and a per-cut
// reset on top of what this bench measures).
// -----------------------------------------------------------------------

fn bench_roll_per_byte<H: RollingHash<Output = u64>>(
    c: &mut Criterion,
    group_name: &str,
    salt: u64,
) {
    let mut group = c.benchmark_group(group_name);
    for &size in INPUT_SIZES {
        let input = random_ascii(size, seed_for(size, salt));
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, input| {
            b.iter(|| {
                let mut h = H::new(ROLL_WINDOW);
                for &byte in input {
                    h.roll(byte);
                }
                black_box(h.digest());
            });
        });
    }
    group.finish();
}

fn bench_gear_roll(c: &mut Criterion) {
    bench_roll_per_byte::<GearHash>(c, "cdc/fingerprint/gear/roll_per_byte", 0xF1);
}
fn bench_buzhash_roll(c: &mut Criterion) {
    bench_roll_per_byte::<Buzhash>(c, "cdc/fingerprint/buzhash/roll_per_byte", 0xF2);
}
fn bench_polynomial_roll(c: &mut Criterion) {
    bench_roll_per_byte::<PolynomialHash>(c, "cdc/fingerprint/polynomial/roll_per_byte", 0xF3);
}
fn bench_rabin_roll(c: &mut Criterion) {
    bench_roll_per_byte::<RabinFingerprint>(c, "cdc/fingerprint/rabin/roll_per_byte", 0xF4);
}

criterion_group!(
    benches,
    bench_fastcdc_8k,
    bench_fastcdc_16k,
    bench_gear_digest_of_slice,
    bench_gear_roll,
    bench_buzhash_roll,
    bench_polynomial_roll,
    bench_rabin_roll,
);
criterion_main!(benches);
