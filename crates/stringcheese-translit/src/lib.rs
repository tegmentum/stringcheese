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
//! four scripts (`ascii` — pure ASCII, gates through the
//! `is_ascii` fast path; `latin_diacritics` — mostly ASCII with
//! sprinkled accents; `cyrillic` — every scalar non-ASCII 2-byte;
//! `cjk` — every scalar 3-byte Han) and three input byte lengths
//! (256 B / 1 KiB / 4 KiB). 12 measurement points total.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`).
//! Wall-clock samples vary ±10-20 % on a laptop under load.
//! Throughput reported over *input bytes* — higher is better.
//!
//! ```text
//! script            / 256 B         / 1 KiB        / 4 KiB
//! ---------------------------------------------------------------
//! ascii             / ~2.1 GiB/s    / ~4.2 GiB/s   / ~4.7 GiB/s
//! latin_diacritics  / ~360 MiB/s    / ~360 MiB/s   / ~370 MiB/s
//! cyrillic          / ~460 MiB/s    / ~510 MiB/s   / ~500 MiB/s
//! cjk               / ~320 MiB/s    / ~480 MiB/s   / ~510 MiB/s
//! ```
//!
//! Read:
//!
//! * **`ascii` lives on the `str::is_ascii` fast path** added
//!   alongside this baseline (`deunicode_impl::transliterate`
//!   short-circuits pure-ASCII input to a plain `to_string`
//!   clone). SIMD `is_ascii` + a single alloc is ~20-40× faster
//!   than walking deunicode's per-scalar substitution table.
//!   Any input with even one non-ASCII byte falls off this path
//!   into the deunicode walker below.
//! * **The three deunicode-path scripts land in the same
//!   350-500 MiB/s band** on larger inputs — deunicode's
//!   per-scalar substitution table is the load-bearing cost
//!   regardless of what the substitution ends up being. There
//!   is no "in-place ASCII passthrough" scan; the fast path is
//!   all-or-nothing, so `latin_diacritics` still walks the
//!   full table.
//! * **`latin_diacritics` is slightly slower per byte than
//!   `cyrillic`** because the mixed ASCII + accented shape means
//!   the substitution table is walked for ASCII bytes too, and
//!   there are more ASCII bytes per input scalar (so more table
//!   lookups per input byte).
//! * **Size dependence is small at 1 KiB and above** for the
//!   deunicode-path scripts — per-call setup cost of
//!   `deunicode::deunicode` is amortised by ~1 KiB input. The
//!   `ascii` fast path scales the opposite way — larger inputs
//!   amortise the single `to_string` allocation over more bytes
//!   copied via bulk memcpy.
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
