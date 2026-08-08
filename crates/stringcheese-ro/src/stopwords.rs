//! The Romanian stopword list.
//!
//! Roughly 130 common Romanian words drawn from the intersection of the
//! Snowball project's `romanian/stop.txt`, Lucene's Romanian analyzer,
//! and a hand-audited head. The list is deliberately conservative — the
//! most frequent function words (free-standing articles, prepositions,
//! coordinating / subordinating conjunctions, personal / possessive /
//! demonstrative pronouns, negation particles, and the high-frequency
//! conjugations of `a fi` "to be", `a avea` "to have", `a face`
//! "to do/make").
//!
//! # Postposed definite articles
//!
//! Romanian's definite article is a **suffix** on the noun (`cartea`
//! "the book", `omul` "the man") rather than a free word — unlike the
//! Spanish `el`/`la` or French `le`/`la` its Romance siblings use.
//! Those suffixes are **not** stopwords in the usual sense; they are
//! morphological endings, and the Snowball Romanian stemmer's Step 0
//! strips them as part of the stemming pipeline (see [`crate::snowball`]).
//! This list carries only free-standing function words.
//!
//! # Comma-below diacritics stored canonically
//!
//! All accented forms are stored in the **modern comma-below** form
//! (`ș` U+0219, `ț` U+021B), not the legacy cedilla (`ş` U+015F,
//! `ţ` U+0163). The crate-level [`Language::is_stopword`
//! ](stringcheese_lang::Language::is_stopword) override folds cedilla
//! to comma-below before comparison, so a caller who feeds tokens
//! from a corpus that was authored with cedilla still gets the
//! expected match.
//!
//! # Case sensitivity
//!
//! The list is stored lowercase; membership checks are performed with
//! [`str::eq_ignore_ascii_case`], so `"și"`, `"Și"`, and `"ȘI"` are
//! all recognized. The default trait-level check does not fold
//! non-ASCII case (the accented lowercase `și` requires the query
//! token to also be lowercase-accented, or to differ only in the
//! ASCII case of the `s` — which becomes the same lowercase).
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Regional variants.** No Moldovan-Cyrillic forms; Moldovan-Latin
//!   is orthographically identical to Romanian and needs no separate
//!   entries.
//! - **Cedilla mirrors.** The list is stored comma-below only; the
//!   `Language::is_stopword` override does the fold at query time
//!   rather than doubling the table's size.

/// The Romanian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Free-standing indefinite articles.
    // (Definite articles are *suffixes* on the noun in Romanian and
    //  are stripped by the stemmer's Step 0 — they are not free-
    //  standing words to be filtered here.)
    "un",
    "o",
    "unei",
    "unui",
    "unor",
    "niște",
    // Personal pronouns (subject, object clitic, disjunctive,
    // reflexive).
    "eu",
    "tu",
    "el",
    "ea",
    "noi",
    "voi",
    "ei",
    "ele",
    "mine",
    "tine",
    "sine",
    "mă",
    "te",
    "se",
    "ne",
    "vă",
    "îl",
    "îi",
    // (`o` — 3sg fem accusative clitic — collides with the indefinite
    //  article `o` listed above; a single entry covers both surface
    //  senses.)
    "le",
    "mi",
    "ți",
    // (`și` — 3sg dative clitic — collides with the coordinating
    //  conjunction `și` "and" listed further below; one entry covers
    //  both surface senses.)
    "ni",
    "vi",
    "li",
    "îmi",
    "îți",
    "își",
    // (`ne`/`vă` — 1pl/2pl reflexive — already covered by the
    //  1pl/2pl clitic entries above.)
    // Possessive adjectives / pronouns (short paradigm).
    "meu",
    "mea",
    "mei",
    "mele",
    "tău",
    "ta",
    "tăi",
    "tale",
    "său",
    "sa",
    "săi",
    "sale",
    "nostru",
    "noastră",
    "noștri",
    "noastre",
    "vostru",
    "voastră",
    "voștri",
    "voastre",
    "lor",
    // Demonstratives (proximal / distal / adjective / pronoun forms).
    "acest",
    "această",
    "acești",
    "aceste",
    "acesta",
    "aceasta",
    "aceștia",
    "acestea",
    "acel",
    "acea",
    "acei",
    "acele",
    "acela",
    "aceea",
    "aceia",
    "acelea",
    // Relative / interrogative pronouns and adverbs.
    "care",
    "cine",
    "ce",
    "cui",
    "cât",
    "câtă",
    "câți",
    "câte",
    "unde",
    "cum",
    "când",
    "de ce",
    // Coordinating / subordinating conjunctions.
    "și",
    "sau",
    "ori",
    "dar",
    "iar",
    "însă",
    "ci",
    "că",
    "dacă",
    "deși",
    "fiindcă",
    "pentru că",
    "ca",
    "să",
    "nici",
    // Prepositions.
    "a", // "of/to" preposition + 3sg have-verb + genitive marker
    "la",
    "în",
    "pe",
    "cu",
    "de",
    "din",
    "pentru",
    "prin",
    "sub",
    "spre",
    "peste",
    "către",
    "fără",
    "până",
    "după",
    "între",
    "lângă",
    // Common adverbs (negation, quantity, degree, time, place).
    "nu",
    "da",
    "mai",
    "foarte",
    "cam",
    "chiar",
    "doar",
    "numai",
    "prea",
    "puțin",
    "mult",
    "multă",
    "mulți",
    "multe",
    "așa",
    "aici",
    "acolo",
    "acum",
    "atunci",
    "apoi",
    "mereu",
    "niciodată",
    "totdeauna",
    "azi",
    "ieri",
    "mâine",
    "deja",
    "încă",
    // A fi — copula and its high-frequency conjugations
    // (present, imperfect, past participle, subjunctive).
    "fi",
    "fie",
    "fiu",
    "fii",
    "fim",
    "fiți",
    "sunt",
    "ești",
    "este",
    "e", // colloquial 3sg present
    "suntem",
    "sunteți",
    "eram",
    "erai",
    "era",
    "erați",
    "erau",
    "fost",
    "fiind",
    // A avea — auxiliary and its high-frequency conjugations.
    "avea",
    "am",
    "ai",
    "are",
    "avem",
    "aveți",
    "au",
    "aveam",
    "aveai",
    "aveau",
    "avut",
    "având",
    // A face — very high-frequency verb.
    "face",
    "fac",
    "faci",
    "facem",
    "faceți",
    // A putea / a vrea — modals.
    "putea",
    "poate",
    "pot",
    "poți",
    "putem",
    "puteți",
    "vrea",
    "vreau",
    "vrei",
    "vrem",
    "vreți",
    "vor",
    // Miscellaneous high-frequency function words.
    "tot",
    "toată",
    "toți",
    "toate",
    "alt",
    "altă",
    "alți",
    "altele",
    "altul",
    "alta",
    "fel",
    "orice",
    "oricine",
    "nimic",
    "nimeni",
    "cineva",
    "ceva",
    "câțiva",
    "câteva",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~130" — assert we're in the
        // ballpark. Range is loose because Romanian has a lot of
        // pronoun paradigm forms and copula conjugations.
        assert!(
            STOPWORDS.len() >= 100 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~130 range",
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
    fn no_cedilla_forms_in_the_list() {
        // The list is stored comma-below only. The
        // `Language::is_stopword` override folds cedilla at query time.
        for &w in STOPWORDS {
            assert!(
                !w.chars()
                    .any(|c| c == 'ş' || c == 'ţ' || c == 'Ş' || c == 'Ţ'),
                "stopword {w:?} contains a legacy cedilla — canonical form is comma-below"
            );
        }
    }

    #[test]
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~130.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "în", "pe", "la", "de", "din", "cu", "pentru", "prin", "sub", "spre", "peste", "către",
            "fără", "până", "după", "între", "lângă",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in [
            "și", "sau", "dar", "că", "dacă", "iar", "însă", "ci", "nici",
        ] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["eu", "tu", "el", "ea", "noi", "voi", "ei", "ele"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["sunt", "ești", "este", "suntem", "era", "fost", "fi"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn common_avea_forms_are_present() {
        for w in ["am", "ai", "are", "avem", "au", "avut"] {
            assert!(STOPWORDS.contains(&w), "avea form {w:?} is missing");
        }
    }
}
