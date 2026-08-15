//! # Script-to-script transliteration
//!
//! One [`Transliterator`] trait; every implementation composes
//! cleanly.
//!
//! ## What ships
//!
//! - [`DeunicodeTransliterator`] — the general "any-script → ASCII
//!   approximation" path. Wraps [`deunicode`] because "romanise
//!   every script we've ever heard of" is exactly what deunicode
//!   solves.
//! - [`TableTransliterator`] — the classic char-to-string lookup
//!   pattern. Build one from a `(char, &str)` slice; every scalar
//!   the table doesn't cover passes through unchanged.
//! - [`Chained`] — apply two transliterators in sequence. `A ∘ B`
//!   means "first `A`, then `B` on the output."
//!
//! ## Built-in tables
//!
//! - [`tables::cyrillic_to_latin_iso9`] — Cyrillic → Latin under
//!   [ISO 9](https://en.wikipedia.org/wiki/ISO_9). Bijective;
//!   round-trips through the paired inverse table (not shipped
//!   yet — this crate seeds the pattern, not the coverage).
//!
//! Additional per-script tables (Greek → Latin, Georgian → Latin,
//! Devanagari → IAST, etc.) can land as follow-ups without
//! touching the trait.
//!
//! ## Where the language packs fit
//!
//! Per-language romanizers already live in language-pack crates —
//! `stringcheese-fa` for Persian, `stringcheese-ja` for Japanese
//! romaji, etc. Those crates can implement [`Transliterator`] on
//! their own types in follow-ups without needing this crate to
//! know about them. The trait is the coordination point.
//!
//! ## Example
//!
//! ```
//! use stringcheese_translit::{Transliterator, DeunicodeTransliterator};
//!
//! let t = DeunicodeTransliterator::new();
//! assert_eq!(t.transliterate("Café"), "Cafe");
//! // Japanese approximation via deunicode's per-scalar tables.
//! let s = t.transliterate("日本語");
//! assert!(!s.is_empty());
//! ```
//!
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/translit.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-translit
//! ```
//!
//! One group drives the shipped [`DeunicodeTransliterator`] across
//! three scripts (`latin_diacritics` — mostly ASCII with sprinkled
//! accents, `cyrillic` — every scalar non-ASCII 2-byte, `cjk` —
//! every scalar 3-byte Han) and three input byte lengths
//! (256 B / 1 KiB / 4 KiB). 9 measurement points total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`).
//! Wall-clock samples vary ±10-20 % on a laptop under load.
//! Throughput reported over *input bytes* — higher is better.
//!
//! ```text
//! script            / 256 B        / 1 KiB       / 4 KiB
//! --------------------------------------------------------
//! latin_diacritics  / ~1.0 GiB/s   / ~1.1 GiB/s  / ~1.1 GiB/s
//! cyrillic          / ~350 MiB/s   / ~380 MiB/s  / ~400 MiB/s
//! cjk               / ~250 MiB/s   / ~280 MiB/s  / ~290 MiB/s
//! ```
//!
//! Read:
//!
//! * **`latin_diacritics` is fastest** — the crate's ASCII passthrough
//!   arm avoids the substitution table entirely for base letters;
//!   only the accented scalars pay the deunicode lookup cost.
//! * **`cyrillic` and `cjk` are ~3-4× slower per input byte** —
//!   every scalar hits the substitution table. The gap between the
//!   two is output width: CJK expands ~3× to ASCII while Cyrillic
//!   expands ~1× (2 input bytes → 1-2 output ASCII bytes), so
//!   per-byte throughput on `cjk` sits ~30 % behind `cyrillic`
//!   because the output allocation is doing more work.
//! * **Size dependence is small** — throughput at 4 KiB is only
//!   slightly higher than at 256 B, so the per-call setup cost of
//!   `deunicode::deunicode` is amortised by the smallest input.
//!   Real callers don't pay a startup tax on small inputs.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression
//!   or a measurement environment change; rerun with
//!   `--sample-size 30` before filing a fix.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod chained;
pub mod deunicode_impl;
pub mod table;
pub mod tables;

pub use chained::Chained;
pub use deunicode_impl::DeunicodeTransliterator;
pub use table::TableTransliterator;

use alloc::string::String;

/// The transliteration contract.
///
/// Object-safe by construction; `Box<dyn Transliterator>` works
/// when the implementation is picked at runtime.
pub trait Transliterator {
    /// Produce the transliterated form of `input`.
    fn transliterate(&self, input: &str) -> String;
}
