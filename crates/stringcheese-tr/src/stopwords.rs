//! The Turkish stopword list.
//!
//! Roughly 180 common Turkish words drawn from the intersection of
//! NLTK's Turkish list and the Snowball project's Turkish stopword
//! collection. The list is deliberately conservative — the most
//! frequent function words (personal / demonstrative / interrogative
//! pronouns, postpositions, conjunctions, the high-frequency
//! conjugations of the copula *olmak* and the auxiliary verbs *etmek*
//! / *yapmak*, common adverbs, and quantifiers).
//!
//! # Non-ASCII stopwords
//!
//! Turkish orthography carries several non-ASCII letters (`ç`, `ğ`,
//! `ı`, `İ`, `ö`, `ş`, `ü`); many of the stopwords below use them.
//! Stopwords are stored in their canonical lowercase form under
//! Turkish case-fold rules (`İ → i`, `I → ı`). Because the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`], the Turkish
//! [`Turkish`](crate::Turkish) implementation overrides the trait
//! method to apply a Turkic-aware case fold before comparison — see
//! [`crate::case_fold`] for the fold rules.
//!
//! # Non-goals
//!
//! - **Regional / historical variants.** No Ottoman-era vocabulary; no
//!   dialect-specific forms.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Turkish stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in lowercase
/// under Turkish case-fold rules (dotted `İ` maps to `i`, dotless
/// `I` maps to `ı`).
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns (subject, object, dative, locative, ablative,
    // genitive) — Turkish shows the case system through suffixes; the
    // pronominal forms are irregular and worth listing individually.
    "ben",
    "sen",
    "o",
    "biz",
    "siz",
    "onlar",
    "beni",
    "seni",
    "onu",
    "bizi",
    "sizi",
    "onları",
    "bana",
    "sana",
    "ona",
    "bize",
    "size",
    "onlara",
    "bende",
    "sende",
    "onda",
    "bizde",
    "sizde",
    "onlarda",
    "benden",
    "senden",
    "ondan",
    "bizden",
    "sizden",
    "onlardan",
    "benim",
    "senin",
    "onun",
    "bizim",
    "sizin",
    "onların",
    "kendi",
    "kendisi",
    "kendileri",
    // Demonstratives.
    "bu",
    "şu",
    "bunlar",
    "şunlar",
    "bunu",
    "şunu",
    "bunun",
    "şunun",
    "bunda",
    "şunda",
    "bundan",
    "şundan",
    "böyle",
    "şöyle",
    "öyle",
    // Interrogatives.
    "ne",
    "kim",
    "hangi",
    "nasıl",
    "niye",
    "neden",
    "niçin",
    "nerede",
    "nereye",
    "nereden",
    "kaç",
    "kime",
    "kimi",
    "kimin",
    // Coordinating / subordinating conjunctions.
    "ve",
    "veya",
    "ya",
    "yada",
    "ile",
    "ama",
    "fakat",
    "lakin",
    "ancak",
    "yalnız",
    "ki",
    "de",
    "da",
    "ise",
    "eğer",
    "çünkü",
    "zira",
    "hem",
    "gerek",
    "ister",
    // Postpositions and prepositional-like markers.
    "için",
    "gibi",
    "kadar",
    "göre",
    "üzere",
    "karşı",
    "beri",
    "diye",
    "dolayı",
    "ötürü",
    "rağmen",
    "önce",
    "sonra",
    // Adverbs (negation, quantity, degree, time, place).
    "değil",
    "yok",
    "hayır",
    "evet",
    "belki",
    "acaba",
    "artık",
    "hâlâ",
    "hala",
    "şimdi",
    "bugün",
    "yarın",
    "dün",
    "hemen",
    "birden",
    "ansızın",
    "sık",
    "sıkça",
    "bazen",
    "bazı",
    "hep",
    "hepsi",
    "her",
    "herkes",
    "hiç",
    "hiçbir",
    "hiçbiri",
    "kimse",
    "biraz",
    "birçok",
    "birkaç",
    "birçoğu",
    "az",
    "çok",
    "daha",
    "en",
    "pek",
    "gayet",
    "oldukça",
    "yine",
    "yeniden",
    "tekrar",
    "ayrıca",
    "üstelik",
    "ilk",
    "önceki",
    "sonraki",
    "aynı",
    "başka",
    "diğer",
    "öteki",
    "beriki",
    "böylece",
    "böylelikle",
    "böylesine",
    "ayrı",
    "bile",
    "dahi",
    // `olmak` copula / auxiliary — high-frequency conjugations.
    "olan",
    "olarak",
    "olmak",
    "oldu",
    "oluyor",
    "olacak",
    "olur",
    "olmuş",
    "olabilir",
    "olsa",
    // `etmek` auxiliary — very high-frequency.
    "etmek",
    "etti",
    "eder",
    "ediyor",
    "edecek",
    "etmiş",
    // Miscellaneous high-frequency function words.
    "şey",
    "bir",
    "iki",
    "üç",
    "tüm",
    "tümü",
    "bütün",
    "bazıları",
    "orada",
    "burada",
    "şurada",
    "oraya",
    "buraya",
    "şuraya",
    "oradan",
    "buradan",
    "şuradan",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~180" — assert we're in the
        // ballpark. Turkish is agglutinative, so many "conjugated"
        // pronoun/demonstrative forms show up in the list.
        assert!(
            STOPWORDS.len() >= 150 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~180 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase_under_turkic_rules() {
        // Under Turkish orthography lowercase, uppercase Latin `I`
        // (U+0049) would map to dotless `ı` (U+0131), and uppercase
        // dotted `İ` (U+0130) would map to `i` (U+0069). The list
        // stores canonical lowercase, so no character in any entry
        // should be an uppercase letter.
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
        // O(n^2) is fine for a static list of ~180.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["ben", "sen", "o", "biz", "siz", "onlar", "bu", "şu"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["ve", "veya", "ile", "ama", "fakat", "ancak", "çünkü"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_postpositions_are_present() {
        for w in ["için", "gibi", "kadar", "göre", "önce", "sonra"] {
            assert!(STOPWORDS.contains(&w), "postposition {w:?} is missing");
        }
    }

    #[test]
    fn common_negation_forms_are_present() {
        for w in ["değil", "yok", "hiç", "hiçbir"] {
            assert!(STOPWORDS.contains(&w), "negation form {w:?} is missing");
        }
    }
}
