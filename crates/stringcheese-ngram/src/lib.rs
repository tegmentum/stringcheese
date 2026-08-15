//! # Sliding-window n-gram generation
//!
//! Every n-gram producer in one place, with the [`NGramUnit`]
//! discipline every StringCheese boundary uses.
//!
//! ## What's here
//!
//! - [`char_ngrams`] — `n` consecutive Unicode code points as `&str`.
//! - [`byte_ngrams`] — `n` consecutive bytes as `&[u8]`.
//! - [`token_ngrams`] — `n` consecutive `&str` tokens as `&[&str]`.
//! - `grapheme_ngrams` — `n` consecutive grapheme clusters as
//!   `&str`, in the `graphemes` module behind the `graphemes`
//!   feature (only compiled in when that feature is enabled).
//!
//! Every function returns an iterator; nothing materialises a `Vec`
//! internally. Gram slices borrow the input for lifetime `'a` — no
//! copies.
//!
//! ## Padding
//!
//! Every constructor also has a `_padded` variant that prepends
//! `n - 1` sentinel units at the start and appends `n - 1` at the
//! end. Padding is what lets similarity metrics on short inputs
//! discriminate — a 5-char string produces only one 5-gram
//! unpadded, but eight 5-grams padded.
//!
//! Sentinel unit per representation:
//!
//! - characters — `'\u{FEFF}'` (BOM; never appears in normal text)
//! - bytes — `0x00`
//! - tokens — the empty string `""`
//!
//! ## Example
//!
//! ```
//! use stringcheese_ngram::char_ngrams;
//!
//! let grams: Vec<&str> = char_ngrams("hello", 3).collect();
//! assert_eq!(grams, vec!["hel", "ell", "llo"]);
//! ```
//!
//! ## Explicit unit
//!
//! No `.chars()` vs `.bytes()` silent default anywhere. The unit
//! is in the function name (`char_ngrams`, `byte_ngrams`,
//! `token_ngrams`, `grapheme_ngrams`) so a code review can point
//! at the wrong choice.
//!
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/ngram.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-ngram
//! ```
//!
//! Three groups drive the shipped unit-explicit producers ([`char_ngrams`],
//! [`byte_ngrams`], and — under `--features graphemes` —
//! `grapheme_ngrams`). Each group runs two input byte lengths
//! (256 B / 4 KiB) crossed with two flavors (`ascii` — one byte per
//! scalar — and `utf8` — Cyrillic, two bytes per scalar) and two
//! `n`-values (`n = 3` and `n = 5`). 24 measurement points when
//! `graphemes` is enabled, 16 without.
//!
//! ## Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`).
//! Wall-clock samples vary ±10-20 % on a laptop under load; treat the
//! order-of-magnitude column as the load-bearing part, not the last
//! digit. Throughput reported over *input bytes* — higher is better.
//!
//! ```text
//! unit     / flavor / n    256 B         4 KiB
//! ------------------------------------------------
//! char     / ascii  / 3    ~145 MiB/s    ~160 MiB/s
//! char     / ascii  / 5    ~130 MiB/s    ~180 MiB/s
//! char     / utf8   / 3    ~295 MiB/s    ~310 MiB/s
//! char     / utf8   / 5    ~270 MiB/s    ~310 MiB/s
//! byte     / ascii  / 3    ~2.0 GiB/s    ~2.5 GiB/s
//! byte     / ascii  / 5    ~2.1 GiB/s    ~2.4 GiB/s
//! byte     / utf8   / 3    ~2.6 GiB/s    ~2.5 GiB/s
//! byte     / utf8   / 5    ~2.4 GiB/s    ~2.3 GiB/s
//! grapheme / ascii  / 3    ~465 MiB/s    ~545 MiB/s  (ASCII fast-path)
//! grapheme / ascii  / 5    ~480 MiB/s    ~540 MiB/s  (ASCII fast-path)
//! grapheme / utf8   / 3    ~90 MiB/s     ~110 MiB/s
//! grapheme / utf8   / 5    ~90 MiB/s     ~90 MiB/s
//! ```
//!
//! The `grapheme / ascii` row runs the ASCII fast-path: strict-ASCII
//! input with no `\r` (which UAX #29 GB3 would combine with a
//! following `\n` into a single cluster) skips the ICU4X segmenter
//! entirely — every byte is its own grapheme, so byte windows equal
//! grapheme windows. Grams are `&str` slices of the caller's input —
//! no per-gram heap allocation — which lifted the ASCII arm ~10× over
//! the pre-borrow baseline (`~55 MiB/s`, bounded by `String::from` per
//! gram). The `grapheme / utf8` row still routes through ICU4X for
//! boundary detection but no longer concatenates cluster slices on
//! every gram — a single up-front boundary walk feeds `&str` window
//! slices — which is where the `~2-3×` lift over the pre-borrow slow
//! path (`~35-40 MiB/s`) comes from.
//!
//! Read:
//!
//! * **[`byte_ngrams`] is memory-bound** — it's [`slice::windows`],
//!   each yielded window forced through `black_box` to defeat the
//!   `.count()` const-fold. This is the throughput ceiling; every
//!   other unit's slowdown against it is boundary-detection cost.
//! * **[`char_ngrams`] is ~15-20× behind byte** because it does a
//!   per-scalar `char_indices` walk to gather boundaries; the walk
//!   is the load-bearing cost, `n` barely affects the number.
//! * **`grapheme_ngrams` (ASCII fast-path) sits above `char_ngrams`**
//!   because it bypasses the boundary walk entirely — one up-front
//!   `is_ascii()` + `\r` scan and the iterator is a byte-window over
//!   the input. `n = 3` and `n = 5` land within noise of each other.
//! * **`grapheme_ngrams` (utf8 slow-path) is ~2-3× behind `char`**
//!   because the ICU4X grapheme-cluster iterator is where the time
//!   goes. This is expected — grapheme boundaries are a Unicode
//!   algorithm, not a scalar-boundary walk — and the arm is only
//!   relevant when the input carries combining marks / ZWJ sequences /
//!   regional-indicator flags where character grams would slice a
//!   user-visible glyph in half. Boundary offsets are collected once
//!   up-front so each yielded gram is a plain `&str` slice — no
//!   per-gram concat, no allocation.
//! * **Per-cell variance is high** at 256 B input — 10-sample runs
//!   here have wide confidence intervals; the 4 KiB row is the more
//!   trustworthy number for regression tracking. Rerun with
//!   `--sample-size 30` for tighter bounds.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression or
//!   a measurement environment change; rerun with `--sample-size 30`
//!   before filing a fix.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod bytes;
pub mod chars;
#[cfg(feature = "graphemes")]
pub mod graphemes;
pub mod tokens;

pub use bytes::{byte_ngrams, byte_ngrams_padded};
pub use chars::{char_ngrams, char_ngrams_padded};
#[cfg(feature = "graphemes")]
pub use graphemes::{grapheme_ngrams, grapheme_ngrams_padded};
pub use tokens::{token_ngrams, token_ngrams_padded};

/// The semantic unit an n-gram operates on.
///
/// Mostly for callers who want to select a producer at runtime; the
/// per-unit free functions are the primary API and are typed to
/// their return shape (bytes → `&[u8]`, everything else → `&str` or
/// `&[&str]`), which this enum can't collapse without erasing
/// them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NGramUnit {
    /// Sliding window over UTF-8 bytes. Grams are `&[u8]`.
    Bytes,
    /// Sliding window over Unicode code points. Grams are `&str`.
    Chars,
    /// Sliding window over extended grapheme clusters. Grams are
    /// `&str`. Requires the `graphemes` feature.
    Graphemes,
    /// Sliding window over caller-supplied tokens. Grams are
    /// `&[&str]`.
    Tokens,
}
