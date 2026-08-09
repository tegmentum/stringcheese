//! Normalize — named pipelines and their primitives.
//!
//! Every named pipeline in `stringcheese-normalize` composes
//! several primitives:
//!
//! - `identifier` = NFKC + case-fold + strip-diacritics
//! - `display_safe` = strip-controls + NFC + collapse-whitespace
//! - `search_key` = identifier + canonicalise-punctuation +
//!   collapse-whitespace + trim
//! - `punctuation_canonical` = canonicalise-punctuation (single
//!   pass)
//!
//! Bench measures each pipeline alongside each primitive so a
//! caller can see which stage dominates which pipeline. Input
//! is realistic mixed text: accented Latin, smart quotes, em
//! dashes, case variation, whitespace runs.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// Doc references to NFKC / NFC / SIMD get flagged as missing
// backticks; the bench header table wants to read cleanly.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_normalize::{
    canonicalize_punctuation, collapse_whitespace, display_safe, identifier, punctuation_canonical,
    search_key, strip_controls, trim,
};

/// A single realistic-text chunk we tile to build each input
/// size. Contains: mixed case, accented Latin, curly quotes,
/// em/en dashes, ellipsis, non-breaking space, multiple spaces,
/// stray tabs. Roughly 200 bytes; every bench input is a
/// repeated tile of this.
const TILE: &str = "Café Résumé v10 \u{2014} \u{201C}Naïve\u{201D} report\u{2026} \
                    See section 3\u{2013}5 for details. \tWords have  double  \
                    spaces \u{00A0}sometimes. NEXT PARAGRAPH.\n\n";

const INPUT_LENGTHS: &[usize] = &[256, 2048, 16384];

fn tiled_input(target_len: usize) -> String {
    let mut out = String::with_capacity(target_len + TILE.len());
    while out.len() < target_len {
        out.push_str(TILE);
    }
    out.truncate(target_len);
    // Truncation may leave a partial UTF-8 sequence — walk back
    // to a valid char boundary.
    while !out.is_char_boundary(out.len()) {
        out.pop();
    }
    out
}

// ---------------------------------------------------------------------
// Named pipelines.
// ---------------------------------------------------------------------

fn bench_pipelines(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize/pipelines");
    for &len in INPUT_LENGTHS {
        let text = tiled_input(len);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(BenchmarkId::new("identifier", len), &text, |b, t| {
            b.iter(|| black_box(identifier(black_box(t))));
        });

        group.bench_with_input(BenchmarkId::new("display_safe", len), &text, |b, t| {
            b.iter(|| black_box(display_safe(black_box(t))));
        });

        group.bench_with_input(BenchmarkId::new("search_key", len), &text, |b, t| {
            b.iter(|| black_box(search_key(black_box(t))));
        });

        group.bench_with_input(
            BenchmarkId::new("punctuation_canonical", len),
            &text,
            |b, t| {
                b.iter(|| black_box(punctuation_canonical(black_box(t))));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Individual primitives — the building blocks each pipeline
// composes. Comparison reveals which primitive dominates its
// containing pipeline.
// ---------------------------------------------------------------------

fn bench_primitives(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize/primitives");
    for &len in INPUT_LENGTHS {
        let text = tiled_input(len);
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("collapse_whitespace", len),
            &text,
            |b, t| {
                b.iter(|| black_box(collapse_whitespace(black_box(t))));
            },
        );

        group.bench_with_input(BenchmarkId::new("trim", len), &text, |b, t| {
            b.iter(|| black_box(trim(black_box(t))));
        });

        group.bench_with_input(BenchmarkId::new("strip_controls", len), &text, |b, t| {
            b.iter(|| black_box(strip_controls(black_box(t))));
        });

        group.bench_with_input(
            BenchmarkId::new("canonicalize_punctuation", len),
            &text,
            |b, t| {
                b.iter(|| black_box(canonicalize_punctuation(black_box(t))));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_pipelines, bench_primitives);
criterion_main!(benches);
