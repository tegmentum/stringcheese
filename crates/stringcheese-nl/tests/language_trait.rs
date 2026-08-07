//! Integration tests for the Dutch [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_nl::DUTCH;

#[test]
fn code_and_name() {
    assert_eq!(DUTCH.code(), "nl");
    assert_eq!(DUTCH.name(), "Dutch");
}

#[test]
fn common_dutch_words_are_stopwords() {
    for w in [
        "de", "het", "een", "en", "van", "in", "op", "aan", "met", "voor", "door", "of",
    ] {
        assert!(DUTCH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(DUTCH.is_stopword("DE"));
    assert!(DUTCH.is_stopword("Het"));
    assert!(DUTCH.is_stopword("EEN"));
    assert!(DUTCH.is_stopword("Van"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["kaas", "fietsen", "algoritme", "supermarkt"] {
        assert!(!DUTCH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_dutch_sentence() {
    let text = "De kat slaapt op de mat.";
    let toks: Vec<&str> = DUTCH.tokenize(text).collect();
    assert_eq!(toks, ["De", "kat", "slaapt", "op", "de", "mat"]);
}

#[test]
fn stem_a_few_dutch_words() {
    assert_eq!(DUTCH.stem("lopen"), "lop");
    assert_eq!(DUTCH.stem("boeken"), "boek");
    assert_eq!(DUTCH.stem("dagen"), "dag");
    assert_eq!(DUTCH.stem("natuurlijk"), "natuur");
}

#[test]
fn phonetic_encoder_is_phonex_nl() {
    let enc = DUTCH
        .phonetic_encoder()
        .expect("Dutch pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-nl");
    assert_eq!(
        enc.encode("Jansen"),
        Some((String::from("J575"), None)),
        "PHONEX-NL(Jansen) should be J575 with no alternate key"
    );
    assert_eq!(
        enc.encode("Bakker"),
        Some((String::from("B260"), None)),
        "PHONEX-NL(Bakker) should be B260 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(DUTCH.collator().is_none());
}
