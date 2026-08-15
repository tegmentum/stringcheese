//! Throughput benchmarks for the four [`CaseEngine`] operations
//! (`to_upper` / `to_lower` / `to_title` / `fold`).
//!
//! Mirrors the shape of `stringcheese-normalize/benches/normalize.rs`:
//! one bench file, criterion-owned `main` via `harness = false`,
//! groups named `icu-case/<op>/<locale>/<flavor>/<len>`. The primary
//! purpose is to demonstrate the ASCII fast-path win (`to_upper` /
//! `to_lower` / `to_title` / `fold`) for non-Turkic locales; the
//! Latin-1 and mixed flavors provide a slow-path baseline.
//!
//! # Groups
//!
//! Four groups — one per shipped operation:
//!
//! * `to_upper` — locale-sensitive uppercasing.
//! * `to_lower` — locale-sensitive lowercasing.
//! * `to_title` — `TitleBoundary::Words` + `lowercase_tail: true`
//!   (the default). The Phase 1 word-boundary rule; measures the
//!   heavier "walk chars + track boundary" cost.
//! * `fold` — `FoldMode::Full`. Locale-independent case fold used
//!   for case-insensitive matching.
//!
//! # Locales
//!
//! Each group runs against five locales that cover the interesting
//! shapes:
//!
//! * `en` / `de` / `fr` / `ru` / `zh` — non-Turkic; the ASCII
//!   fast-path fires and short-circuits the pack walk.
//! * `tr` — Turkish; the ASCII fast-path is DENY-LISTED because the
//!   Turkish pack ships context rules that remap ASCII `I` / `i`.
//!   Numbers on this locale reveal the slow-path baseline the
//!   fast-path avoids.
//!
//! (Fold is locale-independent — its group is run once per flavor,
//! not per locale.)
//!
//! # Sizes
//!
//! Three input byte lengths per (op, locale, flavor): 256 / 1024 /
//! 16384. Small covers per-call overhead; large is where the fast
//! path's SIMD `is_ascii` scan + `to_ascii_*` copy pay off.
//!
//! # Flavors
//!
//! Three per size:
//!
//! * `ascii` — pure ASCII prose. Fast path fires for all non-Turkic
//!   locales; Turkish still walks the slow path.
//! * `latin1` — Latin-1 supplement (accented Latin; `Café`,
//!   `résumé`, German `ß`). Fast path never fires (fails
//!   `is_ascii`); every scalar walks the pack lookup.
//! * `mixed` — mixed scripts (Latin + Greek + Cyrillic + CJK).
//!   Same slow-path characteristics as `latin1` but stresses the
//!   fallback chain more.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-icu-case
//! ```
//!
//! Compile-only smoke check (used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-icu-case --no-run
//! ```

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]
// `ASCII` / `NFC` and friends trip `doc_markdown` when the docs are
// human-facing prose rather than API references. Match the workspace
// bench-file convention of a blanket allow at file scope.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_icu_case::{CaseEngine, CasePack, FoldMode, TitleOptions};
use stringcheese_scud::{
    CAP_CASE, CaseSectionBuilder, ContextKind, SECT_CONTEXT, SECT_FULL_FOLD, SECT_FULL_UPPER,
    SECT_SIMPLE_FOLD, SECT_SIMPLE_LOWER, SECT_SIMPLE_UPPER, ScudWriter,
};

// ---------------------------------------------------------------------------
// Deterministic input construction. No RNG; every `cargo bench`
// invocation feeds byte-identical inputs so criterion's noise floor
// stays meaningful across runs. Seeds mirror the sibling
// `stringcheese-normalize/benches/normalize.rs` layout.
// ---------------------------------------------------------------------------

/// Pure-ASCII prose seed. Every scalar is one byte; the fast-path
/// scan runs `is_ascii` in one SIMD pass and dispatches to
/// `to_ascii_*` for the whole string.
const ASCII_SEED: &str =
    "The quick brown fox jumps over the lazy dog. Every good boy deserves fudge.  ";

/// Latin-1 seed. Accented Latin + German ß + smart quotes. Every
/// operation walks the pack lookup for every scalar; `is_ascii`
/// fails on the first non-ASCII byte.
const LATIN1_SEED: &str =
    "Café résumé façade naïve piñata Zürich Kraków jalapeño coöperate straße  ";

/// Mixed-script seed. Latin + Greek + Cyrillic + CJK to stress the
/// pack-walk and the `char::to_upper` / `char::to_lower` fallback.
const MIXED_SEED: &str = "Hello Καλημέρα Привет 日本語 مرحبا Straße résumé façade Zürich  ";

/// Build a string of *at least* `bytes` bytes by cycling `seed`.
fn build_input(seed: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + seed.len());
    while out.len() < bytes {
        out.push_str(seed);
    }
    out
}

/// Byte lengths swept by every cell.
const SIZES: &[usize] = &[256, 1024, 16 * 1024];

/// The three flavors every group runs.
fn flavors() -> [(&'static str, &'static str); 3] {
    [
        ("ascii", ASCII_SEED),
        ("latin1", LATIN1_SEED),
        ("mixed", MIXED_SEED),
    ]
}

/// Locales the per-locale bench groups walk. `en` / `de` / `fr` /
/// `ru` / `zh` all fast-path on ASCII; `tr` is deny-listed and
/// exposes the slow-path baseline.
const LOCALES: &[&str] = &["en", "de", "fr", "ru", "zh", "tr"];

// ---------------------------------------------------------------------------
// SCUD pack construction. We build the two test packs (English +
// Turkish) once per bench run and reuse them across every group.
// Mirrors the pack shape the crate's inline unit tests use — the
// only load-bearing bit for the bench is that the Turkish pack ships
// the ASCII-touching context rules so `to_upper("i", "tr")` follows
// the slow path.
// ---------------------------------------------------------------------------

fn build_test_en() -> Vec<u8> {
    let mut c = CaseSectionBuilder::new();
    for ch in 'a'..='z' {
        let up = ch.to_ascii_uppercase();
        c.push_simple_lower(up as u32, ch as u32);
        c.push_simple_upper(ch as u32, up as u32);
        c.push_simple_fold(up as u32, ch as u32);
    }
    // German ß — full uppercase to "SS", full fold to "ss".
    c.push_full_upper(0x00DF, &[0x0053, 0x0053]);
    c.push_full_fold(0x00DF, &[0x0073, 0x0073]);
    // Latin capital I with dot above (İ) → i + combining dot above
    // under English fold (default Unicode behaviour).
    c.push_full_fold(0x0130, &[0x0069, 0x0307]);

    let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("en"));
    w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
    w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
    w.append_section(SECT_SIMPLE_FOLD, &c.simple_fold_bytes());
    w.append_section(SECT_FULL_UPPER, &c.full_upper_bytes());
    w.append_section(SECT_FULL_FOLD, &c.full_fold_bytes());
    w.finish()
}

fn build_test_tr() -> Vec<u8> {
    let mut c = CaseSectionBuilder::new();
    // Turkish dotted/dotless-I contextual mappings.
    c.push_context('I' as u32, ContextKind::LocaleOverrideLower, 0x0131);
    c.push_context('i' as u32, ContextKind::LocaleOverrideUpper, 0x0130);
    c.push_simple_lower(0x0130, 0x0069);
    c.push_simple_upper(0x0131, 0x0049);

    let mut w = ScudWriter::new(CAP_CASE, "44.1", Some("tr"));
    w.append_section(SECT_CONTEXT, &c.context_bytes());
    w.append_section(SECT_SIMPLE_LOWER, &c.simple_lower_bytes());
    w.append_section(SECT_SIMPLE_UPPER, &c.simple_upper_bytes());
    w.finish()
}

// ---------------------------------------------------------------------------
// Bench groups.
// ---------------------------------------------------------------------------

fn bench_per_locale(
    c: &mut Criterion,
    group_name: &str,
    op: fn(&CaseEngine<'_>, &str, &str) -> String,
) {
    let en_bytes = build_test_en();
    let tr_bytes = build_test_tr();
    let en_pack = CasePack::from_scud_bytes(&en_bytes).unwrap();
    let tr_pack = CasePack::from_scud_bytes(&tr_bytes).unwrap();
    let engine = CaseEngine::new(vec![en_pack, tr_pack]);

    let mut group = c.benchmark_group(format!("icu-case/{group_name}"));
    for &locale in LOCALES {
        for (flavor, seed) in flavors() {
            for &size in SIZES {
                let input = build_input(seed, size);
                group.throughput(Throughput::Bytes(input.len() as u64));
                let id = BenchmarkId::new(format!("{locale}/{flavor}"), input.len());
                group.bench_with_input(id, &input, |bencher, input| {
                    bencher.iter(|| black_box(op(&engine, black_box(input), black_box(locale))));
                });
            }
        }
    }
    group.finish();
}

fn bench_to_upper(c: &mut Criterion) {
    bench_per_locale(c, "to_upper", |engine, input, locale| {
        engine.to_upper(input, locale)
    });
}

fn bench_to_lower(c: &mut Criterion) {
    bench_per_locale(c, "to_lower", |engine, input, locale| {
        engine.to_lower(input, locale)
    });
}

fn bench_to_title(c: &mut Criterion) {
    bench_per_locale(c, "to_title", |engine, input, locale| {
        engine
            .to_title(input, locale, TitleOptions::default())
            .unwrap()
    });
}

/// Fold takes no locale — sweep FoldMode variants instead. `Simple`
/// and `Full` fast-path on ASCII; `FullTurkic` goes through the slow
/// path (it remaps ASCII `I` → `ı` unconditionally).
fn bench_fold(c: &mut Criterion) {
    let en_bytes = build_test_en();
    let tr_bytes = build_test_tr();
    let en_pack = CasePack::from_scud_bytes(&en_bytes).unwrap();
    let tr_pack = CasePack::from_scud_bytes(&tr_bytes).unwrap();
    let engine = CaseEngine::new(vec![en_pack, tr_pack]);

    let mut group = c.benchmark_group("icu-case/fold");
    for (mode_name, mode) in [
        ("simple", FoldMode::Simple),
        ("full", FoldMode::Full),
        ("full_turkic", FoldMode::FullTurkic),
    ] {
        for (flavor, seed) in flavors() {
            for &size in SIZES {
                let input = build_input(seed, size);
                group.throughput(Throughput::Bytes(input.len() as u64));
                let id = BenchmarkId::new(format!("{mode_name}/{flavor}"), input.len());
                group.bench_with_input(id, &input, |bencher, input| {
                    bencher.iter(|| black_box(engine.fold(black_box(input), mode)));
                });
            }
        }
    }
    group.finish();
}

criterion_group!(
    icu_case,
    bench_to_upper,
    bench_to_lower,
    bench_to_title,
    bench_fold,
);
criterion_main!(icu_case);
