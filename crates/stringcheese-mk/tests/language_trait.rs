//! Integration tests for the Macedonian [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_mk::MACEDONIAN;

#[test]
fn code_and_name() {
    assert_eq!(MACEDONIAN.code(), "mk");
    assert_eq!(MACEDONIAN.name(), "Macedonian");
}

#[test]
fn common_macedonian_words_are_stopwords() {
    for w in ["и", "во", "на", "не", "со", "по", "за", "јас", "ти", "тој"] {
        assert!(MACEDONIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_cyrillic_fold() {
    // Cyrillic case-fold (Rust default Unicode fold): А → а, Ѓ → ѓ,
    // Ќ → ќ, Љ → љ, Њ → њ. Uppercase queries fold correctly to the
    // plain lowercase list.
    assert!(MACEDONIAN.is_stopword("НЕ"));
    assert!(MACEDONIAN.is_stopword("Не"));
    assert!(MACEDONIAN.is_stopword("ВО"));
    assert!(MACEDONIAN.is_stopword("СУМ")); // Uppercase copula form.
    assert!(!MACEDONIAN.is_stopword("СКОПЈЕ")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["книга", "куче", "компјутер", "програма"] {
        assert!(!MACEDONIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_macedonian_sentence() {
    let text = "Здраво, свет! Скопје — главен град на Македонија.";
    let toks: Vec<&str> = MACEDONIAN.tokenize(text).collect();
    assert_eq!(
        toks,
        [
            "Здраво",
            "свет",
            "Скопје",
            "главен",
            "град",
            "на",
            "Македонија"
        ]
    );
}

#[test]
fn stem_three_way_definite_article() {
    // The signature Macedonian move: the definite article carries a
    // proximity contrast, and all three articled surface forms of a
    // noun collapse to the same stem as the bare form.
    let bare = MACEDONIAN.stem("град").into_owned();
    assert_eq!(MACEDONIAN.stem("градот"), bare);
    assert_eq!(MACEDONIAN.stem("градов"), bare);
    assert_eq!(MACEDONIAN.stem("градон"), bare);
}

#[test]
fn stem_a_few_macedonian_words() {
    assert_eq!(MACEDONIAN.stem("книга"), "книг");
    assert_eq!(MACEDONIAN.stem("градови"), "град");
    assert_eq!(MACEDONIAN.stem("правам"), "прав");
    assert_eq!(MACEDONIAN.stem("нови"), "нов");
}

#[test]
fn phonetic_encoder_is_phonex_mk() {
    let enc = MACEDONIAN
        .phonetic_encoder()
        .expect("Macedonian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-mk");
    let (primary, alt) = enc.encode("Скопје").expect("encodes Скопје");
    assert_eq!(primary, "с212");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_macedonian_specific_letters() {
    // ѓ / ќ / љ / њ / џ / ѕ / ј all fold to their Slavic-Soundex class.
    let enc = MACEDONIAN.phonetic_encoder().unwrap();
    // ѓ seed: preserves the letter, then vowel drop.
    let (k, _) = enc.encode("ѓавол").unwrap();
    assert!(k.starts_with('ѓ'));
    // The word encodes to 4 char-count regardless of Cyrillic width.
    assert_eq!(k.chars().count(), 4);
}

#[test]
fn phonetic_encoder_returns_none_for_no_letters() {
    let enc = MACEDONIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    assert!(enc.encode("   ").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(MACEDONIAN.collator().is_none());
}
