//! Integration tests for the Persian [`Language`] implementation.

use stringcheese_fa::PERSIAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(PERSIAN.code(), "fa");
    assert_eq!(PERSIAN.name(), "Persian");
}

#[test]
fn common_persian_prepositions_are_stopwords() {
    for w in ["در", "از", "به", "با", "بر", "برای"] {
        assert!(
            PERSIAN.is_stopword(w),
            "expected preposition {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_persian_pronouns_are_stopwords() {
    for w in ["من", "تو", "او", "ما", "شما"] {
        assert!(
            PERSIAN.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_persian_demonstratives_are_stopwords() {
    for w in ["این", "آن", "همین", "همان"] {
        assert!(
            PERSIAN.is_stopword(w),
            "expected demonstrative {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_persian_conjunctions_are_stopwords() {
    for w in ["و", "یا", "اما", "ولی", "که"] {
        assert!(
            PERSIAN.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_to_be_and_to_have_forms_are_stopwords() {
    for w in ["است", "هست", "بود", "دارد", "داشت"] {
        assert!(
            PERSIAN.is_stopword(w),
            "expected verb form {w:?} to be a stopword"
        );
    }
}

#[test]
fn object_marker_is_a_stopword() {
    assert!(
        PERSIAN.is_stopword("را"),
        "expected object marker را to be a stopword"
    );
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["کتاب", "مدرسه", "algorithm", "supercalifragilistic"] {
        assert!(!PERSIAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_persian_sentence() {
    let toks: Vec<&str> = PERSIAN.tokenize("محمد کتاب خواند").collect();
    assert_eq!(toks, ["محمد", "کتاب", "خواند"]);
}

#[test]
fn tokenize_preserves_zwnj_inside_compound_words() {
    // می‌خواند is one orthographic word — ZWNJ is word-internal.
    let toks: Vec<&str> = PERSIAN.tokenize("محمد کتاب می\u{200C}خواند").collect();
    assert_eq!(toks, ["محمد", "کتاب", "می\u{200C}خواند"]);
}

#[test]
fn tokenize_with_persian_punctuation() {
    // Arabic-block punctuation used in Persian text.
    let toks: Vec<&str> = PERSIAN.tokenize("محمد، کتاب خواند.").collect();
    assert_eq!(toks, ["محمد", "کتاب", "خواند"]);
}

#[test]
fn stem_plural_suffix() {
    // کتاب + ha (plural) → کتاب.
    assert_eq!(PERSIAN.stem("کتاب\u{200C}ها"), "کتاب");
    assert_eq!(PERSIAN.stem("کتابها"), "کتاب");
}

#[test]
fn stem_superlative_beats_comparative() {
    assert_eq!(PERSIAN.stem("بزرگ\u{200C}ترین"), "بزرگ");
    assert_eq!(PERSIAN.stem("بزرگ\u{200C}تر"), "بزرگ");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(PERSIAN.stem("کتاب"), "کتاب");
}

#[test]
fn phonetic_encoder_is_persian_buckwalter() {
    let enc = PERSIAN
        .phonetic_encoder()
        .expect("Persian pack ships a phonetic encoder");
    assert_eq!(enc.name(), "persian-buckwalter");
    assert_eq!(
        enc.encode("پارسی"),
        Some((String::from("pArsy"), None)),
        "Persian-Buckwalter(پارسی) should be pArsy with no alternate key"
    );
    assert_eq!(
        enc.encode("گل"),
        Some((String::from("gl"), None)),
        "Persian-Buckwalter(گل) should be gl"
    );
}

#[test]
fn phonetic_encoder_returns_none_on_scriptless_input() {
    let enc = PERSIAN.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(PERSIAN.collator().is_none());
}
