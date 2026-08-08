//! The Hungarian stopword list.
//!
//! Roughly 180 common Hungarian words drawn from the union of
//! published Hungarian stopword collections (the Ranks NL Hungarian
//! list, the Snowball project's Hungarian stopword file, community
//! NLP inventories). Covers personal / demonstrative / interrogative
//! pronouns, coordinating and subordinating conjunctions, common
//! postpositions, the high-frequency conjugations of the copula
//! *lenni* / *van*, common adverbs, and quantifiers.
//!
//! # Non-ASCII stopwords
//!
//! Hungarian carries the long vowels `á`, `é`, `í`, `ó`, `ú` and the
//! umlaut vowels `ö`, `ü` plus their long counterparts `ő`, `ű`.
//! Many stopwords use them. Stopwords are stored in canonical
//! lowercase; the Hungarian
//! [`Hungarian`](crate::Hungarian) implementation overrides
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! to apply a Unicode lowercase pass before comparison (the default
//! [`str::eq_ignore_ascii_case`] misses uppercase `Á É Í Ó Ö Ő Ú Ü
//! Ű`).
//!
//! # Non-goals
//!
//! - **Regional / historical variants.** No Old Hungarian vocabulary;
//!   no dialect-specific forms.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Hungarian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in canonical
/// lowercase.
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns (nominative).
    "én",
    "te",
    "ő",
    "mi",
    "ti",
    "ők",
    "maga",
    "maguk",
    "önök",
    "ön",
    // Demonstrative pronouns. (`az` also serves as the definite
    // article — listed once, in the articles section further below.)
    "ez",
    "ezek",
    "azok",
    "ilyen",
    "olyan",
    "ilyenek",
    "olyanok",
    "ezt",
    "azt",
    // Interrogatives. (`mi` "what" also serves as the 1pl personal
    // pronoun "we" — listed once in the personal-pronoun section
    // above.)
    "ki",
    "mit",
    "kit",
    "mely",
    "melyik",
    "milyen",
    "hogyan",
    "hogy",
    "miért",
    "mikor",
    "hol",
    "hova",
    "honnan",
    "hány",
    "mennyi",
    // Coordinating / subordinating conjunctions.
    "és",
    "vagy",
    "de",
    "mert",
    "hanem",
    "hisz",
    "hiszen",
    "azaz",
    "avagy",
    "illetve",
    "valamint",
    "ám",
    "azonban",
    "viszont",
    "mégis",
    "tehát",
    "sőt",
    "ugyanis",
    "csakhogy",
    "amikor",
    "ahogy",
    "ahogyan",
    "ahol",
    "amiért",
    "amint",
    "aki",
    "amely",
    "ami",
    // Postpositions and prepositional-like markers.
    "alatt",
    "alá",
    "alól",
    "előtt",
    "elé",
    "elől",
    "mögött",
    "mögé",
    "mögül",
    "között",
    "közé",
    "közül",
    "mellett",
    "mellé",
    "mellől",
    "után",
    "felé",
    "felől",
    "körül",
    "által",
    "ellen",
    "helyett",
    "iránt",
    "miatt",
    "nélkül",
    "óta",
    "szerint",
    "végett",
    "át",
    // Adverbs and quantifiers.
    "nem",
    "sem",
    "igen",
    "talán",
    "csak",
    "még",
    "már",
    "most",
    "ma",
    "tegnap",
    "holnap",
    "itt",
    "ott",
    "amott",
    "mindig",
    "sohasem",
    "soha",
    "néha",
    "gyakran",
    "olykor",
    "hirtelen",
    "egyszer",
    "kétszer",
    "többször",
    "sokszor",
    "mindenki",
    "senki",
    "valaki",
    "bárki",
    "akárki",
    "minden",
    "semmi",
    "valami",
    "bármi",
    "akármi",
    "mindegyik",
    "mindkettő",
    "sok",
    "kevés",
    "több",
    "kevesebb",
    "legtöbb",
    "néhány",
    "egyik",
    "másik",
    "többi",
    "többek",
    "elég",
    "nagyon",
    "eléggé",
    "igazán",
    "körülbelül",
    "valóban",
    "tényleg",
    "esetleg",
    "vagyis",
    "ismét",
    "újra",
    "megint",
    "előbb",
    "később",
    // `lenni` / `van` copula and high-frequency verb forms.
    "van",
    "vannak",
    "volt",
    "voltak",
    "lesz",
    "lesznek",
    "legyen",
    "lenne",
    "lennének",
    "lett",
    "lettek",
    "nincs",
    "nincsen",
    "nincsenek",
    "sincs",
    "sincsenek",
    // Articles and particles.
    "a",
    "az",
    "egy",
    "is",
    "se",
    "pedig",
    "hát",
    "bizony",
    // Common short function words.
    "vele",
    "velem",
    "veled",
    "velünk",
    "veletek",
    "velük",
    "róla",
    "róluk",
    "hozzá",
    "hozzám",
    "hozzád",
    "hozzánk",
    "hozzátok",
    "hozzájuk",
    "neki",
    "nekem",
    "neked",
    "nekünk",
    "nektek",
    "nekik",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~180" — assert we're in the
        // ballpark. Hungarian is agglutinative but stopwords are stored
        // as surface function words, so the count is smaller than the
        // Turkish pack's; still, the union of published lists comes
        // out well over 150.
        assert!(
            STOPWORDS.len() >= 150 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the advertised ~180 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase() {
        // The list stores canonical lowercase, so no character in any
        // entry should be an uppercase letter.
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
        for w in ["én", "te", "ő", "mi", "ti", "ők", "ez", "az"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["és", "vagy", "de", "mert", "hanem"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_postpositions_are_present() {
        for w in ["alatt", "előtt", "között", "után", "nélkül", "miatt"] {
            assert!(STOPWORDS.contains(&w), "postposition {w:?} is missing");
        }
    }

    #[test]
    fn common_negation_forms_are_present() {
        for w in ["nem", "sem", "nincs", "soha"] {
            assert!(STOPWORDS.contains(&w), "negation form {w:?} is missing");
        }
    }
}
