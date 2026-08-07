//! Integration tests for the Ukrainian [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_uk::UKRAINIAN;
#[cfg(feature = "slavic-metaphone")]
use stringcheese_uk::{UKRAINIAN_WITH_SLAVIC_METAPHONE, Ukrainian};

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

// ---------------------------------------------------------------------
// Slavic-Metaphone opt-in encoder variant.
// ---------------------------------------------------------------------

#[cfg(feature = "slavic-metaphone")]
#[test]
fn slavic_metaphone_variant_swaps_the_phonetic_encoder() {
    let enc = UKRAINIAN_WITH_SLAVIC_METAPHONE
        .phonetic_encoder()
        .expect("Ukrainian slavic-metaphone pack ships a phonetic encoder");
    assert_eq!(enc.name(), "slavic-metaphone-2026");
}

#[cfg(feature = "slavic-metaphone")]
#[test]
fn slavic_metaphone_variant_encodes_cyrillic_and_latin_alike() {
    // Ukrainian Cyrillic "Чехов" (rendered as-if borrowing the Russian
    // surname) and its Latin transliteration "Chekhov" hash to the
    // same cross-Slavic key.
    let enc = UKRAINIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();
    let (cyr, _) = enc.encode("Чехов").expect("encodes Cyrillic");
    let (lat, _) = enc.encode("Chekhov").expect("encodes Latin");
    assert_eq!(cyr, lat);
}

#[cfg(feature = "slavic-metaphone")]
#[test]
fn default_encoder_can_be_restored_after_opting_in() {
    // Undo path: `.with_default_encoder()` returns the pack to the
    // Ukrainian GOST 7.79-B transliteration.
    let restored = Ukrainian::new()
        .with_slavic_metaphone_encoder()
        .with_default_encoder();
    let enc = restored
        .phonetic_encoder()
        .expect("restored pack ships an encoder");
    assert_eq!(enc.name(), "gost-7.79-b-uk");
}

#[cfg(feature = "slavic-metaphone")]
#[test]
fn default_ukrainian_constant_preserves_gost_encoder() {
    // Belt-and-braces: the module-level UKRAINIAN constant must keep
    // returning the GOST 7.79-B (UK) adapter even when the
    // slavic-metaphone feature is compiled in.
    let enc = UKRAINIAN
        .phonetic_encoder()
        .expect("default Ukrainian pack ships an encoder");
    assert_eq!(enc.name(), "gost-7.79-b-uk");
}
