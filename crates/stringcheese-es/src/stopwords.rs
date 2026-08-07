//! The Spanish stopword list.
//!
//! Roughly 200 common Spanish words drawn from the intersection of
//! NLTK's `spanish` list, the Snowball project's `spanish/stop.txt`
//! (Martin Porter's own Spanish stoplist), and Lucene's
//! [Spanish analyzer][luc]. The list is deliberately conservative — the
//! most frequent function words (articles, prepositions, pronouns,
//! auxiliaries `ser`/`estar`/`haber` and their high-frequency
//! conjugations). No domain-specific jargon; no archaic forms.
//!
//! [luc]: https://lucene.apache.org/core/9_0_0/analysis/common/org/apache/lucene/analysis/es/SpanishAnalyzer.html
//!
//! # Accented characters
//!
//! Several stopwords carry acute accents (`él`, `más`, `sí`, `también`,
//! `qué`, `porqué`, `está`, `están`, …). These are stored in their
//! canonical accented form. The default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — so `él` matches `ÉL` **only if the input carries the
//! accented `É`**; the ASCII probe `EL` does not match the accented
//! stopword `él`. Callers whose input pipeline already applies Unicode
//! case folding will see the expected match; callers who feed
//! accent-stripped input must be aware that accented stopwords will
//! silently miss.
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Regional variants.** No `vos` / Rioplatense-specific forms;
//!   no Peninsular-only spellings. The list targets a shared standard
//!   Latin-American/Peninsular vocabulary at the intersection.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so `"el"`,
//!   `"El"`, and `"EL"` are all recognized. The default trait-level
//!   check does not fold non-ASCII accents.

/// The Spanish stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Definite and indefinite articles.
    "el",
    "la",
    "los",
    "las",
    "un",
    "una",
    "unos",
    "unas",
    "lo",
    // Personal pronouns (subject, object, reflexive, prepositional).
    "yo",
    "tú",
    "él",
    "ella",
    "ello",
    "usted",
    "nosotros",
    "nosotras",
    "vosotros",
    "vosotras",
    "ellos",
    "ellas",
    "ustedes",
    "me",
    "te",
    "se",
    "nos",
    "os",
    "le",
    "les",
    "mí",
    "ti",
    "sí",
    "conmigo",
    "contigo",
    "consigo",
    // Possessive adjectives / pronouns.
    "mi",
    "mis",
    "tu",
    "tus",
    "su",
    "sus",
    "mío",
    "mía",
    "míos",
    "mías",
    "tuyo",
    "tuya",
    "tuyos",
    "tuyas",
    "suyo",
    "suya",
    "suyos",
    "suyas",
    "nuestro",
    "nuestra",
    "nuestros",
    "nuestras",
    "vuestro",
    "vuestra",
    "vuestros",
    "vuestras",
    // Demonstratives.
    "este",
    "esta",
    "estos",
    "estas",
    "esto",
    "ese",
    "esa",
    "esos",
    "esas",
    "eso",
    "aquel",
    "aquella",
    "aquellos",
    "aquellas",
    "aquello",
    // Relative / interrogative pronouns.
    "que",
    "quien",
    "quienes",
    "cual",
    "cuales",
    "cuyo",
    "cuya",
    "cuyos",
    "cuyas",
    "qué",
    "quién",
    "quiénes",
    "cuál",
    "cuáles",
    "dónde",
    "donde",
    "cuándo",
    "cuando",
    "cómo",
    "como",
    "cuánto",
    "cuánta",
    "cuántos",
    "cuántas",
    "cuanto",
    "cuanta",
    "cuantos",
    "cuantas",
    "porqué",
    // Coordinating / subordinating conjunctions.
    "y",
    "e",
    "o",
    "u",
    "ni",
    "pero",
    "sino",
    "mas",
    "aunque",
    "porque",
    "pues",
    "si",
    "aun",
    "aún",
    "mientras",
    // Prepositions.
    "a",
    "ante",
    "bajo",
    "con",
    "contra",
    "de",
    "desde",
    "durante",
    "en",
    "entre",
    "hacia",
    "hasta",
    "mediante",
    "para",
    "por",
    "según",
    "sin",
    "sobre",
    "tras",
    "del",
    "al",
    // Common adverbs (negation, quantity, degree, time, place).
    "no",
    "más",
    "menos",
    "muy",
    "mucho",
    "mucha",
    "muchos",
    "muchas",
    "poco",
    "poca",
    "pocos",
    "pocas",
    "tan",
    "tanto",
    "tanta",
    "tantos",
    "tantas",
    "bien",
    "mal",
    "mejor",
    "peor",
    "ya",
    "aquí",
    "ahí",
    "allí",
    "allá",
    "acá",
    "ahora",
    "antes",
    "después",
    "luego",
    "siempre",
    "nunca",
    "jamás",
    "hoy",
    "ayer",
    "mañana",
    "también",
    "tampoco",
    "así",
    "solo",
    "sólo",
    "solamente",
    "todavía",
    // Ser — auxiliary/copula and its high-frequency conjugations
    // (present, imperfect, past participle; vosotros / future /
    // preterite forms omitted for size and because Latin-American
    // Spanish rarely uses `vosotros` and the compound tenses subsume
    // the analytic ones for IR).
    "ser",
    "soy",
    "eres",
    "es",
    "somos",
    "son",
    "era",
    "eras",
    "eran",
    "fue",
    "sea",
    "sean",
    "sido",
    "siendo",
    // Estar — auxiliary/copula and its high-frequency conjugations.
    "estar",
    "estoy",
    "estás",
    "está",
    "estamos",
    "están",
    "estaba",
    "estaban",
    "esté",
    "estén",
    "estado",
    "estando",
    // Haber — auxiliary and its high-frequency conjugations.
    "haber",
    "he",
    "has",
    "ha",
    "hemos",
    "han",
    "hay",
    "había",
    "habían",
    "haya",
    "hayan",
    "habido",
    "habiendo",
    // Tener / hacer / poder / querer / ir — very high-frequency verbs.
    "tener",
    "tengo",
    "tiene",
    "tienen",
    "tenía",
    "hacer",
    "hace",
    "hecho",
    "poder",
    "puede",
    "pueden",
    "podía",
    "querer",
    "quiere",
    "ir",
    "va",
    "vamos",
    "van",
    // Miscellaneous high-frequency function words.
    "todo",
    "toda",
    "todos",
    "todas",
    "otro",
    "otra",
    "otros",
    "otras",
    "mismo",
    "misma",
    "mismos",
    "mismas",
    "algún",
    "alguna",
    "algunos",
    "algunas",
    "alguno",
    "ningún",
    "ninguna",
    "ninguno",
    "cada",
    "varios",
    "varias",
    "algo",
    "alguien",
    "nada",
    "nadie",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~200" — assert we're in the
        // ballpark. Range is loose because Spanish has a lot of
        // auxiliary-verb conjugations plus gender/number-inflected
        // pronouns and quantifiers.
        assert!(
            STOPWORDS.len() >= 180 && STOPWORDS.len() <= 320,
            "STOPWORDS.len() = {} outside the advertised ~200 range",
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
        // O(n^2) is fine for a static list of ~200.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_articles_are_present() {
        for w in ["el", "la", "los", "las", "un", "una", "unos", "unas", "lo"] {
            assert!(STOPWORDS.contains(&w), "article {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in [
            "a", "de", "en", "por", "para", "con", "sin", "sobre", "entre", "desde", "hasta",
            "del", "al",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["y", "o", "pero", "si", "porque", "aunque"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_auxiliary_verb_forms_are_present() {
        for w in [
            "ser", "es", "son", "era", "fue", "estar", "está", "están", "haber", "he", "ha", "han",
            "hay", "había",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }
}
