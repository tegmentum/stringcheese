//! Textsplit — RecursiveSplitter, ParagraphSplitter, SentenceSplitter.
//!
//! The three text-oriented chunkers a RAG pipeline reaches for.
//! Bench measures throughput at three input sizes on synthetic
//! prose (paragraphs of variable-length sentences), varying the
//! chunk size to catch the split point where recursion depth
//! starts dominating.
//!
//! ## Expected shape
//!
//! - **RecursiveSplitter** — dominant cost is one pass per input
//!   char through the separator recursion + one pass for the
//!   greedy merge. Roughly linear in input size at a fixed
//!   chunk_size, with a small chunk_size penalty for the extra
//!   merge iterations.
//! - **ParagraphSplitter** — cheapest of the three when
//!   paragraphs fit under chunk_size (one `\n\n` scan, no
//!   recursion). Falls through to RecursiveSplitter for
//!   oversized paragraphs, so worst case is
//!   ParagraphSplitter's own scan PLUS RecursiveSplitter's cost.
//! - **SentenceSplitter** — dominated by the stringcheese-segment
//!   sentence-boundary detection (naive `. `/`? `/`! ` scan
//!   without the `sentences-icu` feature).
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// Splitter names (RecursiveSplitter / ParagraphSplitter /
// SentenceSplitter) show up in the bench header table; clippy's
// doc_markdown flags each — adding backticks harms the table's
// legibility.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::Rng;
use stringcheese_textsplit::{
    ParagraphSplitter, RecursiveSplitter, SentenceSplitter, TextSplitter,
};

const INPUT_LENGTHS: &[usize] = &[1024, 8192, 32768];
const CHUNK_SIZES: &[usize] = &[200, 1000];

/// Synthetic prose — paragraphs of variable-length sentences.
/// Deterministic given the seed. Roughly `target_len` bytes.
fn prose(target_len: usize, seed: u64) -> String {
    let mut rng = Rng::from_seed(seed);
    let mut out = String::with_capacity(target_len + 128);
    while out.len() < target_len {
        // A paragraph: 2-5 sentences.
        let sentence_count = 2 + (rng.next_u64() % 4) as usize;
        for _ in 0..sentence_count {
            let word_count = 5 + (rng.next_u64() % 12) as usize;
            for _ in 0..word_count {
                let word_len = 3 + (rng.next_u64() % 8) as usize;
                for _ in 0..word_len {
                    let b = (rng.next_u64() % 26) as u8 + b'a';
                    out.push(b as char);
                }
                out.push(' ');
            }
            // Sentence terminator.
            out.pop(); // drop trailing space
            out.push('.');
            out.push(' ');
        }
        out.pop(); // drop trailing space
        // Paragraph separator.
        out.push('\n');
        out.push('\n');
    }
    out.truncate(target_len);
    out
}

// ---------------------------------------------------------------------
// Recursive splitter — the full separator-list recursion + merge.
// ---------------------------------------------------------------------

fn bench_recursive(c: &mut Criterion) {
    let mut group = c.benchmark_group("textsplit/recursive");
    for &chunk_size in CHUNK_SIZES {
        let splitter = RecursiveSplitter::new(chunk_size, 0);
        for &input_len in INPUT_LENGTHS {
            let text = prose(input_len, 0x1234 ^ input_len as u64);
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("chunk_{chunk_size}"), input_len),
                &text,
                |bencher, text| {
                    bencher.iter(|| black_box(splitter.split(black_box(text))));
                },
            );
        }
    }
    group.finish();
}

/// Recursive with overlap — measures the overlap-prepend cost.
fn bench_recursive_with_overlap(c: &mut Criterion) {
    let mut group = c.benchmark_group("textsplit/recursive_overlap");
    let splitter = RecursiveSplitter::new(1000, 200);
    for &input_len in INPUT_LENGTHS {
        let text = prose(input_len, 0x5678 ^ input_len as u64);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(input_len),
            &text,
            |bencher, text| {
                bencher.iter(|| black_box(splitter.split(black_box(text))));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Paragraph splitter — should be cheapest when paragraphs fit.
// ---------------------------------------------------------------------

fn bench_paragraph(c: &mut Criterion) {
    let mut group = c.benchmark_group("textsplit/paragraph");
    let splitter = ParagraphSplitter::new(1000, 0);
    for &input_len in INPUT_LENGTHS {
        let text = prose(input_len, 0x9ABC ^ input_len as u64);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(input_len),
            &text,
            |bencher, text| {
                bencher.iter(|| black_box(splitter.split(black_box(text))));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Sentence splitter — dominated by segment-crate boundary scan.
// ---------------------------------------------------------------------

fn bench_sentence(c: &mut Criterion) {
    let mut group = c.benchmark_group("textsplit/sentence");
    let splitter = SentenceSplitter::new(1000);
    for &input_len in INPUT_LENGTHS {
        let text = prose(input_len, 0xDEF0 ^ input_len as u64);
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(input_len),
            &text,
            |bencher, text| {
                bencher.iter(|| black_box(splitter.split(black_box(text))));
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_recursive,
    bench_recursive_with_overlap,
    bench_paragraph,
    bench_sentence,
);
criterion_main!(benches);
