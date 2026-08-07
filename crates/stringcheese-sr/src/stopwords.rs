//! Serbian stopword lists — one per script.
//!
//! Serbian is written in **two scripts** (Cyrillic Vukovica and Gaj's
//! Latin) and both are equally official. Rather than force callers to
//! transliterate to a canonical script before calling
//! [`is_stopword`](stringcheese_lang::Language::is_stopword), the pack
//! ships two parallel lists — [`STOPWORDS_CYR`] and [`STOPWORDS_LAT`]
//! — and the pack's [`Language`](stringcheese_lang::Language) impl
//! looks up in the appropriate list based on the input's script.
//!
//! # Entries and scope
//!
//! Each list has roughly 120 entries covering:
//!
//! - Personal / demonstrative / possessive / interrogative pronouns.
//! - Prepositions.
//! - Coordinating and subordinating conjunctions.
//! - Particles and negation.
//! - High-frequency forms of the copula *biti / бити* (to be) and
//!   the auxiliary *imati / имати* (to have).
//! - A handful of common adverbs.
//!
//! Both lists are stored in lowercase; the
//! [`Language`](stringcheese_lang::Language) impl lowercases queries
//! before scanning.
//!
//! # Non-goals
//!
//! - **Ekavian vs. ijekavian.** Serbian standard admits two pronunciations
//!   of the reflex of Common Slavic `*ě` — ekavian `vek` / `век` versus
//!   ijekavian `vijek` / `вијек`. Where the two variants differ on a
//!   stopword the ekavian form is chosen for the base list (this is the
//!   convention adopted by NLTK's Serbian list); the ijekavian
//!   equivalents `uvijek`, `nikada`, `gdje`, etc. are also included
//!   where they occur in commonly stopword-tagged function words.
//! - **Bosnian / Croatian / Montenegrin.** These share the dual-script
//!   base with Serbian but diverge in vocabulary; each deserves its
//!   own pack. See the crate-level docs for the roadmap.
//! - **Multi-word phrases.** Not included — they would never match a
//!   single token.

/// The Serbian stopword list, Vukovica (Cyrillic) form.
///
/// Every entry mirrors an entry in [`STOPWORDS_LAT`] — the two lists
/// contain the same set of words rendered in the two scripts.
pub const STOPWORDS_CYR: &[&str] = &[
    // Personal pronouns.
    "ја",
    "ти",
    "он",
    "она",
    "оно",
    "ми",
    "ви",
    "они",
    "оне",
    "мене",
    "тебе",
    "њега",
    "ње",
    "њој",
    "њему",
    "њим",
    "њом",
    "њих",
    "њима",
    "нас",
    "вас",
    "нам",
    "вам",
    "нама",
    "вама",
    "мном",
    "тобом",
    "себе",
    "себи",
    "собом",
    // Possessive pronouns.
    "мој",
    "моја",
    "моје",
    "моји",
    "твој",
    "твоја",
    "твоје",
    "твоји",
    "наш",
    "наша",
    "наше",
    "наши",
    "ваш",
    "ваша",
    "ваше",
    "ваши",
    "његов",
    "њен",
    "њихов",
    "свој",
    "своја",
    "своје",
    // Demonstratives.
    "овај",
    "ова",
    "ово",
    "ови",
    "ове",
    "тај",
    "та",
    "то",
    "те",
    "онај",
    // Prepositions.
    "у",
    "на",
    "за",
    "са",
    "с",
    "о",
    "од",
    "до",
    "из",
    "под",
    "над",
    "пред",
    "кроз",
    "ка",
    "по",
    "без",
    "око",
    "међу",
    // Conjunctions.
    "и",
    "а",
    "али",
    "или",
    "но",
    "па",
    "јер",
    "ако",
    "иако",
    "док",
    "када",
    "кад",
    // Particles.
    "да",
    "не",
    "ли",
    "се",
    "већ",
    "само",
    "још",
    "тек",
    "чак",
    // Copula and auxiliary — high-frequency forms.
    "бити",
    "сам",
    "си",
    "је",
    "смо",
    "сте",
    "су",
    "био",
    "била",
    "било",
    "били",
    "биле",
    "имам",
    "имаш",
    "има",
    "имамо",
    "имате",
    "имају",
    // Common adverbs.
    "где",
    "гдје",
    "како",
    "зашто",
    "ту",
    "тамо",
    "сада",
    "тада",
    "увек",
    "увијек",
];

/// The Serbian stopword list, Gaj's Latin form.
///
/// Every entry mirrors an entry in [`STOPWORDS_CYR`] — the two lists
/// contain the same set of words rendered in the two scripts.
pub const STOPWORDS_LAT: &[&str] = &[
    // Personal pronouns.
    "ja", "ti", "on", "ona", "ono", "mi", "vi", "oni", "one", "mene", "tebe", "njega", "nje",
    "njoj", "njemu", "njim", "njom", "njih", "njima", "nas", "vas", "nam", "vam", "nama", "vama",
    "mnom", "tobom", "sebe", "sebi", "sobom", // Possessive pronouns.
    "moj", "moja", "moje", "moji", "tvoj", "tvoja", "tvoje", "tvoji", "naš", "naša", "naše",
    "naši", "vaš", "vaša", "vaše", "vaši", "njegov", "njen", "njihov", "svoj", "svoja", "svoje",
    // Demonstratives.
    "ovaj", "ova", "ovo", "ovi", "ove", "taj", "ta", "to", "te", "onaj", // Prepositions.
    "u", "na", "za", "sa", "s", "o", "od", "do", "iz", "pod", "nad", "pred", "kroz", "ka", "po",
    "bez", "oko", "među", // Conjunctions.
    "i", "a", "ali", "ili", "no", "pa", "jer", "ako", "iako", "dok", "kada", "kad",
    // Particles.
    "da", "ne", "li", "se", "već", "samo", "još", "tek", "čak",
    // Copula and auxiliary — high-frequency forms.
    "biti", "sam", "si", "je", "smo", "ste", "su", "bio", "bila", "bilo", "bili", "bile", "imam",
    "imaš", "ima", "imamo", "imate", "imaju", // Common adverbs.
    "gde", "gdje", "kako", "zašto", "tu", "tamo", "sada", "tada", "uvek", "uvijek",
];

/// Combined view of both stopword lists — the concatenation of
/// [`STOPWORDS_CYR`] and [`STOPWORDS_LAT`]. Handed back from
/// [`Language::stopwords`](stringcheese_lang::Language::stopwords) so
/// callers who inspect the full stopword set see every form.
///
/// Built at compile time via slice concatenation is not straightforward
/// in stable Rust; this constant is a plain slice literal that mirrors
/// the two source lists. Any new entry added to `STOPWORDS_CYR` or
/// `STOPWORDS_LAT` must also be added here.
pub const STOPWORDS_ALL: &[&str] = &[
    // Cyrillic entries (mirror of STOPWORDS_CYR).
    "ја",
    "ти",
    "он",
    "она",
    "оно",
    "ми",
    "ви",
    "они",
    "оне",
    "мене",
    "тебе",
    "њега",
    "ње",
    "њој",
    "њему",
    "њим",
    "њом",
    "њих",
    "њима",
    "нас",
    "вас",
    "нам",
    "вам",
    "нама",
    "вама",
    "мном",
    "тобом",
    "себе",
    "себи",
    "собом",
    "мој",
    "моја",
    "моје",
    "моји",
    "твој",
    "твоја",
    "твоје",
    "твоји",
    "наш",
    "наша",
    "наше",
    "наши",
    "ваш",
    "ваша",
    "ваше",
    "ваши",
    "његов",
    "њен",
    "њихов",
    "свој",
    "своја",
    "своје",
    "овај",
    "ова",
    "ово",
    "ови",
    "ове",
    "тај",
    "та",
    "то",
    "те",
    "онај",
    "у",
    "на",
    "за",
    "са",
    "с",
    "о",
    "од",
    "до",
    "из",
    "под",
    "над",
    "пред",
    "кроз",
    "ка",
    "по",
    "без",
    "око",
    "међу",
    "и",
    "а",
    "али",
    "или",
    "но",
    "па",
    "јер",
    "ако",
    "иако",
    "док",
    "када",
    "кад",
    "да",
    "не",
    "ли",
    "се",
    "већ",
    "само",
    "још",
    "тек",
    "чак",
    "бити",
    "сам",
    "си",
    "је",
    "смо",
    "сте",
    "су",
    "био",
    "била",
    "било",
    "били",
    "биле",
    "имам",
    "имаш",
    "има",
    "имамо",
    "имате",
    "имају",
    "где",
    "гдје",
    "како",
    "зашто",
    "ту",
    "тамо",
    "сада",
    "тада",
    "увек",
    "увијек",
    // Latin entries (mirror of STOPWORDS_LAT).
    "ja",
    "ti",
    "on",
    "ona",
    "ono",
    "mi",
    "vi",
    "oni",
    "one",
    "mene",
    "tebe",
    "njega",
    "nje",
    "njoj",
    "njemu",
    "njim",
    "njom",
    "njih",
    "njima",
    "nas",
    "vas",
    "nam",
    "vam",
    "nama",
    "vama",
    "mnom",
    "tobom",
    "sebe",
    "sebi",
    "sobom",
    "moj",
    "moja",
    "moje",
    "moji",
    "tvoj",
    "tvoja",
    "tvoje",
    "tvoji",
    "naš",
    "naša",
    "naše",
    "naši",
    "vaš",
    "vaša",
    "vaše",
    "vaši",
    "njegov",
    "njen",
    "njihov",
    "svoj",
    "svoja",
    "svoje",
    "ovaj",
    "ova",
    "ovo",
    "ovi",
    "ove",
    "taj",
    "ta",
    "to",
    "te",
    "onaj",
    "u",
    "na",
    "za",
    "sa",
    "s",
    "o",
    "od",
    "do",
    "iz",
    "pod",
    "nad",
    "pred",
    "kroz",
    "ka",
    "po",
    "bez",
    "oko",
    "među",
    "i",
    "a",
    "ali",
    "ili",
    "no",
    "pa",
    "jer",
    "ako",
    "iako",
    "dok",
    "kada",
    "kad",
    "da",
    "ne",
    "li",
    "se",
    "već",
    "samo",
    "još",
    "tek",
    "čak",
    "biti",
    "sam",
    "si",
    "je",
    "smo",
    "ste",
    "su",
    "bio",
    "bila",
    "bilo",
    "bili",
    "bile",
    "imam",
    "imaš",
    "ima",
    "imamo",
    "imate",
    "imaju",
    "gde",
    "gdje",
    "kako",
    "zašto",
    "tu",
    "tamo",
    "sada",
    "tada",
    "uvek",
    "uvijek",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyr_and_lat_lists_have_the_same_size() {
        assert_eq!(STOPWORDS_CYR.len(), STOPWORDS_LAT.len());
    }

    #[test]
    fn both_lists_hit_the_100_word_floor() {
        assert!(
            STOPWORDS_CYR.len() >= 100,
            "STOPWORDS_CYR has only {} entries (want >= 100)",
            STOPWORDS_CYR.len()
        );
        assert!(
            STOPWORDS_LAT.len() >= 100,
            "STOPWORDS_LAT has only {} entries (want >= 100)",
            STOPWORDS_LAT.len()
        );
    }

    #[test]
    fn stopwords_all_size_is_sum_of_both_lists() {
        assert_eq!(
            STOPWORDS_ALL.len(),
            STOPWORDS_CYR.len() + STOPWORDS_LAT.len()
        );
    }

    #[test]
    fn stopwords_all_prefix_matches_cyr() {
        let cyr_len = STOPWORDS_CYR.len();
        assert_eq!(&STOPWORDS_ALL[..cyr_len], STOPWORDS_CYR);
    }

    #[test]
    fn stopwords_all_suffix_matches_lat() {
        let cyr_len = STOPWORDS_CYR.len();
        assert_eq!(&STOPWORDS_ALL[cyr_len..], STOPWORDS_LAT);
    }

    #[test]
    fn cyr_list_is_all_cyrillic_or_ascii_punctuation() {
        for &w in STOPWORDS_CYR {
            let has_cyr = w.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
            assert!(
                has_cyr,
                "Cyrillic stopword {w:?} has no Cyrillic characters"
            );
        }
    }

    #[test]
    fn lat_list_has_no_cyrillic() {
        for &w in STOPWORDS_LAT {
            for c in w.chars() {
                assert!(
                    !('\u{0400}'..='\u{04FF}').contains(&c),
                    "Latin stopword {w:?} contains Cyrillic character {c:?}"
                );
            }
        }
    }

    #[test]
    fn no_uppercase_entries() {
        for list in [STOPWORDS_CYR, STOPWORDS_LAT] {
            for &w in list {
                for c in w.chars() {
                    assert!(
                        !c.is_uppercase(),
                        "stopword {w:?} contains uppercase character {c:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_whitespace_in_entries() {
        for list in [STOPWORDS_CYR, STOPWORDS_LAT] {
            for &w in list {
                assert!(
                    !w.chars().any(char::is_whitespace),
                    "stopword {w:?} contains whitespace"
                );
            }
        }
    }

    #[test]
    fn no_duplicates_within_each_list() {
        for list in [STOPWORDS_CYR, STOPWORDS_LAT] {
            for (i, &w) in list.iter().enumerate() {
                for &v in &list[i + 1..] {
                    assert_ne!(w, v, "duplicate stopword: {w:?}");
                }
            }
        }
    }

    #[test]
    fn common_conjunctions_present_in_both_scripts() {
        for w in ["и", "а", "али", "или"] {
            assert!(STOPWORDS_CYR.contains(&w), "missing Cyrillic {w:?}");
        }
        for w in ["i", "a", "ali", "ili"] {
            assert!(STOPWORDS_LAT.contains(&w), "missing Latin {w:?}");
        }
    }

    #[test]
    fn common_prepositions_present_in_both_scripts() {
        for w in ["у", "на", "за", "од", "до"] {
            assert!(STOPWORDS_CYR.contains(&w), "missing Cyrillic {w:?}");
        }
        for w in ["u", "na", "za", "od", "do"] {
            assert!(STOPWORDS_LAT.contains(&w), "missing Latin {w:?}");
        }
    }
}
