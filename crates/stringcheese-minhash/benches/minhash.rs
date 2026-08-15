//! Baseline throughput benchmarks for MinHash sketch build +
//! Jaccard comparison in `stringcheese-minhash`.
//!
//! Mirrors the shape of the sibling bench files
//! (`stringcheese-compare` / `stringcheese-manip` /
//! `stringcheese-textsplit`): one bench file, criterion-owned `main`
//! via `harness = false`, groups named `<op>/<width>/<n_items>`.
//! Regression trip-wire + last-measured baseline table for the two
//! phases MinHash pipelines actually spend time in.
//!
//! # Groups
//!
//! Two groups — one per phase of a real MinHash pipeline:
//!
//! * `sketch` — [`Sketcher::sketch`], the O(width × n_items) build
//!   cost. Every input scalar is hashed under every permutation;
//!   this is where every MinHash pipeline actually spends its
//!   time. Throughput reported over `n_items` (the number of grams
//!   the sketcher consumed) — comparable to "how many grams per
//!   second can we absorb into a sketch of this width".
//! * `jaccard` — [`Sketch::jaccard`], the O(width) comparison
//!   between two finalised sketches. Vastly cheaper than build;
//!   this is what LSH banding calls on every candidate. Throughput
//!   reported over `width` (the number of hash positions being
//!   compared).
//!
//! # Sizes swept
//!
//! Three sketch widths per group — 64 / 256 / 1024 permutations —
//! chosen to bracket the standard deviations that near-duplicate
//! pipelines actually pick from: width 64 is the loosest useful
//! sketch (σ ≈ 0.06 at true J=0.5), width 1024 the tightest most
//! callers reach for (σ ≈ 0.016).
//!
//! Three input sizes for the `sketch` group — 100 / 1000 / 10000
//! grams — bracketing the typical shingle count for a small
//! paragraph up to a document. For `jaccard`, `n_items` is not a
//! knob (comparison cost is fixed by sketch width, not by how
//! much input either sketch was built from).
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-minhash
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-minhash -- sketch
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-minhash --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]
// `MinHash` / `Jaccard` / `O(...)` trip `doc_markdown` — same allow
// shape the crate-level `src/lib.rs` uses; the docs are for humans.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_minhash::Sketcher;

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG; every `cargo bench` invocation feeds byte-identical
// inputs so criterion's noise floor stays meaningful across runs.
// ---------------------------------------------------------------------------

/// Build a deterministic vector of `n` unique short strings. The
/// exact form doesn't matter for the bench — what matters is that
/// every gram is distinct so no permutation gets stuck accepting
/// the same min value on the first item.
fn build_grams(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("gram-{i:08x}")).collect()
}

/// Sketch widths swept by both groups.
const WIDTHS: &[usize] = &[64, 256, 1024];

/// Gram counts swept by the `sketch` group.
const N_ITEMS: &[usize] = &[100, 1_000, 10_000];

// ---------------------------------------------------------------------------
// Bench groups. Throughput reported per-group over the appropriate
// unit — see doc comment above for the convention.
// ---------------------------------------------------------------------------

fn bench_sketch(c: &mut Criterion) {
    let mut group = c.benchmark_group("minhash/sketch");
    for &width in WIDTHS {
        let sketcher = Sketcher::new(width);
        for &n in N_ITEMS {
            let grams = build_grams(n);
            // Throughput reported over item count — "grams / s". Not
            // bytes because the sketch cost is per-hash, not per-byte,
            // and every gram here is fixed-width.
            group.throughput(Throughput::Elements(n as u64));
            let id = BenchmarkId::new(format!("w{width}"), n);
            group.bench_with_input(id, &grams, |bencher, grams| {
                bencher.iter(|| {
                    black_box(sketcher.sketch(black_box(grams).iter().map(String::as_str)))
                });
            });
        }
    }
    group.finish();
}

fn bench_jaccard(c: &mut Criterion) {
    let mut group = c.benchmark_group("minhash/jaccard");
    for &width in WIDTHS {
        // Build two sketches of the target width. The specific
        // similarity number they land on doesn't matter for the
        // bench — `jaccard` walks every position regardless of
        // match/no-match.
        let sketcher = Sketcher::new(width);
        let a = sketcher.sketch(build_grams(200).iter().map(String::as_str));
        let b = sketcher.sketch(build_grams(200).iter().map(String::as_str));
        // Throughput reported over sketch width — "positions / s".
        group.throughput(Throughput::Elements(width as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(width),
            &(a, b),
            |bencher, (a, b)| {
                bencher.iter(|| black_box(black_box(a).jaccard(black_box(b))));
            },
        );
    }
    group.finish();
}

criterion_group!(minhash, bench_sketch, bench_jaccard);
criterion_main!(minhash);
