//! # Named normalization pipelines
//!
//! `stringcheese-unicode` ships the primitives (NFC / NFKC /
//! case-fold / strip-diacritics) and a builder. This crate ships
//! the pipelines callers actually want — one function each, with
//! the exact composition documented at the top of the doc comment.
//!
//! ## Pipelines
//!
//! - [`identifier`] — NFKC → case-fold → strip-diacritics.
//!   Use for identifier comparison / deduplication.
//! - [`display_safe`] — strip-controls → NFC → collapse-whitespace.
//!   Use before rendering user-supplied text.
//! - [`search_key`] — NFKC → case-fold → strip-diacritics →
//!   canonicalise-punctuation → collapse-whitespace → trim.
//!   Use as the key for a fuzzy-match / suggest index.
//! - [`punctuation_canonical`] — canonicalise-punctuation only.
//!   Use for round-trip-friendly display, or as an ingredient.
//!
//! ## Primitives
//!
//! - [`collapse_whitespace`] — every run of `char::is_whitespace`
//!   collapses to a single U+0020 SPACE. Leading and trailing
//!   whitespace stays (use [`trim`] to strip).
//! - [`trim`] — like [`str::trim`] but returns [`String`] so it
//!   composes with the other primitives in this crate.
//! - [`strip_controls`] — every general-category Cc code point is
//!   removed. Preserves whitespace controls (`\t`, `\n`) unless
//!   the caller explicitly strips those separately.
//! - [`canonicalize_punctuation`] — normalises common
//!   ambiguously-encoded punctuation to a canonical ASCII form:
//!   smart quotes → `"`/`'`, en/em/figure/etc. dashes →
//!   hyphen-minus `-`, non-breaking spaces → ASCII space,
//!   ellipsis → three dots.
//!
//! ## Order matters
//!
//! Normalization stages don't commute. `NFKC then case-fold` is
//! not `case-fold then NFKC` in general — compatibility
//! decomposition can expose characters that case-fold differently
//! than the pre-decomposition input. Every named pipeline in this
//! crate documents its exact stage order in the function docs.
//! If a caller wants a different order, they compose the
//! primitives directly (or reach for
//! [`stringcheese_unicode::PreprocessingPipeline`]).
//!
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at
//! `benches/normalize.rs`; run it with
//!
//! ```text
//! cargo bench -p stringcheese-normalize
//! ```
//!
//! Four groups drive the four shipped preset pipelines
//! ([`identifier`], [`display_safe`], [`search_key`],
//! [`punctuation_canonical`]). Each group runs three input byte
//! lengths (1 KiB / 4 KiB / 16 KiB) crossed with two flavors
//! (`ascii` — the NFC/NFKC fast path — and `diacritics` — mixed
//! Latin with smart quotes / em dashes / non-breaking spaces /
//! combining marks). 24 measurement points total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`);
//! wall-clock samples vary ±10-20 %. Throughput reported over
//! *input bytes* — higher is better.
//!
//! ```text
//! pipeline              / flavor       / 1 KiB      / 4 KiB      / 16 KiB
//! ---------------------------------------------------------------------------
//! punctuation_canonical / ascii        / ~1.1 GiB/s / ~1.2 GiB/s / ~1.2 GiB/s
//! punctuation_canonical / diacritics   / ~750 MiB/s / ~800 MiB/s / ~800 MiB/s
//! display_safe          / ascii        / ~500 MiB/s / ~530 MiB/s / ~540 MiB/s
//! display_safe          / diacritics   / ~70 MiB/s  / ~75 MiB/s  / ~75 MiB/s
//! identifier            / ascii        / ~250 MiB/s / ~260 MiB/s / ~265 MiB/s
//! identifier            / diacritics   / ~28 MiB/s  / ~30 MiB/s  / ~30 MiB/s
//! search_key            / ascii        / ~200 MiB/s / ~210 MiB/s / ~215 MiB/s
//! search_key            / diacritics   / ~25 MiB/s  / ~27 MiB/s  / ~27 MiB/s
//! ```
//!
//! Read:
//!
//! * **`punctuation_canonical` is fastest** — a single-pass in-house
//!   primitive with no ICU cost. ASCII hits the passthrough arm
//!   at ~1.2 GiB/s.
//! * **`display_safe` is 2.5× faster than `identifier`** because NFC
//!   is lighter than NFKC; the diacritics flavor collapses both by
//!   ~7-8× vs their ASCII fast paths.
//! * **`identifier` and `search_key` bottleneck on NFKC** — both
//!   drop to ~30 MiB/s on the diacritics flavor. NFKC is
//!   ~10× slower than any in-house primitive; a caller building
//!   high-throughput indexes on diacritic-heavy text should
//!   consider whether NFC (via `display_safe`) is enough.
//! * **ASCII-flavor `identifier` still sits at 250 MiB/s** — the
//!   NFKC ICU pass is not free even when it has nothing to
//!   decompose; the per-byte scan is the load-bearing cost.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression
//!   or a measurement environment change; rerun with
//!   `--sample-size 30` before filing a fix.
//!
//! ## Prior baseline (2026-08-09, `stringcheese-bench/benches/normalize.rs`)
//!
//! Retained for context — earlier numbers from the workspace-external
//! bench harness against which the current suite's numbers should be
//! compared. Numbers below were taken on realistic mixed text at 16 KB
//! input:
//!
//! | Pipeline                   | throughput | dominant cost |
//! |----------------------------|------------|----------------|
//! | `punctuation_canonical`    | 802 MiB/s  | single-pass primitive |
//! | `display_safe`             |  75 MiB/s  | NFC normalisation |
//! | `identifier`               |  30 MiB/s  | **NFKC normalisation** |
//! | `search_key`               |  27 MiB/s  | NFKC + downstream primitives |
//!
//! Primitives at 2 KB (in-house paths, no Unicode normalisation):
//!
//! | Primitive                  | throughput |
//! |----------------------------|------------|
//! | `trim`                     | 44 GiB/s   |
//! | `collapse_whitespace`      | 956 MiB/s  |
//! | `canonicalize_punctuation` | 802 MiB/s  |
//! | `strip_controls`           | 654 MiB/s  |
//!
//! All three in-house primitives benefited from bench-driven
//! rewrites on 2026-08-09:
//!
//! - `canonicalize_punctuation` (510 → 802 MiB/s, +56 %) —
//!   ASCII bytes stream through in coalesced runs; only
//!   non-ASCII scalars invoke the substitution match.
//! - `collapse_whitespace` (476 → 956 MiB/s, +100 %) —
//!   byte-oriented iteration with an ASCII whitespace lookup
//!   table. An attempted coalesce-non-whitespace-run variant
//!   regressed 20 % because real inputs have whitespace every
//!   ~5 bytes; the per-byte lookup is the sweet spot.
//! - `strip_controls` (520 → 654 MiB/s, +26 %) — coalesces
//!   runs of non-control ASCII, which is most of a real input.
//!
//! **NFKC is the bottleneck** for `identifier` / `search_key` —
//! ~10× slower than any in-house primitive. Callers building
//! high-throughput indexes should be aware: if approximate-case
//! matching is enough, `display_safe` at 75 MiB/s is 2.5× faster
//! than `identifier` because NFC is lighter than NFKC. If
//! diacritic sensitivity matters more than compatibility folding,
//! reach for [`stringcheese_unicode::PreprocessingPipeline`]
//! directly and skip NFKC.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod pipelines;
pub mod primitives;

pub use pipelines::{display_safe, identifier, punctuation_canonical, search_key};
pub use primitives::{canonicalize_punctuation, collapse_whitespace, strip_controls, trim};
