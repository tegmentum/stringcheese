//! # Locale-aware collation
//!
//! One [`Collator`] trait; every implementation composes cleanly
//! with `slice.sort_by(|a, b| collator.compare(a, b))`.
//!
//! ## What ships
//!
//! - [`UcaCollator`] — Unicode Collation Algorithm (root locale)
//!   via [`feruca`]. Sensible default: "à" < "b", "café" ≈ "cafe"
//!   at the primary weight, ordering that a human would expect.
//! - [`NaturalCollator`] — numeric-run-aware ordering built over
//!   any inner [`Collator`]. `file2` < `file10` (not the
//!   lexicographic `file10 < file2` you'd otherwise get).
//! - [`AsciiCiCollator`] — the ASCII-fast path: byte-level
//!   compare with `to_ascii_lowercase` on each byte. For inputs
//!   that are known-ASCII (identifiers, machine keys, log lines)
//!   this is orders of magnitude faster than UCA and produces the
//!   same ordering.
//!
//! ## When to reach for which
//!
//! - **Human-facing text** in any script → `UcaCollator`.
//! - **Filenames / version strings** with embedded numbers →
//!   `NaturalCollator::new(UcaCollator::default())`.
//! - **Machine keys** known to be ASCII → `AsciiCiCollator`
//!   (or plain `str::cmp` if case doesn't matter either).
//!
//! ## Baseline (2026-08-09)
//!
//! Numbers from `stringcheese-bench/benches/collate.rs`
//! (ASCII input; single-item compare and 1000-item `sort_by`):
//!
//! | Kernel                         | compare | sort 1000 |
//! |--------------------------------|---------|-----------|
//! | `AsciiCiCollator`              | 16 ns   | 45 µs     |
//! | `UcaCollator`                  | 73 ns   | 227 µs    |
//! | `NaturalCollator<AsciiCi>`     | 132 ns  | 243 µs    |
//!
//! UCA carries a **~5× per-compare** cost over ASCII-CI on ASCII
//! input; scaled to a whole-slice `sort_by` that becomes **~5×
//! wall clock**. Natural collation adds run-partitioning
//! overhead (~5-8×) even when the input has no digits — the
//! partition happens per compare regardless. Pick UCA when
//! Unicode correctness is required, ASCII-CI when the input is
//! known-ASCII, and Natural only when you actually need
//! `file2 < file10` ordering.
//!
//! ## Sort keys
//!
//! Not exposed today. `slice.sort_by(collator.compare(...))`
//! covers the common case at `O(n log n)` comparisons; explicit
//! sort keys would win in the "sort once, look up many" pattern
//! but require reaching under `feruca`'s public API. A follow-up
//! can add them if the ergonomic pull becomes real.
//!
//! ## Relationship to `stringcheese-icu-collation`
//!
//! This crate is the **in-process, root-locale** collator: three
//! implementations, one trait, no data packs, no WIT boundary,
//! no per-locale tailoring. Reach for it when the caller wants a
//! plain `Ordering` under the CLDR-root Unicode Collation Algorithm
//! (or the ASCII-CI / natural-sort variants) and can spell the
//! import in Rust.
//!
//! [`stringcheese-icu-collation`] is the Phase-2 WIT-i18n crate that
//! **wraps this crate's [`UcaCollator`]** with per-locale SCUD packs
//! (Turkish primary-weight overrides, French backwards-secondary,
//! Russian case-second, German phonebook expansions, …), exposes a
//! `sort_key` byte encoding, walks the BCP 47 fallback chain
//! (`de-DE → de → ""`) at query time, and ships behind a WASM
//! component boundary. Reach for it when the caller needs locale
//! tailoring, sort keys, or WIT-shaped access from a non-Rust host.
//!
//! Rule of thumb: **no locale → this crate. Locale tag →
//! `stringcheese-icu-collation`.**
//!
//! [`stringcheese-icu-collation`]: https://docs.rs/stringcheese-icu-collation

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
// Referenced by test modules and by consumers via
// `slice.sort_by(|a, b| collator.compare(a, b))` patterns that
// pull in Vec through the caller's own alloc. Kept explicit so
// `no_std + alloc` builds compile without magic.
#[allow(unused_extern_crates)]
extern crate alloc;

pub mod ascii_ci;
pub mod natural;
pub mod uca;

pub use ascii_ci::AsciiCiCollator;
pub use natural::NaturalCollator;
pub use uca::UcaCollator;

use core::cmp::Ordering;

/// The collator contract. Every implementation must satisfy
/// `compare(a, b) == compare(b, a).reverse()` — same requirement
/// [`Ord`] imposes.
///
/// Object-safe by construction; `Box<dyn Collator>` works when a
/// runtime dispatch is needed.
pub trait Collator {
    /// Order `a` relative to `b` per this collator's rules.
    fn compare(&self, a: &str, b: &str) -> Ordering;
}
