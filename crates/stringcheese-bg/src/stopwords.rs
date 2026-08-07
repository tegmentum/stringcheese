//! The Bulgarian stopword list.
//!
//! Roughly 236 common Bulgarian words drawn from published Bulgarian
//! stopword collections (Ranks NL Bulgarian, community Snowball-style
//! Slavic function-word inventories, University of Sofia's `BulTreeBank`
//! function-word annotations). The list is deliberately conservative —
//! the most frequent function words (personal / demonstrative /
//! interrogative pronouns, prepositions, conjunctions, particles,
//! high-frequency forms of the copula *съм*, common adverbs, and
//! quantifiers).
//!
//! # Cyrillic stopwords, Bulgarian letter set
//!
//! Every entry below is a Cyrillic string in canonical lowercase form.
//! Bulgarian uses a 30-letter subset of the Cyrillic block: it shares
//! most letters with Russian but **omits** Russian's `ё` (U+0451),
//! `ы` (U+044B), and `э` (U+044D), and **repurposes** `ъ` (U+044A) as
//! a full vowel /ɤ/ rather than the Russian hard-sign glyph.
//!
//! Under Rust's default [`char::to_lowercase`], Bulgarian uppercase
//! letters `А…Я` fold to `а…я` without any locale-specific
//! tailoring, so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (an ASCII-case-insensitive linear scan) would
//! *miss* uppercase Cyrillic input. The Bulgarian
//! [`Language`](stringcheese_lang::Language) impl therefore overrides
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) with a
//! Cyrillic-aware Unicode case-fold.
//!
//! # Definite-article forms
//!
//! Bulgarian's definite article is a noun-suffix, not a separate word,
//! so a query for `книгата` ("the book") will not match a stopword-
//! list entry `книга` ("book") through the `is_stopword` scan alone.
//! For function words that themselves change shape when definite
//! (e.g. `всичко` — "everything" — vs `всичкото`), the list carries
//! both surface forms. The stemmer's article-stripping step
//! (see [`crate::snowball`]) also collapses the two.
//!
//! # Non-goals
//!
//! - **Multi-word phrases.** Entries like `за да` (a two-word
//!   subordinating conjunction "in order to") would never match a
//!   single token — they are left out of the list; downstream systems
//!   that want phrase-level filtering should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Bulgarian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in Cyrillic
/// lowercase.
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns.
    "аз",
    "ти",
    "той",
    "тя",
    "то",
    "ние",
    "вие",
    "те",
    "мен",
    "мене",
    "теб",
    "тебе",
    "него",
    "нея",
    "нас",
    "вас",
    "тях",
    "ми",
    "му",
    "ѝ",
    "ни",
    "ви",
    "им",
    "се",
    "си",
    "ме",
    "го",
    "я",
    "ги",
    // Possessive pronouns.
    "мой",
    "моя",
    "мое",
    "мои",
    "моят",
    "моето",
    "моята",
    "моите",
    "твой",
    "твоя",
    "твое",
    "твои",
    "твоят",
    "твоето",
    "твоята",
    "твоите",
    "негов",
    "негова",
    "негово",
    "негови",
    "неин",
    "нейна",
    "нейно",
    "нейни",
    "наш",
    "наша",
    "наше",
    "наши",
    "нашият",
    "нашето",
    "нашата",
    "нашите",
    "ваш",
    "ваша",
    "ваше",
    "ваши",
    "техен",
    "тяхна",
    "тяхно",
    "техни",
    "свой",
    "своя",
    "свое",
    "свои",
    // Demonstratives.
    "този",
    "тази",
    "това",
    "тези",
    "онзи",
    "онази",
    "онова",
    "онези",
    "такъв",
    "такава",
    "такова",
    "такива",
    "тук",
    "там",
    "насам",
    "натам",
    "оттук",
    "оттам",
    // Interrogatives.
    "кой",
    "коя",
    "кое",
    "кои",
    "какъв",
    "каква",
    "какво",
    "какви",
    "чий",
    "чия",
    "чие",
    "чии",
    "къде",
    "накъде",
    "откъде",
    "кога",
    "как",
    "защо",
    "колко",
    // Coordinating / subordinating conjunctions.
    "и",
    "а",
    "но",
    "или",
    "ли",
    "ако",
    "че",
    "щом",
    "докато",
    "дори",
    "нито",
    "не",
    "да",
    "недей",
    "нима",
    // Prepositions.
    "в",
    "във",
    "на",
    "за",
    "със",
    "с",
    "от",
    "до",
    "по",
    "при",
    "под",
    "над",
    "пред",
    "зад",
    "между",
    "около",
    "срещу",
    "чрез",
    "без",
    "през",
    "върху",
    "относно",
    "спрямо",
    "според",
    // Particles.
    "би",
    "бих",
    "ей",
    "ето",
    "нали",
    "май",
    "чак",
    "точно",
    "именно",
    "все",
    "още",
    "само",
    "тъкмо",
    "също",
    // Copula съм — high-frequency forms. (2sg `си` shares its
    // surface form with the reflexive clitic pronoun in the
    // personal-pronouns section above; the string is stored once.)
    "съм",
    "е",
    "сме",
    "сте",
    "са",
    "бях",
    "беше",
    "бе",
    "бяхме",
    "бяхте",
    "бяха",
    "бил",
    "била",
    "било",
    "били",
    "бъда",
    "бъдеш",
    "бъде",
    "бъдем",
    "бъдете",
    "бъдат",
    "ще",
    "щях",
    "щеше",
    "щяхме",
    "щяха",
    // Common adverbs.
    "сега",
    "тогава",
    "после",
    "днес",
    "вчера",
    "утре",
    "понякога",
    "често",
    "рядко",
    "винаги",
    "никога",
    "много",
    "малко",
    "твърде",
    "почти",
    "съвсем",
    "просто",
    // Quantifiers and generic determiners.
    "един",
    "една",
    "едно",
    "едни",
    "два",
    "две",
    "три",
    "всеки",
    "всяка",
    "всяко",
    "всички",
    "някой",
    "някоя",
    "някое",
    "някои",
    "друг",
    "друга",
    "друго",
    "други",
    "самият",
    "самата",
    "самото",
    "самите",
    "всичко",
    "всичкото",
    "нищо",
    "никой",
    "никоя",
    "никое",
    "никои",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 236". The task spec
        // targets ~120-180 as a soft floor; the test allows a
        // generous range to accommodate future additions.
        assert!(
            STOPWORDS.len() >= 120 && STOPWORDS.len() <= 280,
            "STOPWORDS.len() = {} outside the 120-280 range",
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
        // Bulgarian does not use Russian's ё, ы, э. If any entry
        // slipped through with one of them, the stopword list is
        // mis-scoped. Note: ъ IS a valid Bulgarian letter (it is a
        // vowel), unlike in Russian where it is a hard-sign glyph;
        // ъ is therefore not excluded here.
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !matches!(c, 'ё' | 'ы' | 'э' | 'Ё' | 'Ы' | 'Э'),
                    "stopword {w:?} contains Russian-only letter {c:?}"
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
    fn no_duplicates() {
        for (i, &w) in STOPWORDS.iter().enumerate() {
            for &v in &STOPWORDS[i + 1..] {
                assert_ne!(w, v, "duplicate stopword: {w:?}");
            }
        }
    }

    #[test]
    fn common_pronouns_are_present() {
        for w in ["аз", "ти", "той", "тя", "то", "ние", "вие", "те"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["и", "а", "но", "или", "не", "ако", "че"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["в", "на", "с", "от", "до", "по", "за"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["съм", "е", "сме", "сте", "са", "бях"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn bulgarian_specific_letter_appears() {
        // ъ is a full vowel in Bulgarian and shows up in high-frequency
        // words — the copula past `бях`, the modal `бъда`, and others.
        let joined: String = STOPWORDS.join("");
        assert!(
            joined.contains('ъ'),
            "Bulgarian vowel ъ does not appear anywhere in STOPWORDS"
        );
    }
}
