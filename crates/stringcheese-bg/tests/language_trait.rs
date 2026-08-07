//! Integration tests for the Bulgarian [`Language`] implementation.

use stringcheese_bg::BULGARIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(BULGARIAN.code(), "bg");
    assert_eq!(BULGARIAN.name(), "Bulgarian");
}

#[test]
fn common_bulgarian_words_are_stopwords() {
    for w in ["и", "в", "на", "не", "с", "по", "за", "аз", "ти", "той"] {
        assert!(BULGARIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_cyrillic_fold() {
    // Cyrillic case-fold (Rust default Unicode fold): А → а, Я → я,
    // Ъ → ъ. Uppercase queries fold correctly to the plain lowercase
    // list.
    assert!(BULGARIAN.is_stopword("НЕ"));
    assert!(BULGARIAN.is_stopword("Не"));
    assert!(BULGARIAN.is_stopword("В"));
    assert!(BULGARIAN.is_stopword("СЪМ")); // Uppercase copula form.
    assert!(!BULGARIAN.is_stopword("СОФИЯ")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["книга", "куче", "компютър", "програма"] {
        assert!(!BULGARIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_bulgarian_sentence() {
    let text = "Здравей, свят! София — столица на България.";
    let toks: Vec<&str> = BULGARIAN.tokenize(text).collect();
    assert_eq!(
        toks,
        ["Здравей", "свят", "София", "столица", "на", "България"]
    );
}

#[test]
fn stem_definite_article_pack() {
    // The signature Bulgarian move: the definite article is a
    // suffix, not a separate word, and the stemmer collapses articled
    // and bare forms to the same stem.
    assert_eq!(BULGARIAN.stem("книгата"), BULGARIAN.stem("книга"));
    assert_eq!(BULGARIAN.stem("човекът"), "човек");
    assert_eq!(BULGARIAN.stem("учителят"), "учител");
    assert_eq!(BULGARIAN.stem("столовете"), "стол");
    assert_eq!(BULGARIAN.stem("красивата"), "красив");
}

#[test]
fn stem_a_few_bulgarian_words() {
    assert_eq!(BULGARIAN.stem("книга"), "книг");
    assert_eq!(BULGARIAN.stem("столове"), "стол");
    assert_eq!(BULGARIAN.stem("правя"), "прав");
    assert_eq!(BULGARIAN.stem("правим"), "прав");
}

#[test]
fn phonetic_encoder_is_gost_779_b_bg() {
    let enc = BULGARIAN
        .phonetic_encoder()
        .expect("Bulgarian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "gost-7.79-b-bg");
    let (primary, alt) = enc.encode("София").expect("encodes София");
    assert_eq!(primary, "sofiya");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_bulgarian_specific_letters() {
    // The Bulgarian-vs-Russian divergences: щ → sht (not shh), ъ → a
    // (not '', because ъ is a vowel in Bulgarian).
    let enc = BULGARIAN.phonetic_encoder().unwrap();
    let (shht, _) = enc.encode("щастие").unwrap();
    assert_eq!(shht, "shtastie");
    let (bg, _) = enc.encode("България").unwrap();
    assert_eq!(bg, "balgariya");
}

#[test]
fn phonetic_encoder_returns_none_for_no_cyrillic() {
    let enc = BULGARIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(BULGARIAN.collator().is_none());
}
