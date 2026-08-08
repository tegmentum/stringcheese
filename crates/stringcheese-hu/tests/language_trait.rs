//! Integration tests for the Hungarian [`Language`] implementation.

use stringcheese_hu::HUNGARIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(HUNGARIAN.code(), "hu");
    assert_eq!(HUNGARIAN.name(), "Hungarian");
}

#[test]
fn common_hungarian_words_are_stopwords() {
    for w in ["és", "vagy", "de", "nem", "ez", "az", "az", "van"] {
        assert!(HUNGARIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_unicode_fold() {
    // Unicode case-fold: `ÉS → és`, `Ő → ő`, `Á → á`.
    assert!(HUNGARIAN.is_stopword("ÉS"));
    assert!(HUNGARIAN.is_stopword("Ő"));
    assert!(HUNGARIAN.is_stopword("VAN"));
    assert!(HUNGARIAN.is_stopword("De"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["kutya", "macska", "algoritmus", "programozás"] {
        assert!(!HUNGARIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_hungarian_sentence() {
    let text = "Szia, világ! Budapest szép város.";
    let toks: Vec<&str> = HUNGARIAN.tokenize(text).collect();
    assert_eq!(toks, ["Szia", "világ", "Budapest", "szép", "város"]);
}

#[test]
fn stem_a_few_hungarian_words() {
    // Case-suffix strips demonstrating front/back vowel harmony —
    // every rule is a surface-form entry in the unified table.
    assert_eq!(HUNGARIAN.stem("házban"), "ház"); // back inessive
    assert_eq!(HUNGARIAN.stem("kertben"), "kert"); // front inessive
    assert_eq!(HUNGARIAN.stem("házak"), "ház"); // back plural
    assert_eq!(HUNGARIAN.stem("kertek"), "kert"); // front plural
    assert_eq!(HUNGARIAN.stem("házhoz"), "ház"); // back allative
    assert_eq!(HUNGARIAN.stem("kerthez"), "kert"); // front allative
    assert_eq!(HUNGARIAN.stem("körhöz"), "kör"); // front-rounded allative
}

#[test]
fn phonetic_encoder_is_phonex_hu() {
    let enc = HUNGARIAN
        .phonetic_encoder()
        .expect("Hungarian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-hu");
    let (primary, alt) = enc.encode("Budapest").expect("PHONEX-HU encodes Budapest");
    assert_eq!(primary, "B317");
    assert!(alt.is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(HUNGARIAN.collator().is_none());
}
