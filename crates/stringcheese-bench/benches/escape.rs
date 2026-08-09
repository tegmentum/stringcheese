//! Escape — URI / HTML / JSON / shell side-by-side.
//!
//! Four escape targets under one Escape enum; three wrap crates
//! (percent-encoding, html-escape, shlex), one in-house (JSON
//! string body). This bench measures encode + unescape
//! throughput across all four at three input sizes, on two
//! input shapes:
//!
//! - **Plain ASCII** — very few characters need escaping.
//!   Measures the "walk the input, decide nothing changes"
//!   fast path.
//! - **Metachar-heavy** — every 10th byte is a metachar for the
//!   target grammar (quotes for HTML/JSON, `<`/`>`/`&` for HTML,
//!   `;`/`|`/`$` for shell, spaces + `?`/`&` for URI).
//!   Measures the actual escape work.
//!
//! ## Why this bench matters
//!
//! `crates/stringcheese-escape/src/json.rs` implements the JSON
//! string-body grammar in-house rather than wrapping serde_json.
//! The rationale is "avoid pulling serde_json for a string-escape
//! utility." That decision only holds if the in-house
//! implementation is roughly competitive with a wrapped
//! alternative on plain input. This bench captures the number
//! so the tradeoff is legible.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]
// URI / HTML / JSON / POSIX are proper-noun-ish names that
// clippy's doc_markdown flags; wrapping them harms readability
// of a bench file whose whole purpose is the escape comparison.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use stringcheese_bench::inputs::random_ascii;
use stringcheese_escape::{Escape, escape, unescape};

const LENGTHS: &[usize] = &[128, 1024, 8192];

fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

/// Plain ASCII lowercase letters — nothing needs escaping in
/// any target grammar. Measures the "walk + decide nothing
/// changes" fast path.
fn plain_text(len: usize, salt: u64) -> String {
    let bytes = random_ascii(len, seed_for(len, salt));
    // Filter down to just lowercase letters for a fully-safe input.
    bytes
        .iter()
        .map(|&b| {
            let x = b.to_ascii_lowercase();
            if x.is_ascii_lowercase() {
                x as char
            } else {
                'a'
            }
        })
        .collect()
}

/// Metachar-heavy input — sprinkle target grammar metachars
/// throughout so the escape work actually happens per byte.
/// Uses a single "hard" set (quotes, angle brackets, backslashes,
/// ampersands, semicolons, spaces) that hits at least one metachar
/// in every target grammar.
fn metachar_text(len: usize, salt: u64) -> String {
    let base = plain_text(len, salt);
    let hard = b"\"<>&; \\'";
    let mut out = String::with_capacity(len);
    for (i, c) in base.chars().enumerate() {
        if i % 10 == 0 {
            out.push(hard[i / 10 % hard.len()] as char);
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------
// Encode — the hot path for outbound rendering.
// ---------------------------------------------------------------------

fn bench_encode_plain(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape/encode_plain");
    for &len in LENGTHS {
        let text = plain_text(len, 0xF1);
        group.throughput(Throughput::Bytes(len as u64));
        for target in [
            Escape::UriComponent,
            Escape::Html,
            Escape::JsonString,
            Escape::ShellWord,
        ] {
            group.bench_with_input(
                BenchmarkId::new(format!("{target:?}"), len),
                &(target, &text),
                |bencher, (target, text)| {
                    bencher.iter(|| black_box(escape(black_box(text), *target)));
                },
            );
        }
    }
    group.finish();
}

fn bench_encode_metachar_heavy(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape/encode_metachar");
    for &len in LENGTHS {
        let text = metachar_text(len, 0xF2);
        group.throughput(Throughput::Bytes(len as u64));
        for target in [
            Escape::UriComponent,
            Escape::Html,
            Escape::JsonString,
            Escape::ShellWord,
        ] {
            group.bench_with_input(
                BenchmarkId::new(format!("{target:?}"), len),
                &(target, &text),
                |bencher, (target, text)| {
                    bencher.iter(|| black_box(escape(black_box(text), *target)));
                },
            );
        }
    }
    group.finish();
}

// ---------------------------------------------------------------------
// Round-trip — encode then decode. Only for targets that
// support unescape (all four do).
// ---------------------------------------------------------------------

fn bench_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape/round_trip");
    for &len in LENGTHS {
        let text = metachar_text(len, 0xF3);
        group.throughput(Throughput::Bytes(len as u64));
        for target in [
            Escape::UriComponent,
            Escape::Html,
            Escape::JsonString,
            Escape::ShellWord,
        ] {
            group.bench_with_input(
                BenchmarkId::new(format!("{target:?}"), len),
                &(target, &text),
                |bencher, (target, text)| {
                    bencher.iter(|| {
                        let encoded = escape(black_box(text), *target);
                        black_box(unescape(&encoded, *target).ok());
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_encode_plain,
    bench_encode_metachar_heavy,
    bench_round_trip,
);
criterion_main!(benches);
