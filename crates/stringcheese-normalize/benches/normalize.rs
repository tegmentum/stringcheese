//! Baseline throughput benchmarks for the four named normalization
//! pipelines in `stringcheese-normalize`.
//!
//! Mirrors the shape of the sibling bench files
//! (`stringcheese-compare` / `stringcheese-manip` /
//! `stringcheese-textsplit`): one bench file, criterion-owned `main`
//! via `harness = false`, groups named `<pipeline>/<flavor>/<len>`.
//! Regression trip-wire + last-measured baseline table for the
//! four presets this crate ships.
//!
//! # Groups
//!
//! Four groups — one per shipped preset pipeline:
//!
//! * `identifier` — NFKC → case-fold → strip-diacritics. Used for
//!   identifier comparison / deduplication. NFKC is the
//!   bottleneck.
//! * `display_safe` — strip-controls → NFC → collapse-whitespace.
//!   Used before rendering user-supplied text. NFC is lighter
//!   than NFKC so this arm is ~2.5× faster than `identifier`.
//! * `search_key` — NFKC → case-fold → strip-diacritics →
//!   canonicalise-punctuation → collapse-whitespace → trim. The
//!   heaviest arm; NFKC + all downstream primitives on top.
//! * `punctuation_canonical` — canonicalise-punctuation only. The
//!   lightest arm; a single-pass in-house primitive with no
//!   ICU cost.
//!
//! # Sizes
//!
//! Three input byte lengths per (pipeline, flavor): 1 KiB / 4 KiB
//! / 16 KiB. Matches the shape the crate's earlier baseline table
//! documents; the 16 KiB row is where the pipeline reaches steady
//! state and per-call setup cost is fully amortised.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `ascii` — deterministic English prose. The NFKC / NFC arms
//!   still run but land in their fast path (nothing to
//!   decompose); every ASCII scalar passes through the strip-
//!   controls / whitespace / punctuation primitives at full
//!   speed. Numbers here are the "ambient" per-byte cost.
//! * `diacritics` — mixed Latin prose with smart quotes, em
//!   dashes, non-breaking spaces, and every combining-mark form
//!   NFKC has to fold. This is the realistic real-world input
//!   the pipelines were designed for and where the ICU
//!   normalisation cost shows up.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-normalize
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-normalize -- identifier
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-normalize --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]
// `NFC` / `NFKC` / `ICU` trip `doc_markdown` — same allow shape the
// crate-level `src/lib.rs` uses; the docs are for humans.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_normalize::{display_safe, identifier, punctuation_canonical, search_key};

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG; every `cargo bench` invocation feeds byte-identical
// inputs so criterion's noise floor stays meaningful across runs.
// ---------------------------------------------------------------------------

/// ASCII prose seed — every scalar is one byte; the NFC/NFKC arms
/// hit their fast path (nothing to decompose).
const ASCII_SEED: &str =
    "The quick brown fox jumps over the lazy dog. Every good boy deserves fudge.  ";

/// Diacritics-heavy seed — mixed Latin prose with smart quotes, em
/// dashes, non-breaking spaces, combining marks; every stage of
/// every pipeline does load-bearing work here.
const DIACRITICS_SEED: &str =
    "Café \u{201c}résumé\u{201d} — façade\u{a0}naïve piñata Zürich Kraków\u{2026} \
     jalapeño coöperate\u{200b}nöel.  ";

/// Build a string of *at least* `bytes` bytes by cycling `seed`.
fn build_input(seed: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + seed.len());
    while out.len() < bytes {
        out.push_str(seed);
    }
    out
}

/// Byte lengths swept by every (pipeline, flavor) cell.
const SIZES: &[usize] = &[1024, 4 * 1024, 16 * 1024];

/// The two flavors every group runs.
fn flavors() -> [(&'static str, &'static str); 2] {
    [("ascii", ASCII_SEED), ("diacritics", DIACRITICS_SEED)]
}

/// One helper per group — every pipeline shares the shape
/// `fn(&str) -> String`, so a parametrised helper collapses the
/// four groups into one code path.
fn bench_pipeline(c: &mut Criterion, group_name: &str, pipeline: fn(&str) -> String) {
    let mut group = c.benchmark_group(format!("normalize/{group_name}"));
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            group.throughput(Throughput::Bytes(input.len() as u64));
            let id = BenchmarkId::new(flavor, input.len());
            group.bench_with_input(id, &input, |bencher, input| {
                bencher.iter(|| black_box(pipeline(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_identifier(c: &mut Criterion) {
    bench_pipeline(c, "identifier", identifier);
}
fn bench_display_safe(c: &mut Criterion) {
    bench_pipeline(c, "display_safe", display_safe);
}
fn bench_search_key(c: &mut Criterion) {
    bench_pipeline(c, "search_key", search_key);
}
fn bench_punctuation_canonical(c: &mut Criterion) {
    bench_pipeline(c, "punctuation_canonical", punctuation_canonical);
}

criterion_group!(
    normalize,
    bench_identifier,
    bench_display_safe,
    bench_search_key,
    bench_punctuation_canonical,
);
criterion_main!(normalize);
