//! The Czech stopword list.
//!
//! Roughly 275 common Czech words drawn from the union of published
//! Czech stopword collections (the Ranks NL Czech list, the ÚFAL
//! Prague Dependency Treebank stopword set, NLTK-adjacent Slavic
//! function-word inventories). The count runs a bit higher than the
//! standard ~150-200 mark because Czech has rich morphology on its
//! function words (the copula *být* alone contributes ~30 conjugated
//! forms, spanning the negated `ne-` prefix; every gender / number
//! variant of the possessive and demonstrative pronouns is stored). The list is deliberately conservative
//! — the most frequent function words (personal / demonstrative /
//! interrogative pronouns, prepositions, conjunctions, particles,
//! high-frequency forms of the copula *být*, common adverbs, and
//! quantifiers).
//!
//! # Czech letter set
//!
//! Every entry below is a Czech string in canonical lowercase form.
//! Czech uses the extended Latin block: alongside the ASCII letters,
//! Czech carries the haček (caron) consonants **č**, **ď**, **ň**,
//! **ř**, **š**, **ť**, **ž**; the e-caron **ě**; and the long vowels
//! **á**, **é**, **í**, **ó**, **ú**, **ý** plus the ring-over-u
//! **ů**. Under Rust's default [`char::to_lowercase`], uppercase
//! Czech-specific letters fold to their lowercase counterparts without
//! any locale-specific tailoring, so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (an ASCII-case-insensitive linear scan) would *miss*
//! uppercase inputs like `NENÍ` or `KDYŽ`. The Czech
//! [`Language`](stringcheese_lang::Language) impl therefore overrides
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) with a
//! Unicode case-fold.
//!
//! # Homographs
//!
//! Czech has several strings that are function words under one
//! part of speech and pronouns / demonstratives under another:
//!
//! * **`je`** — both the accusative-plural personal pronoun ("them")
//!   and the third-singular present of *být* ("is"). Kept once.
//! * **`ty`** — both the nominative second-singular personal pronoun
//!   ("you") and the masculine-inanimate-plural demonstrative
//!   ("those"). Kept once.
//! * **`ti`** — both the dative second-singular personal pronoun
//!   ("to you") and the masculine-animate-plural demonstrative
//!   ("those"). Kept once.
//! * **`se`** — both the reflexive personal pronoun ("oneself") and
//!   the preposition ("with"). Kept once.
//! * **`ona`** / **`ono`** — both the third-person personal pronouns
//!   and archaic demonstratives ("that"). Kept once (personal-pronoun
//!   reading is more common in modern Czech).
//!
//! Each surface form appears exactly once in the list; downstream
//! applications that need part-of-speech-aware filtering should carry
//! their own annotations.
//!
//! # Non-goals
//!
//! - **Multi-word phrases.** Entries like `i když` (a two-word
//!   concessive conjunction) would never match a single token — they
//!   are left out of the list; downstream systems that want
//!   phrase-level filtering should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Slovak.** Slovak function words overlap with Czech but diverge
//!   — a dedicated Slovak pack is a follow-up.

/// The Czech stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is stored in Czech lowercase.
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns (nominative + oblique cases).
    "já",
    "ty",
    "on",
    "ona",
    "ono",
    "my",
    "vy",
    "oni",
    "ony",
    "mě",
    "tě",
    "ho",
    "ji",
    "jej",
    "nás",
    "vás",
    "jich",
    "mi",
    "ti",
    "mu",
    "jí",
    "nám",
    "vám",
    "jim",
    "mnou",
    "tebou",
    "jím",
    "ní",
    "námi",
    "vámi",
    "jimi",
    "se",
    "si",
    "sebe",
    "sobě",
    "sebou",
    // Possessive pronouns.
    "můj",
    "moje",
    "má",
    "mé",
    "mí",
    "mou",
    "moji",
    "tvůj",
    "tvoje",
    "tvá",
    "tvé",
    "tví",
    "náš",
    "naše",
    "naši",
    "váš",
    "vaše",
    "vaši",
    "svůj",
    "svoje",
    "svá",
    "své",
    "jeho",
    "její",
    "jejich",
    // Demonstratives.
    "ten",
    "ta",
    "to",
    "toto",
    "tato",
    "tyto",
    "tuto",
    "tenhle",
    "tahle",
    "tohle",
    "onen",
    "takový",
    "taková",
    "takové",
    "takoví",
    // Interrogatives + relative pronouns.
    "kdo",
    "co",
    "který",
    "která",
    "které",
    "kteří",
    "jaký",
    "jaká",
    "jaké",
    "jací",
    "čí",
    "kde",
    "kam",
    "odkud",
    "kudy",
    "kdy",
    "jak",
    "proč",
    "kolik",
    "což",
    "jenž",
    "jež",
    // Coordinating / subordinating conjunctions.
    "a",
    "i",
    "ani",
    "nebo",
    "anebo",
    "ale",
    "však",
    "avšak",
    "nýbrž",
    "že",
    "aby",
    "když",
    "kdyby",
    "protože",
    "jelikož",
    "poněvadž",
    "pokud",
    "jestli",
    "jestliže",
    "jakmile",
    "než",
    "ač",
    "ačkoli",
    "ačkoliv",
    "sice",
    "totiž",
    // Prepositions.
    "v",
    "ve",
    "na",
    "za",
    "pod",
    "nad",
    "před",
    "po",
    "do",
    "z",
    "ze",
    "s",
    "u",
    "k",
    "ke",
    "ku",
    "od",
    "pro",
    "bez",
    "o",
    "při",
    "mezi",
    "kolem",
    "kromě",
    "mimo",
    "podle",
    "vedle",
    "díky",
    "místo",
    "proti",
    "vůči",
    // Particles + negation.
    "ne",
    "ano",
    "jo",
    "prý",
    "asi",
    "snad",
    "možná",
    "vlastně",
    "opravdu",
    "právě",
    "hlavně",
    "zejména",
    "již",
    "už",
    "ještě",
    "také",
    "též",
    "jen",
    "jenom",
    "pouze",
    "aspoň",
    "alespoň",
    "prostě",
    // Copula být — high-frequency forms.
    "být",
    "jsem",
    "jsi",
    "je",
    "jsme",
    "jste",
    "jsou",
    "byl",
    "byla",
    "bylo",
    "byli",
    "byly",
    "bude",
    "budu",
    "budeš",
    "budeme",
    "budete",
    "budou",
    "není",
    "nejsem",
    "nejsi",
    "nejsme",
    "nejste",
    "nejsou",
    "nebyl",
    "nebyla",
    "nebylo",
    "nebyli",
    "nebyly",
    "nebude",
    // Common adverbs.
    "tady",
    "tu",
    "zde",
    "tam",
    "sem",
    "onde",
    "všude",
    "nikde",
    "někde",
    "kdesi",
    "teď",
    "nyní",
    "tehdy",
    "potom",
    "pak",
    "dnes",
    "včera",
    "zítra",
    "vždy",
    "vždycky",
    "nikdy",
    "občas",
    "často",
    "zřídka",
    "velmi",
    "hodně",
    "málo",
    "moc",
    "trochu",
    "docela",
    "úplně",
    "zcela",
    // Quantifiers + generic determiners.
    "jeden",
    "jedna",
    "jedno",
    "dva",
    "dvě",
    "tři",
    "všichni",
    "všechny",
    "všechna",
    "všechno",
    "všech",
    "každý",
    "každá",
    "každé",
    "jiný",
    "jiná",
    "jiné",
    "další",
    "žádný",
    "žádná",
    "žádné",
    "nikdo",
    "nic",
    "něco",
    "někdo",
    "sám",
    "sama",
    "samo",
    "sami",
    "samý",
    "samá",
    "samé",
    "více",
    "méně",
    "nejvíce",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 275". The test allows
        // a generous 200-320 range to accommodate future additions
        // and light trimming.
        assert!(
            STOPWORDS.len() >= 200 && STOPWORDS.len() <= 320,
            "STOPWORDS.len() = {} outside the 200-320 range",
            STOPWORDS.len()
        );
    }

    #[test]
    fn every_stopword_is_lowercase() {
        // Every character of every entry should be lowercase under
        // default Unicode rules.
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !c.is_uppercase(),
                    "stopword {w:?} contains an uppercase character {c:?}"
                );
            }
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
    fn no_duplicates() {
        // O(n^2) is fine for a static list of ~180 entries. Homograph
        // surface forms (`je` = pronoun / copula, `ty` = pronoun /
        // demonstrative, etc.) are stored exactly once — see the
        // module-level docs.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["já", "ty", "on", "ona", "ono", "my", "vy", "oni"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["a", "i", "nebo", "ale", "že", "když", "protože"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["v", "na", "za", "s", "z", "do", "pro", "od", "k", "o"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in [
            "být", "jsem", "je", "jsou", "byl", "byla", "bylo", "bude", "není",
        ] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn czech_specific_letters_appear_somewhere() {
        // Sanity: several Czech-specific letters should show up in the
        // stopword list.
        let joined: String = STOPWORDS.join("");
        for c in ['á', 'í', 'ě', 'ž', 'ř', 'š', 'č', 'ů'] {
            assert!(
                joined.contains(c),
                "Czech-specific letter {c:?} does not appear anywhere in STOPWORDS"
            );
        }
    }
}
