//! Integration tests for the Estonian [`Language`] implementation.

use stringcheese_et::ESTONIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(ESTONIAN.code(), "et");
    assert_eq!(ESTONIAN.name(), "Estonian");
}

#[test]
fn common_estonian_words_are_stopwords() {
    for w in [
        "ja", "või", "et", "kui", "on", "ei", "mina", "sina", "aga", "see",
    ] {
        assert!(ESTONIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_unicode_fold() {
    // Estonian has no locale-specific case-fold quirks — the default
    // Unicode fold covers every letter.
    assert!(ESTONIAN.is_stopword("SEE"));
    assert!(ESTONIAN.is_stopword("KÜLL"));
    assert!(ESTONIAN.is_stopword("ÄRA"));
    assert!(ESTONIAN.is_stopword("Või"));
    assert!(ESTONIAN.is_stopword("JA"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["raamat", "arvuti", "algoritm", "tarkvara"] {
        assert!(!ESTONIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_estonian_sentence() {
    let text = "Tere, maailm! Tallinn on ilus.";
    let toks: Vec<&str> = ESTONIAN.tokenize(text).collect();
    assert_eq!(toks, ["Tere", "maailm", "Tallinn", "on", "ilus"]);
}

#[test]
fn stem_a_few_estonian_words() {
    // Case-only strips on `maja` "house".
    assert_eq!(ESTONIAN.stem("majas"), "maja");
    assert_eq!(ESTONIAN.stem("majale"), "maja");
    assert_eq!(ESTONIAN.stem("majaga"), "maja");
    assert_eq!(ESTONIAN.stem("majasse"), "maja");
    // Plural on `kass` "cat".
    assert_eq!(ESTONIAN.stem("kassid"), "kass");
    // Diminutive on `linnu` "bird".
    assert_eq!(ESTONIAN.stem("linnukene"), "linnu");
}

#[test]
fn phonetic_encoder_is_phonex_et() {
    let enc = ESTONIAN
        .phonetic_encoder()
        .expect("Estonian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-et");
    let (primary, alt) = enc.encode("Tallinn").expect("PHONEX-ET encodes Tallinn");
    assert_eq!(primary, "T450");
    assert!(alt.is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(ESTONIAN.collator().is_none());
}

#[test]
fn stopword_list_is_non_empty() {
    assert!(!ESTONIAN.stopwords().is_empty());
    assert!(ESTONIAN.stopwords().len() >= 80);
}
