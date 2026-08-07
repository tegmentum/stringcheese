//! Integration tests for the Russian [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_ru::RUSSIAN;
#[cfg(feature = "slavic-metaphone")]
use stringcheese_ru::{RUSSIAN_WITH_SLAVIC_METAPHONE, Russian};

#[test]
fn code_and_name() {
    assert_eq!(RUSSIAN.code(), "ru");
    assert_eq!(RUSSIAN.name(), "Russian");
}

#[test]
fn common_russian_words_are_stopwords() {
    for w in ["и", "в", "на", "не", "с", "по", "для", "я", "ты", "он"] {
        assert!(RUSSIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_cyrillic_fold() {
    // Cyrillic case-fold (Rust default Unicode fold): А → а, Я → я, etc.
    // Uppercase queries fold correctly to the plain-е lowercase list.
    assert!(RUSSIAN.is_stopword("НЕ"));
    assert!(RUSSIAN.is_stopword("Не"));
    assert!(RUSSIAN.is_stopword("В"));
    assert!(!RUSSIAN.is_stopword("МОСКВА")); // Not a stopword.
}

#[test]
fn stopword_lookup_folds_yo_to_ye() {
    // The list stores "еще" — a query with `ё` should match too.
    assert!(RUSSIAN.is_stopword("ещё"));
    assert!(RUSSIAN.is_stopword("ЕЩЁ"));
    assert!(RUSSIAN.is_stopword("Ещё"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["книга", "собака", "компьютер", "программа"] {
        assert!(!RUSSIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_russian_sentence() {
    let text = "Привет, мир! Москва — столица России.";
    let toks: Vec<&str> = RUSSIAN.tokenize(text).collect();
    assert_eq!(toks, ["Привет", "мир", "Москва", "столица", "России"]);
}

#[test]
fn stem_a_few_russian_words() {
    // Adjective + noun + derivational + adjectival-participle chains.
    // See the `snowball_reference` test for the wider set.
    assert_eq!(RUSSIAN.stem("красивая"), "красив");
    assert_eq!(RUSSIAN.stem("красивый"), "красив");
    assert_eq!(RUSSIAN.stem("столы"), "стол");
    assert_eq!(RUSSIAN.stem("полезность"), "полезн");
}

#[test]
fn phonetic_encoder_is_gost_779_b() {
    let enc = RUSSIAN
        .phonetic_encoder()
        .expect("Russian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "gost-7.79-b");
    let (primary, alt) = enc.encode("Москва").expect("encodes Москва");
    assert_eq!(primary, "moskva");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_returns_none_for_no_cyrillic() {
    let enc = RUSSIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(RUSSIAN.collator().is_none());
}

// ---------------------------------------------------------------------
// Slavic-Metaphone opt-in encoder variant.
// ---------------------------------------------------------------------

#[cfg(feature = "slavic-metaphone")]
#[test]
fn slavic_metaphone_variant_swaps_the_phonetic_encoder() {
    let enc = RUSSIAN_WITH_SLAVIC_METAPHONE
        .phonetic_encoder()
        .expect("Russian slavic-metaphone pack ships a phonetic encoder");
    assert_eq!(enc.name(), "slavic-metaphone-2026");
}

#[cfg(feature = "slavic-metaphone")]
#[test]
fn slavic_metaphone_variant_encodes_cyrillic_and_latin_alike() {
    // The whole point of the cross-Slavic encoder: Russian Cyrillic
    // "Чехов" and its Latin transliteration "Chekhov" hash to the
    // same key.
    let enc = RUSSIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();
    let (cyr, _) = enc.encode("Чехов").expect("encodes Cyrillic");
    let (lat, _) = enc.encode("Chekhov").expect("encodes Latin");
    assert_eq!(cyr, lat);
}

#[cfg(feature = "slavic-metaphone")]
#[test]
fn default_encoder_can_be_restored_after_opting_in() {
    // Undo path: `.with_default_encoder()` returns the pack to the
    // GOST 7.79-B transliteration.
    let restored = Russian::new()
        .with_slavic_metaphone_encoder()
        .with_default_encoder();
    let enc = restored
        .phonetic_encoder()
        .expect("restored pack ships an encoder");
    assert_eq!(enc.name(), "gost-7.79-b");
}

#[cfg(feature = "slavic-metaphone")]
#[test]
fn default_russian_constant_preserves_gost_encoder() {
    // Belt-and-braces: the module-level RUSSIAN constant must keep
    // returning the GOST 7.79-B adapter even when the slavic-metaphone
    // feature is compiled in.
    let enc = RUSSIAN
        .phonetic_encoder()
        .expect("default Russian pack ships an encoder");
    assert_eq!(enc.name(), "gost-7.79-b");
}
