//! Encode-throughput bench for `stringcheese-tokenizer-hf` on the
//! real `cl100k_base` vocab.
//!
//! Only compiled with `--features parity-real-vocab` (see the
//! `required-features` on the bench declaration in `Cargo.toml`).
//! Measures the two engines side-by-side on a fixed 8 KiB input so
//! the design doc's ~1-3 M tok/s tiktoken bar can be tracked
//! locally without opting the main workspace bench matrix into a
//! network fetch.
//!
//! Run with:
//!
//! ```text
//! cargo bench --manifest-path \
//!     crates/stringcheese-tokenizer-tiktoken-conformance/Cargo.toml \
//!     --features parity-real-vocab
//! ```

#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_tiktoken_conformance::{fetch, parity, variant};

fn deterministic_prose(bytes: usize) -> String {
    let words: &[&str] = &[
        "the", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog", "and", "then",
        "runs", "back", "through", "the", "forest", "toward", "the", "little", "cabin", "on",
        "the", "hill", "where", "smoke", "curls", "from", "the", "chimney", "into", "the", "cold",
        "morning", "air",
    ];
    // Deterministic wrapping — no RNG, so successive runs produce a
    // byte-identical input.
    let mut out = String::with_capacity(bytes + 64);
    let mut idx = 0usize;
    while out.len() < bytes {
        out.push_str(words[idx % words.len()]);
        out.push(' ');
        idx += 1;
    }
    out
}

fn bench_encode_throughput(c: &mut Criterion) {
    // Load once, reuse across the entire bench group.
    let plaintext =
        fetch::load_variant(&variant::CL100K_BASE).expect("cl100k_base blob must load for bench");
    let tok = parity::build_stringcheese_tokenizer(variant::CL100K_BASE.name, &plaintext)
        .expect("cl100k_base tokenizer must build for bench");
    let oracle =
        parity::load_oracle(&variant::CL100K_BASE).expect("cl100k_base oracle must load for bench");

    let input = deterministic_prose(8 * 1024);

    let mut group = c.benchmark_group("tiktoken_parity/cl100k_base");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.bench_function("stringcheese_encode", |b| {
        b.iter(|| {
            let enc = tok.encode(black_box(&input)).unwrap();
            black_box(enc);
        });
    });
    group.bench_function("tiktoken_rs_encode_ordinary", |b| {
        b.iter(|| {
            let ids = oracle.encode_ordinary(black_box(&input));
            black_box(ids);
        });
    });
    group.finish();
}

criterion_group!(tiktoken_parity, bench_encode_throughput);
criterion_main!(tiktoken_parity);
