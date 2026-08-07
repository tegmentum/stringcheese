//! Integration tests for the Hebrew [`Language`] implementation.

use stringcheese_he::HEBREW;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(HEBREW.code(), "he");
    assert_eq!(HEBREW.name(), "Hebrew");
}

#[test]
fn common_hebrew_prepositions_are_stopwords() {
    for w in ["של", "על", "אל", "עם", "בין"] {
        assert!(
            HEBREW.is_stopword(w),
            "expected preposition {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_hebrew_pronouns_are_stopwords() {
    for w in ["אני", "אתה", "הוא", "היא", "אנחנו", "הם"] {
        assert!(
            HEBREW.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_hebrew_demonstratives_are_stopwords() {
    for w in ["זה", "זאת", "אלה", "הזה"] {
        assert!(
            HEBREW.is_stopword(w),
            "expected demonstrative {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_hebrew_conjunctions_are_stopwords() {
    for w in ["ו", "או", "אבל", "כי"] {
        assert!(
            HEBREW.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["ספר", "בית", "algorithm", "supercalifragilistic"] {
        assert!(!HEBREW.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_hebrew_sentence() {
    let toks: Vec<&str> = HEBREW.tokenize("אני אוהב לקרוא ספרים").collect();
    assert_eq!(toks, ["אני", "אוהב", "לקרוא", "ספרים"]);
}

#[test]
fn tokenize_hebrew_sentence_with_punctuation() {
    let toks: Vec<&str> = HEBREW.tokenize("שלום, עולם!").collect();
    assert_eq!(toks, ["שלום", "עולם"]);
}

#[test]
fn tokenize_preserves_maqaf_compound() {
    // The tokenizer keeps compound words joined by maqaf as one token.
    let toks: Vec<&str> = HEBREW.tokenize("אני לומד בבית־ספר").collect();
    assert_eq!(toks, ["אני", "לומד", "בבית־ספר"]);
}

#[test]
fn stem_definite_article() {
    // ה + ספר → ספר.
    assert_eq!(HEBREW.stem("הספר"), "ספר");
}

#[test]
fn stem_prefix_and_suffix() {
    // ה + ילד + ים → ילד.
    assert_eq!(HEBREW.stem("הילדים"), "ילד");
    // וה + ילד + ות → ילד.
    assert_eq!(HEBREW.stem("והילדות"), "ילד");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    // Roots chosen to NOT begin with a prefix letter (`ה ו ב כ ל מ ש`)
    // so the light stemmer's ambiguous-prefix over-strip does not
    // fire. `בית` (starts with the `ב` prefix letter) would be
    // stripped to `ית` — a documented light-stemmer limitation.
    assert_eq!(HEBREW.stem("ספר"), "ספר");
    assert_eq!(HEBREW.stem("אור"), "אור");
}

#[test]
fn phonetic_encoder_is_iso259_he() {
    let enc = HEBREW
        .phonetic_encoder()
        .expect("Hebrew pack ships a phonetic encoder");
    assert_eq!(enc.name(), "iso-259-he");
    assert_eq!(
        enc.encode("שלום"),
        Some((String::from("$lwm"), None)),
        "Iso259(שלום) should be $lwm with no alternate key"
    );
    // Final kaf folds to base kaf in the phonetic key.
    assert_eq!(
        enc.encode("מלך"),
        Some((String::from("mlk"), None)),
        "Iso259(מלך) should fold final kaf to base kaf"
    );
}

#[test]
fn phonetic_encoder_returns_none_on_hebrewless_input() {
    let enc = HEBREW.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(HEBREW.collator().is_none());
}
