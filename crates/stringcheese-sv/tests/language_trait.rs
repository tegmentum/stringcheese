//! Integration tests for the Swedish [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_sv::SWEDISH;

#[test]
fn code_and_name() {
    assert_eq!(SWEDISH.code(), "sv");
    assert_eq!(SWEDISH.name(), "Swedish");
}

#[test]
fn common_swedish_words_are_stopwords() {
    for w in [
        "och", "att", "det", "en", "ett", "på", "för", "med", "är", "att",
    ] {
        assert!(SWEDISH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_ascii() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(SWEDISH.is_stopword("OCH"));
    assert!(SWEDISH.is_stopword("Att"));
    assert!(SWEDISH.is_stopword("DET"));
    assert!(SWEDISH.is_stopword("En"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["ost", "flicka", "algoritm", "biblotek"] {
        assert!(!SWEDISH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_swedish_sentence() {
    let text = "Katten sover på mattan.";
    let toks: Vec<&str> = SWEDISH.tokenize(text).collect();
    assert_eq!(toks, ["Katten", "sover", "på", "mattan"]);
}

#[test]
fn stem_a_few_swedish_words() {
    assert_eq!(SWEDISH.stem("flickorna"), "flick");
    assert_eq!(SWEDISH.stem("husen"), "hus");
    assert_eq!(SWEDISH.stem("hundarna"), "hund");
    assert_eq!(SWEDISH.stem("underfullt"), "underfull");
}

#[test]
fn phonetic_encoder_is_phonex_sv() {
    let enc = SWEDISH
        .phonetic_encoder()
        .expect("Swedish pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-sv");
    assert_eq!(
        enc.encode("Andersson"),
        Some((String::from("A536"), None)),
        "PHONEX-SV(Andersson) should be A536 with no alternate key"
    );
    assert_eq!(
        enc.encode("Johansson"),
        Some((String::from("J575"), None)),
        "PHONEX-SV(Johansson) should be J575 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(SWEDISH.collator().is_none());
}
