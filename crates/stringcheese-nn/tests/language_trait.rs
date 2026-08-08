//! Integration tests for the Nynorsk [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_nn::NYNORSK;

#[test]
fn code_and_name() {
    // BCP-47 `"nn"` — Norwegian Nynorsk specifically. The
    // macrolanguage tag `"no"` is deliberately not used; the Bokmål
    // sibling `stringcheese-no` registers `"nb"` in its turn.
    assert_eq!(NYNORSK.code(), "nn");
    assert_eq!(NYNORSK.name(), "Norwegian Nynorsk");
}

#[test]
fn common_nynorsk_words_are_stopwords() {
    for w in [
        "og", "i", "eit", "ein", "ei", "til", "er", "som", "på", "med", "av", "ikkje", "kva", "ho",
        "eg",
    ] {
        assert!(NYNORSK.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(NYNORSK.is_stopword("OG"));
    assert!(NYNORSK.is_stopword("Eit"));
    assert!(NYNORSK.is_stopword("EIN"));
    assert!(NYNORSK.is_stopword("Ikkje"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["fisk", "bilane", "algoritme", "supermarknad"] {
        assert!(!NYNORSK.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_nynorsk_sentence() {
    let text = "Katten søv på matta.";
    let toks: Vec<&str> = NYNORSK.tokenize(text).collect();
    assert_eq!(toks, ["Katten", "søv", "på", "matta"]);
}

#[test]
fn stem_a_few_nynorsk_words() {
    // Well-known Snowball Norwegian reductions applied to Nynorsk
    // input.
    assert_eq!(NYNORSK.stem("bilane"), "bil");
    assert_eq!(NYNORSK.stem("huset"), "hus");
    assert_eq!(NYNORSK.stem("guttene"), "gutt");
    assert_eq!(NYNORSK.stem("sannheter"), "sann");
    assert_eq!(NYNORSK.stem("krevande"), "krev");
    assert_eq!(NYNORSK.stem("høgast"), "høg");
}

#[test]
fn phonetic_encoder_is_phonex_nn() {
    let enc = NYNORSK
        .phonetic_encoder()
        .expect("Nynorsk pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-nn");
    assert_eq!(
        enc.encode("Hansen"),
        Some((String::from("A575"), None)),
        "PHONEX-NN(Hansen) should be A575 with no alternate key"
    );
    assert_eq!(
        enc.encode("Olsen"),
        Some((String::from("O475"), None)),
        "PHONEX-NN(Olsen) should be O475 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(NYNORSK.collator().is_none());
}
