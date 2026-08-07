//! Integration tests for the German [`Language`] implementation.

use stringcheese_de::GERMAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(GERMAN.code(), "de");
    assert_eq!(GERMAN.name(), "German");
}

#[test]
fn common_german_words_are_stopwords() {
    for w in [
        "der", "die", "das", "und", "in", "zu", "den", "ist", "nicht", "ein",
    ] {
        assert!(GERMAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_on_ascii() {
    // ASCII stopwords compare with `eq_ignore_ascii_case`.
    assert!(GERMAN.is_stopword("Der"));
    assert!(GERMAN.is_stopword("DER"));
    assert!(GERMAN.is_stopword("dER"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["Käse", "Algorithmus", "Snowball", "String", "Cheese"] {
        assert!(!GERMAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_german_sentence() {
    let text = "Der schnelle braune Fuchs springt über den faulen Hund.";
    let toks: Vec<&str> = GERMAN.tokenize(text).collect();
    assert_eq!(
        toks,
        [
            "Der", "schnelle", "braune", "Fuchs", "springt", "über", "den", "faulen", "Hund",
        ]
    );
}

#[test]
fn stem_a_few_german_words() {
    // See tests/snowball_reference.rs for the reference-pair table.
    assert_eq!(GERMAN.stem("Häuser"), "haus");
    assert_eq!(GERMAN.stem("haben"), "hab");
    assert_eq!(GERMAN.stem("Kinder"), "kind");
    assert_eq!(GERMAN.stem("größer"), "gross");
}

#[test]
fn phonetic_encoder_is_koelner_phonetik() {
    let enc = GERMAN
        .phonetic_encoder()
        .expect("German pack ships a phonetic encoder");
    assert_eq!(enc.name(), "koelner-phonetik");
    assert_eq!(
        enc.encode("Müller"),
        Some((String::from("657"), None)),
        "Kölner Phonetik(Müller) should be 657 with no alternate key"
    );
    assert_eq!(
        enc.encode("Schmidt"),
        Some((String::from("862"), None)),
        "Kölner Phonetik(Schmidt) should be 862 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    // See the crate-level docs — the DIN 5007 collator is a follow-up.
    assert!(GERMAN.collator().is_none());
}
