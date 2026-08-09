//! The English stopword list.
//!
//! The list contains ~150 common English words drawn from the union
//! of the traditional van Rijsbergen list (as taught in the classic
//! IR literature), NLTK's `english` list, and scikit-learn's
//! `ENGLISH_STOP_WORDS`. Modest by design — no domain-specific
//! jargon, no archaic forms.
//!
//! # Source of truth
//!
//! The list is **generated** at build time from `rules/en.toml` by
//! `stringcheese-lang-gen`. This module re-exports the generated
//! `STOPWORDS` slice under its historical path so downstream callers
//! continue to reach it as `stringcheese_en::stopwords::STOPWORDS`.
//! Editing the list means editing `rules/en.toml`; touching this
//! module has no runtime effect.
//!
//! # Non-goals (unchanged)
//!
//! - **Contraction fragments.** Words like `n't`, `'ll`, `'re`,
//!   `'ve` are absent because the shipped tokenizer does not split
//!   contractions.
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific corpora extends the general list; downstream
//!   applications carry their own.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so
//!   `"the"`, `"The"`, and `"THE"` all match.

/// The English stopword list.
///
/// A `&'static [&'static str]` — [`Language::stopwords`] hands back
/// exactly this slice.
///
/// [`Language::stopwords`]: stringcheese_lang::Language::stopwords
pub const STOPWORDS: &[&str] = crate::CAPABILITIES.stopwords;
