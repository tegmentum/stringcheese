//! Hamming benchmarks.
//!
//! Two shapes: exact `hamming_distance` and bounded
//! `hamming_distance_within` with tight/permissive/no cutoffs. Hamming is
//! defined for equal-length inputs only, so every corpus is built from
//! [`similar_pair_equal_len`] (which perturbs a random string by
//! substitution only, keeping lengths equal).
//!
//! # SIMD vs scalar
//!
//! The `hamming/scalar_vs_simd` group compares the generic scalar kernel
//! ([`hamming_distance`]) directly against the SIMD-dispatched byte-slice
//! entry point ([`Hamming::distance_bytes`]) across the length band the
//! SIMD backend actually kicks in on (32 bytes and up, following the
//! amenability threshold defined in `hamming::simd`). Inputs are drawn
//! from four difference densities — 0 %, 25 %, 50 %, 100 % — so any
//! backend that mis-accumulates a fully-matched or fully-mismatched block
//! surfaces here as a wrong-answer bench, not just a perf regression.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use stringcheese_bench::inputs::{identical_pair, random_ascii, similar_pair_equal_len};
use stringcheese_compare::hamming::{Hamming, hamming_distance, hamming_distance_within};

/// Hamming is O(n); large n stays cheap, so we sweep the same set of
/// lengths the DP-based kernels use and extend up to 2048.
const LENGTHS: &[usize] = &[8, 32, 128, 512, 2048];

/// Extended length sweep used by the SIMD-vs-scalar group. Covers the
/// full SIMD-relevant band (32 bytes crosses the amenability threshold;
/// 4096 exercises the many-block regime where the per-block win matters
/// most).
const SIMD_LENGTHS: &[usize] = &[16, 32, 64, 128, 256, 1024, 4096];

#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn build_equal_len_pair(len: usize, kind: &str) -> (Vec<u8>, Vec<u8>) {
    match kind {
        "random" => {
            // Two independent random strings of equal length. The
            // expected Hamming distance is 25/26 * len (all positions
            // mismatch except by chance) — the "mostly-dissimilar" case.
            (
                random_ascii(len, seed_for(len, 0x11)),
                random_ascii(len, seed_for(len, 0x12)),
            )
        }
        "similar" => similar_pair_equal_len(len, 0.05, seed_for(len, 0x13)),
        "identical" => identical_pair(len, seed_for(len, 0x14)),
        _ => unreachable!("unknown similarity regime"),
    }
}

/// Deterministic equal-length pair with a target difference density.
///
/// * `density = 0.0` — identical
/// * `density = 1.0` — every byte differs
/// * intermediate — approximately `density * len` positions differ
///   (positions may collide under substitution, so the observed count
///   is a slight undercount at high densities, matching what
///   [`similar_pair_equal_len`] does).
fn build_density_pair(len: usize, density: f64) -> (Vec<u8>, Vec<u8>) {
    // Convert `density` to a stable per-call seed. `density` is fed only
    // known-safe values (0.0, 0.25, 0.5, 1.0) from the caller, so the
    // cast to `u64` is exact by construction.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "caller-supplied density is in [0.0, 1.0] by construction; the cast is exact for the small integer seed value"
    )]
    let density_seed = (density * 1000.0) as u64;
    let seed = seed_for(len, density_seed);
    if density == 0.0 {
        return identical_pair(len, seed);
    }
    if density >= 1.0 {
        let a = random_ascii(len, seed);
        // Complement every byte to guarantee every position differs.
        let b: Vec<u8> = a.iter().map(|&x| x ^ 0xff).collect();
        return (a, b);
    }
    similar_pair_equal_len(len, density, seed)
}

fn bench_exact(c: &mut Criterion) {
    let mut group = c.benchmark_group("hamming/exact");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| hamming_distance(black_box(a.as_slice()), black_box(b.as_slice())));
            });
        }
    }
    group.finish();
}

fn bench_within_no_cutoff(c: &mut Criterion) {
    // Cutoff = u32::MAX means "no early termination"; measures
    // `hamming_distance_within`'s per-iteration overhead vs.
    // `hamming_distance`.
    let mut group = c.benchmark_group("hamming/within_no_cutoff");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| {
                    hamming_distance_within(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        black_box(u32::MAX),
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_within_permissive(c: &mut Criterion) {
    // Cutoff = len is the "tight" bound in the sense that a mostly-
    // dissimilar random pair will still fit under it (expected distance
    // ≈ 0.96 · len). No early-out expected on random inputs; on similar
    // and identical, the loop runs to completion but never exceeds.
    let mut group = c.benchmark_group("hamming/within_permissive_k_len");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            let cutoff = u32::try_from(len).unwrap_or(u32::MAX);
            group.bench_with_input(
                BenchmarkId::new(kind, len),
                &(a, b, cutoff),
                |bencher, (a, b, cutoff)| {
                    bencher.iter(|| {
                        hamming_distance_within(
                            black_box(a.as_slice()),
                            black_box(b.as_slice()),
                            black_box(*cutoff),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_within_tight(c: &mut Criterion) {
    // Cutoff = 3: the spellcheck bound. Expected wins are:
    //   * random inputs: dramatic — early termination fires within ~3
    //     positions on average, so timing should be roughly constant in
    //     `len` rather than linear.
    //   * similar inputs (5% edit rate): mixed; at len=8 the whole
    //     string fits, at len=512 we're far above cutoff and terminate
    //     early.
    //   * identical inputs: still walks the whole string (count stays
    //     at 0), so no win.
    let mut group = c.benchmark_group("hamming/within_tight_k3");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &kind in &["random", "similar", "identical"] {
            let (a, b) = build_equal_len_pair(len, kind);
            group.bench_with_input(BenchmarkId::new(kind, len), &(a, b), |bencher, (a, b)| {
                bencher.iter(|| {
                    hamming_distance_within(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        black_box(3u32),
                    )
                });
            });
        }
    }
    group.finish();
}

/// Direct scalar-vs-SIMD comparison across the SIMD-relevant length band
/// and four difference densities.
///
/// Each `(len, density, path)` triple is a separate criterion measurement;
/// the `path` axis is either `scalar` (calls [`hamming_distance`], which
/// bypasses SIMD entirely) or `simd` (calls
/// [`Hamming::distance_bytes`], which dispatches to the best backend
/// available on the host CPU). The ratio between the two per `(len,
/// density)` cell is the SIMD delta reported in the crate's docs.
fn bench_scalar_vs_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("hamming/scalar_vs_simd");
    let alg = Hamming;
    for &len in SIMD_LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &density in &[0.0f64, 0.25, 0.50, 1.0] {
            let (a, b) = build_density_pair(len, density);
            // Encode density as a percentage string in the bench id.
            // `density` is a bench-fixture constant in [0.0, 1.0], so the
            // multiply-by-100 fits in a u32 without truncation.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "density is a fixture constant in [0.0, 1.0]; the cast is exact for the small integer tag value"
            )]
            let pct = (density * 100.0) as u32;
            let density_tag = format!("d{pct:03}");
            group.bench_with_input(
                BenchmarkId::new(format!("scalar/{density_tag}"), len),
                &(a.clone(), b.clone()),
                |bencher, (a, b)| {
                    bencher.iter(|| {
                        hamming_distance(black_box(a.as_slice()), black_box(b.as_slice()))
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("simd/{density_tag}"), len),
                &(a, b),
                |bencher, (a, b)| {
                    bencher.iter(|| {
                        alg.distance_bytes(black_box(a.as_slice()), black_box(b.as_slice()))
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_exact,
    bench_within_no_cutoff,
    bench_within_permissive,
    bench_within_tight,
    bench_scalar_vs_simd,
);
criterion_main!(benches);
