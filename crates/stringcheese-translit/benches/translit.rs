//! Baseline throughput benchmarks for script-to-script
//! transliteration in `stringcheese-translit`.
//!
//! Mirrors the shape of the sibling bench files
//! (`stringcheese-compare` / `stringcheese-manip` /
//! `stringcheese-textsplit`): one bench file, criterion-owned `main`
//! via `harness = false`, groups named `<impl>/<script>/<len>`.
//! Regression trip-wire + last-measured baseline table for the one
//! general-purpose transliterator this crate currently ships.
//!
//! # Groups
//!
//! One group per shipped [`Transliterator`] implementation. The
//! crate seed-ships one — `DeunicodeTransliterator` — and a
//! `TableTransliterator` template for follow-up per-script tables.
//! The bench covers the deunicode arm because it's the one
//! callers reach for today; the table arm has no built-in table
//! wide enough to be interesting to sweep (the ISO 9 Cyrillic →
//! Latin table only covers Cyrillic letters, so the bench would
//! measure passthrough for every other flavor).
//!
//! # Scripts
//!
//! Three per size — each stresses a different `deunicode` code
//! path:
//!
//! * `latin_diacritics` — Latin-alphabet prose with accents
//!   (French / Spanish flavor). Every base letter is ASCII;
//!   accented scalars are the only ones that hit deunicode's
//!   substitution table. Fast path.
//! * `cyrillic` — Cyrillic prose. Every scalar is non-ASCII;
//!   every scalar hits the substitution table. This is where
//!   deunicode does its actual work — the transliterator
//!   throughput here is what "cost per non-ASCII scalar" looks
//!   like.
//! * `cjk` — Han-script prose. Every scalar is 3 bytes in UTF-8
//!   and every one goes through deunicode's per-scalar romanisation
//!   table. Numbers on this flavor are the slowest per input byte
//!   because the substitution output is typically 2-3 ASCII bytes
//!   per input scalar — output allocation dominates.
//!
//! # Sizes
//!
//! Three input byte lengths per (impl, script): 256 B / 1 KiB /
//! 4 KiB. Matches the shape the sibling benches use for
//! transliteration-adjacent surfaces (`stringcheese-ident`'s
//! `slugify` baseline).
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-translit
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-translit -- cjk
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-translit --no-run
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

use stringcheese_translit::{DeunicodeTransliterator, Transliterator};

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG; every `cargo bench` invocation feeds byte-identical
// inputs so criterion's noise floor stays meaningful across runs.
// ---------------------------------------------------------------------------

/// Latin prose with diacritics — mostly ASCII with sprinkled
/// accented scalars.
const LATIN_SEED: &str =
    "Café résumé façade naïve piñata jalapeño. L'écriture à la française avec accents. ";

/// Cyrillic prose seed — every scalar is non-ASCII (2 bytes).
const CYRILLIC_SEED: &str =
    "Быстрая коричневая лиса прыгает через ленивую собаку. Дождь идёт всю ночь. ";

/// CJK (Han) prose seed — every scalar is 3 bytes; deunicode's
/// per-scalar table hits on every one.
const CJK_SEED: &str = "日本語のテキストを含む長い文字列。中文字符也在这里出现，用于测试音译速度。";

/// Build a string of *at least* `bytes` bytes by cycling `seed`.
fn build_input(seed: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + seed.len());
    while out.len() < bytes {
        out.push_str(seed);
    }
    out
}

/// Byte lengths swept by every (script) cell.
const SIZES: &[usize] = &[256, 1024, 4 * 1024];

/// The three scripts every group runs.
fn scripts() -> [(&'static str, &'static str); 3] {
    [
        ("latin_diacritics", LATIN_SEED),
        ("cyrillic", CYRILLIC_SEED),
        ("cjk", CJK_SEED),
    ]
}

// ---------------------------------------------------------------------------
// Bench group. Throughput reported over *input bytes* — higher is
// better. The output size differs per script (CJK expands ~3× on
// deunicode; Latin-diacritics contracts to 1 byte per non-ASCII
// scalar), which is why "per input byte" is the load-bearing
// number for comparing scripts.
// ---------------------------------------------------------------------------

fn bench_deunicode(c: &mut Criterion) {
    let mut group = c.benchmark_group("translit/deunicode");
    let t = DeunicodeTransliterator::new();
    for (script, seed) in scripts() {
        for &size in SIZES {
            let input = build_input(seed, size);
            group.throughput(Throughput::Bytes(input.len() as u64));
            let id = BenchmarkId::new(script, input.len());
            group.bench_with_input(id, &input, |bencher, input| {
                bencher.iter(|| black_box(t.transliterate(black_box(input))));
            });
        }
    }
    group.finish();
}

criterion_group!(translit, bench_deunicode);
criterion_main!(translit);
