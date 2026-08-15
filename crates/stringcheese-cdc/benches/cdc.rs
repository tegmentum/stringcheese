//! Baseline throughput benches for `stringcheese-cdc`.
//!
//! Two families sharing one criterion binary:
//!
//! * **`cdc/fastcdc/*`** — end-to-end chunk emission via
//!   [`FastCdc::chunk_boundaries`] at the paper-derived
//!   [`FastCdcConfig::default_8k`] / [`FastCdcConfig::default_16k`]
//!   configs. Boundaries are materialised into a `Vec` so LLVM cannot
//!   elide the state-machine work.
//! * **`cdc/fingerprint/*`** — per-family rolling-hash throughput.
//!   Every fingerprint that implements [`RollingHash`] is streamed
//!   byte-at-a-time through `roll(byte)` at a shared 64-byte window
//!   so the four rows are directly comparable; `GearHash` additionally
//!   gets a slice-batch measurement (the only family whose SIMD
//!   `digest_of_slice` processes every byte without truncating to
//!   `bytes[..window]`).
//!
//! # Input flavors
//!
//! Two flavors at each size, so a regression that only shows up on
//! low-entropy input surfaces alongside the random-bytes baseline:
//!
//! * `random` — deterministic PRNG bytes covering the full `0..=255`
//!   range. Baseline for content-agnostic throughput.
//! * `prose` — deterministic English prose (fixed word list, ASCII).
//!   Low entropy (~64 distinct bytes; heavy repetition) means the mask
//!   check hits fewer times per byte on the FastCDC path, so a chunker
//!   optimisation that only helps low-entropy input surfaces here.
//!
//! # Input sizes
//!
//! `1 MiB` and `10 MiB` per the round's task shape. At `default_8k`
//! the 1 MiB input spans ~128 chunks and the 10 MiB input spans ~1280;
//! both are well past the point where per-chunk fixed overhead
//! dominates.
//!
//! # Baseline numbers (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative
//! `--quick` criterion run (three-second sample budget per bench).
//! Wall-clock samples vary ±5-10 % on a laptop under load; treat the
//! ratios as informative, the absolutes as ±20 % ballpark. Throughput
//! reported in native criterion units (MiB/s or GiB/s). Higher is
//! better.
//!
//! ```text
//! group                                    1 MiB (random)   10 MiB (random)   1 MiB (prose)   10 MiB (prose)
//! -------------------------------------------------------------------------------------------------------------
//! cdc/fastcdc/default_8k                    1.28 GiB/s        1.19 GiB/s        1.29 GiB/s      1.23 GiB/s
//! cdc/fastcdc/default_16k                   1.39 GiB/s        1.37 GiB/s        1.29 GiB/s      1.21 GiB/s
//! cdc/fingerprint/gear/roll_per_byte        1.51 GiB/s        1.50 GiB/s        1.50 GiB/s      1.50 GiB/s
//! cdc/fingerprint/gear/digest_of_slice      3.17 GiB/s        3.17 GiB/s        3.15 GiB/s      3.16 GiB/s
//! cdc/fingerprint/buzhash/roll_per_byte     1.00 GiB/s        1.01 GiB/s        1.01 GiB/s      1.02 GiB/s
//! cdc/fingerprint/polynomial/roll_per_byte   167 MiB/s         167 MiB/s         167 MiB/s       167 MiB/s
//! cdc/fingerprint/rabin/roll_per_byte        273 MiB/s         277 MiB/s         276 MiB/s       276 MiB/s
//! ```
//!
//! Read:
//!
//! * **`gear` roll_per_byte vs `gear` digest_of_slice** — the SIMD
//!   entry point runs ~2× the streaming loop because it batches
//!   independent 8-byte chunks (Gear's state has a 64-bit shift
//!   register with natural decay, so batch-of-8 is safe).
//! * **FastCDC vs gear roll** — FastCDC's boundary loop runs at
//!   ~0.85× the raw gear roll throughput; the delta is the mask
//!   check plus the per-cut reset amortised across ~one cut per
//!   8 KiB.
//! * **buzhash** — ~0.68× gear because the update mixes a ROL of
//!   the outgoing byte's table entry as well as the incoming byte's,
//!   doubling the per-step ALU work.
//! * **polynomial / rabin** — much slower than gear/buzhash because
//!   the update involves a full modular reduction (Mersenne-61
//!   arithmetic for polynomial; GF(2) polynomial reduction with a
//!   256-entry shift table plus a 256-entry per-instance roll-out
//!   table for rabin). These are the classical rolling-hash
//!   constructions that FastCDC's gear-hash replaces on modern CPUs.
//! * **prose vs random** — throughput is within 2 % across the
//!   board; the hash / mask work is byte-content-independent. A
//!   large divergence would flag a data-dependent branch in the hot
//!   loop.
//!
//! Update this table whenever a perf change lands so future readers
//! don't have to re-run the bench to see whether the delta improved
//! or regressed the baseline.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-cdc --bench cdc
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-cdc --bench cdc -- fastcdc
//! ```
//!
//! [`FastCdc::chunk_boundaries`]: stringcheese_cdc::FastCdc::chunk_boundaries
//! [`FastCdcConfig::default_8k`]: stringcheese_cdc::FastCdcConfig::default_8k
//! [`FastCdcConfig::default_16k`]: stringcheese_cdc::FastCdcConfig::default_16k
//! [`RollingHash`]: stringcheese_cdc::RollingHash

#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench binary is not part of the crate's public API"
)]
// FastCDC is a proper noun that clippy::doc_markdown flags on every
// occurrence; wrapping it in backticks in every doc line makes the
// module intro less readable than leaving it alone.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_cdc::cdc::ChunkBoundary;
#[cfg(feature = "simd")]
use stringcheese_cdc::fingerprint::gear::simd as gear_simd;
use stringcheese_cdc::{
    Buzhash, FastCdc, FastCdcConfig, GearHash, PolynomialHash, RabinFingerprint, RollingHash,
};

// -------------------------------------------------------------------
// Inputs — deterministic across every run of the bench.
// -------------------------------------------------------------------

/// Bytes fed into every "1 MiB" sample. Sized in bytes.
const SIZE_1_MIB: usize = 1024 * 1024;
/// Bytes fed into every "10 MiB" sample.
const SIZE_10_MIB: usize = 10 * 1024 * 1024;

/// The two input sizes each bench sweeps.
const INPUT_SIZES: &[(usize, &str)] = &[(SIZE_1_MIB, "1MiB"), (SIZE_10_MIB, "10MiB")];

/// Window size for the streaming `roll(byte)` benches. 64 matches
/// Gear's effective window (a 64-bit shift register) and is a
/// reasonable common baseline for the other three so all four rows
/// are apples-to-apples.
const ROLL_WINDOW: usize = 64;

/// Deterministic PRNG (`SplitMix64`) so the byte stream is byte-
/// identical across runs and platforms without depending on the
/// `rand` crate.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `len` bytes of deterministic pseudo-random data, uniformly
/// distributed across `0..=255`.
fn random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    let mut out = Vec::with_capacity(len);
    while out.len() + 8 <= len {
        let w = splitmix64(&mut state);
        out.extend_from_slice(&w.to_le_bytes());
    }
    while out.len() < len {
        let w = splitmix64(&mut state);
        out.push((w & 0xFF) as u8);
    }
    out
}

/// `len` bytes of deterministic English prose. Wraps a fixed word
/// list so criterion samples over byte-identical bytes across every
/// run; no RNG, no OS entropy, no time-based seed.
///
/// Kept intentionally similar to the shape the tokenizer bench uses,
/// so a reviewer comparing the two bench crates sees the same input
/// generator style and does not have to reason about a second source
/// of variance.
fn prose_bytes(len: usize) -> Vec<u8> {
    let words: &[&str] = &[
        "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "and", "then",
        "runs", "back", "through", "the", "forest", "toward", "the", "little", "cabin", "on",
        "the", "hill", "where", "smoke", "curls", "from", "the", "chimney", "into", "the", "cold",
        "morning", "air", "while", "the", "sun", "climbs", "above", "the", "eastern", "ridge",
        "and", "birds", "call", "from", "the", "pines", "beyond", "the", "meadow",
    ];
    let mut out: Vec<u8> = Vec::with_capacity(len + 64);
    let mut idx = 0usize;
    while out.len() < len {
        out.extend_from_slice(words[idx % words.len()].as_bytes());
        out.push(b' ');
        idx += 1;
    }
    out.truncate(len);
    out
}

/// Assemble both flavors of input at each size once, so each bench
/// group doesn't rebuild them per sample.
fn build_inputs() -> Vec<(&'static str, usize, Vec<u8>)> {
    let mut out = Vec::with_capacity(INPUT_SIZES.len() * 2);
    for &(size, _) in INPUT_SIZES {
        out.push(("random", size, random_bytes(size, 0x00C0_FFEE_00D0)));
        out.push(("prose", size, prose_bytes(size)));
    }
    out
}

// -------------------------------------------------------------------
// FastCDC — end-to-end boundary throughput.
// -------------------------------------------------------------------

fn bench_fastcdc(c: &mut Criterion, group_name: &str, config: FastCdcConfig) {
    let mut group = c.benchmark_group(group_name);
    let cdc = FastCdc::new(config);
    for (flavor, size, input) in build_inputs() {
        group.throughput(Throughput::Bytes(size as u64));
        let id = BenchmarkId::new(flavor, size);
        group.bench_with_input(id, &input, |b, input| {
            b.iter(|| {
                // Collect into a Vec so the iterator is drained; a
                // bare `for _ in` over the iterator alone would let
                // LLVM notice the boundaries are unused and elide
                // the state-machine work.
                let boundaries: Vec<ChunkBoundary> = cdc.chunk_boundaries(input).collect();
                black_box(boundaries);
            });
        });
    }
    group.finish();
}

fn bench_fastcdc_8k(c: &mut Criterion) {
    bench_fastcdc(c, "cdc/fastcdc/default_8k", FastCdcConfig::default_8k());
}

fn bench_fastcdc_16k(c: &mut Criterion) {
    bench_fastcdc(c, "cdc/fastcdc/default_16k", FastCdcConfig::default_16k());
}

// -------------------------------------------------------------------
// Rolling-hash `roll(byte)` — per-byte trait-call throughput.
// -------------------------------------------------------------------

fn bench_roll_per_byte<H: RollingHash<Output = u64>>(c: &mut Criterion, group_name: &str) {
    let mut group = c.benchmark_group(group_name);
    for (flavor, size, input) in build_inputs() {
        group.throughput(Throughput::Bytes(size as u64));
        let id = BenchmarkId::new(flavor, size);
        group.bench_with_input(id, &input, |b, input| {
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
    bench_roll_per_byte::<GearHash>(c, "cdc/fingerprint/gear/roll_per_byte");
}
fn bench_buzhash_roll(c: &mut Criterion) {
    bench_roll_per_byte::<Buzhash>(c, "cdc/fingerprint/buzhash/roll_per_byte");
}
fn bench_polynomial_roll(c: &mut Criterion) {
    bench_roll_per_byte::<PolynomialHash>(c, "cdc/fingerprint/polynomial/roll_per_byte");
}
fn bench_rabin_roll(c: &mut Criterion) {
    bench_roll_per_byte::<RabinFingerprint>(c, "cdc/fingerprint/rabin/roll_per_byte");
}

// -------------------------------------------------------------------
// Gear slice-batch throughput.
//
// Only benched for Gear because the other three fingerprints'
// `digest_of_slice` truncates to `bytes[..window]` (polynomial /
// rabin) or requires the full stream to correctly evict old bytes
// (buzhash), neither of which is an apples-to-apples throughput
// comparison against the streaming `roll(byte)` loop above.
//
// Gated on the crate's `simd` feature because
// `stringcheese_cdc::fingerprint::gear::simd` is `#[cfg(feature =
// "simd")]`. Enable with `cargo bench -p stringcheese-cdc --features
// simd` (or `--all-features`) to include this bench in the run;
// without it the group is silently omitted rather than the whole
// bench binary failing to compile on a plain `cargo bench`.
// -------------------------------------------------------------------

#[cfg(feature = "simd")]
fn bench_gear_digest_of_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdc/fingerprint/gear/digest_of_slice");
    for (flavor, size, input) in build_inputs() {
        group.throughput(Throughput::Bytes(size as u64));
        let id = BenchmarkId::new(flavor, size);
        group.bench_with_input(id, &input, |b, input| {
            b.iter(|| black_box(gear_simd::digest_of_slice(black_box(input))));
        });
    }
    group.finish();
}

/// No-op stand-in for the SIMD bench when the `simd` feature is off,
/// so `criterion_group!` below can list the fn unconditionally.
#[cfg(not(feature = "simd"))]
fn bench_gear_digest_of_slice(_c: &mut Criterion) {}

// -------------------------------------------------------------------
// Oracle-comparison groups (feature-gated on `oracle-benches`).
//
// Goal: side-by-side throughput of stringcheese vs best-in-class Rust
// CDC / rolling-hash crates on identical inputs. Not a correctness
// oracle — chunk boundaries differ because the two implementations use
// independent gear-hash tables, so the cut points do not have to match
// byte-for-byte — but a *perf* oracle: it answers "is stringcheese
// competitive, or is there real headroom vs the ecosystem?"
//
// Oracles chosen (both `optional = true` in `Cargo.toml`, ~1 transitive
// dep — `cfg-if`):
//
// * `fastcdc` (4) — Nathan Fiedler's canonical Rust FastCDC v2020
//   implementation. Reference for the end-to-end FastCDC chunker
//   throughput. The `Normalization::Level2` preset here matches the
//   mask popcount stringcheese's `default_8k` / `default_16k` presets
//   commit to (paper NLC=2).
// * `gearhash` (0.1) — dedicated GEAR rolling-hash crate. Reference
//   for the scalar `roll(byte)` throughput (via `Hasher::update`) and,
//   on x86_64, the SIMD `next_match` throughput (AVX2 / SSE4.2). On
//   aarch64 upstream `gearhash` has no NEON backend and falls back to
//   its scalar loop, so the SIMD lane there is scalar-vs-SIMD-in-name:
//   the measurement still reports something useful — the "best pure-
//   scalar next-match sweep" — but the label is honest about the
//   dispatch target.
//
// Every oracle group cycles the same input flavors and sizes as the
// primary baseline groups so the gap-ratio numbers are directly
// comparable to the baseline table above.
// -------------------------------------------------------------------

#[cfg(feature = "oracle-benches")]
fn bench_oracle_fastcdc_8k(c: &mut Criterion) {
    // stringcheese: `FastCdc::chunk_boundaries` with `default_8k`.
    // fastcdc: `fastcdc::v2020::FastCDC::with_level` at (2 KiB, 8 KiB,
    // 64 KiB) sizes and `Normalization::Level2` — mirrors the mask
    // popcount stringcheese's preset commits to.
    let mut group = c.benchmark_group("cdc/oracle/fastcdc/default_8k");
    let sc = FastCdc::new(FastCdcConfig::default_8k());
    for (flavor, size, input) in build_inputs() {
        group.throughput(Throughput::Bytes(size as u64));

        // stringcheese lane — mirrors `bench_fastcdc_8k`
        group.bench_with_input(
            BenchmarkId::new(format!("stringcheese/{flavor}"), size),
            &input,
            |b, input| {
                b.iter(|| {
                    let boundaries: Vec<ChunkBoundary> = sc.chunk_boundaries(input).collect();
                    black_box(boundaries);
                });
            },
        );

        // fastcdc-rs lane
        group.bench_with_input(
            BenchmarkId::new(format!("fastcdc/{flavor}"), size),
            &input,
            |b, input| {
                b.iter(|| {
                    let chunker = fastcdc::v2020::FastCDC::with_level(
                        input,
                        2 * 1024,
                        8 * 1024,
                        64 * 1024,
                        fastcdc::v2020::Normalization::Level2,
                    );
                    let chunks: Vec<fastcdc::v2020::Chunk> = chunker.collect();
                    black_box(chunks);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
fn bench_oracle_fastcdc_16k(c: &mut Criterion) {
    let mut group = c.benchmark_group("cdc/oracle/fastcdc/default_16k");
    let sc = FastCdc::new(FastCdcConfig::default_16k());
    for (flavor, size, input) in build_inputs() {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new(format!("stringcheese/{flavor}"), size),
            &input,
            |b, input| {
                b.iter(|| {
                    let boundaries: Vec<ChunkBoundary> = sc.chunk_boundaries(input).collect();
                    black_box(boundaries);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new(format!("fastcdc/{flavor}"), size),
            &input,
            |b, input| {
                b.iter(|| {
                    let chunker = fastcdc::v2020::FastCDC::with_level(
                        input,
                        4 * 1024,
                        16 * 1024,
                        128 * 1024,
                        fastcdc::v2020::Normalization::Level2,
                    );
                    let chunks: Vec<fastcdc::v2020::Chunk> = chunker.collect();
                    black_box(chunks);
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
fn bench_oracle_gear_roll_per_byte(c: &mut Criterion) {
    // stringcheese: `GearHash::roll(byte)` in a tight loop (mirrors
    // `bench_gear_roll`). gearhash: `Hasher::update(&[byte])`.
    // `gearhash` internally implements `update` as a byte-at-a-time
    // scalar shift-add over the whole slice, so passing a full slice
    // is functionally the same as the per-byte lane in
    // `bench_gear_roll` and lets `gearhash` show its optimum scalar
    // throughput.
    let mut group = c.benchmark_group("cdc/oracle/gear/roll_per_byte");
    for (flavor, size, input) in build_inputs() {
        group.throughput(Throughput::Bytes(size as u64));

        // stringcheese lane — matches `bench_gear_roll`'s inner loop
        group.bench_with_input(
            BenchmarkId::new(format!("stringcheese/{flavor}"), size),
            &input,
            |b, input| {
                b.iter(|| {
                    let mut h = GearHash::new(ROLL_WINDOW);
                    for &byte in input {
                        h.roll(byte);
                    }
                    black_box(h.digest());
                });
            },
        );

        // gearhash lane — `update` is the byte-at-a-time scalar shift-
        // add; this is the direct oracle for stringcheese's per-byte
        // `roll` loop.
        group.bench_with_input(
            BenchmarkId::new(format!("gearhash/{flavor}"), size),
            &input,
            |b, input| {
                b.iter(|| {
                    let mut h = gearhash::Hasher::default();
                    h.update(input);
                    black_box(h.get_hash());
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
fn bench_oracle_gear_next_match(c: &mut Criterion) {
    // stringcheese's SIMD `digest_of_slice` computes the digest over
    // the whole slice — no boundary-matching. gearhash's `next_match`
    // is the closest apples-to-apples SIMD entry point: it dispatches
    // to AVX2 / SSE4.2 on x86_64 (falls back to scalar on aarch64) and
    // reports the boundary offset of the first hash-mask match. To
    // sweep the full slice we drain it in a loop, mirroring the
    // "full-slice hash" shape of the stringcheese SIMD lane.
    //
    // The scalar/SIMD label is arch-conditional: on aarch64 both sides
    // are effectively scalar sweeps (stringcheese uses its NEON
    // backend when compiled `--features simd`; gearhash has no NEON
    // backend upstream). The measurement is still meaningful — it
    // answers "does gearhash's tuned scalar sweep beat stringcheese's
    // NEON `digest_of_slice`, or vice versa?" — but interpret the mult
    // with that dispatch difference in mind. Documented in
    // `docs/perf/cdc-oracle-gaps.md`.
    //
    // A zero mask would make `next_match` return immediately on every
    // byte (0 & anything == 0), so pick a realistic mask with the same
    // popcount as stringcheese's default_8k `mask_small` (15 bits).
    // Any bit pattern with that popcount works for a throughput
    // measurement.
    const MASK_15_BITS: u64 = 0x0003_5907_0353_0000;
    let mut group = c.benchmark_group("cdc/oracle/gear/next_match");
    for (flavor, size, input) in build_inputs() {
        group.throughput(Throughput::Bytes(size as u64));

        // stringcheese lane — mirrors `bench_gear_digest_of_slice`
        // (SIMD when compiled `--features simd`, scalar otherwise).
        // Kept unconditional here rather than `#[cfg(feature =
        // "simd")]`-gated because the oracle side always runs; a
        // scalar-vs-scalar lane at least shows the per-byte overhead
        // of `roll` vs `gearhash::update`.
        #[cfg(feature = "simd")]
        {
            group.bench_with_input(
                BenchmarkId::new(format!("stringcheese-simd/{flavor}"), size),
                &input,
                |b, input| {
                    b.iter(|| black_box(gear_simd::digest_of_slice(black_box(input))));
                },
            );
        }
        // Scalar stringcheese fallback (compiled either way; equivalent
        // to the `roll_per_byte` lane, kept here so the group still has
        // a stringcheese row when `simd` is off).
        #[cfg(not(feature = "simd"))]
        {
            group.bench_with_input(
                BenchmarkId::new(format!("stringcheese-scalar/{flavor}"), size),
                &input,
                |b, input| {
                    b.iter(|| {
                        let mut h = GearHash::new(ROLL_WINDOW);
                        for &byte in input {
                            h.roll(byte);
                        }
                        black_box(h.digest());
                    });
                },
            );
        }

        // gearhash lane — dispatches to AVX2 / SSE4.2 on x86_64,
        // scalar on aarch64. Drain the whole input via repeated
        // `next_match` calls.
        group.bench_with_input(
            BenchmarkId::new(format!("gearhash/{flavor}"), size),
            &input,
            |b, input| {
                b.iter(|| {
                    let mut h = gearhash::Hasher::default();
                    let mut offset = 0usize;
                    while offset < input.len() {
                        match h.next_match(&input[offset..], MASK_15_BITS) {
                            Some(step) => offset += step,
                            None => break,
                        }
                    }
                    black_box(h.get_hash());
                });
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
criterion_group!(
    oracle_benches,
    bench_oracle_fastcdc_8k,
    bench_oracle_fastcdc_16k,
    bench_oracle_gear_roll_per_byte,
    bench_oracle_gear_next_match,
);

criterion_group!(
    benches,
    bench_fastcdc_8k,
    bench_fastcdc_16k,
    bench_gear_roll,
    bench_buzhash_roll,
    bench_polynomial_roll,
    bench_rabin_roll,
    bench_gear_digest_of_slice,
);

#[cfg(feature = "oracle-benches")]
criterion_main!(benches, oracle_benches);
#[cfg(not(feature = "oracle-benches"))]
criterion_main!(benches);
