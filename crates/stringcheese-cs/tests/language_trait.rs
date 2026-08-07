//! Integration tests for the Czech [`Language`] implementation.

use stringcheese_cs::CZECH;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(CZECH.code(), "cs");
    assert_eq!(CZECH.name(), "Czech");
}

#[test]
fn common_czech_words_are_stopwords() {
    for w in [
        "a", "i", "v", "na", "za", "je", "být", "když", "protože", "ale", "že",
    ] {
        assert!(CZECH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_unicode_fold() {
    // Unicode case-fold: Č → č, Ě → ě, Ř → ř, Š → š, Ž → ž, Á → á, etc.
    assert!(CZECH.is_stopword("NENÍ"));
    assert!(CZECH.is_stopword("Není"));
    assert!(CZECH.is_stopword("KDYŽ"));
    assert!(CZECH.is_stopword("PROTOŽE"));
    assert!(!CZECH.is_stopword("KOČKA")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["kniha", "pes", "program", "žlutá"] {
        assert!(!CZECH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_czech_sentence() {
    let text = "Kočka spí na koberci.";
    let toks: Vec<&str> = CZECH.tokenize(text).collect();
    assert_eq!(toks, ["Kočka", "spí", "na", "koberci"]);
}

#[test]
fn stem_a_few_czech_words() {
    // See `stemmer_reference` for the wider set.
    assert_eq!(CZECH.stem("krásný"), "krásn");
    assert_eq!(CZECH.stem("krásná"), "krásn");
    assert_eq!(CZECH.stem("pracoval"), "prac");
    assert_eq!(CZECH.stem("ženám"), "žen");
}

#[test]
fn phonetic_encoder_is_phonex_cs() {
    let enc = CZECH
        .phonetic_encoder()
        .expect("Czech pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-cs");
    let (primary, alt) = enc.encode("Novák").expect("encodes Novák");
    assert_eq!(primary, "N120");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_folds_haceks_to_base_letters() {
    let enc = CZECH.phonetic_encoder().unwrap();
    let (with_hacek, _) = enc.encode("žena").unwrap();
    let (without, _) = enc.encode("zena").unwrap();
    assert_eq!(with_hacek, without);
}

#[test]
fn collator_is_none_by_default() {
    assert!(CZECH.collator().is_none());
}
