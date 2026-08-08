//! Integration tests for the Thai [`Language`] implementation.

use stringcheese_lang::Language;
use stringcheese_th::THAI;

#[test]
fn code_and_name() {
    assert_eq!(THAI.code(), "th");
    assert_eq!(THAI.name(), "Thai");
}

#[test]
fn common_thai_function_words_are_stopwords() {
    for w in ["และ", "แต่", "หรือ", "เป็น", "ที่", "ของ", "ใน", "ไม่"]
    {
        assert!(
            THAI.is_stopword(w),
            "expected function word {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_thai_pronouns_are_stopwords() {
    for w in ["ฉัน", "ผม", "เรา", "คุณ", "เขา"] {
        assert!(
            THAI.is_stopword(w),
            "expected pronoun {w:?} to be a stopword"
        );
    }
}

#[test]
fn common_conjunctions_and_particles_are_stopwords() {
    for w in ["และ", "แต่", "หรือ", "ครับ", "ค่ะ"] {
        assert!(
            THAI.is_stopword(w),
            "expected conjunction/particle {w:?} to be a stopword"
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in [
        "ประเทศ",
        "สวัสดี",
        "algorithm",
        "supercalifragilisticexpialidocious",
    ] {
        assert!(!THAI.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_mixed_script_sentence() {
    let toks: Vec<&str> = THAI.tokenize("hello ไทย 2026").collect();
    assert_eq!(toks, ["hello", "ไท", "ย", "2026"]);
}

#[test]
fn tokenize_thai_only_splits_at_cluster_boundaries() {
    // เป็น = pre-vowel เ + consonant ป + vowel mark ็ + consonant น.
    // Naive rule ends the first cluster at the second consonant.
    let toks: Vec<&str> = THAI.tokenize("เป็น").collect();
    assert_eq!(toks, ["เป็", "น"]);
}

#[test]
fn tokenize_whitespace_separates_thai_tokens() {
    // Whitespace between Thai phrases separates tokens.
    let toks: Vec<&str> = THAI.tokenize("ฉัน รัก").collect();
    assert_eq!(toks, ["ฉั", "น", "รั", "ก"]);
}

#[test]
fn stem_strips_verbal_nominalizer_kaan() {
    // การเรียน "learning" → เรียน.
    assert_eq!(THAI.stem("การเรียน"), "เรียน");
}

#[test]
fn stem_strips_abstract_nominalizer_khwaam() {
    // ความสุข "happiness" → สุข.
    assert_eq!(THAI.stem("ความสุข"), "สุข");
}

#[test]
fn stem_strips_agent_prefix_phu() {
    // ผู้พูด "speaker" → พูด.
    assert_eq!(THAI.stem("ผู้พูด"), "พูด");
}

#[test]
fn stem_strips_agent_prefix_nak() {
    // นักเรียน "student" → เรียน.
    assert_eq!(THAI.stem("นักเรียน"), "เรียน");
}

#[test]
fn stem_folds_reduplication() {
    // เด็กเด็ก → เด็ก (children).
    assert_eq!(THAI.stem("เด็กเด็ก"), "เด็ก");
}

#[test]
fn stem_is_identity_on_bare_content_words() {
    // Thai has no inflection — content words pass through.
    assert_eq!(THAI.stem("สวัสดี"), "สวัสดี");
    assert_eq!(THAI.stem("ประเทศ"), "ประเทศ");
    assert_eq!(THAI.stem("hello"), "hello");
}

#[test]
fn phonetic_encoder_is_phonex_th() {
    let enc = THAI
        .phonetic_encoder()
        .expect("Thai pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-th");
    let (primary, alt) = enc.encode("สวัสดี").expect("encodes สวัสดี");
    assert!(!primary.is_empty());
    assert_eq!(primary.chars().count(), 4);
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_returns_none_for_no_thai() {
    let enc = THAI.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    assert!(enc.encode("hello").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(THAI.collator().is_none());
}

#[test]
fn stopwords_slice_is_non_empty() {
    assert!(!THAI.stopwords().is_empty());
    // The specific 40-90 range mirrors the src/stopwords.rs test.
    assert!(THAI.stopwords().len() >= 40);
}
