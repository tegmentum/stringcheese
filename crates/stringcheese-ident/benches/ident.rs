//! Baseline throughput benchmarks for the identifier / slug /
//! sanitizer surfaces in `stringcheese-ident`.
//!
//! Mirrors the shape of the sibling bench files
//! (`stringcheese-compare` / `stringcheese-manip` /
//! `stringcheese-textsplit`): one bench file, criterion-owned `main`
//! via `harness = false`, groups named `<surface>/<flavor>/<len>`.
//! Regression trip-wire + last-measured baseline table for the four
//! public surfaces `stringcheese-bench/benches/ident.rs` used to
//! cover.
//!
//! # Groups
//!
//! Four groups, one per public surface:
//!
//! * `to_case` — [`to_case`], the heck-backed case conversion. Four
//!   target variants swept per size × flavor: [`Case::Snake`],
//!   [`Case::Camel`], [`Case::Pascal`], [`Case::Kebab`]. Historically
//!   the slowest surface (heck's per-word walk + re-encoding).
//! * `detect` — [`Case::detect`], the byte-level scan classifier.
//!   Should hit multi-GiB/s territory — a memory-bound arm.
//! * `slugify` — [`slugify`], the deunicode → filter → collapse
//!   pipeline. ASCII input skips deunicode's substitution table;
//!   accented input pays it once per non-ASCII scalar.
//! * `sanitize` — [`Sanitizer::sanitize`] with the default policy.
//!   Filters non-identifier characters, enforces valid start,
//!   caps length.
//!
//! # Sizes
//!
//! Two input byte lengths per (surface, flavor): 256 B / 4 KiB.
//! Matches the sibling benches; wider sweeps didn't add signal
//! given the algorithms are all O(N) with small constants.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `ascii` — deterministic English prose / snake identifiers.
//!   Every byte is ASCII; the fast paths of every surface here
//!   fire.
//! * `accented` — same shape with sprinkled Latin diacritics.
//!   `slugify` pays the deunicode substitution cost; `sanitize`
//!   filters the non-identifier scalars; `to_case` and `detect`
//!   are less affected because they operate on the case /
//!   separator structure, not the specific scalars.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-ident
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-ident -- slugify
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-ident --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_ident::{Case, Sanitizer, slugify, to_case};

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG; every `cargo bench` invocation feeds byte-identical
// inputs so criterion's noise floor stays meaningful across runs.
// ---------------------------------------------------------------------------

/// ASCII identifier-shaped seed — `snake_case` words separated by
/// underscores. Cycled to build inputs of arbitrary size.
const ASCII_SEED: &str =
    "hello_world_foo_bar_baz_quux_alpha_beta_gamma_delta_epsilon_zeta_eta_theta ";

/// Accented seed — mostly ASCII with sprinkled Latin diacritics.
/// Same shape as ASCII (so structural surfaces like `detect` /
/// `to_case` still find identifier boundaries) with per-word
/// non-ASCII scalars for the `slugify` / `sanitize` paths to
/// substitute or filter.
const ACCENTED_SEED: &str =
    "café résumé façade naïve piñata jalapeño Zürich München São_Paulo Kraków ";

/// Build a string of *at least* `bytes` bytes by cycling `seed`.
fn build_input(seed: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + seed.len());
    while out.len() < bytes {
        out.push_str(seed);
    }
    out
}

/// Byte lengths swept by every (surface, flavor) cell.
const SIZES: &[usize] = &[256, 4 * 1024];

/// The two flavors every group runs.
fn flavors() -> [(&'static str, &'static str); 2] {
    [("ascii", ASCII_SEED), ("accented", ACCENTED_SEED)]
}

/// The four case targets swept by the `to_case` group.
fn case_targets() -> [(&'static str, Case); 4] {
    [
        ("snake", Case::Snake),
        ("camel", Case::Camel),
        ("pascal", Case::Pascal),
        ("kebab", Case::Kebab),
    ]
}

// ---------------------------------------------------------------------------
// Bench groups. Throughput reported over *input bytes* — higher is
// better.
// ---------------------------------------------------------------------------

fn bench_to_case(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/to_case");
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            for (case_name, target) in case_targets() {
                group.throughput(Throughput::Bytes(input.len() as u64));
                let id = BenchmarkId::new(format!("{flavor}/{case_name}"), input.len());
                group.bench_with_input(id, &input, |bencher, input| {
                    bencher.iter(|| black_box(to_case(black_box(input), target)));
                });
            }
        }
    }
    group.finish();
}

fn bench_detect(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/detect");
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            group.throughput(Throughput::Bytes(input.len() as u64));
            let id = BenchmarkId::new(flavor, input.len());
            group.bench_with_input(id, &input, |bencher, input| {
                bencher.iter(|| black_box(Case::detect(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_slugify(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/slugify");
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            group.throughput(Throughput::Bytes(input.len() as u64));
            let id = BenchmarkId::new(flavor, input.len());
            group.bench_with_input(id, &input, |bencher, input| {
                bencher.iter(|| black_box(slugify(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_sanitize(c: &mut Criterion) {
    let mut group = c.benchmark_group("ident/sanitize");
    let sanitizer = Sanitizer::default();
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            group.throughput(Throughput::Bytes(input.len() as u64));
            let id = BenchmarkId::new(flavor, input.len());
            group.bench_with_input(id, &input, |bencher, input| {
                bencher.iter(|| black_box(sanitizer.sanitize(black_box(input))));
            });
        }
    }
    group.finish();
}

criterion_group!(
    ident,
    bench_to_case,
    bench_detect,
    bench_slugify,
    bench_sanitize,
);
criterion_main!(ident);
