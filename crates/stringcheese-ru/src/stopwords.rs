//! The Russian stopword list.
//!
//! Roughly 235 common Russian words drawn from the union of NLTK's
//! Russian list and the Snowball project's Russian stopword collection.
//! The list is deliberately conservative — the most frequent function
//! words (personal / demonstrative / interrogative pronouns,
//! prepositions, conjunctions, particles, the high-frequency forms of
//! the copula *быть*, common adverbs, and quantifiers).
//!
//! # Cyrillic stopwords
//!
//! Every entry below is a Cyrillic string in canonical lowercase form.
//! Under Rust's default [`char::to_lowercase`], Cyrillic uppercase
//! letters `А…Я` and `Ё` fold to `а…я` and `ё` respectively without
//! any locale-specific tailoring — Russian has no Turkic-style
//! case-fold quirks — so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (an ASCII-case-insensitive linear scan) would
//! *miss* uppercase Cyrillic input. The Russian
//! [`Language`](stringcheese_lang::Language) impl therefore overrides
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) with a
//! Cyrillic-aware Unicode case-fold.
//!
//! # ё vs е
//!
//! Russian orthography historically drops the diaeresis on `ё`, so
//! `ежик` and `ёжик` both refer to the same word. The Russian
//! `is_stopword` override performs `ё → е` folding at the call site
//! before comparison, and the stopword list stores entries in their
//! plain-`е` form for consistency.
//!
//! # Non-goals
//!
//! - **Regional / historical variants.** No pre-1918 orthography (the
//!   `ѣ`, `і`, `ѳ`, `ѵ` letters are not carried).
//! - **Multi-word phrases.** Entries like `потому что` (a two-word
//!   causal conjunction) would never match a single token — they are
//!   left out of the list; downstream systems that want phrase-level
//!   filtering should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Russian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor
/// hands back exactly this slice. Every entry is stored in Cyrillic
/// lowercase with plain-`е` (no `ё` — the pack's `is_stopword` override
/// folds `ё → е` before comparison).
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns.
    "я",
    "ты",
    "он",
    "она",
    "оно",
    "мы",
    "вы",
    "они",
    "меня",
    "тебя",
    "его",
    "ее",
    "нас",
    "вас",
    "их",
    "мне",
    "тебе",
    "ему",
    "ей",
    "нам",
    "вам",
    "им",
    "мной",
    "мною",
    "тобой",
    "тобою",
    "ним",
    "ней",
    "нею",
    "нами",
    "вами",
    "ими",
    "себя",
    "себе",
    "собой",
    "собою",
    // Possessive pronouns.
    "мой",
    "моя",
    "мое",
    "мои",
    "твой",
    "твоя",
    "твое",
    "твои",
    "наш",
    "наша",
    "наше",
    "наши",
    "ваш",
    "ваша",
    "ваше",
    "ваши",
    "свой",
    "своя",
    "свое",
    "свои",
    // Demonstratives + locative adverbs.
    "этот",
    "эта",
    "это",
    "эти",
    "тот",
    "та",
    "то",
    "те",
    "такой",
    "такая",
    "такое",
    "такие",
    "здесь",
    "тут",
    "там",
    "туда",
    "сюда",
    "оттуда",
    "отсюда",
    // Interrogatives.
    "кто",
    "что",
    "какой",
    "какая",
    "какое",
    "какие",
    "чей",
    "чья",
    "чьи",
    "где",
    "куда",
    "откуда",
    "когда",
    "как",
    "почему",
    "зачем",
    "сколько",
    // Coordinating / subordinating conjunctions.
    "и",
    "а",
    "но",
    "или",
    "либо",
    "да",
    "же",
    "ни",
    "не",
    "нет",
    "ли",
    "чтобы",
    "если",
    "потому",
    "хотя",
    "пока",
    "чем",
    "чтоб",
    // Prepositions.
    "в",
    "во",
    "на",
    "за",
    "под",
    "над",
    "перед",
    "по",
    "к",
    "ко",
    "с",
    "со",
    "у",
    "о",
    "об",
    "обо",
    "от",
    "ото",
    "до",
    "из",
    "изо",
    "для",
    "без",
    "про",
    "при",
    "через",
    "между",
    "около",
    "среди",
    // Particles.
    "бы",
    "б",
    "ведь",
    "вот",
    "уж",
    "уже",
    "еще",
    "лишь",
    "только",
    "тоже",
    "также",
    "вон",
    // Modal / auxiliary high-frequency verbs.
    "нельзя",
    "можно",
    "нужно",
    "надо",
    "должен",
    "должна",
    "должно",
    "должны",
    "может",
    "могут",
    "хочу",
    "хочет",
    "хотим",
    "хотят",
    // Copula быть — high-frequency forms.
    "быть",
    "был",
    "была",
    "было",
    "были",
    "есть",
    "будет",
    "будут",
    "буду",
    "будешь",
    "будем",
    "будете",
    // Common adverbs.
    "теперь",
    "сейчас",
    "тогда",
    "потом",
    "затем",
    "сначала",
    "сегодня",
    "вчера",
    "завтра",
    "иногда",
    "часто",
    "редко",
    "всегда",
    "никогда",
    "очень",
    "почти",
    "совсем",
    "просто",
    "именно",
    "конечно",
    "разве",
    "неужели",
    // Quantifiers and generic determiners.
    "один",
    "одна",
    "одно",
    "одни",
    "два",
    "две",
    "три",
    "много",
    "мало",
    "больше",
    "меньше",
    "самый",
    "самая",
    "самое",
    "самые",
    "каждый",
    "каждая",
    "каждое",
    "каждые",
    "любой",
    "любая",
    "любое",
    "любые",
    "другой",
    "другая",
    "другое",
    "другие",
    "весь",
    "вся",
    "все",
    "всех",
    "всякий",
    "никто",
    "ничто",
    "ничего",
    "никого",
    "никому",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 235". The task spec
        // targets ~150-200 as a soft floor; the test allows a
        // generous 150-260 to accommodate future additions.
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
    fn no_entries_contain_yo() {
        // Entries store plain `е`; the pack's ё-fold takes care of the
        // `ё` spelling at call time.
        for &w in STOPWORDS {
            assert!(
                !w.contains('ё'),
                "stopword {w:?} contains ё — should be stored with plain е"
            );
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
        for w in ["я", "ты", "он", "она", "оно", "мы", "вы", "они"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["и", "а", "но", "или", "не", "если"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["в", "на", "с", "у", "к", "по", "от", "до", "для"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["быть", "был", "была", "было", "были", "есть"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }
}
