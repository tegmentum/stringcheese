//! Integration tests for the Greek [`Language`] implementation.

use stringcheese_el::GREEK;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(GREEK.code(), "el");
    assert_eq!(GREEK.name(), "Greek");
}

#[test]
fn common_greek_words_are_stopwords() {
    for w in ["και", "η", "να", "σε", "με", "για", "αν", "εγω", "εσυ"] {
        assert!(GREEK.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_greek_fold() {
    // Greek case-fold (Rust default Unicode fold): Α → α, Θ → θ, etc.
    // Uppercase queries fold correctly.
    assert!(GREEK.is_stopword("ΚΑΙ"));
    assert!(GREEK.is_stopword("Και"));
    assert!(GREEK.is_stopword("ΝΑ"));
    assert!(!GREEK.is_stopword("ΚΟΣΜΟΣ")); // Not a stopword.
}

#[test]
fn stopword_lookup_folds_accents() {
    // The list stores accent-stripped `ειμαι`; a query with the accent
    // should match too.
    assert!(GREEK.is_stopword("είμαι"));
    assert!(GREEK.is_stopword("ΕΊΜΑΙ"));
    assert!(GREEK.is_stopword("Είμαι"));
    assert!(GREEK.is_stopword("ηταν"));
    assert!(GREEK.is_stopword("ήταν"));
}

#[test]
fn stopword_lookup_folds_final_sigma() {
    // The stopword list stores `αυτοσ` (non-final sigma); a query with
    // the final-sigma spelling `αυτός` should match too.
    assert!(GREEK.is_stopword("αυτός"));
    assert!(GREEK.is_stopword("αυτοσ"));
    assert!(GREEK.is_stopword("ΑΥΤΌΣ"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["βιβλίο", "σκύλος", "υπολογιστής", "πρόγραμμα"] {
        assert!(!GREEK.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_greek_sentence() {
    let text = "Καλημέρα, κόσμε! Αθήνα — πρωτεύουσα Ελλάδας.";
    let toks: Vec<&str> = GREEK.tokenize(text).collect();
    assert_eq!(
        toks,
        ["Καλημέρα", "κόσμε", "Αθήνα", "πρωτεύουσα", "Ελλάδας"]
    );
}

#[test]
fn stem_a_few_greek_words() {
    // See the `snowball_reference` test for the wider set.
    assert_eq!(GREEK.stem("καλός"), "καλ");
    assert_eq!(GREEK.stem("κόσμος"), "κοσμ");
    assert_eq!(GREEK.stem("γράφω"), "γραφ");
    assert_eq!(GREEK.stem("γάτα"), "γατ");
}

#[test]
fn phonetic_encoder_is_iso_843() {
    let enc = GREEK
        .phonetic_encoder()
        .expect("Greek pack ships a phonetic encoder");
    assert_eq!(enc.name(), "iso-843-el");
    let (primary, alt) = enc.encode("Αθήνα").expect("encodes Αθήνα");
    assert_eq!(primary, "athina");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_returns_none_for_no_greek() {
    let enc = GREEK.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(GREEK.collator().is_none());
}
