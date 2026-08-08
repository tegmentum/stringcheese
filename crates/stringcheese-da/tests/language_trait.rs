//! Integration tests for the Danish [`Language`] implementation.

use stringcheese_da::DANISH;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(DANISH.code(), "da");
    assert_eq!(DANISH.name(), "Danish");
}

#[test]
fn common_danish_words_are_stopwords() {
    for w in [
        "og", "i", "det", "at", "en", "et", "den", "til", "er", "som", "på", "med", "af", "ikke",
    ] {
        assert!(DANISH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(DANISH.is_stopword("OG"));
    assert!(DANISH.is_stopword("Det"));
    assert!(DANISH.is_stopword("EN"));
    assert!(DANISH.is_stopword("Ikke"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["fisk", "kærlighed", "algoritme", "supermarked"] {
        assert!(!DANISH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_danish_sentence() {
    let text = "Katten sover på måtten.";
    let toks: Vec<&str> = DANISH.tokenize(text).collect();
    assert_eq!(toks, ["Katten", "sover", "på", "måtten"]);
}

#[test]
fn stem_a_few_danish_words() {
    // Well-known Snowball Danish reductions (hand-traced through the
    // algorithm).
    assert_eq!(DANISH.stem("sæden"), "sæd");
    assert_eq!(DANISH.stem("havet"), "hav");
    assert_eq!(DANISH.stem("kærlighed"), "kær");
    assert_eq!(DANISH.stem("kærlig"), "kær");
}

#[test]
fn phonetic_encoder_is_phonex_da() {
    let enc = DANISH
        .phonetic_encoder()
        .expect("Danish pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-da");
    assert_eq!(
        enc.encode("Hansen"),
        Some((String::from("A575"), None)),
        "PHONEX-DA(Hansen) should be A575 with no alternate key"
    );
    assert_eq!(
        enc.encode("Jensen"),
        Some((String::from("J575"), None)),
        "PHONEX-DA(Jensen) should be J575 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(DANISH.collator().is_none());
}
