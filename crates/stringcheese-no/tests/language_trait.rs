//! Integration tests for the Norwegian [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_no::NORWEGIAN;

#[test]
fn code_and_name() {
    // BCP-47 `"nb"` — Norwegian Bokmål specifically. The macrolanguage
    // tag `"no"` is deliberately not used so a future
    // `stringcheese-nn` (Nynorsk) sibling can register `"nn"` without
    // shadowing this pack.
    assert_eq!(NORWEGIAN.code(), "nb");
    assert_eq!(NORWEGIAN.name(), "Norwegian Bokmål");
}

#[test]
fn common_norwegian_words_are_stopwords() {
    for w in [
        "og", "i", "det", "at", "en", "et", "den", "til", "er", "som", "på", "med", "av", "ikke",
    ] {
        assert!(NORWEGIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    // ASCII case folding via `str::eq_ignore_ascii_case`.
    assert!(NORWEGIAN.is_stopword("OG"));
    assert!(NORWEGIAN.is_stopword("Det"));
    assert!(NORWEGIAN.is_stopword("EN"));
    assert!(NORWEGIAN.is_stopword("Ikke"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["fisk", "bilene", "algoritme", "supermarked"] {
        assert!(!NORWEGIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_norwegian_sentence() {
    let text = "Katten sover på matten.";
    let toks: Vec<&str> = NORWEGIAN.tokenize(text).collect();
    assert_eq!(toks, ["Katten", "sover", "på", "matten"]);
}

#[test]
fn stem_a_few_norwegian_words() {
    // Well-known Snowball Norwegian reductions.
    assert_eq!(NORWEGIAN.stem("bilene"), "bil");
    assert_eq!(NORWEGIAN.stem("huset"), "hus");
    assert_eq!(NORWEGIAN.stem("guttene"), "gutt");
    assert_eq!(NORWEGIAN.stem("sannheter"), "sann");
}

#[test]
fn phonetic_encoder_is_phonex_no() {
    let enc = NORWEGIAN
        .phonetic_encoder()
        .expect("Norwegian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-no");
    assert_eq!(
        enc.encode("Hansen"),
        Some((String::from("A575"), None)),
        "PHONEX-NO(Hansen) should be A575 with no alternate key"
    );
    assert_eq!(
        enc.encode("Olsen"),
        Some((String::from("O475"), None)),
        "PHONEX-NO(Olsen) should be O475 with no alternate key"
    );
}

#[test]
fn collator_is_none_by_default() {
    assert!(NORWEGIAN.collator().is_none());
}
