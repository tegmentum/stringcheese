//! Baseline throughput benchmarks for the three shipped text
//! splitters in `stringcheese-textsplit`.
//!
//! Mirrors the shape of the sibling bench files (`stringcheese-compare`
//! / `stringcheese-manip` / `stringcheese-pattern` /
//! `stringcheese-stats`): one bench file, criterion-owned `main` via
//! `harness = false`, groups named `<splitter>/<flavor>/<len>`.
//! Regression trip-wire + last-measured baseline table for the
//! LLM-oriented splitter family.
//!
//! # Splitters
//!
//! Three groups, one per shipped [`TextSplitter`] implementation:
//!
//! * `recursive` — [`RecursiveSplitter`], the classic LangChain-style
//!   splitter. Configured with `chunk_size = 1000, overlap = 0` so
//!   the bench measures the greedy-merge cost without the extra
//!   ~15 % overlap-tail cost.
//! * `paragraph` — [`ParagraphSplitter`], one chunk per
//!   double-newline-delimited paragraph unless a paragraph exceeds
//!   the size limit (in which case it falls through to the recursive
//!   splitter). Same `chunk_size = 1000` for cross-group parity.
//! * `sentence` — [`SentenceSplitter`], collects whole sentences
//!   until the size threshold is reached. Uses
//!   [`stringcheese_segment`] for sentence-boundary detection —
//!   without the `sentences-icu` feature enabled at build time the
//!   detection is the naive `. `/`? `/`! `/`\n\n` fallback, which is
//!   what this bench exercises.
//!
//! # Sizes
//!
//! Three input byte lengths per (splitter, flavor): 1 KiB / 10 KiB /
//! 100 KiB. Chosen to match the shape the crate's baseline table
//! already documents (1 KB / 8 KB / 32 KB), one order of magnitude
//! wider at the top end so the 100 KiB row exercises the full
//! chunk-emission steady state.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `prose` — deterministic English prose with paragraph breaks
//!   every ~500 bytes and sentence-ending punctuation sprinkled
//!   through. This is the shape all three splitters were designed
//!   for; the paragraph splitter emits ~one chunk per `\n\n` block,
//!   the sentence splitter groups sentences up to the size cap, and
//!   the recursive splitter walks the separator list down to `. `
//!   before falling into the space-split arm.
//! * `code` — a code-ish flavor with fewer paragraphs and no
//!   sentence-ending punctuation (curly braces, semicolons, and
//!   line-only breaks). This forces the paragraph splitter's
//!   fall-through-to-recursive path and forces the sentence
//!   splitter into its degenerate "one giant sentence" regime
//!   which then triggers over-budget chunk emission. Numbers on
//!   this flavor are typically ~30-50 % worse per byte because the
//!   fast paths no longer fire.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-textsplit
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-textsplit -- recursive
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-textsplit --no-run
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

use stringcheese_textsplit::{
    ParagraphSplitter, RecursiveSplitter, SentenceSplitter, TextSplitter,
};

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG, no time-based seed; every `cargo bench` invocation
// feeds byte-identical inputs so criterion's noise floor stays
// meaningful across runs.
// ---------------------------------------------------------------------------

/// One prose paragraph, cycled to build inputs of arbitrary size. Uses
/// `. ` between clauses so the sentence splitter has real work to do
/// (its fallback detection is regex-free and looks for the literal
/// `. `/`? `/`! ` marker).
const PROSE_PARAGRAPH: &str = "\
The quick brown fox jumps over the lazy dog. \
The mountain wind carried a scent of pine and snow through the empty valley. \
Every morning the villagers gathered at the well to draw water. \
Some spoke of the road that led north; most did not. \
There was work to do before the frost set in, and none of it would wait for tales. \
The blacksmith's hammer rang against iron long after the sun had set. \
A single lantern moved along the ridge as the last traveler descended.\n\n";

/// One code-ish paragraph — no sentence-ending punctuation, few
/// paragraph breaks, plenty of line breaks. Forces both the paragraph
/// and sentence splitters off their fast paths.
const CODE_PARAGRAPH: &str = "\
fn compute(x: usize, y: usize) -> usize {\n    \
    let mut acc = 0usize\n    \
    for i in 0..x {\n        \
        for j in 0..y {\n            \
            acc = acc.wrapping_add(i.wrapping_mul(j))\n        \
        }\n    \
    }\n    \
    acc\n\
}\n";

/// Build a prose input of *at least* `bytes` bytes. All bytes are
/// ASCII so the byte / scalar / grapheme counts coincide.
fn prose_input(bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + 64);
    while out.len() < bytes {
        out.push_str(PROSE_PARAGRAPH);
    }
    out
}

/// Build a code-flavor input of at least `bytes` bytes. Same
/// all-ASCII structure as prose — the difference is *shape*, not
/// scalar width.
fn code_input(bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + 64);
    while out.len() < bytes {
        out.push_str(CODE_PARAGRAPH);
    }
    out
}

/// Byte lengths swept by every (splitter, flavor) cell.
const SIZES: &[usize] = &[1024, 10 * 1024, 100 * 1024];

/// Builder function signature: `(bytes) -> String`. Kept as a
/// dedicated alias so the return type of [`flavors`] does not trip
/// `clippy::type_complexity`.
type InputBuilder = fn(usize) -> String;

/// The two flavors every group runs.
fn flavors() -> [(&'static str, InputBuilder); 2] {
    [("prose", prose_input), ("code", code_input)]
}

/// Target chunk size used by every splitter here. Matches the shape
/// documented in the crate's baseline table so the numbers this bench
/// emits are directly comparable to the earlier
/// `stringcheese-bench`-driven baseline row.
const CHUNK_SIZE: usize = 1000;

// ---------------------------------------------------------------------------
// Splitter groups. Every splitter is constructed once per (flavor,
// size) cell and reused across iterations — construction cost is not
// the measurement target; per-call split cost is. Throughput reported
// over *input bytes* — higher is better.
// ---------------------------------------------------------------------------

fn bench_recursive(c: &mut Criterion) {
    let mut group = c.benchmark_group("textsplit/recursive");
    // `overlap = 0`: measure the greedy-merge cost without the extra
    // ~15 % overlap-tail cost documented in the crate's earlier
    // baseline table. A follow-up round could add an overlap arm.
    let splitter = RecursiveSplitter::new(CHUNK_SIZE, 0);
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |bencher, input| {
                bencher.iter(|| black_box(splitter.split(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_paragraph(c: &mut Criterion) {
    let mut group = c.benchmark_group("textsplit/paragraph");
    let splitter = ParagraphSplitter::new(CHUNK_SIZE, 0);
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |bencher, input| {
                bencher.iter(|| black_box(splitter.split(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_sentence(c: &mut Criterion) {
    let mut group = c.benchmark_group("textsplit/sentence");
    let splitter = SentenceSplitter::new(CHUNK_SIZE);
    for (flavor, build) in flavors() {
        for &n in SIZES {
            let input = build(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, n), &input, |bencher, input| {
                bencher.iter(|| black_box(splitter.split(black_box(input))));
            });
        }
    }
    group.finish();
}

criterion_group!(textsplit, bench_recursive, bench_paragraph, bench_sentence);
criterion_main!(textsplit);
