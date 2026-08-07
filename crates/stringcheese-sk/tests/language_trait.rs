//! Integration tests for the Slovak [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_sk::SLOVAK;

#[test]
fn code_and_name() {
    assert_eq!(SLOVAK.code(), "sk");
    assert_eq!(SLOVAK.name(), "Slovak");
}

#[test]
fn common_slovak_words_are_stopwords() {
    for w in [
        "a", "i", "v", "na", "za", "je", "byť", "keď", "pretože", "ale", "že", "lebo",
    ] {
        assert!(SLOVAK.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_unicode_fold() {
    // Unicode case-fold: Č → č, Š → š, Ž → ž, Á → á, Ľ → ľ, Ä → ä, etc.
    assert!(SLOVAK.is_stopword("NIE"));
    assert!(SLOVAK.is_stopword("Keď"));
    assert!(SLOVAK.is_stopword("KEĎ"));
    assert!(SLOVAK.is_stopword("PRETOŽE"));
    assert!(SLOVAK.is_stopword("NAJMÄ"));
    assert!(SLOVAK.is_stopword("MÔJ"));
    assert!(!SLOVAK.is_stopword("MAČKA")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["kniha", "pes", "program", "žltá"] {
        assert!(!SLOVAK.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_slovak_sentence() {
    let text = "Mačka spí na koberci.";
    let toks: Vec<&str> = SLOVAK.tokenize(text).collect();
    assert_eq!(toks, ["Mačka", "spí", "na", "koberci"]);
}

#[test]
fn stem_a_few_slovak_words() {
    // See `stemmer_reference` for the wider set.
    assert_eq!(SLOVAK.stem("pekný"), "pekn");
    assert_eq!(SLOVAK.stem("pekná"), "pekn");
    assert_eq!(SLOVAK.stem("pracoval"), "prac");
    assert_eq!(SLOVAK.stem("pracovať"), "prac"); // Slovak -ť infinitive.
    assert_eq!(SLOVAK.stem("ženám"), "žen");
}

#[test]
fn phonetic_encoder_is_phonex_sk() {
    let enc = SLOVAK
        .phonetic_encoder()
        .expect("Slovak pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-sk");
    let (primary, alt) = enc.encode("Novák").expect("encodes Novák");
    assert_eq!(primary, "N120");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_folds_haceks_to_base_letters() {
    let enc = SLOVAK.phonetic_encoder().unwrap();
    let (with_hacek, _) = enc.encode("žena").unwrap();
    let (without, _) = enc.encode("zena").unwrap();
    assert_eq!(with_hacek, without);
}

#[test]
fn phonetic_encoder_folds_slovak_specific_letters() {
    let enc = SLOVAK.phonetic_encoder().unwrap();
    // ľ → L: kráľ ≡ kral
    assert_eq!(enc.encode("kráľ"), enc.encode("kral"));
    // ô → O: kôň ≡ kon
    assert_eq!(enc.encode("kôň"), enc.encode("kon"));
    // ä → E: späť ≡ spet
    assert_eq!(enc.encode("späť"), enc.encode("spet"));
}

#[test]
fn collator_is_none_by_default() {
    assert!(SLOVAK.collator().is_none());
}
