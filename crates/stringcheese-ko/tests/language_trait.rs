//! Integration tests for the Korean [`Language`] implementation.

use stringcheese_ko::KOREAN;
use stringcheese_lang::Language;

#[test]
fn code_and_name() {
    assert_eq!(KOREAN.code(), "ko");
    assert_eq!(KOREAN.name(), "Korean");
}

#[test]
fn common_korean_conjunctions_are_stopwords() {
    for w in ["그리고", "하지만", "그러나", "또는", "그래서"] {
        assert!(
            KOREAN.is_stopword(w),
            "expected conjunction {w:?} to be a stopword",
        );
    }
}

#[test]
fn common_demonstratives_are_stopwords() {
    for w in ["이", "그", "저", "이것", "그것", "저것"] {
        assert!(
            KOREAN.is_stopword(w),
            "expected demonstrative {w:?} to be a stopword",
        );
    }
}

#[test]
fn interrogatives_are_stopwords() {
    for w in ["누구", "무엇", "어디", "언제", "왜", "어떻게"] {
        assert!(
            KOREAN.is_stopword(w),
            "expected interrogative {w:?} to be a stopword",
        );
    }
}

#[test]
fn non_stopwords_are_not_recognized() {
    for w in ["한국", "서울", "김치", "algorithm", "supercalifragilistic"] {
        assert!(!KOREAN.is_stopword(w), "{w:?} should not be a stopword");
    }
}

#[test]
fn tokenize_simple_sentence() {
    let toks: Vec<&str> = KOREAN.tokenize("나는 학교에 갑니다.").collect();
    assert_eq!(toks, ["나는", "학교에", "갑니다"]);
}

#[test]
fn tokenize_mixed_hangul_latin() {
    // Korean-English mixed phrasing — Latin fuses with adjacent
    // Hangul; whitespace and punctuation are the only separators.
    let toks: Vec<&str> = KOREAN.tokenize("iOS앱 2025년 릴리스").collect();
    assert_eq!(toks, ["iOS앱", "2025년", "릴리스"]);
}

#[test]
fn stem_case_particles() {
    // Topic marker.
    assert_eq!(KOREAN.stem("나는"), "나");
    // Locative.
    assert_eq!(KOREAN.stem("학교에"), "학교");
    // Ablative — longest match beats the shorter `-에`.
    assert_eq!(KOREAN.stem("학교에서"), "학교");
}

#[test]
fn stem_iteratively_peels_particle_stacks() {
    // `학교에서도` = 학교 + -에서 + -도 — the iterative loop peels
    // both particles.
    assert_eq!(KOREAN.stem("학교에서도"), "학교");
}

#[test]
fn stem_leaves_bare_nouns_alone() {
    // No suffix matches on a bare noun — the stemmer returns the
    // input unchanged.
    assert_eq!(KOREAN.stem("한국"), "한국");
    assert_eq!(KOREAN.stem("사람"), "사람");
}

#[test]
fn phonetic_encoder_is_phonex_ko() {
    let enc = KOREAN
        .phonetic_encoder()
        .expect("Korean pack ships a phonetic encoder");
    assert_eq!(enc.name(), "phonex-ko");
    // 한국 → RR "hanguk" → PHONEX "H522" (see the phonetic-module
    // reference test for the derivation).
    let (primary, alt) = enc.encode("한국").expect("한국 has phonex key");
    assert_eq!(primary, "H522");
    assert!(alt.is_none());
}

#[test]
fn phonetic_encoder_returns_none_on_letterless_input() {
    let enc = KOREAN.phonetic_encoder().unwrap();
    assert!(enc.encode("").is_none());
    assert!(enc.encode("...").is_none());
    assert!(enc.encode("123").is_none());
}

#[test]
fn collator_is_none_by_default() {
    assert!(KOREAN.collator().is_none());
}
