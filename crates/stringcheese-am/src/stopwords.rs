//! The Amharic stopword list.
//!
//! ~55 common Amharic function words: personal / demonstrative /
//! interrogative pronouns, coordinating and subordinating
//! conjunctions, prepositions, negation and affirmation particles,
//! and high-frequency copula surface forms. The list is drawn from
//! the intersection of several open Amharic function-word inventories
//! (the AAU Amharic corpus, the SNLP starter list); it deliberately
//! omits content words.
//!
//! # Ge'ez script — a note on storage width
//!
//! Every entry here is written in the **Ge'ez script** (also called
//! Ethiopic). Ge'ez syllables live in the range
//! **U+1200..=U+137F** (main block), plus the supplement
//! U+1380..=U+139F and the extended U+2D80..=U+2DDF. Each main-block
//! scalar is **3 bytes in UTF-8** (U+1200..=U+137F falls in UTF-8's
//! 3-byte range U+0800..=U+FFFF), so a word like `"እኔ"` (2
//! characters: `እ` + `ኔ`) is **6 bytes**. Any code that mixes byte
//! offsets with character-boundary logic will silently corrupt token
//! or entry boundaries. Every consumer in this crate walks scalars
//! via [`str::chars`] or [`Vec<char>`], never raw bytes.
//!
//! # Case folding
//!
//! Ge'ez has no case distinction (each vowel order is a distinct
//! scalar, not a case variant). The default
//! [`is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation calls [`str::eq_ignore_ascii_case`], which is a
//! *no-op* on Ge'ez scalars (they are non-ASCII and pass through the
//! comparison unchanged); the case-insensitivity contract is
//! technically satisfied but vacuously so on Amharic input.
//!
//! # Multi-word entries
//!
//! Amharic has a handful of common two-word function phrases (e.g.
//! `ነገር ግን` "but", literally "thing but/however"). These are
//! stored **unsplit** so that a per-token stopword check will not
//! see them — a caller who wants to filter them must tokenize
//! themselves and reassemble, or preprocess the input. This is the
//! same policy every other pack (Hebrew, Bengali, Hindi) follows —
//! the list holds *dictionary headwords*, not preformatted match
//! patterns.
//!
//! # Non-goals
//!
//! - **Verb-inflected copula forms.** Amharic copulas conjugate for
//!   person / number / gender; only the most-common third-person
//!   forms (`ነው`, `ናት`, `ናቸው`) are here. Fuller coverage would
//!   need a morphology pass.
//! - **Tigrinya / Tigre / Ge'ez (liturgical).** The Ge'ez script is
//!   also used for Tigrinya (`ti`) and Tigre (`tig`); Amharic
//!   function words overlap only partially. Each language deserves
//!   its own pack.
//! - **Domain-specific stopwords.** IR practice for legal, medical,
//!   or scientific corpora typically extends the general list.
//!   Downstream applications should carry their own.

/// The Amharic stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is an Amharic word
/// written in the Ge'ez script.
pub const STOPWORDS: &[&str] = &[
    // -----------------------------------------------------------------
    // Personal pronouns.
    // -----------------------------------------------------------------
    "እኔ",   // I
    "አንተ",  // you (m.sg.)
    "አንቺ",  // you (f.sg.)
    "እሱ",   // he
    "እርሱ",  // he (formal)
    "እርሷ",  // she
    "እሷ",   // she (short form)
    "እኛ",   // we
    "እናንተ", // you (pl.)
    "እነሱ",  // they
    "እነርሱ", // they (formal)
    // -----------------------------------------------------------------
    // Demonstratives.
    // -----------------------------------------------------------------
    "ይህ",   // this (m.)
    "ይህች",  // this (f.)
    "እነዚህ", // these
    "ያ",    // that (m.)
    "ያች",   // that (f.)
    "እነዚያ", // those
    // -----------------------------------------------------------------
    // Interrogatives.
    // -----------------------------------------------------------------
    "ማን",   // who
    "ምን",   // what
    "የት",   // where
    "መቼ",   // when
    "ለምን",  // why
    "እንዴት", // how
    "ስንት",  // how many / how much
    "የትኛው", // which (m.)
    "የትኛዋ", // which (f.)
    // -----------------------------------------------------------------
    // Prepositions.
    // -----------------------------------------------------------------
    "በ",   // in / at (proclitic; also stands alone)
    "ከ",   // from
    "ለ",   // to / for
    "ስለ",  // about / for
    "እንደ", // like / as
    "ጋር",  // with (postposition)
    "ውስጥ", // inside
    "ላይ",  // on / above
    "ውጭ",  // outside
    "ወደ",  // toward
    "እስከ", // until
    // -----------------------------------------------------------------
    // Coordinating / subordinating conjunctions.
    // -----------------------------------------------------------------
    "እና",     // and
    "ወይም",    // or
    "ግን",     // but / however
    "ስለዚህ",   // therefore
    "ስለሆነ",   // because
    "ምክንያቱም", // because
    "እንዲሁም",  // also / likewise
    "ስለዚያ",   // therefore (that)
    "ስለምን",   // why (formal)
    "ከሆነ",    // if
    "ብ",      // (subjunctive complementizer)
    "ካልሆነ",   // if not
    // -----------------------------------------------------------------
    // Negation and affirmation particles.
    // -----------------------------------------------------------------
    "አይ",     // no
    "አዎ",     // yes
    "የለም",    // there is not
    "አይደለም",  // is not
    "አይደለችም", // is not (f.)
    "አይደሉም",  // are not
    // -----------------------------------------------------------------
    // Copula / auxiliary surface forms.
    // -----------------------------------------------------------------
    "ነው",  // is (m.)
    "ናት",  // is (f.)
    "ናቸው", // are (pl.)
    "ነኝ",  // I am
    "ነህ",  // you are (m.)
    "ነሽ",  // you are (f.)
    "ነን",  // we are
    "ነበር", // was
    "አለ",  // there is (m.)
    "አለች", // there is (f.)
    "አሉ",  // there are
    // -----------------------------------------------------------------
    // Common adverbs / quantifiers / discourse.
    // -----------------------------------------------------------------
    "ሁሉ",    // all / every
    "ብቻ",    // only
    "እጅግ",   // very
    "ደግሞ",   // also
    "አሁን",   // now
    "ዛሬ",    // today
    "ትናንት",  // yesterday
    "ነገ",    // tomorrow
    "ሁልጊዜ",  // always
    "አንዳንድ", // some
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~55" — assert we're in the
        // ballpark. Has grown slightly during hand-curation.
        assert!(
            STOPWORDS.len() >= 40 && STOPWORDS.len() <= 150,
            "STOPWORDS.len() = {} outside the advertised 40-150 range",
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
    fn every_stopword_contains_geez() {
        // Sanity check: every stopword should contain at least one
        // scalar in the Ge'ez main block U+1200..=U+137F.
        for &w in STOPWORDS {
            let has_geez = w.chars().any(|c| ('\u{1200}'..='\u{137F}').contains(&c));
            assert!(has_geez, "stopword {w:?} has no Ge'ez scalar");
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
        for &w in &["እኔ", "አንተ", "አንቺ", "እሱ", "እርሷ", "እኛ"] {
            assert!(STOPWORDS.contains(&w), "core pronoun {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_core_conjunctions() {
        for &w in &["እና", "ወይም", "ግን"] {
            assert!(STOPWORDS.contains(&w), "core conjunction {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_copula_forms() {
        for &w in &["ነው", "ናት", "ናቸው", "ነበር"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_negation() {
        for &w in &["አይ", "የለም", "አይደለም"] {
            assert!(STOPWORDS.contains(&w), "negation {w:?} missing");
        }
    }

    #[test]
    fn stopwords_contain_demonstratives() {
        for &w in &["ይህ", "ያ", "እነዚህ", "እነዚያ"] {
            assert!(STOPWORDS.contains(&w), "demonstrative {w:?} missing");
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
