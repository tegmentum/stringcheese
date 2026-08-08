//! Integration tests for the Finnish [`Language`] implementation.

use stringcheese_fi::FINNISH;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(FINNISH.code(), "fi");
    assert_eq!(FINNISH.name(), "Finnish");
}

#[test]
fn common_finnish_words_are_stopwords() {
    for w in [
        "ja", "tai", "että", "kun", "on", "ei", "sinä", "hän", "mutta",
    ] {
        assert!(FINNISH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_unicode_fold() {
    // Finnish has no locale-specific case-fold quirks — the default
    // Unicode fold covers `ä ↔ Ä`, `ö ↔ Ö`, `å ↔ Å` correctly.
    assert!(FINNISH.is_stopword("SINÄ"));
    assert!(FINNISH.is_stopword("MYÖS"));
    assert!(FINNISH.is_stopword("Että"));
    assert!(FINNISH.is_stopword("JA"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["kirjasto", "tietokone", "algoritmi", "ohjelmisto"] {
        assert!(!FINNISH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_finnish_sentence() {
    let text = "Hei, maailma! Helsinki on kaunis.";
    let toks: Vec<&str> = FINNISH.tokenize(text).collect();
    assert_eq!(toks, ["Hei", "maailma", "Helsinki", "on", "kaunis"]);
}

#[test]
fn stem_a_few_finnish_words() {
    // The flagship agglutinative test: "in my house also" → "house".
    assert_eq!(FINNISH.stem("talossanikin"), "talo");
    // Simpler case-only strips.
    assert_eq!(FINNISH.stem("talossa"), "talo");
    assert_eq!(FINNISH.stem("talosta"), "talo");
    assert_eq!(FINNISH.stem("talon"), "talo");
    // Front-harmony equivalent.
    assert_eq!(FINNISH.stem("kylässä"), "kylä");
}

#[test]
fn phonetic_encoder_is_phonex_fi() {
    let enc = FINNISH
        .phonetic_encoder()
        .expect("Finnish pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-fi");
    let (primary, alt) = enc.encode("Helsinki").expect("PHONEX-FI encodes Helsinki");
    assert_eq!(primary, "H475");
    assert!(alt.is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(FINNISH.collator().is_none());
}

#[test]
fn stopword_list_is_non_empty() {
    assert!(!FINNISH.stopwords().is_empty());
    assert!(FINNISH.stopwords().len() >= 150);
}
