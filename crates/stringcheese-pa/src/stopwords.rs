//! The Punjabi (Eastern, Gurmukhi script) stopword list.
//!
//! ~55 common Punjabi function words: personal / demonstrative /
//! interrogative pronouns, postpositions, conjunctions, particles, the
//! high-frequency forms of the copula *ਹੋਣਾ* ("to be"), and common
//! adverbs. The list is drawn from the intersection of several open
//! Punjabi stopword corpora; it deliberately omits content words.
//!
//! # Gurmukhi script — a note on storage width
//!
//! Every entry here is written in the Gurmukhi script. Gurmukhi
//! scalars live in the range **U+0A00..=U+0A7F**, which falls in
//! UTF-8's 3-byte range (U+0800..=U+FFFF). A word like `"ਮੈਂ"` (3
//! characters: `ਮ` + `ੈ` + `ਂ`) is **9 bytes**. Any code that mixes
//! byte offsets with character-boundary logic will silently corrupt
//! token or entry boundaries. The stopword list itself is a plain
//! `&[&str]` of static strings, so this concern only surfaces in
//! downstream iteration; every consumer in this crate walks scalars
//! via [`str::chars`] or [`Vec<char>`], not raw bytes.
//!
//! # Case folding
//!
//! Punjabi has no case distinction. The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Gurmukhi scalars (they are non-ASCII and pass through
//! the comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Gurmukhi input.
//!
//! # Non-goals
//!
//! - **Shahmukhi stopwords.** Punjabi in Pakistan is written in a
//!   Perso-Arabic derivative called Shahmukhi. The function-word
//!   inventory overlaps with Gurmukhi Punjabi but the surface
//!   spellings are entirely different; a `stringcheese-pa-arab`
//!   companion pack would carry those.
//! - **Colloquial / regional forms.** Written Punjabi (Majhi /
//!   standard) diverges from regional dialects (Doabi, Malwai,
//!   Puadhi, Pothohari) in the surface form of many function words.
//!   The list targets the written / newswire register.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Punjabi stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is a Gurmukhi string.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "ਮੈਂ",     // I
    "ਮੈਨੂੰ",    // to me
    "ਮੇਰਾ",   // my (masc)
    "ਮੇਰੀ",   // my (fem)
    "ਅਸੀਂ",   // we
    "ਸਾਡਾ",  // our (masc)
    "ਸਾਡੀ",  // our (fem)
    "ਤੂੰ",     // you (familiar)
    "ਤੁਸੀਂ",   // you (polite / plural)
    "ਤੁਹਾਨੂੰ",  // to you (polite / plural)
    "ਤੇਰਾ",   // your (familiar, masc)
    "ਤੇਰੀ",   // your (familiar, fem)
    "ਤੁਹਾਡਾ", // your (polite, masc)
    "ਓਹ",    // he / she / they (distal)
    "ਉਹ",    // that / he / she
    "ਉਸ",    // him / her (oblique)
    "ਉਹਨਾਂ",  // they / them
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "ਇਹ",   // this
    "ਇਸ",   // this (oblique)
    "ਇਹਨਾਂ", // these
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "ਕੀ",    // what
    "ਕੌਣ",    // who
    "ਕਿਉਂ",   // why
    "ਕਿਵੇਂ",   // how
    "ਕਿੱਥੇ",   // where
    "ਕਦੋਂ",    // when
    "ਕਿਹੜਾ", // which
    // -----------------------------------------------------------------
    // Postpositions (independent orthographic words).
    // -----------------------------------------------------------------
    "ਦਾ",  // of (genitive, masc sg)
    "ਦੇ",   // of (genitive, masc pl / obl)
    "ਦੀ",  // of (genitive, fem sg)
    "ਦੀਆਂ", // of (genitive, fem pl)
    "ਨੂੰ",   // accusative / dative (to)
    "ਵਿੱਚ", // in / inside
    "ਤੋਂ",   // from
    "ਨਾਲ", // with
    "ਲਈ",  // for
    "ਉੱਤੇ",  // on / above
    "ਹੇਠਾਂ", // below / under
    // -----------------------------------------------------------------
    // Coordinating / subordinating conjunctions.
    // -----------------------------------------------------------------
    "ਅਤੇ",    // and
    "ਜਾਂ",    // or
    "ਪਰ",    // but
    "ਜੇ",     // if
    "ਜੇਕਰ",   // if (formal)
    "ਤਾਂ",    // then
    "ਕਿਉਂਕਿ", // because
    "ਜੋ",     // that / who (relative)
    // -----------------------------------------------------------------
    // Negation.
    // -----------------------------------------------------------------
    "ਨਹੀਂ", // no / not
    "ਨਾ",  // not (prohibitive)
    // -----------------------------------------------------------------
    // Copula (ਹੋਣਾ) — high-frequency forms. `ਹਾਂ` doubles as the
    // affirmative particle "yes"; listed once.
    // -----------------------------------------------------------------
    "ਹੈ",  // is (3sg)
    "ਹਾਂ", // am / yes
    "ਹੈਂ",  // are (2sg familiar)
    "ਹੋ",  // are (2pl polite)
    "ਹਨ", // are (3pl)
    "ਸੀ", // was (3sg)
    "ਸਨ", // were (3pl)
    // -----------------------------------------------------------------
    // Common adverbs and quantifiers.
    // -----------------------------------------------------------------
    "ਸਭ",  // all
    "ਸਾਰੇ", // all / whole
    "ਕੁਝ",  // some / something
    "ਬਹੁਤ", // very / much
    "ਹੁਣ",  // now
    "ਫਿਰ", // then / again
    "ਇੱਥੇ",  // here
    "ਉੱਥੇ",  // there
    "ਇੱਕ",  // one
    "ਦੋ",   // two
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~55" — assert we're in that
        // ballpark. The task spec asks for ~50 entries.
        assert!(
            STOPWORDS.len() >= 45 && STOPWORDS.len() <= 150,
            "STOPWORDS.len() = {} outside the advertised 45-150 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_non_empty() {
        for &w in STOPWORDS {
            assert!(!w.is_empty(), "empty stopword entry");
        }
    }

    #[test]
    fn every_stopword_contains_gurmukhi() {
        // Sanity check: every stopword should contain at least one
        // scalar in the Gurmukhi block U+0A00..=U+0A7F.
        for &w in STOPWORDS {
            let has_gurmukhi = w.chars().any(|c| ('\u{0A00}'..='\u{0A7F}').contains(&c));
            assert!(has_gurmukhi, "stopword {w:?} has no Gurmukhi scalar");
        }
    }

    #[test]
    fn no_entries_contain_ascii_whitespace() {
        // Multi-word phrases would never match a single token; keep the
        // list to single orthographic words.
        for &w in STOPWORDS {
            assert!(
                !w.chars().any(char::is_whitespace),
                "stopword {w:?} contains whitespace — must be a single word"
            );
        }
    }

    #[test]
    fn stopwords_contain_core_pronouns() {
        for &w in &["ਮੈਂ", "ਤੂੰ", "ਤੁਸੀਂ", "ਅਸੀਂ", "ਓਹ"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["ਅਤੇ", "ਜਾਂ", "ਪਰ", "ਜੇ"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_common_to_be_forms() {
        for &w in &["ਹੈ", "ਹਾਂ", "ਸੀ"] {
            assert!(STOPWORDS.contains(&w), "to-be form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation_particles() {
        for &w in &["ਨਹੀਂ", "ਨਾ"] {
            assert!(STOPWORDS.contains(&w), "negation particle {w:?} missing");
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~55.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
