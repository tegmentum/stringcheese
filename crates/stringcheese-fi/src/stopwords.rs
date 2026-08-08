//! The Finnish stopword list.
//!
//! Roughly 170 common Finnish words drawn from the intersection of
//! NLTK's Finnish list, the Snowball project's Finnish stopword
//! collection, and standard IR-community Finnish stopword sets. The
//! list is deliberately conservative — the most frequent function
//! words (personal / demonstrative / interrogative pronouns, common
//! conjunctions and particles, high-frequency conjugations of the
//! copular *olla* "to be", and common adverbs).
//!
//! # Non-ASCII stopwords
//!
//! Finnish orthography carries three non-ASCII vowels — `ä` (U+00E4),
//! `ö` (U+00F6), and (in loanwords) `å` (U+00E5). Many of the
//! stopwords below use `ä`; a handful use `ö`. Stopwords are stored
//! in their canonical lowercase form. Because the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`], the Finnish
//! [`Finnish`](crate::Finnish) implementation overrides the trait
//! method to apply a full Unicode case-fold before comparison — a
//! critical detail for inputs like `SINÄ`, `MITÄ`, `MYÖS`.
//!
//! # Agglutinative morphology
//!
//! Finnish is agglutinative: a single "word" carries suffixed cases,
//! possessives, and particles. Rather than list every inflected form
//! of every pronoun (which would balloon the list into thousands of
//! entries), the stopword list carries the bare pronoun; downstream
//! systems reach for the Snowball Finnish stemmer to fold inflected
//! forms back to their base. A few high-frequency case-inflected
//! pronouns (dative `minulle`, ablative `sinulta`, …) are listed
//! explicitly because they show up in NLTK's canonical Finnish
//! stoplist.
//!
//! # Non-goals
//!
//! - **Regional / historical variants.** No Old Finnish forms; no
//!   dialect vocabulary (Karelian, Ingrian, Meänkieli — separate
//!   languages under any modern classification).
//! - **Domain-specific stopwords.** Legal, medical, or scientific
//!   corpora typically extend the general list. Downstream applications
//!   should carry their own.

/// The Finnish stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in canonical
/// lowercase.
pub const STOPWORDS: &[&str] = &[
    // ------------------------------------------------------------------
    // Personal pronouns — subject / object / dative / locative forms.
    // Finnish has no grammatical gender; the third person `hän` covers
    // he/she/it (though `se` also serves as a colloquial 3sg).
    // ------------------------------------------------------------------
    "minä",
    "sinä",
    "hän",
    "me",
    "te",
    "he",
    "se",
    "ne", // Nominative + demonstrative.
    "minun",
    "sinun",
    "hänen",
    "meidän",
    "teidän",
    "heidän", // Genitive.
    "minua",
    "sinua",
    "häntä",
    "meitä",
    "teitä",
    "heitä", // Partitive.
    "minulle",
    "sinulle",
    "hänelle",
    "meille",
    "teille",
    "heille", // Allative.
    "minulta",
    "sinulta",
    "häneltä",
    "meiltä",
    "teiltä",
    "heiltä", // Ablative.
    "minulla",
    "sinulla",
    "hänellä",
    "meillä",
    "teillä",
    "heillä", // Adessive.
    "minussa",
    "sinussa",
    "hänessä",
    "meissä",
    "teissä",
    "heissä", // Inessive.
    "itse",
    // ------------------------------------------------------------------
    // Demonstratives.
    // ------------------------------------------------------------------
    "tämä",
    "tuo",
    "nämä",
    "nuo",
    "tässä",
    "tuossa",
    "siinä",
    "täällä",
    "tuolla",
    "siellä",
    "tänne",
    "tuonne",
    "sinne",
    "täältä",
    "tuolta",
    "sieltä",
    "tuon",
    "tämän",
    "sen",
    // ------------------------------------------------------------------
    // Interrogatives.
    // ------------------------------------------------------------------
    "mikä",
    "kuka",
    "kenen",
    "ketä",
    "mitä",
    "milloin",
    "missä",
    "mistä",
    "minne",
    "miksi",
    "miten",
    "kuinka",
    "montako",
    "monta",
    "kumpi",
    // ------------------------------------------------------------------
    // Coordinating / subordinating conjunctions and particles.
    // ------------------------------------------------------------------
    "ja",
    "sekä",
    "tai",
    "vai",
    "eli",
    "että",
    "mutta",
    "vaan",
    "kun",
    "kuin",
    "jos",
    "jotta",
    "koska",
    "sillä",
    "vaikka",
    "jotka",
    "joka",
    "joita",
    "joiden",
    "jonka",
    "jota",
    "kunhan",
    "ennen",
    "jälkeen",
    "ellei",
    "ettei",
    "kunnes",
    "mikäli",
    // ------------------------------------------------------------------
    // Common negation forms — Finnish has a full negation-verb paradigm.
    // ------------------------------------------------------------------
    "ei",
    "en",
    "et",
    "emme",
    "ette",
    "eivät",
    "älä",
    "älkää",
    // ------------------------------------------------------------------
    // Copular / auxiliary — high-frequency conjugations of *olla*
    // "to be".
    // ------------------------------------------------------------------
    "olla",
    "olen",
    "olet",
    "on",
    "olemme",
    "olette",
    "ovat",
    "oli",
    "olivat",
    "olisi",
    "olisit",
    "olisimme",
    "olisitte",
    "olisivat",
    "ollut",
    "olleet",
    "olemassa",
    "oleva",
    "olevat",
    "ollen",
    "ollaan",
    // ------------------------------------------------------------------
    // Common postpositions / adverbs of place, time, degree.
    // ------------------------------------------------------------------
    "kanssa",
    "vastaan",
    "vieressä",
    "yli",
    "alla",
    "päällä",
    "edessä",
    "takana",
    "keskellä",
    "luona",
    "sisällä",
    "ulkona",
    "välillä",
    "ympärillä",
    "läpi",
    "kohti",
    "asti",
    "saakka",
    "varten",
    "vuoksi",
    "takia",
    "avulla",
    "myötä",
    "nyt",
    "silloin",
    "aina",
    "ikinä",
    "koskaan",
    "jo",
    "vielä",
    "juuri",
    "heti",
    "pian",
    "usein",
    "harvoin",
    "joskus",
    "aikaisin",
    "myöhään",
    "eilen",
    "tänään",
    "huomenna",
    "tuolloin",
    // ------------------------------------------------------------------
    // Quantifiers / determiners.
    // ------------------------------------------------------------------
    "kaikki",
    "koko",
    "jokainen",
    "moni",
    "muutama",
    "useat",
    "monet",
    "muut",
    "jotkin",
    "jotkut",
    "muutamat",
    "yksi",
    "kaksi",
    "kolme",
    "neljä",
    "viisi",
    "molemmat",
    // ------------------------------------------------------------------
    // Common adverbs of quantity, degree, and manner.
    // ------------------------------------------------------------------
    "hyvin",
    "erittäin",
    "todella",
    "aivan",
    "melko",
    "aika",
    "hieman",
    "vähän",
    "paljon",
    "enemmän",
    "vähemmän",
    "eniten",
    "vähiten",
    "yhtä",
    "niin",
    "myös",
    "myöskin",
    "kuitenkin",
    "silti",
    "toki",
    "vain",
    "ainoastaan",
    "pelkästään",
    "muuten",
    "muutoin",
    "esimerkiksi",
    "siis",
    "siten",
    "näin",
    "noin",
    // ------------------------------------------------------------------
    // Common answer particles / discourse markers.
    // ------------------------------------------------------------------
    "kyllä",
    "joo",
    "ehkä",
    "eiköhän",
    "muka",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~170" — assert we're in the
        // ballpark. Finnish is agglutinative, so many inflected
        // pronoun forms are already reachable through the stemmer;
        // we still list the highest-frequency ones explicitly.
        assert!(
            STOPWORDS.len() >= 150 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~170 range",
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
        // O(n^2) is fine for a static list of ~170.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["minä", "sinä", "hän", "me", "te", "he", "se", "ne"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["ja", "tai", "että", "mutta", "kun", "jos", "koska"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_copulas_are_present() {
        for w in ["on", "olen", "olet", "olla", "ovat", "oli"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn common_negations_are_present() {
        for w in ["ei", "en", "et", "emme", "ette", "eivät"] {
            assert!(STOPWORDS.contains(&w), "negation {w:?} is missing");
        }
    }

    #[test]
    fn front_vowel_entries_are_present() {
        // A sanity check that ä / ö carrying entries survived: these
        // words absolutely must be in the list for a Finnish stopword
        // set to be useful.
        for w in ["että", "häntä", "myös", "sinä", "mitä"] {
            assert!(
                STOPWORDS.contains(&w),
                "front-vowel stopword {w:?} is missing"
            );
        }
    }
}
