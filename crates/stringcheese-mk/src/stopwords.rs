//! The Macedonian stopword list.
//!
//! Roughly 90 common Macedonian words drawn from published Macedonian
//! function-word inventories (Ranks NL-style Slavic lists, community
//! Macedonian corpora function-word lists). The list is deliberately
//! conservative — the most frequent function words (personal /
//! demonstrative / interrogative pronouns, prepositions, conjunctions,
//! particles, high-frequency forms of the copula *сум*, common adverbs,
//! and quantifiers).
//!
//! # Cyrillic stopwords, Macedonian letter set
//!
//! Every entry below is a Cyrillic string in canonical lowercase form.
//! The Macedonian alphabet is a 31-letter subset of the Cyrillic block
//! and carries **five Macedonian-specific letters** the Bulgarian /
//! Russian / Ukrainian alphabets do not: `ѓ` (U+0453), `ќ` (U+045C),
//! `љ` (U+0459), `њ` (U+045A), `џ` (U+045F), `ѕ` (U+0455), and shares
//! `ј` (U+0458) with Serbian. Macedonian does not use `ъ`, `ь`, `ы`,
//! `э`, `ё`, `й`, `щ`, or `ю` / `я` (the /ju/ and /ja/ sequences are
//! written as `ју` and `ја`).
//!
//! Under Rust's default [`char::to_lowercase`], Macedonian uppercase
//! letters `А…Ш` fold to `а…ш` without any locale-specific tailoring,
//! so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (an ASCII-case-insensitive linear scan) would
//! *miss* uppercase Cyrillic input. The Macedonian
//! [`Language`](stringcheese_lang::Language) impl therefore overrides
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) with a
//! Cyrillic-aware Unicode case-fold.
//!
//! # Definite-article forms
//!
//! Macedonian's definite article is a noun-suffix, not a separate word,
//! with a **three-way proximal / medial / distal distinction** (a
//! feature Bulgarian lacks): `-ов` / `-ва` / `-во` / `-ве` for
//! near / this-here, `-от` / `-та` / `-то` / `-те` for the neutral
//! medial article, and `-он` / `-на` / `-но` / `-не` for distal /
//! that-yonder. A query for `книгата` ("the book") will not match a
//! stopword-list entry `книга` ("book") through the `is_stopword` scan
//! alone. The stemmer's article-stripping step (see
//! [`crate::stemmer`]) handles that collapse.
//!
//! # Non-goals
//!
//! - **Multi-word phrases.** Left out; downstream systems that want
//!   phrase-level filtering should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Macedonian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in Cyrillic
/// lowercase.
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns and clitic forms. (`си` and `се` also serve as
    // 2sg / 3pl copula forms; the reflexive `се` shares its surface form
    // with the reflexive clitic. Each string is stored once.)
    "јас",
    "ти",
    "тој",
    "таа",
    "тоа",
    "ние",
    "вие",
    "тие",
    "мене",
    "тебе",
    "него",
    "неа",
    "нив",
    "ме",
    "те",
    "го",
    "ја",
    "ги",
    "му",
    "ѝ",
    "им",
    "си",
    "се",
    "нѐ",
    "ве",
    // Possessive pronouns.
    "мој",
    "моја",
    "мое",
    "мои",
    "твој",
    "твоја",
    "твое",
    "твои",
    "негов",
    "нејзин",
    "наш",
    "наша",
    "наше",
    "наши",
    "ваш",
    "нивен",
    "свој",
    // Proximal / distal demonstratives. (The medial `тој / таа / тоа /
    // тие` also serves as the 3sg / 3pl personal pronouns, listed
    // once above.)
    "овој",
    "оваа",
    "ова",
    "овие",
    "оној",
    "онаа",
    "она",
    "оние",
    // Interrogatives.
    "кој",
    "која",
    "кое",
    "кои",
    "каков",
    "каква",
    "какво",
    "какви",
    "каде",
    "кога",
    "како",
    "зошто",
    "колку",
    "што",
    // Coordinating / subordinating conjunctions.
    "и",
    "а",
    "но",
    "или",
    "ако",
    "оти",
    "дека",
    "додека",
    "туку",
    "ниту",
    "не",
    "да",
    "па",
    // Prepositions.
    "во",
    "на",
    "за",
    "со",
    "од",
    "до",
    "по",
    "при",
    "под",
    "над",
    "пред",
    "зад",
    "меѓу",
    "околу",
    "без",
    "кон",
    "низ",
    "врз",
    "спрема",
    "според",
    // Particles.
    "би",
    "ќе",
    "ли",
    "нели",
    "ете",
    "еве",
    "уште",
    "само",
    "исто",
    // Copula сум — high-frequency forms not already listed above.
    "сум",
    "е",
    "сме",
    "сте",
    "бев",
    "беше",
    "бе",
    "бевме",
    "бевте",
    "беа",
    "бил",
    "била",
    "било",
    "биле",
    // Common adverbs.
    "сега",
    "тогаш",
    "потоа",
    "денес",
    "вчера",
    "утре",
    "често",
    "секогаш",
    "никогаш",
    "многу",
    "малку",
    "речиси",
    "веќе",
    // Quantifiers and generic determiners.
    "еден",
    "една",
    "едно",
    "едни",
    "два",
    "две",
    "три",
    "секој",
    "секоја",
    "секое",
    "сите",
    "некој",
    "некоја",
    "некое",
    "некои",
    "друг",
    "друга",
    "друго",
    "други",
    "сѐ",
    "ништо",
    "никој",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The task spec targets ~70 words as a soft floor. The list is
        // slightly larger to keep coverage of copula forms and the
        // demonstrative triple (proximal / medial / distal), and the
        // test allows a generous ceiling to accommodate future
        // additions.
        assert!(
            STOPWORDS.len() >= 70 && STOPWORDS.len() <= 200,
            "STOPWORDS.len() = {} outside the 70-200 range",
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
    fn no_entries_contain_non_macedonian_cyrillic() {
        // Macedonian does not use Russian's ё, ы, э, ъ, ь, й, щ, ю, я,
        // or Ukrainian's і, ї, є, ґ. If any entry slipped through with
        // one of them, the stopword list is mis-scoped.
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !matches!(
                        c,
                        'ё' | 'ы'
                            | 'э'
                            | 'ъ'
                            | 'ь'
                            | 'й'
                            | 'щ'
                            | 'ю'
                            | 'я'
                            | 'і'
                            | 'ї'
                            | 'є'
                            | 'ґ'
                    ),
                    "stopword {w:?} contains non-Macedonian letter {c:?}"
                );
            }
        }
    }

    #[test]
    fn no_entries_contain_ascii_whitespace() {
        for &w in STOPWORDS {
            assert!(
                !w.chars().any(char::is_whitespace),
                "stopword {w:?} contains whitespace — must be a single word"
            );
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["јас", "ти", "тој", "таа", "тоа", "ние", "вие", "тие"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["и", "а", "но", "или", "не", "ако", "да"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["во", "на", "со", "од", "до", "по", "за"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["сум", "е", "сме", "сте", "бев"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn macedonian_specific_letters_appear() {
        // At least one Macedonian-specific letter (ѓ ќ љ њ џ ѕ ј) should
        // appear somewhere in the list. `ј` shows up prolifically —
        // `јас`, `мој`, `тој`, `кој`, and many others.
        let mut has_j = false;
        for &w in STOPWORDS {
            if w.contains('ј') {
                has_j = true;
                break;
            }
        }
        assert!(
            has_j,
            "Macedonian letter ј does not appear anywhere in STOPWORDS"
        );
    }

    #[test]
    fn no_duplicates() {
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }
}
