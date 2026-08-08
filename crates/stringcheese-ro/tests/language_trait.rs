//! Integration tests for the Romanian [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_ro::ROMANIAN;

#[test]
fn code_and_name() {
    assert_eq!(ROMANIAN.code(), "ro");
    assert_eq!(ROMANIAN.name(), "Romanian");
}

#[test]
fn common_romanian_words_are_stopwords() {
    for w in [
        "și", "sau", "dar", "în", "pe", "la", "de", "cu", "pentru", "un", "o", "nu", "este", "sunt",
    ] {
        assert!(ROMANIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(ROMANIAN.is_stopword("UN"));
    assert!(ROMANIAN.is_stopword("Un"));
    assert!(ROMANIAN.is_stopword("La"));
    assert!(ROMANIAN.is_stopword("DA"));
}

#[test]
fn cedilla_form_stopwords_are_recognized() {
    // The `Language::is_stopword` override folds cedilla to comma-
    // below before comparison, so a caller feeding cedilla-form
    // tokens still matches the comma-below stopword list.
    //
    // `și` (comma-below) is in the list; `şi` (cedilla) must also
    // match.
    assert!(ROMANIAN.is_stopword("şi"));
    // `ești` (comma-below) is in the list; `eşti` (cedilla) too.
    assert!(ROMANIAN.is_stopword("eşti"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["brânză", "mămăligă", "algoritm", "supercalifragilistic"] {
        assert!(!ROMANIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_romanian_sentence() {
    let text = "Bună ziua, prietene!";
    let toks: Vec<&str> = ROMANIAN.tokenize(text).collect();
    assert_eq!(toks, ["Bună", "ziua", "prietene"]);
}

#[test]
fn stem_a_few_romanian_words() {
    // Postposed article stripping.
    assert_eq!(ROMANIAN.stem("omul"), "om");
    // Trailing vowel drop in RV.
    assert_eq!(ROMANIAN.stem("carte"), "cart");
    // Postposed article + trailing-vowel cascade.
    assert_eq!(ROMANIAN.stem("casele"), "cas");
}

#[test]
fn stem_is_cedilla_stable() {
    // The stemmer folds cedilla to comma-below on entry, so the
    // stem of a cedilla-form word equals the stem of its comma-
    // below-form twin.
    assert_eq!(ROMANIAN.stem("aşa"), ROMANIAN.stem("așa"));
    assert_eq!(ROMANIAN.stem("ţară"), ROMANIAN.stem("țară"));
}

#[test]
fn phonetic_encoder_is_phonex_ro() {
    let enc = ROMANIAN
        .phonetic_encoder()
        .expect("Romanian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-ro");
    assert_eq!(
        enc.encode("Popescu"),
        Some((String::from("P172"), None)),
        "PHONEX-RO(Popescu) should be P172 with no alternate key"
    );
    assert_eq!(
        enc.encode("Ionescu"),
        Some((String::from("I572"), None)),
        "PHONEX-RO(Ionescu) should be I572 with no alternate key"
    );
}

#[test]
fn phonetic_encoder_folds_cedilla() {
    let enc = ROMANIAN.phonetic_encoder().unwrap();
    assert_eq!(enc.encode("ţară"), enc.encode("țară"));
    assert_eq!(enc.encode("eşti"), enc.encode("ești"));
}

#[test]
fn collator_is_none_by_default() {
    assert!(ROMANIAN.collator().is_none());
}
