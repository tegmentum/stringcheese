//! Integration tests for the Belarusian [`Language`] implementation.

use stringcheese_be::BELARUSIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(BELARUSIAN.code(), "be");
    assert_eq!(BELARUSIAN.name(), "Belarusian");
}

#[test]
fn common_belarusian_words_are_stopwords() {
    for w in ["і", "у", "ў", "на", "не", "з", "па", "для", "я", "ты", "ён"] {
        assert!(BELARUSIAN.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup_uses_cyrillic_fold() {
    // Cyrillic case-fold (Rust default Unicode fold): А → а, Я → я,
    // Ў → ў, І → і. Uppercase queries fold correctly to the plain
    // lowercase list.
    assert!(BELARUSIAN.is_stopword("НЕ"));
    assert!(BELARUSIAN.is_stopword("Не"));
    assert!(BELARUSIAN.is_stopword("У"));
    assert!(BELARUSIAN.is_stopword("Ў")); // Belarusian-specific letter.
    assert!(BELARUSIAN.is_stopword("І")); // Belarusian/Ukrainian-specific letter.
    assert!(!BELARUSIAN.is_stopword("МІНСК")); // Not a stopword.
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["кніга", "сабака", "камп'ютар", "праграма"] {
        assert!(!BELARUSIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_belarusian_sentence() {
    let text = "Прывітанне, сям'я! Мінск — сталіца Беларусі.";
    let toks: Vec<&str> = BELARUSIAN.tokenize(text).collect();
    assert_eq!(
        toks,
        ["Прывітанне", "сям'я", "Мінск", "сталіца", "Беларусі"]
    );
}

#[test]
fn tokenize_preserves_apostrophe_words() {
    // The distinctive Belarusian tokenization behaviour: ASCII
    // apostrophe (U+0027) between two alphanumerics is word-internal.
    let toks: Vec<&str> = BELARUSIAN.tokenize("аб'ект пад'езд сям'я").collect();
    assert_eq!(toks, ["аб'ект", "пад'езд", "сям'я"]);
}

#[test]
fn stem_a_few_belarusian_words() {
    // Adjective + noun + verb chains. See the `stemmer_reference`
    // test for the wider set.
    assert_eq!(BELARUSIAN.stem("красівы"), "красів");
    assert_eq!(BELARUSIAN.stem("красівая"), "красів");
    assert_eq!(BELARUSIAN.stem("сталы"), "стал");
    assert_eq!(BELARUSIAN.stem("чытаць"), "чыта");
}

#[test]
fn phonetic_encoder_is_phonex_be() {
    let enc = BELARUSIAN
        .phonetic_encoder()
        .expect("Belarusian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-be");
    let (primary, alt) = enc.encode("Мінск").expect("encodes Мінск");
    assert_eq!(primary, "M572");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_handles_short_u() {
    // Belarusian-specific ў → W (labial class 1).
    let enc = BELARUSIAN.phonetic_encoder().unwrap();
    let (key, _) = enc.encode("аўтар").unwrap();
    assert_eq!(key, "A136");
}

#[test]
fn phonetic_encoder_handles_dz_digraph() {
    // Belarusian дз → single Z placeholder (class 7).
    let enc = BELARUSIAN.phonetic_encoder().unwrap();
    let (key, _) = enc.encode("падзея").unwrap();
    assert_eq!(key, "P700");
}

#[test]
fn phonetic_encoder_handles_dj_digraph() {
    // Belarusian дж → single J placeholder (class 7).
    let enc = BELARUSIAN.phonetic_encoder().unwrap();
    let (key, _) = enc.encode("джэм").unwrap();
    assert_eq!(key, "J500");
}

#[test]
fn phonetic_encoder_returns_none_for_no_cyrillic() {
    let enc = BELARUSIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(BELARUSIAN.collator().is_none());
}
