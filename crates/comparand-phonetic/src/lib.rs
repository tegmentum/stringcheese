//! Phonetic encoding and matching for the Comparand toolkit.
//!
//! # Phonetics as a first-class subsystem
//!
//! Phonetic matching is not just "another comparison function". It is a
//! two-stage process — **encode** an input into a phonetic key, then
//! **compare** keys — with metadata requirements (language, script, region)
//! and a modularity story (language-specific rule tables live in feature-gated
//! sibling crates) that no other Comparand subsystem shares. This crate
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
//! [phon]: https://github.com/zacharywhitley/comparand/blob/main/docs/design/phonetic-subsystem.md
//!
//! # Encoding vs comparison
//!
//! Every phonetic match is two steps, and Comparand keeps them separately
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
//! crates: `comparand-phonetic-germanic`, `comparand-phonetic-romance`,
//! `comparand-phonetic-slavic`, and so on. This crate's facade will re-export
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
pub mod soundex;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
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
pub use soundex::Soundex;
