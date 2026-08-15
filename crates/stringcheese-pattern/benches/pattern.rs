//! Baseline throughput benchmarks for the three shipped pattern
//! engines in `stringcheese-pattern`.
//!
//! Mirrors the shape of the sibling bench files (`stringcheese-compare`
//! / `stringcheese-manip`): one bench file, criterion-owned `main` via
//! `harness = false`, groups named `<engine>/<complexity>/<len>`. The
//! purpose is a regression trip-wire and a last-measured baseline
//! table for the pattern subsystem.
//!
//! # Engines
//!
//! Three groups, one per shipped [`Pattern`] implementation:
//!
//! * `literal` — the `memchr`-accelerated exact-substring engine
//!   ([`Literal`]). Fastest by construction; every byte of the
//!   haystack is scanned with SSE2/AVX2/NEON depending on target.
//! * `wildcard` — the `?` / `*` engine ([`Wildcard`]), which
//!   compiles to a regex under the hood. `anywhere(...)` is used so
//!   the engine has to scan the entire haystack, not just verify a
//!   whole-string match.
//! * `glob` — POSIX-style with character classes ([`Glob`]), also
//!   regex-backed, also `anywhere(...)`. The extra `[abc]` /
//!   `[a-z]` primitives raise the compilation cost but not the
//!   per-byte scan cost meaningfully (the regex engine still walks
//!   the DFA once per byte).
//!
//! # Complexities
//!
//! Three per engine (nine cells per size):
//!
//! * `simple` — a short single-token needle. Literal: `"the"`.
//!   Wildcard: `"the*"`. Glob: `"the*"` (identical to wildcard at
//!   this complexity — the extra glob syntax only kicks in on the
//!   `medium` and `complex` rows).
//! * `medium` — a mid-complexity needle. Literal: `"quickbrown"`.
//!   Wildcard: `"the*fox*dog"`. Glob: `"the*[a-z]??fox*dog"`.
//! * `complex` — a long or highly-branching needle. Literal:
//!   `"themiddleofthisstring"`. Wildcard: `"*over*lazy*dog*"`. Glob:
//!   `"*[Tt]he*[a-z]?[a-z]*over*[!0-9]*dog*"`.
//!
//! # Sizes
//!
//! Three input lengths per (engine, complexity): 256 B, 4 KiB, 64 KiB.
//! Realistic for the "one pattern applied to many documents" workload
//! the crate is built for; the 64 KiB row is where scan-loop steady-
//! state dominates and per-call fixed overhead amortizes out.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-pattern
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-pattern -- literal
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-pattern --no-run
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

use stringcheese_pattern::{Glob, Literal, MatchUnit, Pattern, Wildcard};

// ---------------------------------------------------------------------------
// Deterministic input construction.
//
// Same shape as `stringcheese-manip/benches/manip.rs`: a small fixed
// word pool cycled until the target byte length is met. Every run
// produces byte-identical inputs, so criterion's noise floor stays
// meaningful across runs.
// ---------------------------------------------------------------------------

/// Prose word list containing the tokens the medium and complex
/// patterns look for (`the`, `quick`, `brown`, `fox`, `over`, `lazy`,
/// `dog`). Keeps the patterns from degenerating into the no-match
/// early-return path — every group actually walks the full haystack.
const WORDS: &[&str] = &[
    "the",
    "quick",
    "brown",
    "fox",
    "jumps",
    "over",
    "the",
    "lazy",
    "dog",
    "and",
    "then",
    "runs",
    "back",
    "through",
    "the",
    "forest",
    "toward",
    "the",
    "cabin",
    "on",
    "the",
    "hill",
    "themiddleofthisstring",
    "where",
    "smoke",
    "curls",
];

/// Build a haystack of *at least* `bytes` UTF-8 bytes (all ASCII here
/// — the word pool is ASCII-only) and truncate to a scalar boundary
/// at or above `bytes`. The truncation is char-boundary safe by
/// construction (every ASCII byte is a scalar).
fn haystack(bytes: usize) -> String {
    let mut out = String::with_capacity(bytes + 32);
    let mut idx = 0usize;
    while out.len() < bytes {
        out.push_str(WORDS[idx % WORDS.len()]);
        out.push(' ');
        idx += 1;
    }
    out
}

/// Byte lengths swept by every (engine, complexity) cell. 256 B
/// catches the per-call overhead; 4 KiB is a middle-ground doc; 64 KiB
/// is where the inner scan loop dominates.
const SIZES: &[usize] = &[256, 4 * 1024, 64 * 1024];

/// The three complexity tiers. Kept as an array so
/// `BenchmarkId::new("simple", 256)` reads left-to-right in criterion
/// output as `<engine>/<complexity>/<len>`.
const COMPLEXITIES: &[&str] = &["simple", "medium", "complex"];

// ---------------------------------------------------------------------------
// Engine groups.
//
// Every engine is constructed once per (complexity, size) cell and
// reused across criterion iterations — construction cost is not the
// measurement target; per-call scan cost is. `Pattern::find_iter` is
// drained via `.count()` to force the full walk (otherwise criterion
// would measure only the iterator constructor).
// ---------------------------------------------------------------------------

fn bench_literal(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern/literal");
    for &complexity in COMPLEXITIES {
        let needle = match complexity {
            "simple" => "the",
            "medium" => "quickbrown",
            "complex" => "themiddleofthisstring",
            _ => unreachable!(),
        };
        let pat = Literal::new(needle, MatchUnit::Bytes);
        for &n in SIZES {
            let input = haystack(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(complexity, n), &input, |bencher, input| {
                bencher.iter(|| black_box(pat.find_iter(black_box(input)).count()));
            });
        }
    }
    group.finish();
}

fn bench_wildcard(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern/wildcard");
    for &complexity in COMPLEXITIES {
        let pattern = match complexity {
            "simple" => "the*",
            "medium" => "the*fox*dog",
            "complex" => "*over*lazy*dog*",
            _ => unreachable!(),
        };
        let pat = Wildcard::anywhere(pattern, MatchUnit::Bytes);
        for &n in SIZES {
            let input = haystack(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(complexity, n), &input, |bencher, input| {
                bencher.iter(|| black_box(pat.find_iter(black_box(input)).count()));
            });
        }
    }
    group.finish();
}

fn bench_glob(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern/glob");
    for &complexity in COMPLEXITIES {
        let pattern = match complexity {
            "simple" => "the*",
            "medium" => "the*[a-z]??fox*dog",
            "complex" => "*[Tt]he*[a-z]?[a-z]*over*[!0-9]*dog*",
            _ => unreachable!(),
        };
        let pat = Glob::anywhere(pattern, MatchUnit::Bytes);
        for &n in SIZES {
            let input = haystack(n);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(complexity, n), &input, |bencher, input| {
                bencher.iter(|| black_box(pat.find_iter(black_box(input)).count()));
            });
        }
    }
    group.finish();
}

criterion_group!(pattern, bench_literal, bench_wildcard, bench_glob);
criterion_main!(pattern);
