//! The Estonian stopword list.
//!
//! Roughly 90 common Estonian words covering the most frequent function
//! words — personal / demonstrative / interrogative pronouns, common
//! conjunctions and particles, high-frequency conjugations of the
//! copular *olema* "to be", common adverbs, and quantifiers.
//!
//! # Non-ASCII stopwords
//!
//! Estonian orthography carries four native non-ASCII vowels — `ä`
//! (U+00E4), `ö` (U+00F6), `ü` (U+00FC), `õ` (U+00F5) — and two
//! loanword consonants — `š` (U+0161) and `ž` (U+017E). Several of
//! the stopwords below use `õ` (Estonian's signature letter, unique
//! among European languages) and a handful use `ü` / `ä`. Stopwords
//! are stored in their canonical lowercase form. Because the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`], the Estonian
//! [`Estonian`](crate::Estonian) implementation overrides the trait
//! method to apply a full Unicode case-fold before comparison — a
//! critical detail for inputs like `SEE`, `KÜLL`, `ÕNNETU`, `ÄRA`.
//!
//! # Agglutinative morphology
//!
//! Estonian is agglutinative: a single "word" carries suffixed cases
//! and number markers. Rather than list every inflected form of every
//! pronoun, the stopword list carries the bare pronoun; downstream
//! systems reach for the [`EstonianStemmer`](crate::EstonianStemmer)
//! to fold inflected forms back to their base. Unlike Finnish,
//! Estonian has **largely lost the stacked possessive suffixes** —
//! possession is expressed with a separate genitive pronoun instead —
//! so the pronoun paradigm is shorter than the Finnish equivalent.
//!
//! # Non-goals
//!
//! - **Regional / historical variants.** No Old Estonian forms; no
//!   dialect vocabulary (Võro, Seto — separate literary standards
//!   under any modern classification).
//! - **Domain-specific stopwords.** Legal, medical, or scientific
//!   corpora typically extend the general list. Downstream applications
//!   should carry their own.

/// The Estonian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in canonical
/// lowercase.
pub const STOPWORDS: &[&str] = &[
    // ------------------------------------------------------------------
    // Personal pronouns — nominative + high-frequency case forms.
    // Estonian has no grammatical gender; `ta` and `tema` cover
    // he/she/it. Both the short (unstressed) and long (stressed)
    // forms of every personal pronoun are in common use.
    // ------------------------------------------------------------------
    "ma", "mina", // 1sg short + long
    "sa", "sina", // 2sg short + long
    "ta", "tema", // 3sg short + long
    "me", "meie", // 1pl short + long
    "te", "teie", // 2pl short + long
    "nad", "nemad", // 3pl short + long
    "mu", "minu", // 1sg genitive short + long
    "su", "sinu", // 2sg genitive short + long
    "ise",
    // ------------------------------------------------------------------
    // Demonstratives.
    // ------------------------------------------------------------------
    "see",   // this / that / it (default demonstrative)
    "need",  // these / those (plural of see)
    "too",   // that (distal)
    "nood",  // those (plural of too)
    "selle", // genitive of see
    "seda",  // partitive of see
    "siin",  // here
    "seal",  // there
    "sinna", // to there
    "siia",  // to here
    "sealt", // from there
    "siit",  // from here
    // ------------------------------------------------------------------
    // Interrogatives.
    // ------------------------------------------------------------------
    "mis",    // what / which
    "kes",    // who
    "kelle",  // whose
    "keda",   // whom
    "mida",   // what (partitive)
    "millal", // when
    "kus",    // where
    "kuhu",   // to where
    "kust",   // from where
    "miks",   // why
    "kuidas", // how
    "kui",    // if / as / when
    "kumb",   // which of two
    // ------------------------------------------------------------------
    // Coordinating / subordinating conjunctions and particles.
    // ------------------------------------------------------------------
    "ja",    // and
    "ning",  // and (formal)
    "või",   // or
    "kas",   // whether / question particle
    "et",    // that (subordinator)
    "aga",   // but
    "kuid",  // but (formal)
    "vaid",  // but rather
    "sest",  // because
    "kuna",  // while / because
    "kuigi", // although
    "ehk",   // or / perhaps
    "nagu",  // like / as
    // ------------------------------------------------------------------
    // Common negation forms — Estonian uses invariant `ei` for indicative
    // negation, `ära` / `ärge` for imperative negation.
    // ------------------------------------------------------------------
    "ei",    // no / not (indicative negation)
    "ära",   // don't (imperative singular)
    "ärge",  // don't (imperative plural)
    "mitte", // not (contrastive / infinitive negation)
    // ------------------------------------------------------------------
    // Copular / auxiliary — high-frequency conjugations of *olema*
    // "to be".
    // ------------------------------------------------------------------
    "olema", // to be (infinitive)
    "olen",  // I am
    "oled",  // you are
    "on",    // he/she/it is / they are (invariant present)
    "oleme", // we are
    "olete", // you (pl) are
    "olin",  // I was
    "olid",  // you were / they were
    "oli",   // he/she/it was
    "olime", // we were
    "olite", // you (pl) were
    "olnud", // been (past participle)
    "olev",  // being (present participle)
    "oleks", // would be (conditional)
    // ------------------------------------------------------------------
    // Common postpositions / adverbs of place, time, degree.
    // ------------------------------------------------------------------
    "juures", // at / next to
    "peal",   // on top of
    "all",    // under
    "ees",    // in front of
    "taga",   // behind
    "kõrval", // beside
    "vahel",  // between
    "sees",   // inside
    "väljas", // outside
    "üle",    // over
    "läbi",   // through
    "vastu",  // against
    "ilma",   // without
    "koos",   // together with
    "pärast", // after
    "enne",   // before
    "nüüd",   // now
    "siis",   // then
    "kohe",   // immediately
    "juba",   // already
    "veel",   // still / yet
    "alati",  // always
    "kunagi", // ever / never
    "eile",   // yesterday
    "täna",   // today
    "homme",  // tomorrow
    // ------------------------------------------------------------------
    // Quantifiers / determiners.
    // ------------------------------------------------------------------
    "kõik",  // all
    "iga",   // every
    "mõni",  // some
    "palju", // much / many
    "vähe",  // little / few
    "üks",   // one
    "kaks",  // two
    "kolm",  // three
    "neli",  // four
    "viis",  // five
    // ------------------------------------------------------------------
    // Common adverbs of degree and manner.
    // ------------------------------------------------------------------
    "väga",     // very
    "üsna",     // rather
    "päris",    // quite
    "ainult",   // only
    "ka",       // also / too
    "samuti",   // likewise
    "nii",      // so
    "niimoodi", // in this way
    // ------------------------------------------------------------------
    // Common answer / discourse particles.
    // ------------------------------------------------------------------
    "jah",  // yes
    "küll", // indeed / certainly
    "ju",   // (emphatic particle)
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~90" — assert we're in the
        // ballpark.
        assert!(
            STOPWORDS.len() >= 80 && STOPWORDS.len() <= 150,
            "STOPWORDS.len() = {} outside the advertised ~90 range",
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
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~90.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in [
            "mina", "sina", "tema", "meie", "teie", "nemad", "see", "need",
        ] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["ja", "või", "et", "aga", "kui", "sest", "kuid"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_copulas_are_present() {
        for w in ["on", "olen", "oled", "olema", "oli", "olid"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn common_negations_are_present() {
        for w in ["ei", "ära", "ärge", "mitte"] {
            assert!(STOPWORDS.contains(&w), "negation {w:?} is missing");
        }
    }

    #[test]
    fn diacritic_entries_are_present() {
        // A sanity check that ä / ö / ü / õ carrying entries survived:
        // these words absolutely must be in the list for an Estonian
        // stopword set to be useful.
        for w in ["ära", "või", "küll", "täna", "üle"] {
            assert!(
                STOPWORDS.contains(&w),
                "diacritic-bearing stopword {w:?} is missing"
            );
        }
    }
}
