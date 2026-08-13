//! Integration tests for the Italian [`Language`] implementation.

use stringcheese_it::ITALIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(ITALIAN.code(), "it");
    assert_eq!(ITALIAN.name(), "Italian");
}

#[test]
fn common_italian_words_are_stopwords() {
    for w in [
        "il", "la", "lo", "gli", "le", "un", "una", "di", "a", "da", "in", "con", "per", "e", "o",
        "ma", "che", "non",
    ] {
        assert!(ITALIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`. Italian
    // capitalises the first letter of sentences and proper nouns
    // like every other Latin-script language; the MVP stopword
    // list is stored lowercase but must match capitalised
    // sentence-initial forms.
    assert!(ITALIAN.is_stopword("IL"));
    assert!(ITALIAN.is_stopword("La"));
    assert!(ITALIAN.is_stopword("CHE"));
    assert!(ITALIAN.is_stopword("Non"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "formaggio",
        "algoritmo",
        "linguistica",
        "supercalifragilistico",
    ] {
        assert!(!ITALIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_italian_sentence() {
    let text = "Il gatto dorme sul tappeto.";
    let toks: Vec<&str> = ITALIAN.tokenize(text).collect();
    assert_eq!(toks, ["Il", "gatto", "dorme", "sul", "tappeto"]);
}

#[test]
fn stem_is_identity() {
    // Italian ships an identity stemmer for the MVP release — a
    // Snowball Italian port is a documented follow-up. Callers get
    // the input verbatim.
    assert_eq!(ITALIAN.stem("parlando"), "parlando");
    assert_eq!(ITALIAN.stem("libri"), "libri");
    assert_eq!(ITALIAN.stem("casa"), "casa");
    assert_eq!(ITALIAN.stem("parlare"), "parlare");
}

#[test]
fn phonetic_encoder_is_none_by_default() {
    // Italian PHONEX is a documented follow-up — the base pack
    // ships no phonetic encoder.
    assert!(ITALIAN.phonetic_encoder().is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(ITALIAN.collator().is_none());
}
