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
//! ## Baseline (2026-08-09)
//!
//! Numbers from `stringcheese-bench/benches/normalize.rs` on
//! realistic mixed text (accented Latin + curly quotes + em
//! dashes + case variation + whitespace runs). Pipelines at 16 KB
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
