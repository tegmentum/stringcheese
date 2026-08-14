//! Baseline throughput benchmarks for `stringcheese-align`.
//!
//! Mirrors the shape of the reference bench crate
//! `stringcheese-tokenizer-hf-bench`: one bench file, criterion-owned
//! `main` via `harness = false`, groups named
//! `<kernel>/<flavor>/<size>`. The purpose is a regression trip-wire
//! plus a last-measured baseline table for the crate that owns the
//! v0.2 Needleman-Wunsch and Smith-Waterman kernels.
//!
//! # Kernels
//!
//! Six groups pairing every (algorithm, gap-model, output) combination
//! the crate exposes:
//!
//! * `nw_score_linear` — Needleman-Wunsch global-alignment *score* with
//!   a single-matrix linear-gap DP. Rolling-row scratch, single-cell
//!   backtrack column, no edit-script cost.
//! * `nw_score_affine` — same, but with a three-matrix Gotoh (1982)
//!   affine-gap DP. Roughly 3× the per-cell work of the linear-gap
//!   score kernel.
//! * `nw_align_linear` — Needleman-Wunsch *align* (score + reconstructed
//!   edit script). Full-matrix DP with a backtrace from `(m, n)` to
//!   `(0, 0)`; adds the O(m·n) matrix retention and the O(m + n)
//!   backtrace on top of the score-only cost.
//! * `sw_score_linear` — Smith-Waterman local-alignment score. Same
//!   single-row rolling scratch as `nw_score_linear` plus a
//!   running-max update per cell.
//! * `sw_score_affine` — Smith-Waterman score with affine-gap Gotoh
//!   DP.
//! * `sw_align_linear` — Smith-Waterman *align* (score + script + start
//!   indices of the aligned substrings).
//!
//! # Input sizes
//!
//! Three sizes per group: 32×32, 128×128, 512×512. Both aligners are
//! O(m·n) — 512² is ≈260k cell updates, still comfortably sub-second
//! on modern hardware; anything larger becomes noisy on a shared
//! runner and pushes the *align* kernels' backtrace matrix past a
//! MiB of allocation.
//!
//! # Flavors
//!
//! Two per size, chosen to bracket the DP's runtime constant:
//!
//! * `matching` — mostly-matching byte sequences (a common prefix
//!   plus a tail differing at ~5% of positions). This is the shape
//!   Smith-Waterman's local-alignment optimisation is designed for:
//!   its running-max fast-forwards through low-scoring regions.
//! * `divergent` — deterministic random `[a-z]` bytes on both operands.
//!   No score-friendly common substring; both algorithms fall into
//!   the DP-full-fill worst case.
//!
//! # Sequence type
//!
//! Byte slices (`&[u8]`) throughout. The align kernels are generic
//! over `&[T]` where `T: Eq`, but bytes are the canonical bio-sequence
//! encoding the crate's design-doc examples use; adding a `&[char]`
//! flavor would multiply group count without giving new signal
//! (`char` is 4× the bytes per element and the DP work is scalar
//! `==`-driven regardless).
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-align
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-align -- nw_score_linear
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-align --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use stringcheese_align::needleman_wunsch::NeedlemanWunsch;
use stringcheese_align::scoring::{AffineGap, LinearGap};
use stringcheese_align::smith_waterman::SmithWaterman;
use stringcheese_align::workspace::AlignmentWorkspace;

// ---------------------------------------------------------------------------
// Deterministic RNG + input generation. Self-contained (no cross-crate
// dev-deps) so the bench file compiles standalone.
// ---------------------------------------------------------------------------

/// Size grid swept by every group. Both operands share the size, so
/// each entry `n` translates to an n × n DP grid.
const SIZES: &[usize] = &[32, 128, 512];

/// Deterministic per-(size, salt) seed. Uses the size as an entropy
/// mixer so adding rows to `SIZES` does not reshuffle unrelated pairs.
#[inline]
fn seed_for(size: usize, salt: u64) -> u64 {
    (size as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

/// Minimal `SplitMix64` PRNG — see the equivalent helper in
/// `stringcheese-compare/benches/compare.rs` for the rationale for
/// keeping this self-contained.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_ascii_lower(&mut self) -> u8 {
        // Modulo-26 of a u64 fits in a u8 by construction; the truncation
        // is intentional and the wrapping add to b'a' stays within ASCII.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "value is bounded by % 26, always fits in u8"
        )]
        let byte = (self.next_u64() % 26) as u8;
        b'a' + byte
    }

    /// Uniform-ish `usize` in `[0, bound)`. Bench-only, so the modulo
    /// bias is acceptable — every draw is used to perturb one input
    /// position, and the pattern is deterministic per seed anyway.
    fn next_bounded(&mut self, bound: usize) -> usize {
        // On 32-bit targets `u64 as usize` truncates the top 32 bits;
        // that only biases the low bits, which is fine for corpus
        // perturbation. On 64-bit targets the cast is lossless.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bench-only bounded draw; low-bit bias acceptable"
        )]
        let r = self.next_u64() as usize;
        r % bound.max(1)
    }
}

/// Two divergent random `[a-z]` byte strings of length `n`. Every
/// symbol pair is independent, so the DP falls into the mostly-full-fill
/// worst case with a mostly-negative-tending running score.
fn divergent_pair(n: usize) -> (Vec<u8>, Vec<u8>) {
    let mut a = Vec::with_capacity(n);
    let mut b = Vec::with_capacity(n);
    let mut ra = Rng::new(seed_for(n, 0x31));
    let mut rb = Rng::new(seed_for(n, 0x32));
    for _ in 0..n {
        a.push(ra.next_ascii_lower());
        b.push(rb.next_ascii_lower());
    }
    (a, b)
}

/// Two mostly-matching byte strings of length `n`: `b` starts as a copy
/// of `a`, then ~5% of positions are perturbed with independent
/// random bytes. This is the shape Smith-Waterman's running-max
/// short-circuit is designed for.
fn matching_pair(n: usize) -> (Vec<u8>, Vec<u8>) {
    let mut a = Vec::with_capacity(n);
    let mut ra = Rng::new(seed_for(n, 0x41));
    for _ in 0..n {
        a.push(ra.next_ascii_lower());
    }
    let mut b = a.clone();
    let mut rb = Rng::new(seed_for(n, 0x42));
    let edits = n.div_ceil(20);
    for _ in 0..edits {
        let idx = rb.next_bounded(n);
        b[idx] = rb.next_ascii_lower();
    }
    (a, b)
}

/// A `(flavor, (a, b))` pair — one row per bench flavor.
type BenchPair = (&'static str, (Vec<u8>, Vec<u8>));

/// Build both flavors for a given size. Returned in the order they
/// appear in the module's flavor list (`matching`, then `divergent`).
fn build_pairs(size: usize) -> Vec<BenchPair> {
    vec![
        ("matching", matching_pair(size)),
        ("divergent", divergent_pair(size)),
    ]
}

// ---------------------------------------------------------------------------
// Needleman-Wunsch groups
// ---------------------------------------------------------------------------

fn bench_nw_score_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("align/nw_score_linear");
    let nw = NeedlemanWunsch::new(LinearGap::simple());
    for &size in SIZES {
        // Throughput reports as "bytes" on one operand; the true DP work
        // scales as size² but "bytes/s" gives criterion a numeric baseline
        // that changes proportionally to per-cell cost regressions.
        group.throughput(Throughput::Bytes(size as u64));
        for (flavor, (a, b)) in build_pairs(size) {
            let id = BenchmarkId::new(flavor, size);
            group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                let mut ws = AlignmentWorkspace::new();
                bencher.iter(|| {
                    nw.score_with_workspace(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_nw_score_affine(c: &mut Criterion) {
    let mut group = c.benchmark_group("align/nw_score_affine");
    let nw = NeedlemanWunsch::new(AffineGap::default_affine());
    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        for (flavor, (a, b)) in build_pairs(size) {
            let id = BenchmarkId::new(flavor, size);
            group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                let mut ws = AlignmentWorkspace::new();
                bencher.iter(|| {
                    nw.score_with_workspace(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_nw_align_linear(c: &mut Criterion) {
    // `align` retains the full DP matrix for the backtrace, so this row
    // is a stress-test of allocation reuse across iters as well as of the
    // DP fill itself.
    let mut group = c.benchmark_group("align/nw_align_linear");
    let nw = NeedlemanWunsch::new(LinearGap::simple());
    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        for (flavor, (a, b)) in build_pairs(size) {
            let id = BenchmarkId::new(flavor, size);
            group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                let mut ws = AlignmentWorkspace::new();
                bencher.iter(|| {
                    nw.align_with_workspace(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Smith-Waterman groups
// ---------------------------------------------------------------------------

fn bench_sw_score_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("align/sw_score_linear");
    let sw = SmithWaterman::new(LinearGap::simple());
    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        for (flavor, (a, b)) in build_pairs(size) {
            let id = BenchmarkId::new(flavor, size);
            group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                let mut ws = AlignmentWorkspace::new();
                bencher.iter(|| {
                    sw.score_with_workspace(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_sw_score_affine(c: &mut Criterion) {
    let mut group = c.benchmark_group("align/sw_score_affine");
    let sw = SmithWaterman::new(AffineGap::default_affine());
    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        for (flavor, (a, b)) in build_pairs(size) {
            let id = BenchmarkId::new(flavor, size);
            group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                let mut ws = AlignmentWorkspace::new();
                bencher.iter(|| {
                    sw.score_with_workspace(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

fn bench_sw_align_linear(c: &mut Criterion) {
    let mut group = c.benchmark_group("align/sw_align_linear");
    let sw = SmithWaterman::new(LinearGap::simple());
    for &size in SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        for (flavor, (a, b)) in build_pairs(size) {
            let id = BenchmarkId::new(flavor, size);
            group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                let mut ws = AlignmentWorkspace::new();
                bencher.iter(|| {
                    sw.align_with_workspace(
                        black_box(a.as_slice()),
                        black_box(b.as_slice()),
                        &mut ws,
                    )
                });
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_nw_score_linear,
    bench_nw_score_affine,
    bench_nw_align_linear,
    bench_sw_score_linear,
    bench_sw_score_affine,
    bench_sw_align_linear,
);
criterion_main!(benches);
