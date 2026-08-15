//! Baseline throughput benchmarks for the four escape targets
//! in `stringcheese-escape`.
//!
//! Mirrors the shape of the sibling bench files
//! (`stringcheese-compare` / `stringcheese-manip` /
//! `stringcheese-textsplit`): one bench file, criterion-owned `main`
//! via `harness = false`, groups named `<target>/<flavor>/<len>`.
//! Regression trip-wire + last-measured baseline table for the
//! four target grammars this crate ships.
//!
//! # Groups
//!
//! Four groups, one per [`Escape`] variant:
//!
//! * `uri` — [`Escape::UriComponent`], wraps `percent-encoding`.
//!   Every reserved character becomes `%XX`.
//! * `html` — [`Escape::Html`], escapes `& < > " ' /`.
//! * `json` — [`Escape::JsonString`], the in-house escape path
//!   (128-entry ASCII table + bulk-copy passthrough).
//! * `shell` — [`Escape::ShellWord`], wraps `shlex`. Single-quotes
//!   and backslash-escapes embedded single quotes.
//!
//! # Sizes
//!
//! Three input byte lengths per (target, flavor): 256 B / 1 KiB /
//! 4 KiB. Matches the sibling benches; the sizes bracket the
//! range where a single-pass escape amortises its per-call setup
//! cost.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `plain` — ASCII prose (`"the quick brown fox …"`). JSON,
//!   HTML, and shell all take their fast paths here (no
//!   backslash/entity/quote metacharacters in prose). URI is the
//!   exception: `NON_ALPHANUMERIC` treats space (0x20) as needing
//!   `%20`, so the prose seed's ~18 % spaces break URI's
//!   passthrough path on every space — the URI `plain` cell
//!   measures a partial-escape workload, not the passthrough hot
//!   path. See the crate-level `//!` docs for the resulting
//!   ~4× gap between URI `plain` and the earlier
//!   all-alphanumeric baseline.
//! * `meta` — dense with characters each target has to escape.
//!   URI and JSON pay per-scalar substitution cost; HTML pays
//!   per-entity substitution; shell pays for every embedded
//!   quote. Numbers here are what a "worst-case" real input
//!   looks like.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-escape
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-escape -- json
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-escape --no-run
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

use stringcheese_escape::{Escape, escape};

// ---------------------------------------------------------------------------
// Deterministic input construction — same shape as the sibling bench
// files. No RNG; every `cargo bench` invocation feeds byte-identical
// inputs so criterion's noise floor stays meaningful across runs.
// ---------------------------------------------------------------------------

/// Plain ASCII prose seed with no metacharacters. Every target's
/// fast path applies.
const PLAIN_SEED: &str = "the quick brown fox jumps over the lazy dog and finds a safe input ";

/// Metachar-heavy seed. Includes:
///
/// - `&`, `<`, `>`, `"`, `'` — HTML entities
/// - `\n`, `\t`, `\\` — JSON escapes
/// - `/`, `?`, `#`, ` `, `=`, `&`, `+` — URI reserved
/// - `'`, `$`, `` ` ``, `|`, `&`, `;` — shell metacharacters
///
/// A single seed exercises every target's substitution arm.
const META_SEED: &str =
    "hello & world <tag attr=\"v\">'quoted'</tag>\t/path?q=1&r=2\n$foo|bar;baz `cmd`\\";

/// Build a string of *at least* `bytes` bytes by cycling `seed`.
fn build_input(seed: &str, bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + seed.len());
    while out.len() < bytes {
        out.push_str(seed);
    }
    out
}

/// Byte lengths swept by every (target, flavor) cell.
const SIZES: &[usize] = &[256, 1024, 4 * 1024];

/// The two flavors every group runs.
fn flavors() -> [(&'static str, &'static str); 2] {
    [("plain", PLAIN_SEED), ("meta", META_SEED)]
}

/// One helper per group — every target shares the shape
/// `escape(input, Escape::X)`, so a parametrised helper collapses
/// the four groups into one code path.
fn bench_target(c: &mut Criterion, group_name: &str, target: Escape) {
    let mut group = c.benchmark_group(format!("escape/{group_name}"));
    for (flavor, seed) in flavors() {
        for &size in SIZES {
            let input = build_input(seed, size);
            group.throughput(Throughput::Bytes(input.len() as u64));
            let id = BenchmarkId::new(flavor, input.len());
            group.bench_with_input(id, &input, |bencher, input| {
                bencher.iter(|| black_box(escape(black_box(input), target)));
            });
        }
    }
    group.finish();
}

fn bench_uri(c: &mut Criterion) {
    bench_target(c, "uri", Escape::UriComponent);
}
fn bench_html(c: &mut Criterion) {
    bench_target(c, "html", Escape::Html);
}
fn bench_json(c: &mut Criterion) {
    bench_target(c, "json", Escape::JsonString);
}
fn bench_shell(c: &mut Criterion) {
    bench_target(c, "shell", Escape::ShellWord);
}

criterion_group!(escape_bench, bench_uri, bench_html, bench_json, bench_shell);
criterion_main!(escape_bench);
