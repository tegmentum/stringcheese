//! Unicode preprocessing for the Comparand sequence-comparison toolkit.
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
//!   [`comparand_core::IndexableSequence`]. This is the crate that finally
//!   lets a distance kernel compare "over graphemes": before it,
//!   Levenshtein could only compare over bytes or Unicode scalar values.
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
//! [`comparand_core::NormalizationPolicy`], which names how a raw
//! distance is scaled into `[0.0, 1.0]`. The two never collide; see the
//! cross-reference in the design document.
//!
//! # `no_std`
//!
//! - `default` — pulls in `std` for convenience.
//! - `no_std + alloc` — the entire public surface is available. All
//!   Unicode-preprocessing operations return an owned `String` or
//!   `Vec` and therefore require `alloc`.
//! - `no_std` alone (no `alloc`) — the crate exposes an empty surface.
//!   Consumers that need Unicode preprocessing must enable `alloc`.
//!
//! # Dependency footprint
//!
//! Three third-party crates:
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
//!
//! Every dependency is declared with `default-features = false` and only
//! the features this crate uses are re-enabled.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod case_folding;
#[cfg(feature = "alloc")]
pub mod diacritics;
#[cfg(feature = "alloc")]
pub mod graphemes;
#[cfg(feature = "alloc")]
pub mod normalization;
#[cfg(feature = "alloc")]
pub mod preprocessing;

#[cfg(all(feature = "alloc", test))]
mod golden;
#[cfg(all(feature = "alloc", test))]
mod property_tests;

// Re-export the public surface at the crate root so consumers can write
// `use comparand_unicode::PreprocessingPipeline` rather than reaching
// into modules.
#[cfg(feature = "alloc")]
pub use case_folding::{case_fold, case_fold_turkic, simple_case_fold};
#[cfg(feature = "alloc")]
pub use diacritics::strip_diacritics;
#[cfg(feature = "alloc")]
pub use graphemes::{GraphemeSequence, graphemes};
#[cfg(feature = "alloc")]
pub use normalization::{Normalization, nfc, nfd, nfkc, nfkd};
#[cfg(feature = "alloc")]
pub use preprocessing::{PreprocessingPipeline, PreprocessingStep};
