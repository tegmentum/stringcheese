//! # String statistics and characterization
//!
//! Small primitives for asking "what does this string look like?"
//! without committing to a downstream operation. The characterisation
//! feeds decisions — which tokenizer, which normaliser, which
//! fallback — that would otherwise require a full parse.
//!
//! Each primitive is deliberately narrow:
//!
//! - [`entropy()`] — Shannon entropy over Unicode code points, in
//!   bits per code point.
//! - [`histogram`] — full Unicode general-category tally
//!   (`Lu`, `Ll`, `Nd`, `Po`, …), plus the coarse major-category
//!   roll-up (Letter / Mark / Number / Punctuation / Symbol /
//!   Separator / Other).
//! - [`Ratios`] — printable / control / whitespace / digit /
//!   alphabetic / punctuation ratios in `[0.0, 1.0]`.
//! - [`Lengths`] — byte / code-point / (optional) grapheme count
//!   for a string.
//!
//! ## Explicit units
//!
//! Every function documents which Unicode unit it operates on.
//! Entropy is over code points (byte-level entropy is arithmetically
//! trivial from `.bytes()`; ambiguity is not worth adding). The
//! histogram walks code points. Ratios are per-code-point. Lengths
//! carries all three.
//!
//! ## Benchmarks
//!
//! An in-crate criterion bench harness lives at `benches/stats.rs`;
//! run it with
//!
//! ```text
//! cargo bench -p stringcheese-stats
//! ```
//!
//! Four groups drive every public entry point ([`entropy()`],
//! [`Histogram::of`], [`Ratios::of`], [`Lengths::of`]). Each group
//! runs three input byte lengths (1 KiB / 10 KiB / 100 KiB) crossed
//! with two flavors (`ascii` prose and a diverse `utf8` pool). 24
//! measurement points total.
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
//! primitive   / flavor    1 KiB         10 KiB        100 KiB
//! ----------------------------------------------------------------
//! entropy     / ascii      340 MiB/s     393 MiB/s     355 MiB/s
//! entropy     / utf8       361 MiB/s     628 MiB/s     673 MiB/s
//! histogram   / ascii      418 MiB/s     460 MiB/s     461 MiB/s
//! histogram   / utf8       449 MiB/s     458 MiB/s     442 MiB/s
//! ratios      / ascii      638 MiB/s     658 MiB/s     644 MiB/s
//! ratios      / utf8      69.4 MiB/s    65.9 MiB/s    71.3 MiB/s
//! lengths     / ascii     19.6 GiB/s    20.8 GiB/s    19.0 GiB/s
//! lengths     / utf8      19.5 GiB/s    21.8 GiB/s    21.5 GiB/s
//! ```
//!
//! Read:
//!
//! * **`lengths` is memory-bound.** `str::len` is O(1) and
//!   `chars().count()` is a byte-sequential UTF-8 scan; both flavors
//!   hit multi-GiB/s and the ratio between them is noise.
//! * **`ratios` shows the biggest ASCII/UTF-8 gap** (~9×). The ASCII
//!   fast path is a 128-entry `ASCII_CLASS` lookup table + six
//!   bit-tests per byte; the UTF-8 path goes through the full
//!   `unicode-general-category` table per scalar. The gap is the
//!   documented cost of the fast path, not a regression.
//! * **`entropy` on utf8 is faster than on ascii.** Both flavors
//!   hash-count per-code-point; the utf8 pool is 15 tokens vs
//!   ASCII's 34 words, so the observed alphabet is smaller and the
//!   `HashMap` slots hit fewer distinct entries. This is a corpus
//!   effect, not an algorithmic one.
//! * **`histogram` is flat across flavors.** The ASCII sub-array
//!   accumulator and the general `HashMap` path both cost ~2 ns per
//!   char, so the per-byte throughput tracks the average bytes-per-
//!   char of the input pool (which is ~1 for ASCII and ~2 for utf8).
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression or
//!   a measurement environment change (thermal throttling, background
//!   load); rerun with `--sample-size 30` and a quiet host before
//!   filing a fix.
//!
//! ## Prior baseline (2026-08-09, `stringcheese-bench/benches/stats.rs`)
//!
//! Retained for context — the earlier ASCII-only rows against which
//! the current suite's numbers should be compared:
//!
//! | Primitive      | 128 B     | 1 KB      | 8 KB      |
//! |----------------|-----------|-----------|-----------|
//! | `Lengths::of`  | 17 GiB/s  | 22 GiB/s  | 23 GiB/s  |
//! | `Ratios::of`   | 683 MiB/s | 700 MiB/s | 713 MiB/s |
//! | `Histogram::of`| 466 MiB/s | 476 MiB/s | 476 MiB/s |
//! | `entropy`      | 160 MiB/s | 360 MiB/s | 348 MiB/s |
//!
//! `Lengths` is memory-bound (as expected — `str::len` is O(1)
//! and `chars().count()` is byte-sequential). `Ratios` at
//! ~700 MiB/s leads the character-scanning primitives thanks to
//! a bench-driven redesign: an ASCII fast-path lookup table
//! packs six classification flags into one `u8`, so the hot path
//! is one table lookup + six bit-tests per ASCII byte (2× the
//! previous 340-425 MiB/s baseline). `Histogram` at ~476 MiB/s
//! sits second after the same fix pattern: ASCII bytes
//! accumulate into a small fixed-size array indexed by the
//! ASCII sub-category, and the array merges into the `HashMap`
//! once at the end — ~30 % faster than the naive `.entry()`
//! per char (before: 350-380 MiB/s). `entropy` stays around
//! 350 MiB/s; it needs a per-char hash of the actual code point
//! (not just its category), so the same trick doesn't apply.
//!
//! An earlier `entropy` implementation used `BTreeMap<char, u64>`
//! and dropped to 67 MiB/s at 8 KB inputs — `.entry().or_insert()`
//! per-char cost climbed with tree depth as the observed alphabet
//! filled. The current `hashbrown::HashMap` implementation is
//! ~5× faster at 8 KB and flat vs. input size. Recorded here so
//! the design tradeoff (map choice → scaling curve) doesn't
//! silently regress.
//!
//! ## Bench-driven negative result: NEON did NOT help `Ratios::of`
//!
//! An aarch64 NEON implementation was prototyped: 16-byte
//! vectorised loads, five range-based classifications in-lane
//! (`printable`/`control`/`whitespace`/`digit`/`alphabetic`),
//! per-flag popcount into running counters. **Bench measured a
//! 30 % regression** at every input size (700 → 540 MiB/s).
//!
//! Three concrete causes surfaced:
//!
//! 1. ASCII punctuation is scattered (`! " # % & ' ( ) * , - . /
//!    : ; ? @ [ \ ] _ { }`), can't be range-classified with a
//!    handful of compares. The prototype fell back to a per-byte
//!    scalar loop inside every SIMD chunk — cancelling most of
//!    the vectorisation win.
//! 2. Six `vaddvq_u8` cross-lane reductions per 16-byte chunk
//!    are expensive on aarch64. Deferred reduction (`uint8x16_t`
//!    accumulators across ~200 chunks then a single reduction)
//!    was not landed here; would be needed to reclaim the win.
//! 3. Modern scalar CPUs with an L1-resident 128-byte lookup
//!    table are already very fast — the scalar baseline was
//!    ~700 MiB/s. The SIMD ceiling was competitive at best.
//!
//! Kept the scalar `ASCII_CLASS` table + `is_ascii_punctuation`
//! path. A future SIMD attempt would need: nibble-decomposition
//! byte-classification (Langdale/Lemire), deferred reduction,
//! and per-target backends (AVX2 / SSE2 / NEON / SIMD128). That
//! is a substantial engineering arc; the current scalar shape
//! is the sweet spot for the effort budget.
//!
//! ## Example
//!
//! ```
//! use stringcheese_stats::{entropy, Ratios};
//!
//! let e = entropy("hello world");
//! assert!(e > 2.5 && e < 3.5);
//!
//! let r = Ratios::of("Hello, World!");
//! assert!(r.alphabetic > 0.7);
//! assert!(r.punctuation < 0.2);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
// Statistical primitives cast integer counts to f64 for division;
// the resulting ratio is what the caller wants, not a lossless
// integer conversion. Same story for `assert_eq!(x, 0.0)` in the
// tests — 0.0 and 1.0 are exactly representable in IEEE 754, and
// the equality is the assertion, not an accident.
#![allow(clippy::cast_precision_loss, clippy::float_cmp)]

#[cfg(feature = "alloc")]
extern crate alloc;

// `entropy` needs `f64::log2` which lives on the trait `num_traits::Float`
// in no_std or on the `libm` crate — but in std it's an inherent method.
// Gating the whole module on `std` keeps the crate dep-lean (no `libm` in
// the no_std build) while histogram/ratios/lengths stay available under
// `alloc`-only. A caller who needs entropy without std adds `libm` and
// takes the trait method themselves; this is documented behaviour, not a
// silent limitation.
#[cfg(feature = "std")]
pub mod entropy;
pub mod histogram;
pub mod lengths;
pub mod ratios;

#[cfg(feature = "std")]
pub use entropy::entropy;
pub use histogram::{Histogram, MajorCategory};
pub use lengths::Lengths;
pub use ratios::Ratios;
