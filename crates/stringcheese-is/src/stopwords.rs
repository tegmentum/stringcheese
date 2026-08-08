//! The Icelandic stopword list.
//!
//! ~90 common Icelandic words: the ranked head of Icelandic function
//! words (articles / pronouns / prepositions / conjunctions) plus the
//! full paradigms of the copula `vera` "to be" and the auxiliary
//! `hafa` "have".
//!
//! # Accented characters
//!
//! Icelandic uses the letters `á`, `ð`, `é`, `í`, `ó`, `ú`, `ý`, `þ`,
//! `æ`, `ö` in everyday vocabulary — including in function words like
//! `í` "in", `á` "on", `það` "it", `þú` "you", `þið` "you (pl.)",
//! `þau` "they (n.)". Because the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation uses [`str::eq_ignore_ascii_case`] — an ASCII-only
//! case fold — the uppercase variants `Þ`, `Ð`, `Æ`, `Ö` are **not**
//! automatically recognized by the default check; only the lowercase
//! spellings stored here match. Callers who need Unicode case-fold on
//! Icelandic stopwords should wrap the lookup themselves (e.g., via
//! `str::to_lowercase` from `std`).
//!
//! # Non-goals
//!
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Case sensitivity.** The list is stored lowercase; membership
//!   checks are performed with [`str::eq_ignore_ascii_case`], so `"og"`,
//!   `"Og"`, and `"OG"` are all recognized. The default trait-level
//!   check does not fold non-ASCII accents.

/// The Icelandic stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice.
pub const STOPWORDS: &[&str] = &[
    // Ranked head — the commonest Icelandic function words.
    // Conjunctions / discourse particles.
    "og",
    "en",
    "eða",
    "að",
    "sem",
    "því",
    "þó",
    "svo",
    "nú",
    "þá",
    // Prepositions and adverbial particles. (`við` — the preposition
    // "against, near" — also serves as the 1pl pronoun "we"; listed
    // once here to keep the paradigm sections dedup-clean.)
    "í",
    "á",
    "af",
    "til",
    "frá",
    "með",
    "um",
    "yfir",
    "undir",
    "við",
    "eftir",
    "fyrir",
    "hjá",
    "milli",
    "gegn",
    "án",
    // Articles and demonstratives (Icelandic uses a definite suffix
    // rather than a free article, but these free demonstratives are
    // still common).
    "það",
    "þessi",
    "þetta",
    "þessa",
    "sá",
    "sú",
    "þau",
    "þeir",
    "þær",
    "þess",
    // Pronouns — personal (nominative + oblique paradigms). `við`
    // (1pl) is already listed in the prepositions section above.
    "ég",
    "mig",
    "mér",
    "mín",
    "þú",
    "þig",
    "þér",
    "þín",
    "hann",
    "hana",
    "honum",
    "hans",
    "hún",
    "henni",
    "hennar",
    "okkur",
    "okkar",
    "þið",
    "ykkur",
    "ykkar",
    "þeim",
    "þeirra",
    // Interrogatives.
    "hver",
    "hvað",
    "hvor",
    "hvenær",
    "hvar",
    "hvernig",
    "hvers",
    // Negation and modality adverbs.
    "ekki",
    "aldrei",
    "alltaf",
    "kannski",
    "líka",
    "bara",
    "aðeins",
    "mjög",
    // The copula VERA "to be" — indicative present / past + subjunctive
    // + past participle.
    "er",
    "ert",
    "erum",
    "eruð",
    "eru",
    "var",
    "varst",
    "vorum",
    "voruð",
    "voru",
    "sé",
    "vera",
    "verið",
    // The auxiliary HAFA "have" — indicative present / past + past
    // participle.
    "hef",
    "hefur",
    "höfum",
    "hafið",
    "hafa",
    "hafði",
    "hafðir",
    "höfðum",
    "höfðuð",
    "höfðu",
    "haft",
    // Modals: skulu / vilja / geta / mega.
    "skal",
    "skalt",
    "skulum",
    "skuluð",
    "skulu",
    "skyldi",
    "vil",
    "vilt",
    "viljum",
    "viljið",
    "vilja",
    "vildi",
    "get",
    "getur",
    "getum",
    "getið",
    "geta",
    "gat",
    "má",
    "mátt",
    "megum",
    "megið",
    "mega",
    "mátti",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "~90" — assert we're in the
        // ballpark. Range is loose to accommodate the auxiliary-verb
        // paradigm expansion (vera / hafa / skulu / vilja / geta / mega).
        assert!(
            STOPWORDS.len() >= 70 && STOPWORDS.len() <= 200,
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
    fn common_prepositions_are_present() {
        for w in [
            "í", "á", "af", "til", "frá", "með", "um", "við", "eftir", "fyrir",
        ] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["og", "en", "eða", "að", "sem", "því"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["ég", "þú", "hann", "hún", "við", "þið", "þau"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_auxiliary_verb_forms_are_present() {
        // Representative subset for VERA "to be" and HAFA "have" plus
        // core modals.
        for w in [
            "er", "var", "vera", "hef", "hefur", "hafa", "hafði", "skal", "skulu", "vil", "vilja",
            "vildi", "get", "geta", "má", "mega",
        ] {
            assert!(STOPWORDS.contains(&w), "verb form {w:?} is missing");
        }
    }

    #[test]
    fn icelandic_specific_letters_are_present() {
        // Sanity-check that entries with `þ` / `ð` / `æ` / `ö` / vowel-
        // accents are stored as the actual Unicode scalars, not folded
        // to ASCII.
        for w in ["þú", "þið", "þau", "það", "í", "á", "ég", "eða"] {
            assert!(
                STOPWORDS.contains(&w),
                "Icelandic-specific-letter stopword {w:?} is missing"
            );
        }
    }
}
