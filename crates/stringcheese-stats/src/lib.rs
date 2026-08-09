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
//! ## Baseline (2026-08-09)
//!
//! Numbers from `stringcheese-bench/benches/stats.rs` on random
//! ASCII input:
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
