//! The Belarusian stopword list.
//!
//! Roughly 85 common Belarusian words drawn from the union of published
//! Belarusian stopword collections (Slavic function-word inventories
//! adapted for Belarusian, Wikipedia's `be.wikipedia` corpora function-
//! word extractions, and pymorphy2-adjacent lists). The list targets
//! the Narkamaŭka orthography — Belarus's official standard — rather
//! than the classical Taraškievič variant; the two orthographies share
//! most function words but differ on assimilation patterns for a
//! handful of pronoun forms (Taraškievič `нікога` vs Narkamaŭka
//! `нікога` are the same here; the two would only diverge on the
//! placement of the soft sign in words like `сьвет` vs `свет` — the
//! stopword list carries the Narkamaŭka shape throughout).
//!
//! # Cyrillic stopwords, Belarusian letter set
//!
//! Every entry below is a Cyrillic string in canonical lowercase form.
//! Belarusian uses the extended Cyrillic block: alongside the letters
//! shared with Russian, Belarusian carries **ў (U+045E)** and
//! **і (U+0456)**, and it does *not* carry Russian's **и (U+0438)**,
//! **щ (U+0449)**, **ъ (U+044A)**, or **э (U+044D — Belarusian uses
//! `э` — but the classical Taraškievič form also drops it in some
//! positions)**. Belarusian *does* carry `э` (U+044D) — it is a
//! full Belarusian vowel and appears in `гэта`, `яшчэ`. Under Rust's
//! default [`char::to_lowercase`], Belarusian uppercase letters
//! `А…Я`, `Ў`, `І` fold to `а…я`, `ў`, `і` respectively without any
//! locale-specific tailoring, so the default
//! [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//! implementation (an ASCII-case-insensitive linear scan) would *miss*
//! uppercase Cyrillic input. The Belarusian
//! [`Language`](stringcheese_lang::Language) impl therefore overrides
//! [`is_stopword`](stringcheese_lang::Language::is_stopword) with a
//! Cyrillic-aware Unicode case-fold.
//!
//! # Non-goals
//!
//! - **Taraškievič-only forms.** The classical orthography spells a
//!   handful of function words with an explicit soft sign
//!   (`сьвет`, `дзеньнік`); the shipped list carries the Narkamaŭka
//!   shape without the extra soft sign.
//! - **Multi-word phrases.** Entries like `таму што` (a two-word causal
//!   conjunction) would never match a single token — they are left
//!   out of the list; downstream systems that want phrase-level
//!   filtering should carry their own list.
//! - **Domain-specific stopwords.** IR practice for legal, medical, or
//!   scientific corpora typically extends the general list. Downstream
//!   applications should carry their own.

/// The Belarusian stopword list.
///
/// A `&'static [&'static str]` — the language pack's
/// [`stopwords`](stringcheese_lang::Language::stopwords) accessor hands
/// back exactly this slice. Every entry is stored in Cyrillic
/// lowercase.
pub const STOPWORDS: &[&str] = &[
    // Personal pronouns.
    "я",
    "ты",
    "ён",
    "яна",
    "яно",
    "мы",
    "вы",
    "яны",
    // Object / oblique pronouns.
    "мяне",
    "цябе",
    "яго",
    "яе",
    "нас",
    "вас",
    "іх",
    "мне",
    "табе",
    "яму",
    "ёй",
    "ім",
    // Possessive pronouns.
    "мой",
    "мая",
    "маё",
    "мае",
    "твой",
    "твая",
    "наш",
    "наша",
    "ваш",
    "ваша",
    "свой",
    // Demonstratives.
    "гэта",
    "гэты",
    "гэтая",
    "гэтае",
    "той",
    "тая",
    "тое",
    "тыя",
    // Interrogatives.
    "хто",
    "што",
    "які",
    "якая",
    "якое",
    "якія",
    "як",
    "дзе",
    "куды",
    "калі",
    "чаму",
    // Coordinating / subordinating conjunctions.
    "і",
    "а",
    "але",
    "або",
    "ці",
    "ды",
    "ж",
    // Prepositions. `у` and `ў` are both prepositions ("in") whose
    // choice depends on whether the preceding word ends in a vowel;
    // both appear as stopwords.
    "у",
    "ў",
    "на",
    "за",
    "пад",
    "над",
    "перад",
    "па",
    "да",
    "з",
    "ад",
    "для",
    "без",
    "аб",
    "пры",
    "праз",
    // Particles.
    "не",
    "ні",
    "бы",
    "б",
    "ужо",
    "яшчэ",
    "толькі",
    "таксама",
    // Copula быць — high-frequency forms.
    "быць",
    "ёсць",
    "быў",
    "была",
    "было",
    "былі",
    "будзе",
    "будуць",
    // Common adverbs.
    "тут",
    "там",
    "вось",
    "вельмі",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopword_list_size_is_within_the_advertised_range() {
        // The doc-comment above says "roughly 85". The task spec targets
        // ~70 as a soft floor; the test allows a generous 60-120 to
        // accommodate future additions.
        assert!(
            STOPWORDS.len() >= 60 && STOPWORDS.len() <= 120,
            "STOPWORDS.len() = {} outside the 60-120 range",
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
        // Belarusian does not use Russian's и, щ, ъ. If any entry
        // slipped through with one of them, the stopword list is
        // mis-scoped. (Belarusian *does* use э and ы, so those are
        // allowed.)
        for &w in STOPWORDS {
            for c in w.chars() {
                assert!(
                    !matches!(c, 'и' | 'щ' | 'ъ' | 'И' | 'Щ' | 'Ъ'),
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
        for w in ["я", "ты", "ён", "яна", "яно", "мы", "вы", "яны"] {
            assert!(STOPWORDS.contains(&w), "pronoun {w:?} is missing");
        }
    }

    #[test]
    fn common_conjunctions_are_present() {
        for w in ["і", "а", "але", "або", "не"] {
            assert!(STOPWORDS.contains(&w), "conjunction {w:?} is missing");
        }
    }

    #[test]
    fn common_prepositions_are_present() {
        for w in ["у", "ў", "на", "з", "да", "ад", "па", "для"] {
            assert!(STOPWORDS.contains(&w), "preposition {w:?} is missing");
        }
    }

    #[test]
    fn common_copula_forms_are_present() {
        for w in ["быць", "быў", "была", "было", "былі", "ёсць"] {
            assert!(STOPWORDS.contains(&w), "copula form {w:?} is missing");
        }
    }

    #[test]
    fn belarusian_short_u_appears_somewhere() {
        // Sanity: the Belarusian-specific ў should show up somewhere in
        // the stopword list (at minimum as the preposition variant).
        let joined: String = STOPWORDS.join("");
        assert!(
            joined.contains('ў'),
            "Belarusian-specific letter 'ў' does not appear anywhere in STOPWORDS"
        );
    }

    #[test]
    fn belarusian_i_appears_somewhere() {
        // Belarusian uses і (U+0456) rather than Russian's и (U+0438).
        let joined: String = STOPWORDS.join("");
        assert!(
            joined.contains('і'),
            "Belarusian-specific letter 'і' does not appear anywhere in STOPWORDS"
        );
    }
}
