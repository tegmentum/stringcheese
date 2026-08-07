//! Integration tests for the Ukrainian [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_uk::UKRAINIAN;

#[test]
fn code_and_name() {
    assert_eq!(UKRAINIAN.code(), "uk");
    assert_eq!(UKRAINIAN.name(), "Ukrainian");
}

#[test]
fn common_ukrainian_words_are_stopwords() {
    for w in ["і", "в", "на", "не", "з", "по", "для", "я", "ти", "він"] {
        assert!(UKRAINIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_cyrillic_fold() {
    // Cyrillic case-fold (Rust default Unicode fold): А → а, Я → я,
    // Ґ → ґ, Є → є, І → і, Ї → ї. Uppercase queries fold correctly
    // to the plain lowercase list.
    assert!(UKRAINIAN.is_stopword("НЕ"));
    assert!(UKRAINIAN.is_stopword("Не"));
    assert!(UKRAINIAN.is_stopword("В"));
    assert!(UKRAINIAN.is_stopword("І")); // Ukrainian-specific letter.
    assert!(!UKRAINIAN.is_stopword("КИЇВ")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["книга", "собака", "комп'ютер", "програма"] {
        assert!(!UKRAINIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_ukrainian_sentence() {
    let text = "Привіт, сім'я! Київ — столиця України.";
    let toks: Vec<&str> = UKRAINIAN.tokenize(text).collect();
    assert_eq!(toks, ["Привіт", "сім'я", "Київ", "столиця", "України"]);
}

#[test]
fn tokenize_preserves_apostrophe_words() {
    // The distinctive Ukrainian tokenization behaviour: ASCII
    // apostrophe (U+0027) between two alphanumerics is word-internal.
    let toks: Vec<&str> = UKRAINIAN.tokenize("п'ять м'яких об'єктів").collect();
    assert_eq!(toks, ["п'ять", "м'яких", "об'єктів"]);
}

#[test]
fn stem_a_few_ukrainian_words() {
    // Adjective + noun + verb chains. See the `snowball_reference`
    // test for the wider set.
    assert_eq!(UKRAINIAN.stem("красивий"), "красив");
    assert_eq!(UKRAINIAN.stem("красива"), "красив");
    assert_eq!(UKRAINIAN.stem("столи"), "стол");
    assert_eq!(UKRAINIAN.stem("читати"), "чита");
}

#[test]
fn phonetic_encoder_is_gost_779_b_uk() {
    let enc = UKRAINIAN
        .phonetic_encoder()
        .expect("Ukrainian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "gost-7.79-b-uk");
    let (primary, alt) = enc.encode("Київ").expect("encodes Київ");
    assert_eq!(primary, "kyyiv");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_distinguishes_g_and_h() {
    // The Ukrainian-vs-Russian divergence: г → h, ґ → g.
    let enc = UKRAINIAN.phonetic_encoder().unwrap();
    let (h, _) = enc.encode("гора").unwrap();
    assert_eq!(h, "hora");
    let (g, _) = enc.encode("ґанок").unwrap();
    assert_eq!(g, "ganok");
}

#[test]
fn phonetic_encoder_returns_none_for_no_cyrillic() {
    let enc = UKRAINIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(UKRAINIAN.collator().is_none());
}
