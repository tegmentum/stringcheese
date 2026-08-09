//! The Hindi stopword list.
//!
//! ~150 personal / demonstrative / interrogative pronouns,
//! postpositions (Hindi uses them as separate tokens), the copula
//! *होना* (to be) surface forms, the auxiliary *करना* (to do),
//! common adverbs, and negation particles. Every entry is stored
//! in NFC-normalized Devanagari.
//!
//! # Source of truth
//!
//! The list is **generated** at build time from `rules/hi.toml` by
//! `stringcheese-lang-gen`. This module re-exports the generated
//! `STOPWORDS` slice under its historical path so downstream callers
//! continue to reach it as `stringcheese_hi::stopwords::STOPWORDS`.
//! Editing the list means editing `rules/hi.toml`; touching this
//! module has no runtime effect.
//!
//! # Non-goals (unchanged)
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific Hindi corpora extends the general list;
//!   downstream applications carry their own.
//! - **Colloquial / regional forms.** The list uses standard
//!   Modern Hindi (शुद्ध हिन्दी). Bombay / Bhojpuri /
//!   Punjabi-influenced colloquial variants aren't included.
//! - **NFC normalization at query time.** Callers whose input might
//!   be in NFD Devanagari should normalize before the membership
//!   check — the stored list is NFC by construction.

/// The Hindi stopword list.
///
/// A `&'static [&'static str]` — [`Language::stopwords`] hands back
/// exactly this slice.
///
/// [`Language::stopwords`]: stringcheese_lang::Language::stopwords
pub const STOPWORDS: &[&str] = crate::CAPABILITIES.stopwords;
