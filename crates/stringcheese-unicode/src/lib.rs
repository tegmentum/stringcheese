//! Unicode preprocessing for the StringCheese sequence-comparison toolkit.
//!
//! This crate provides the string-to-string preprocessing infrastructure a
//! [`PreprocessingPipeline`] composes. Its job is to bridge raw UTF-8 input
//! and the representation an algorithm actually compares:
//!
//! - **Unicode normalization** (NFC / NFD / NFKC / NFKD) via
//!   [`normalization`].
//! - **Full Unicode case folding** — the operation designed for
//!   case-insensitive comparison — via [`case_folding`]. This is
//!   deliberately distinct from `str::to_lowercase()`; see the module
//!   documentation for the differences (Turkish dotless I, ß → ss, and
//!   final sigma all diverge).
//! - **Grapheme-cluster segmentation** via [`mod@graphemes`], including a
//!   materialized [`GraphemeSequence`] that implements
//!   [`stringcheese_core::IndexableSequence`]. This is the crate that finally
//!   lets a distance kernel compare "over graphemes": before it,
//!   Levenshtein could only compare over bytes or Unicode scalar values.
//! - **UAX #29 word segmentation** via [`mod@words`] (gated on
//!   `feature = "word-segmentation"`, default on), with a matching
//!   materialized [`WordSequence`]. Extends the "compare over units"
//!   story from graphemes up to words.
//! - **UAX #29 sentence segmentation** via [`mod@sentences`] (gated on
//!   `feature = "sentence-segmentation"`, default on), with a matching
//!   materialized [`SentenceSequence`].
//! - **UAX #14 line breaking** via [`mod@line_breaks`] (gated on
//!   `feature = "line-breaking"`, default on), with a matching
//!   materialized [`LineBreakSequence`]. Complements the UAX #29
//!   word- and sentence-segmentation surfaces by identifying the
//!   *soft-wrap* opportunities a downstream word-wrapper uses to fit
//!   text into a column.
//! - **Diacritic stripping** via [`diacritics`] (NFD → drop combining
//!   marks → NFC). This is a lossy operation; see the module documentation
//!   for what it does and does not cover.
//! - **Composable pipeline** via [`preprocessing`], a builder that chains
//!   the above into a single, inspectable, reusable object.
//!
//! # Relationship to the design documents
//!
//! The definitive specification for the pipeline shape is
//! `docs/design/preprocessing-pipeline.md`. This crate is the *first*
//! realization of that design; a future `Comparator` will fold this
//! pipeline together with a distance metric so the whole `raw string →
//! metric result` chain is a single value. That combined type is future
//! work.
//!
//! The `Normalization` enum in [`normalization`] names Unicode normal
//! forms (NFC/NFD/NFKC/NFKD). It is **not** the same concept as
//! [`stringcheese_core::NormalizationPolicy`], which names how a raw
//! distance is scaled into `[0.0, 1.0]`. The two never collide; see the
//! cross-reference in the design document.
//!
//! # `no_std`
//!
//! - `default` — pulls in `std`, the `case-fold` module, and the baked
//!   `compiled-case-data` for convenience.
//! - `no_std + alloc` — normalization, diacritic stripping, grapheme
//!   segmentation, and the preprocessing pipeline are available. Case
//!   folding and the `case_folding` module are additionally gated on
//!   the `case-fold` feature.
//! - `no_std` alone (no `alloc`) — the crate exposes an empty surface.
//!   Consumers that need Unicode preprocessing must enable `alloc`.
//!
//! # Feature flags — the wasm-size axis
//!
//! In addition to the standard `std` / `alloc` split, this crate
//! exposes two features that gate the ICU case-mapping surface. Both
//! are on by default so a casual caller sees the full API; disable
//! them for wasm-size-critical builds.
//!
//! - `case-fold` (default: on) — enables the [`case_folding`] module
//!   and pulls in `icu_casemap`. Turning it off drops `icu_casemap`
//!   and its ~30 transitive ICU4X data crates entirely, saving roughly
//!   110 KB in a `wasm-opt -Oz` build. Callers who only need
//!   normalization, diacritic stripping, and grapheme iteration can
//!   set `default-features = false, features = ["std"]` (or
//!   `["alloc"]`) and pay none of the case-fold surface cost.
//! - `compiled-case-data` (default: on) — bakes the ICU case-mapping
//!   tables into the binary so [`case_fold`], [`simple_case_fold`],
//!   [`case_fold_turkic`], and the [`PreprocessingStep::CaseFold`]
//!   pipeline variant work with no runtime setup. Turning it off (while
//!   leaving `case-fold` on) trims another ~110 KB and requires the
//!   caller to construct a [`case_folding::CaseMapper`] from a runtime
//!   `DataProvider` and use the `_with_mapper` variants
//!   ([`case_folding::case_fold_with_mapper`], etc.).
//! - `word-segmentation` (default: on) — enables the [`mod@words`]
//!   module (UAX #29 word iteration, `WordSequence`,
//!   `SplitWordBoundsBehavior`, and `word_bounds`). The underlying
//!   `unicode-segmentation` crate pulls in the `Word_Break` tables
//!   only when a word-boundary API is reached; toggling this feature
//!   off drops both the module and (via LTO) that table pressure.
//! - `sentence-segmentation` (default: on) — enables the
//!   [`mod@sentences`] module (UAX #29 sentence iteration and
//!   `SentenceSequence`). Independently toggleable from
//!   `word-segmentation` so callers can shed either surface.
//! - `line-breaking` (default: on) — enables the [`mod@line_breaks`]
//!   module (UAX #14 line-break opportunities, `LineBreakSequence`,
//!   and `line_breaks`). Pulls in the `unicode-linebreak` crate and
//!   its `Line_Break` property table. Independently toggleable so a
//!   caller who only needs word-wrap opportunities can enable this
//!   without paying for the `Word_Break` / `Sentence_Break` tables,
//!   and vice versa.
//!
//! The wasm-size gate documented in `docs/wasm-binary-size.md` measures
//! the `--no-default-features --features std` configuration — the
//! smallest useful surface — so the tracked baseline reflects what a
//! size-conscious wasm caller actually pays.
//!
//! # Dependency footprint
//!
//! Four third-party crates:
//!
//! - [`unicode-normalization`](https://docs.rs/unicode-normalization) —
//!   NFC/NFD/NFKC/NFKD and the `is_combining_mark` predicate used by
//!   diacritic stripping. Small, from `unicode-rs`. Supports `no_std`.
//! - [`unicode-segmentation`](https://docs.rs/unicode-segmentation) —
//!   grapheme-cluster boundary detection per Unicode Standard Annex #29.
//!   Small, from `unicode-rs`. Supports `no_std`.
//! - [`icu_casemap`](https://docs.rs/icu_casemap) — full Unicode case
//!   folding (including multi-character expansions such as ß → ss and
//!   Turkic dotted/dotless I). Part of the ICU4X project and the
//!   authoritative Unicode-Consortium implementation. Explicitly
//!   `no_std` with baked-in `compiled_data`. Heavier than the
//!   alternatives (30+ transitive crates, all official) but the only
//!   widely available crate that provides *full* folding out of the
//!   box; the smaller `unicode-case-mapping` implements only *simple*
//!   folding and cannot express the ß → ss expansion the design calls
//!   out.
//! - [`unicode-linebreak`](https://docs.rs/unicode-linebreak) —
//!   UAX #14 line-break opportunity detection (soft wrap points plus
//!   mandatory line terminators). Small, `#![no_std]`, tracks
//!   Unicode 15.0.0. Preferred over `xi-unicode` (see the module
//!   documentation for the tradeoff).
//!
//! Every dependency is declared with `default-features = false` and only
//! the features this crate uses are re-enabled.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(all(feature = "alloc", feature = "case-fold"))]
pub mod case_folding;
#[cfg(feature = "alloc")]
pub mod diacritics;
#[cfg(feature = "alloc")]
pub mod graphemes;
#[cfg(all(feature = "alloc", feature = "line-breaking"))]
pub mod line_breaks;
#[cfg(feature = "alloc")]
pub mod normalization;
#[cfg(feature = "alloc")]
pub mod preprocessing;
#[cfg(all(feature = "alloc", feature = "sentence-segmentation"))]
pub mod sentences;
#[cfg(all(feature = "alloc", feature = "word-segmentation"))]
pub mod words;

#[cfg(all(feature = "alloc", test))]
mod golden;
#[cfg(all(feature = "alloc", test))]
#[cfg(not(target_family = "wasm"))]
mod property_tests;

// Re-export the public surface at the crate root so consumers can write
// `use stringcheese_unicode::PreprocessingPipeline` rather than reaching
// into modules.
#[cfg(all(feature = "alloc", feature = "compiled-case-data"))]
pub use case_folding::{case_fold, case_fold_turkic, simple_case_fold};
#[cfg(all(feature = "alloc", feature = "case-fold"))]
pub use case_folding::{
    case_fold_turkic_with_mapper, case_fold_with_mapper, simple_case_fold_with_mapper,
};
#[cfg(feature = "alloc")]
pub use diacritics::strip_diacritics;
#[cfg(feature = "alloc")]
pub use graphemes::{GraphemeSequence, graphemes};
#[cfg(all(feature = "alloc", feature = "line-breaking"))]
pub use line_breaks::{LineBreak, LineBreakSequence, line_breaks};
#[cfg(feature = "alloc")]
pub use normalization::{Normalization, nfc, nfd, nfkc, nfkd};
#[cfg(feature = "alloc")]
pub use preprocessing::{PreprocessingPipeline, PreprocessingStep};
#[cfg(all(feature = "alloc", feature = "sentence-segmentation"))]
pub use sentences::{SentenceSequence, sentence_indices, sentences};
#[cfg(all(feature = "alloc", feature = "word-segmentation"))]
pub use words::{
    SplitWordBoundsBehavior, WordSequence, word_bound_indices, word_bounds, word_indices, words,
};
