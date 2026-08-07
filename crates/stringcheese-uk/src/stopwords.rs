//! The Ukrainian stopword list.
//!
//! Roughly 220 common Ukrainian words drawn from the union of
//! published Ukrainian stopword collections (Solariz's
//! `ukrainian-stopwords`, the Snowball community Ukrainian list, and
//! NLTK-adjacent Slavic function-word inventories). The list is
//! deliberately conservative —
//! the most frequent function words (personal / demonstrative /
//! interrogative pronouns, prepositions, conjunctions, particles,
//! high-frequency forms of the copula *бути*, common adverbs, and
//! quantifiers).
//!
//! # Cyrillic stopwords, Ukrainian letter set
//!
//! Every entry below is a Cyrillic string in canonical lowercase form.
//! Ukrainian uses the extended Cyrillic block: alongside the letters
//! shared with Russian, Ukrainian carries **ґ (U+0491)**,
//! **є (U+0454)**, **і (U+0456)**, **ї (U+0457)**, and it does *not*
//! carry Russian's **ъ (U+044A)**, **ы (U+044B)**, **ё (U+0451)**, or
//! **э (U+044D)**. Under Rust's default [`char::to_lowercase`],
//! Ukrainian uppercase letters `А…Я`, `Ґ`, `Є`, `І`, `Ї` fold to
//! `а…я`, `ґ`, `є`, `і`, `ї` respectively without any locale-specific
//! tailoring, so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (an ASCII-case-insensitive linear scan) would *miss*
//! uppercase Cyrillic input. The Ukrainian
//! [`Language`](stringcheese_lang::Language) impl therefore overrides
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) with a
//! Cyrillic-aware Unicode case-fold.
//!
//! # Non-goals
//!
//! - **Regional / historical variants.** No pre-orthographic-reform
//!   spellings (the historical `ѣ` scalar is not carried).
//! - **Multi-word phrases.** Entries like `тому що` (a two-word causal
//!   conjunction) would never match a single token — they are left out
//!   of the list; downstream systems that want phrase-level filtering
//!   should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.
//! - **Homograph disambiguation.** Ukrainian has several strings that
//!   are function words under one part of speech and content words
//!   under another (`та` = "and" conjunction / feminine demonstrative
//!   "that"; `саме` = "exactly" adverb / neuter form of "самий"). The
//!   list stores each surface form once; downstream applications that
//!   need part-of-speech-aware filtering should carry their own
//!   annotations.

/// The Ukrainian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is stored in Cyrillic
/// lowercase.
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns.
    "я",
    "ти",
    "він",
    "вона",
    "воно",
    "ми",
    "ви",
    "вони",
    "мене",
    "тебе",
    "його",
    "її",
    "нас",
    "вас",
    "їх",
    "мені",
    "тобі",
    "йому",
    "їй",
    "нам",
    "вам",
    "їм",
    "мною",
    "тобою",
    "ним",
    "нею",
    "нами",
    "вами",
    "ними",
    "себе",
    "собі",
    "собою",
    // Possessive pronouns.
    "мій",
    "моя",
    "моє",
    "мої",
    "твій",
    "твоя",
    "твоє",
    "твої",
    "наш",
    "наша",
    "наше",
    "наші",
    "ваш",
    "ваша",
    "ваше",
    "ваші",
    "свій",
    "своя",
    "своє",
    "свої",
    // Demonstratives + locative adverbs. Note: `та` appears here as
    // the feminine "that" AND (see conjunctions section) as the
    // conjunction "and"; the string is kept in the demonstrative
    // section since that is its more common categorization in
    // Ukrainian IR stopword collections.
    "цей",
    "ця",
    "це",
    "ці",
    "той",
    "та",
    "те",
    "ті",
    "такий",
    "така",
    "таке",
    "такі",
    "тут",
    "там",
    "туди",
    "сюди",
    "звідти",
    "звідси",
    // Interrogatives.
    "хто",
    "що",
    "який",
    "яка",
    "яке",
    "які",
    "чий",
    "чия",
    "чиї",
    "де",
    "куди",
    "звідки",
    "коли",
    "як",
    "чому",
    "навіщо",
    "скільки",
    // Coordinating / subordinating conjunctions. (`та` — the additive
    // conjunction "and" — shares its surface form with the feminine
    // demonstrative and lives in that section above.)
    "і",
    "й",
    "а",
    "але",
    "або",
    "чи",
    "ж",
    "ні",
    "не",
    "хоча",
    "поки",
    "якщо",
    "щоб",
    "щоби",
    "бо",
    "тому",
    "адже",
    // Prepositions.
    "в",
    "у",
    "на",
    "за",
    "під",
    "над",
    "перед",
    "по",
    "до",
    "з",
    "зі",
    "із",
    "від",
    "для",
    "без",
    "про",
    "при",
    "через",
    "між",
    "коло",
    "серед",
    "біля",
    "після",
    "крім",
    // Particles.
    "б",
    "би",
    "ось",
    "от",
    "вже",
    "ще",
    "лише",
    "тільки",
    "також",
    "теж",
    "нехай",
    "хай",
    // Modal / auxiliary high-frequency verbs and modal predicates.
    "можна",
    "треба",
    "потрібно",
    "повинен",
    "повинна",
    "повинно",
    "повинні",
    "може",
    "можуть",
    "хочу",
    "хоче",
    "хочемо",
    "хочуть",
    // Copula бути — high-frequency forms.
    "бути",
    "був",
    "була",
    "було",
    "були",
    "є",
    "буде",
    "будуть",
    "буду",
    "будеш",
    "будемо",
    "будете",
    // Common adverbs. (`саме` — "exactly" adverb — shares its surface
    // form with the neuter of "самий" and lives in the quantifier
    // section below.)
    "тепер",
    "зараз",
    "тоді",
    "потім",
    "спочатку",
    "сьогодні",
    "вчора",
    "завтра",
    "іноді",
    "часто",
    "рідко",
    "завжди",
    "ніколи",
    "дуже",
    "майже",
    "зовсім",
    "просто",
    "звичайно",
    "хіба",
    "невже",
    // Quantifiers and generic determiners.
    "один",
    "одна",
    "одно",
    "одні",
    "два",
    "дві",
    "три",
    "багато",
    "мало",
    "більше",
    "менше",
    "самий",
    "сама",
    "саме",
    "самі",
    "кожен",
    "кожна",
    "кожне",
    "кожні",
    "інший",
    "інша",
    "інше",
    "інші",
    "весь",
    "вся",
    "все",
    "всіх",
    "всякий",
    "ніхто",
    "ніщо",
    "нічого",
    "нікого",
    "нікому",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 220". The task spec
        // targets ~150-200 as a soft floor; the test allows a generous
        // 150-260 to accommodate future additions.
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
    fn no_entries_contain_russian_only_letters() {
        // Ukrainian does not use Russian's ъ, ы, ё, э. If any entry
        // slipped through with one of them, the stopword list is
        // mis-scoped.
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !matches!(c, 'ъ' | 'ы' | 'ё' | 'э' | 'Ъ' | 'Ы' | 'Ё' | 'Э'),
                    "stopword {w:?} contains Russian-only letter {c:?}"
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
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["я", "ти", "він", "вона", "воно", "ми", "ви", "вони"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["і", "й", "а", "але", "або", "не", "якщо"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["в", "у", "на", "з", "до", "від", "по", "для"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["бути", "був", "була", "було", "були", "є"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn ukrainian_only_letters_appear_somewhere() {
        // Sanity: the Ukrainian-specific letters є, і, ї should show
        // up somewhere in the stopword list. ґ is rare in function
        // words so is not asserted here.
        let joined: String = STOPWORDS.join("");
        for c in ['і', 'ї', 'є'] {
            assert!(
                joined.contains(c),
                "Ukrainian-specific letter {c:?} does not appear anywhere in STOPWORDS"
            );
        }
    }
}
