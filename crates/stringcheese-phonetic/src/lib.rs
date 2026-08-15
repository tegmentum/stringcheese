//! Phonetic encoding and matching for the StringCheese toolkit.
//!
//! # Phonetics as a first-class subsystem
//!
//! Phonetic matching is not just "another comparison function". It is a
//! two-stage process — **encode** an input into a phonetic key, then
//! **compare** keys — with metadata requirements (language, script, region)
//! and a modularity story (language-specific rule tables live in feature-gated
//! sibling crates) that no other StringCheese subsystem shares. This crate
//! implements the language-agnostic infrastructure and the three English-only
//! encoders that constitute the 0.1 baseline:
//!
//! * [`Soundex`] — the 1918 NARA/American Soundex, the entity-resolution
//!   workhorse for a century of US census indexing and countless downstream
//!   record-linkage systems.
//! * [`DoubleMetaphone`] — Lawrence Philips' 1999 revision of Metaphone,
//!   supporting up to two keys per input to reflect regional pronunciation
//!   variance. This crate ships both variants: a primary-only variant
//!   ([`DoubleMetaphoneVariant::PrimaryOnly`]) that produces the primary
//!   key only, and a full variant ([`DoubleMetaphoneVariant::Full`]) that
//!   additionally computes the alternate key.
//! * [`Nysiis`] — Robert L. Taft's 1970 New York State Identification and
//!   Intelligence System encoder, developed for the New York State Division
//!   of Criminal Justice and still widely deployed in record-linkage tooling.
//!
//! See [`docs/design/phonetic-subsystem.md`][phon] for the definitive
//! specification.
//!
//! [phon]: https://github.com/tegmentum/stringcheese/blob/main/docs/design/phonetic-subsystem.md
//!
//! # Encoding vs comparison
//!
//! Every phonetic match is two steps, and StringCheese keeps them separately
//! identifiable:
//!
//! 1. A [`PhoneticEncoder`] transforms input into a phonetic key.
//! 2. A [`PhoneticMatcher`] wraps an encoder and decides whether two inputs
//!    match by key equality.
//!
//! The two stages compose. Different encoders can share a matcher; different
//! matchers can share an encoder. A discrepancy against another library can
//! be localized: if the encoded key agrees, the matcher disagrees; if the
//! encoded key disagrees, the encoder does.
//!
//! # Language and applicability
//!
//! Every encoder declares its [`Applicability`] — the languages, scripts, and
//! regions its rules were designed for — as a `const` on the encoder type.
//! Building a pipeline that feeds French input to an English-only encoder is
//! not automatically prevented (the type system cannot know a string's
//! language), but the mismatch is inspectable via `encoder.applicability()`
//! and can be surfaced in explainability output.
//!
//! All three encoders in this initial delivery are English-only. Future
//! language packs (per the phonetic-subsystem design) will live in sibling
//! crates: `stringcheese-phonetic-germanic`, `stringcheese-phonetic-romance`,
//! `stringcheese-phonetic-slavic`, and so on. This crate's facade will re-export
//! the packs selected by Cargo features so a minimal Wasm build for an
//! English-only entity-resolution workload never carries the Cyrillic
//! transliteration tables.
//!
//! TODO: multilingual packs — the sibling crates listed above are on the
//! roadmap for version 0.2.
//!
//! # Sequence type
//!
//! Phonetic encoders consume `&str` — not `&[u8]` or `&[char]` — because the
//! algorithms are defined in terms of *letters* rather than raw bytes or
//! Unicode scalars. Callers pass strings directly; the encoders filter to
//! ASCII letters as their first step. Non-ASCII input is treated per the
//! per-algorithm rules (typically: non-letters are stripped; non-ASCII
//! letters are stripped too, since none of the three algorithms defines
//! behavior for characters outside the English alphabet).
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every encoder returns owned
//! `String`s (or a small struct containing them), so **the entire public
//! surface is behind the `alloc` feature.** Under `--no-default-features`
//! the crate compiles to an empty module, which is what makes the crate
//! safe to add as a dependency in embedded configurations that only need
//! to link against the substrate crates.
//!
//! # Structure
//!
//! * [`encoder`] — the [`PhoneticEncoder`] trait and the [`Applicability`] /
//!   [`LanguageTag`] / [`ScriptTag`] / [`RegionTag`] metadata types.
//! * [`comparator`] — [`PhoneticMatcher`], the encoder-plus-key-equality
//!   comparator, with an [`MatchMode`] enum controlling the multi-key
//!   cross-product rule.
//! * [`soundex`] — the [`Soundex`] encoder.
//! * [`double_metaphone`] — the [`DoubleMetaphone`] encoder and its
//!   [`DoubleMetaphoneKey`] output type.
//! * [`nysiis`] — the [`Nysiis`] encoder.
//! * [`mod@slavic_metaphone`] — the [`SlavicMetaphone`] encoder, a
//!   variable-length Metaphone-shaped encoder for the Slavic language
//!   family across both Cyrillic and Latin scripts.
//!
//! # Benchmarks
//!
//! A criterion bench harness lives at `benches/phonetic.rs`; run it
//! with
//!
//! ```text
//! cargo bench -p stringcheese-phonetic
//! ```
//!
//! Five groups drive every encoder the crate exports (Soundex, NYSIIS,
//! Double Metaphone primary-only, Double Metaphone full, and Slavic
//! Metaphone). Each group runs three input character counts
//! (32 / 256 / 2048) crossed with two flavors: `ascii` (deterministic
//! random `[a-z]` bytes — the English-encoder hot path) and `utf8`
//! (Cyrillic-weighted diverse-Unicode input — the [`SlavicMetaphone`]
//! hot path and the strip-and-scan cost for the English-only encoders).
//!
//! # Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 2 --sample-size 10`).
//! Wall-clock samples vary ±10-20 % on a laptop under load; treat the
//! order-of-magnitude column as the load-bearing part, not the last
//! digit. Throughput reported over *input bytes* — higher is better.
//!
//! ```text
//! group                                     32          256        2048
//! -----------------------------------------------------------------------
//! soundex                     / ascii     1.45 GiB/s  9.30 GiB/s  69.6 GiB/s
//! soundex                     / utf8      1.40 GiB/s  10.1 GiB/s  43.9 GiB/s
//! nysiis                      / ascii      192 MiB/s   260 MiB/s   265 MiB/s
//! nysiis                      / utf8      1.13 GiB/s   755 MiB/s  1.29 GiB/s
//! double_metaphone_primary    / ascii      210 MiB/s   572 MiB/s   635 MiB/s
//! double_metaphone_primary    / utf8       483 MiB/s  1.29 GiB/s  1.87 GiB/s
//! double_metaphone_full       / ascii      127 MiB/s   312 MiB/s   507 MiB/s
//! double_metaphone_full       / utf8       601 MiB/s   736 MiB/s   947 MiB/s
//! slavic_metaphone            / ascii     88.3 MiB/s   129 MiB/s   117 MiB/s
//! slavic_metaphone            / utf8      90.7 MiB/s   145 MiB/s   129 MiB/s
//! ```
//!
//! Read:
//!
//! * **Soundex is by far the fastest.** The algorithm collapses to at
//!   most four output characters, so past the first four letters most
//!   input bytes are skipped after a table lookup. Reported input-bytes
//!   throughput is misleading in the same way `manip/trim/trim` is —
//!   the O(4) output cap means the effective work per byte falls as
//!   the input grows, and the "GiB/s" numbers should be read as an
//!   *upper bound* the trip-wire needs to see, not a sustainable
//!   phonetic-encoding rate.
//! * **NYSIIS on utf8 is faster than NYSIIS on ascii** at the same
//!   character count. Both flavors take the same ASCII-letter fast
//!   path, but the utf8 pool is Cyrillic-heavy — those bytes are
//!   *skipped* in the strip step, so a 2048-char utf8 input carries
//!   ~4 KiB of bytes that count against the throughput denominator
//!   without exercising the O(n) NYSIIS rewrite rules. Same reading
//!   caveat as Soundex: the numerator is real, but the denominator
//!   isn't the work.
//! * **`double_metaphone_full` vs `_primary`**: the full-two-key
//!   variant additionally computes the alternate branch at every
//!   documented divergence point, so it's consistently ~1.5-2× slower
//!   than the primary-only variant. Callers who don't need the
//!   alternate key should prefer `primary_only()`.
//! * **`slavic_metaphone` is the throughput floor** at ~100-150 MiB/s
//!   because its rule table is the biggest (12 languages × Cyrillic +
//!   Latin arms) and every input character resolves through a
//!   variable-length replacement table rather than a single
//!   `match`-emit step. That is expected for a per-family Metaphone
//!   encoder; a future round could look at splitting the Cyrillic /
//!   Latin arms into script-specific tables so the hot path stays in
//!   one arm.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15-20 %. A number outside
//!   that band on a subsequent run is either a genuine regression or
//!   a measurement environment change (thermal throttling, background
//!   load); rerun with `--sample-size 30` and a quiet host before
//!   filing a fix.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod comparator;
#[cfg(feature = "alloc")]
pub mod double_metaphone;
#[cfg(feature = "alloc")]
pub mod encoder;
#[cfg(feature = "alloc")]
pub mod nysiis;
#[cfg(feature = "alloc")]
pub mod slavic_metaphone;
#[cfg(feature = "alloc")]
pub mod soundex;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
#[cfg(not(target_family = "wasm"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use comparator::{MatchMode, PhoneticMatcher};
#[cfg(feature = "alloc")]
pub use double_metaphone::{DoubleMetaphone, DoubleMetaphoneKey, DoubleMetaphoneVariant};
#[cfg(feature = "alloc")]
pub use encoder::{Applicability, LanguageTag, PhoneticEncoder, RegionTag, ScriptTag};
#[cfg(feature = "alloc")]
pub use nysiis::Nysiis;
#[cfg(feature = "alloc")]
pub use slavic_metaphone::{
    DEFAULT_MAX_KEY_LEN, HARD_MAX_KEY_LEN, SlavicMetaphone, SlavicMetaphoneOptions,
    slavic_metaphone,
};
#[cfg(feature = "alloc")]
pub use soundex::Soundex;
