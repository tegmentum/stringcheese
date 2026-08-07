//! Integration tests for the German [`Language`] implementation.

use std::cmp::Ordering;

use stringcheese_de::{GERMAN, GERMAN_WITH_DIN5007_DICTIONARY, GERMAN_WITH_DIN5007_PHONEBOOK};
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
    // The default pack declines to pick a DIN 5007 convention;
    // callers opt in via GERMAN_WITH_DIN5007_DICTIONARY or
    // GERMAN_WITH_DIN5007_PHONEBOOK.
    assert!(GERMAN.collator().is_none());
}

#[test]
fn din5007_dictionary_pack_wires_variant1() {
    let c = GERMAN_WITH_DIN5007_DICTIONARY
        .collator()
        .expect("dictionary pack ships a collator");
    // Bär (=Bar under DIN-1) sorts equal to Bar.
    assert_eq!(c.compare("Bär", "Bar"), Ordering::Equal);
    // Straße (=Strasse) sorts equal to Strasse.
    assert_eq!(c.compare("Straße", "Strasse"), Ordering::Equal);
    // Classic Müller / Munk / Muster ordering.
    let mut ws = ["Muster", "Müller", "Munk"];
    ws.sort_by(|a, b| c.compare(a, b));
    assert_eq!(ws, ["Müller", "Munk", "Muster"]);
}

#[test]
fn din5007_phonebook_pack_wires_variant2() {
    let c = GERMAN_WITH_DIN5007_PHONEBOOK
        .collator()
        .expect("phonebook pack ships a collator");
    // Bär (=Baer under DIN-2) sorts equal to Baer and before Bar.
    assert_eq!(c.compare("Bär", "Baer"), Ordering::Equal);
    assert_eq!(c.compare("Bär", "Bar"), Ordering::Less);
    // Straße (=Strasse) sorts equal to Strasse under both variants.
    assert_eq!(c.compare("Straße", "Strasse"), Ordering::Equal);
    // Under phonebook, Müller (=Mueller) sorts before Muller.
    let mut ws = ["Muller", "Muster", "Müller", "Munk"];
    ws.sort_by(|a, b| c.compare(a, b));
    assert_eq!(ws, ["Müller", "Muller", "Munk", "Muster"]);
}
