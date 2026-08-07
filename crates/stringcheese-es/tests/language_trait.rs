//! Integration tests for the Spanish [`Language`] implementation.

use stringcheese_es::SPANISH;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(SPANISH.code(), "es");
    assert_eq!(SPANISH.name(), "Spanish");
}

#[test]
fn common_spanish_words_are_stopwords() {
    for w in [
        "el", "la", "los", "las", "y", "o", "de", "en", "un", "una", "por", "para",
    ] {
        assert!(SPANISH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(SPANISH.is_stopword("EL"));
    assert!(SPANISH.is_stopword("El"));
    assert!(SPANISH.is_stopword("La"));
    assert!(SPANISH.is_stopword("Y"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["queso", "algoritmo", "lingüística", "supercalifragilístico"] {
        assert!(!SPANISH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_spanish_sentence() {
    let text = "¿Cómo estás? Bien, gracias.";
    let toks: Vec<&str> = SPANISH.tokenize(text).collect();
    assert_eq!(toks, ["Cómo", "estás", "Bien", "gracias"]);
}

#[test]
fn stem_a_few_spanish_words() {
    assert_eq!(SPANISH.stem("hablando"), "habl");
    assert_eq!(SPANISH.stem("niños"), "niñ");
    assert_eq!(SPANISH.stem("casa"), "cas");
    assert_eq!(SPANISH.stem("hablar"), "habl");
}

#[test]
fn phonetic_encoder_is_phonex_es() {
    let enc = SPANISH
        .phonetic_encoder()
        .expect("Spanish pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-es");
    assert_eq!(
        enc.encode("García"),
        Some((String::from("G620"), None)),
        "PHONEX-ES(García) should be G620 with no alternate key"
    );
    assert_eq!(
        enc.encode("Martínez"),
        Some((String::from("M635"), None)),
        "PHONEX-ES(Martínez) should be M635 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(SPANISH.collator().is_none());
}
