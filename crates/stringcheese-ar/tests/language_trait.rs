//! Integration tests for the Arabic [`Language`] implementation.

use stringcheese_ar::ARABIC;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(ARABIC.code(), "ar");
    assert_eq!(ARABIC.name(), "Arabic");
}

#[test]
fn common_arabic_prepositions_are_stopwords() {
    for w in ["في", "من", "إلى", "على", "عن", "مع", "عند"] {
        assert!(
            ARABIC.is_stopword(w),
            "expected preposition {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_arabic_pronouns_are_stopwords() {
    for w in ["أنا", "نحن", "هو", "هي", "هم", "هن"] {
        assert!(
            ARABIC.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_arabic_demonstratives_are_stopwords() {
    for w in ["هذا", "هذه", "ذلك", "تلك", "هؤلاء"] {
        assert!(
            ARABIC.is_stopword(w),
            "expected demonstrative {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_arabic_conjunctions_are_stopwords() {
    for w in ["و", "أو", "ثم", "لكن", "أن"] {
        assert!(
            ARABIC.is_stopword(w),
            "expected conjunction {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["كتاب", "مدرسة", "algorithm", "supercalifragilistic"] {
        assert!(!ARABIC.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_arabic_sentence() {
    let toks: Vec<&str> = ARABIC.tokenize("محمد يحب القراءة").collect();
    assert_eq!(toks, ["محمد", "يحب", "القراءة"]);
}

#[test]
fn tokenize_with_arabic_punctuation() {
    let toks: Vec<&str> = ARABIC.tokenize("محمد يحب القراءة، وهو طالب.").collect();
    assert_eq!(toks, ["محمد", "يحب", "القراءة", "وهو", "طالب"]);
}

#[test]
fn stem_definite_article() {
    // ال + كتاب (book) → كتاب.
    assert_eq!(ARABIC.stem("الكتاب"), "كتاب");
}

#[test]
fn stem_prefix_and_suffix() {
    // ال + طالب + ات → طالب.
    assert_eq!(ARABIC.stem("الطالبات"), "طالب");
    // وال + طالب + ات → طالب.
    assert_eq!(ARABIC.stem("والطالبات"), "طالب");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    assert_eq!(ARABIC.stem("كتاب"), "كتاب");
}

#[test]
fn phonetic_encoder_is_buckwalter() {
    let enc = ARABIC
        .phonetic_encoder()
        .expect("Arabic pack ships a phonetic encoder");
    assert_eq!(enc.name(), "buckwalter");
    assert_eq!(
        enc.encode("محمد"),
        Some((String::from("mHmd"), None)),
        "Buckwalter(محمد) should be mHmd with no alternate key"
    );
    assert_eq!(
        enc.encode("أحمد"),
        Some((String::from(">Hmd"), None)),
        "Buckwalter(أحمد) should encode hamza-above-alef as '>'"
    );
}

#[test]
fn phonetic_encoder_returns_none_on_arabicless_input() {
    let enc = ARABIC.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(ARABIC.collator().is_none());
}
