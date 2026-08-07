//! The Slovak stopword list.
//!
//! Roughly 240 common Slovak words drawn from published Slovak stopword
//! collections (Ranks NL-style Slavic lists, the community `sk`
//! stopword inventories used in Slovak IR and search corpora, and
//! NLTK-adjacent function-word coverage). The count runs a bit higher
//! than the standard ~150-200 mark because Slovak, like Czech, has
//! rich morphology on its function words (the copula *byť* alone
//! contributes ~20 conjugated forms spanning the negated `ne-`
//! prefix; every gender / number variant of the possessive and
//! demonstrative pronouns is stored). The list is deliberately
//! conservative — the most frequent function words (personal /
//! demonstrative / interrogative pronouns, prepositions, conjunctions,
//! particles, high-frequency forms of the copula *byť*, common
//! adverbs, and quantifiers).
//!
//! # Slovak letter set
//!
//! Every entry below is a Slovak string in canonical lowercase form.
//! Slovak uses the extended Latin block: alongside the ASCII letters,
//! Slovak carries the haček (caron) consonants **č**, **ď**, **ľ**,
//! **ň**, **š**, **ť**, **ž**; the long vowels **á**, **é**, **í**,
//! **ó**, **ú**, **ý**; the syllabic long consonants **ĺ** and **ŕ**;
//! and the Slovak-specific **ä** (open-e vowel) and **ô** (diphthong
//! /uo/). Under Rust's default [`char::to_lowercase`], uppercase
//! Slovak-specific letters fold to their lowercase counterparts
//! without any locale-specific tailoring, so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (an ASCII-case-insensitive linear scan) would *miss*
//! uppercase inputs like `NIE` or `KEĎ`. The Slovak
//! [`Language`](stringcheese_lang::Language) impl therefore overrides
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) with a
//! Unicode case-fold.
//!
//! # Slovak vs. Czech
//!
//! The list overlaps with the Czech pack's stopword list but is not a
//! copy — Slovak function words differ in:
//!
//! * **Infinitive `-ť`.** Czech's `byt` / `byť` is `byť` (Slovak); the
//!   copula infinitive is spelled with `ť` here.
//! * **Slovak-only letters.** Entries like `keď`, `preč`, `späť`, and
//!   `najmä` carry the `ä` / `ô` / `ĺ` / `ľ` letters Czech does not
//!   have.
//! * **Different words entirely.** Slovak has *hej* (yes, colloq.),
//!   *lebo* (because), *keďže* (since), *hoci* (although),
//!   *ktorý/ktorá/ktoré*, whereas Czech has *ano*, *protože*,
//!   *jelikož/poněvadž*, *ačkoli*, *který/která/které*.
//! * **`ju/jej/ňou/im`** for third-person oblique instead of Czech's
//!   `ji/ni/jim`.
//!
//! # Homographs
//!
//! Slovak has several strings that are function words under one part
//! of speech and pronouns / demonstratives under another:
//!
//! * **`je`** — both the accusative-plural personal pronoun ("them")
//!   and the third-singular present of *byť* ("is"). Kept once.
//! * **`si`** — both the reflexive dative pronoun ("to oneself") and
//!   the second-singular present of *byť* ("you are"). Kept once.
//! * **`sa`** — the reflexive accusative pronoun ("oneself"). Kept
//!   once.
//! * **`ti`** — both the dative second-singular personal pronoun
//!   ("to you") and the masculine-personal-plural demonstrative
//!   ("those"). Kept once.
//! * **`i`** — both the additive conjunction ("and, also") and (rare
//!   / poetic) an emphatic particle. Kept once.
//!
//! Each surface form appears exactly once in the list; downstream
//! applications that need part-of-speech-aware filtering should carry
//! their own annotations.
//!
//! # Non-goals
//!
//! - **Multi-word phrases.** Entries like `aj keď` (a two-word
//!   concessive conjunction) would never match a single token — they
//!   are left out of the list; downstream systems that want
//!   phrase-level filtering should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Slovak stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is stored in Slovak lowercase.
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns (nominative + oblique cases).
    "ja",
    "ty",
    "on",
    "ona",
    "ono",
    "my",
    "vy",
    "oni",
    "ony",
    "ma",
    "ťa",
    "ho",
    "ju",
    "nás",
    "vás",
    "ich",
    "mi",
    "ti",
    "mu",
    "jej",
    "nám",
    "vám",
    "im",
    "mnou",
    "tebou",
    "ním",
    "ňou",
    "nami",
    "vami",
    "nimi",
    "sa",
    "si",
    "seba",
    "sebe",
    "sebou",
    // Possessive pronouns.
    "môj",
    "moja",
    "moje",
    "moji",
    "tvoj",
    "tvoja",
    "tvoje",
    "tvoji",
    "náš",
    "naša",
    "naše",
    "naši",
    "váš",
    "vaša",
    "vaše",
    "vaši",
    "svoj",
    "svoja",
    "svoje",
    "jeho",
    // Demonstratives.
    "ten",
    "tá",
    "to",
    "tí",
    "tie",
    "tento",
    "táto",
    "toto",
    "títo",
    "tieto",
    "taký",
    "taká",
    "také",
    "takí",
    // Interrogatives + relative pronouns.
    "kto",
    "čo",
    "ktorý",
    "ktorá",
    "ktoré",
    "ktorí",
    "aký",
    "aká",
    "aké",
    "akí",
    "čí",
    "kde",
    "kam",
    "odkiaľ",
    "kadiaľ",
    "kedy",
    "ako",
    "prečo",
    "koľko",
    // Coordinating / subordinating conjunctions.
    "a",
    "i",
    "ani",
    "alebo",
    "či",
    "ale",
    "však",
    "avšak",
    "že",
    "aby",
    "keď",
    "keby",
    "pretože",
    "lebo",
    "keďže",
    "ak",
    "pokiaľ",
    "akonáhle",
    "než",
    "hoci",
    "aj",
    "síce",
    "totiž",
    // Prepositions.
    "v",
    "vo",
    "na",
    "za",
    "pod",
    "nad",
    "pred",
    "po",
    "do",
    "z",
    "zo",
    "s",
    "so",
    "u",
    "k",
    "ku",
    "od",
    "pre",
    "bez",
    "o",
    "pri",
    "medzi",
    "okolo",
    "okrem",
    "mimo",
    "podľa",
    "vedľa",
    "vďaka",
    "miesto",
    "proti",
    "voči",
    // Particles + negation.
    "nie",
    "áno",
    "hej",
    "vraj",
    "asi",
    "snáď",
    "možno",
    "vlastne",
    "naozaj",
    "práve",
    "hlavne",
    "najmä",
    "už",
    "ešte",
    "tiež",
    "len",
    "iba",
    "aspoň",
    "jednoducho",
    // Copula byť — high-frequency forms.
    "byť",
    "som",
    "je",
    "sme",
    "ste",
    "sú",
    "bol",
    "bola",
    "bolo",
    "boli",
    "budem",
    "budeš",
    "bude",
    "budeme",
    "budete",
    "budú",
    "nebol",
    "nebola",
    "nebolo",
    "neboli",
    "nebude",
    // Common adverbs.
    "tu",
    "sem",
    "tam",
    "hore",
    "dole",
    "všade",
    "nikde",
    "niekde",
    "teraz",
    "potom",
    "dnes",
    "včera",
    "zajtra",
    "vždy",
    "nikdy",
    "občas",
    "často",
    "zriedka",
    "veľmi",
    "veľa",
    "málo",
    "trochu",
    "dosť",
    "úplne",
    "celkom",
    "späť",
    // Quantifiers + generic determiners.
    "jeden",
    "jedna",
    "jedno",
    "dva",
    "dve",
    "tri",
    "všetci",
    "všetky",
    "všetko",
    "každý",
    "každá",
    "každé",
    "iný",
    "iná",
    "iné",
    "ďalší",
    "žiadny",
    "žiadna",
    "žiadne",
    "nikto",
    "nič",
    "niečo",
    "niekto",
    "sám",
    "sama",
    "samo",
    "viac",
    "menej",
    "najviac",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 180-240". The test
        // allows a generous 150-260 range to accommodate future
        // additions and light trimming.
        assert!(
            STOPWORDS.len() >= 150 && STOPWORDS.len() <= 260,
            "STOPWORDS.len() = {} outside the 150-260 range",
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
        // surface forms (`je` = pronoun / copula, `si` = reflexive /
        // copula, `ti` = pronoun / demonstrative, etc.) are stored
        // exactly once — see the module-level docs.
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["ja", "ty", "on", "ona", "ono", "my", "vy", "oni"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["a", "i", "alebo", "ale", "že", "keď", "pretože", "lebo"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["v", "na", "za", "s", "z", "do", "pre", "od", "k", "o"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["byť", "som", "je", "sú", "bol", "bola", "bolo", "bude"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn slovak_specific_letters_appear_somewhere() {
        // Sanity: several Slovak-specific letters should show up in
        // the stopword list. `ä` (najmä), `ô` (môj), `ľ` (podľa,
        // koľko), `č` (čo, či), `š` (náš, váš), `ž` (že, žiadny),
        // `á/é/í/ó/ú/ý` (many entries).
        let joined: String = STOPWORDS.join("");
        for c in ['á', 'í', 'ý', 'ž', 'š', 'č', 'ä', 'ô', 'ľ', 'ť', 'ď', 'ň'] {
            assert!(
                joined.contains(c),
                "Slovak-specific letter {c:?} does not appear anywhere in STOPWORDS"
            );
        }
    }
}
