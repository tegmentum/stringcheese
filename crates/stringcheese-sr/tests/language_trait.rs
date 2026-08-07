//! Integration tests for the Serbian [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_sr::SERBIAN;

#[test]
fn code_and_name() {
    assert_eq!(SERBIAN.code(), "sr");
    assert_eq!(SERBIAN.name(), "Serbian");
}

#[test]
fn common_cyrillic_stopwords_are_recognized() {
    for w in ["и", "у", "на", "не", "за", "је", "да"] {
        assert!(SERBIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn common_latin_stopwords_are_recognized() {
    for w in ["i", "u", "na", "ne", "za", "je", "da"] {
        assert!(SERBIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_works_in_both_scripts() {
    // Uppercase Cyrillic.
    assert!(SERBIAN.is_stopword("НЕ"));
    assert!(SERBIAN.is_stopword("Не"));
    // Uppercase Latin.
    assert!(SERBIAN.is_stopword("NE"));
    assert!(SERBIAN.is_stopword("Ne"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "књига",
        "kuća",
        "рачунар",
        "kompjuter",
        "програм",
        "program",
    ] {
        assert!(!SERBIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_cyrillic_sentence() {
    let text = "Београд је главни град Србије.";
    let toks: Vec<&str> = SERBIAN.tokenize(text).collect();
    assert_eq!(toks, ["Београд", "је", "главни", "град", "Србије"]);
}

#[test]
fn tokenize_latin_sentence() {
    let text = "Beograd je glavni grad Srbije.";
    let toks: Vec<&str> = SERBIAN.tokenize(text).collect();
    assert_eq!(toks, ["Beograd", "je", "glavni", "grad", "Srbije"]);
}

#[test]
fn stem_a_few_latin_words() {
    assert_eq!(SERBIAN.stem("lepa"), "lep");
    assert_eq!(SERBIAN.stem("gradovi"), "grad");
    assert_eq!(SERBIAN.stem("kućama"), "kuć");
    assert_eq!(SERBIAN.stem("pisati"), "pis");
}

#[test]
fn stem_a_few_cyrillic_words() {
    assert_eq!(SERBIAN.stem("лепа"), "леп");
    assert_eq!(SERBIAN.stem("градови"), "град");
    assert_eq!(SERBIAN.stem("писати"), "пис");
}

#[test]
fn phonetic_encoder_is_sr_latin() {
    let enc = SERBIAN
        .phonetic_encoder()
        .expect("Serbian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "sr-latin");
}

#[test]
fn phonetic_encoder_unifies_scripts() {
    let enc = SERBIAN
        .phonetic_encoder()
        .expect("Serbian pack ships a phonetic encoder");
    let (cyr_key, _) = enc.encode("Београд").expect("encodes Cyrillic");
    let (lat_key, _) = enc.encode("Beograd").expect("encodes Latin");
    assert_eq!(cyr_key, lat_key);
    assert_eq!(cyr_key, "beograd");
}

#[test]
fn phonetic_encoder_returns_none_for_empty() {
    let enc = SERBIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(SERBIAN.collator().is_none());
}

#[test]
fn stopwords_slice_includes_both_scripts() {
    // The combined slice contains entries from both scripts.
    let all = SERBIAN.stopwords();
    assert!(all.contains(&"и"), "combined stopwords should include и");
    assert!(all.contains(&"i"), "combined stopwords should include i");
}
