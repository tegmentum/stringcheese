//! The German stopword list.
//!
//! The list contains ~250 common German words drawn primarily from
//! NLTK's `german` list (which itself descends from the widely-cited
//! Snowball / Porter German stopword list, mirrored on
//! [snowballstem.org][ref]).
//!
//! [ref]: https://snowballstem.org/algorithms/german/stop.txt
//!
//! # Source of truth
//!
//! The list is **generated** at build time from `rules/de.toml` by
//! `stringcheese-lang-gen`. This module re-exports the generated
//! `STOPWORDS` slice under its historical path so downstream callers
//! continue to reach it as `stringcheese_de::stopwords::STOPWORDS`.
//! Editing the list means editing `rules/de.toml`; touching this
//! module has no runtime effect.
//!
//! # Non-goals (unchanged)
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific German corpora extends the general list;
//!   downstream applications carry their own.
//! - **Swiss / Austrian forms.** The list uses the standard German
//!   orthography reformed 1996; Swiss usage drops `ß` for `ss` and
//!   several regional forms of the pronouns and modals are not
//!   included. Swiss corpora need a tailored list.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so
//!   `"der"`, `"Der"`, and `"DER"` are all recognized. Note that
//!   this is ASCII-case-only — the umlaut vowels compare exactly,
//!   and words like `"Über"` need to be lower-cased through
//!   Unicode-aware case folding before the check if the caller
//!   wants full case insensitivity.

/// The German stopword list.
///
/// A `&'static [&'static str]` — [`Language::stopwords`] hands back
/// exactly this slice.
///
/// [`Language::stopwords`]: stringcheese_lang::Language::stopwords
pub const STOPWORDS: &[&str] = crate::CAPABILITIES.stopwords;
