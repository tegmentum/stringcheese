//! Integration tests for the English [`Language`] implementation.

use stringcheese_en::ENGLISH;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(ENGLISH.code(), "en");
    assert_eq!(ENGLISH.name(), "English");
}

#[test]
fn common_english_words_are_stopwords() {
    for w in [
        "the", "and", "of", "a", "in", "to", "is", "it", "for", "you",
    ] {
        assert!(ENGLISH.is_stopword(w), "expected {w:?} to be a stopword");
    }
}

#[test]
fn case_insensitive_stopword_lookup() {
    assert!(ENGLISH.is_stopword("The"));
    assert!(ENGLISH.is_stopword("THE"));
    assert!(ENGLISH.is_stopword("tHe"));
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "cheese",
        "algorithm",
        "porter",
        "linguistic",
        "supercalifragilistic",
    ] {
        assert!(!ENGLISH.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_english_sentence() {
    let text = "The quick brown fox jumps over the lazy dog.";
    let toks: Vec<&str> = ENGLISH.tokenize(text).collect();
    assert_eq!(
        toks,
        [
            "The", "quick", "brown", "fox", "jumps", "over", "the", "lazy", "dog"
        ]
    );
}

#[test]
fn stem_a_few_english_words() {
    assert_eq!(ENGLISH.stem("caresses"), "caress");
    assert_eq!(ENGLISH.stem("ponies"), "poni");
    assert_eq!(ENGLISH.stem("running"), "run");
    // Full 5-step Porter output; see tests/porter_reference.rs.
    assert_eq!(ENGLISH.stem("agreed"), "agre");
    assert_eq!(ENGLISH.stem("hopping"), "hop");
}

#[test]
fn phonetic_encoder_is_soundex() {
    let enc = ENGLISH
        .phonetic_encoder()
        .expect("English pack ships a phonetic encoder");
    assert_eq!(enc.name(), "soundex");
    assert_eq!(
        enc.encode("SMITH"),
        Some((String::from("S530"), None)),
        "Soundex(SMITH) should be S530 with no alternate key"
    );
    assert_eq!(
        enc.encode("Robert"),
        Some((String::from("R163"), None)),
        "Soundex(Robert) should be R163 with no alternate key"
    );
}

#[test]
fn collator_is_the_english_dictionary_collator() {
    use core::cmp::Ordering;

    let c = ENGLISH
        .collator()
        .expect("English pack ships a dictionary-order collator");
    // Ignore leading articles: "The Beatles" strips to "Beatles",
    // which sorts after "Abbey Road".
    assert_eq!(c.compare("Abbey Road", "The Beatles"), Ordering::Less);
    // ASCII case-fold.
    assert_eq!(c.compare("banana", "BANANA"), Ordering::Equal);
    // Digits sort after letters.
    assert_eq!(c.compare("banana", "1st"), Ordering::Less);
}
