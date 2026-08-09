//! Sketch construction and comparison — the fingerprint trio.
//!
//! MinHash, SimHash, and winnowing all share a natural upstream:
//! stringcheese-ngram produces the grams / hashed grams they
//! consume. This bench measures the full pipeline (grams → sketch
//! → compare) at three input sizes so a regression in any layer
//! surfaces as a per-corpus throughput change.
//!
//! Wall-clock only; no allocation tracking. The sketches use
//! seeded RNG state and stringcheese-bench's deterministic input
//! helpers, so criterion's sample-to-sample variance stays
//! dominated by the machine rather than by input entropy.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// `hashes` / `hasher` are the natural bench-local variable names;
// the collision is superficial and renaming to `hs` / `hshr` would
// harm readability. `MinHash` / `SimHash` are proper nouns that
// docs::doc_markdown flags — wrapping them in backticks looks
// worse than leaving them.
#![allow(clippy::similar_names, clippy::doc_markdown)]

use core::hash::{BuildHasher, BuildHasherDefault};
use std::collections::hash_map::DefaultHasher;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::random_ascii;
use stringcheese_minhash::Sketcher as MinHashSketcher;
use stringcheese_simhash::Sketcher as SimHashSketcher;
use stringcheese_winnowing::Winnower;

const LENGTHS: &[usize] = &[128, 1024, 8192];

fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn text_for(len: usize, salt: u64) -> String {
    let bytes = random_ascii(len, seed_for(len, salt));
    String::from_utf8(bytes).expect("random_ascii returns valid UTF-8")
}

// ---------------------------------------------------------------------
// MinHash — sketch build + Jaccard estimate.
// ---------------------------------------------------------------------

fn bench_minhash_sketch(c: &mut Criterion) {
    let mut group = c.benchmark_group("sketches/minhash/sketch");
    let sketcher = MinHashSketcher::new(128);
    for &len in LENGTHS {
        let text = text_for(len, 0xA1);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &text, |bencher, text| {
            bencher.iter(|| {
                let grams = stringcheese_ngram::char_ngrams(black_box(text), 5);
                black_box(sketcher.sketch(grams));
            });
        });
    }
    group.finish();
}

fn bench_minhash_jaccard(c: &mut Criterion) {
    let mut group = c.benchmark_group("sketches/minhash/jaccard");
    let sketcher = MinHashSketcher::new(128);
    for &len in LENGTHS {
        let a = text_for(len, 0xA2);
        let b = text_for(len, 0xA3);
        let sa = sketcher.sketch(stringcheese_ngram::char_ngrams(&a, 5));
        let sb = sketcher.sketch(stringcheese_ngram::char_ngrams(&b, 5));
        group.bench_with_input(
            BenchmarkId::from_parameter(len),
            &(sa, sb),
            |bencher, (sa, sb)| {
                bencher.iter(|| black_box(sa.jaccard(black_box(sb))));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------
// SimHash — sketch build + Hamming similarity.
// ---------------------------------------------------------------------

fn bench_simhash_sketch(c: &mut Criterion) {
    let mut group = c.benchmark_group("sketches/simhash/sketch64");
    for &len in LENGTHS {
        let text = text_for(len, 0xB1);
        group.throughput(Throughput::Bytes(len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(len), &text, |bencher, text| {
            bencher.iter(|| {
                let grams = stringcheese_ngram::char_ngrams(black_box(text), 5);
                black_box(SimHashSketcher::new().add_all(grams).finalize_64());
            });
        });
    }
    group.finish();
}

fn bench_simhash_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("sketches/simhash/similarity");
    for &len in LENGTHS {
        let a = text_for(len, 0xB2);
        let b = text_for(len, 0xB3);
        let sa = SimHashSketcher::new()
            .add_all(stringcheese_ngram::char_ngrams(&a, 5))
            .finalize_64();
        let sb = SimHashSketcher::new()
            .add_all(stringcheese_ngram::char_ngrams(&b, 5))
            .finalize_64();
        group.bench_with_input(
            BenchmarkId::from_parameter(len),
            &(sa, sb),
            |bencher, (sa, sb)| {
                bencher.iter(|| black_box(sa.similarity(black_box(sb))));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Winnowing — sliding-minimum throughput on a hashed gram stream.
// ---------------------------------------------------------------------

fn bench_winnowing(c: &mut Criterion) {
    let mut group = c.benchmark_group("sketches/winnowing/select");
    let hasher = BuildHasherDefault::<DefaultHasher>::default();
    let window = 8usize;
    let winnower = Winnower::new(window);

    for &len in LENGTHS {
        let text = text_for(len, 0xC1);
        // Pre-hash so the bench measures winnowing throughput,
        // not the hash function. `+&text` avoids borrow-vs-move
        // ambiguity in the closure below.
        let hashes: Vec<u64> = stringcheese_ngram::char_ngrams(&text, 5)
            .map(|g| hasher.hash_one(g))
            .collect();
        group.throughput(Throughput::Elements(hashes.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(len),
            &hashes,
            |bencher, hashes| {
                bencher.iter(|| {
                    let count = winnower.select(hashes.iter().copied()).count();
                    black_box(count);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_minhash_sketch,
    bench_minhash_jaccard,
    bench_simhash_sketch,
    bench_simhash_similarity,
    bench_winnowing,
);
criterion_main!(benches);
