//! The Italian stopword list.
//!
//! An MVP list of roughly 30 hand-curated high-frequency Italian
//! words: articles, prepositions, coordinating conjunctions, the
//! high-frequency personal pronouns, and the bare-infinitive forms
//! of the most common auxiliary and modal verbs.
//!
//! This is a **starter list**, not the full published Italian IR
//! stopword set — the Snowball project's `italian/stop.txt`
//! distribution runs to ~280 entries with the full paradigms of
//! `essere` / `avere` / `stare` / `fare`. Extending this list to
//! that level of coverage is a documented follow-up; it belongs
//! with the Snowball Italian stemmer work (which requires a
//! Snowball binding not currently vendored into the workspace).
//!
//! # Accented characters
//!
//! Italian uses the grave accent on final stressed vowels
//! (`caffè`, `città`, `perché`) and the acute on `é`. None of the
//! ~30 entries in this MVP list carries a diacritic — every entry
//! is ASCII — so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (which uses [`str::eq_ignore_ascii_case`])
//! is correct without an override.
//!
//! # Non-goals
//!
//! - **Full paradigms.** No `sono` / `sei` / `è` / `siamo` / `siete`
//!   split-out for `essere`; no `ho` / `hai` / `ha` / `abbiamo` /
//!   `avete` / `hanno` split-out for `avere`. This starter list
//!   ships bare infinitives only.
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific corpora typically extends the general list.
//! - **Regional dialects.** Sicilian / Sardinian / Neapolitan
//!   variants are separate CLDR locales (`scn`, `sc`, `nap`) with
//!   their own vocabulary and are not covered here.

/// The Italian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
#[rustfmt::skip]
pub const STOPWORDS: &[&str] = &[
    // Definite and indefinite articles.
    "il", "la", "lo", "i", "gli", "le", "un", "uno", "una",
    // Common prepositions.
    "di", "a", "da", "in", "con", "su", "per", "tra", "fra",
    // Coordinating and disjunctive conjunctions.
    "e", "o", "ma", "che",
    // High-frequency personal pronouns.
    "io", "tu", "noi", "voi", "mi", "si",
    // Negation.
    "non",
    // Adverbs.
    "come",
    // Bare-infinitive auxiliary and modal verbs.
    "essere", "avere", "fare", "dire", "andare", "potere", "dovere", "volere", "sapere",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~30" — assert we're in the
        // ballpark. Range is loose to accommodate future MVP tweaks.
        assert!(
            STOPWORDS.len() >= 25 && STOPWORDS.len() <= 60,
            "STOPWORDS.len() = {} outside the advertised ~30 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase() {
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !c.is_uppercase(),
                    "stopword {w:?} contains an uppercase character"
                );
            }
        }
    }

    #[test]
    fn every_stopword_is_ascii() {
        // The MVP list is deliberately ASCII-only so the default
        // ASCII-case-insensitive `is_stopword` lookup is complete.
        for &w in STOPWORDS {
            assert!(w.is_ascii(), "stopword {w:?} carries non-ASCII bytes");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~30.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_articles_are_present() {
        for w in ["il", "la", "lo", "i", "gli", "le", "un", "una"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["di", "a", "da", "in", "con", "su", "per"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_and_negation_are_present() {
        for w in ["e", "o", "ma", "che", "non"] {
            assert!(
                STOPWORDS.contains(&w),
                "conjunction/negation {w:?} is missing"
            );
        }
    }

    #[test]
    fn common_auxiliary_and_modal_verbs_are_present() {
        for w in [
            "essere", "avere", "fare", "potere", "dovere", "volere", "sapere",
        ] {
            assert!(STOPWORDS.contains(&w), "verb {w:?} is missing");
        }
    }
}
