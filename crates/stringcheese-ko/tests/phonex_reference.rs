//! Reference values for the Korean PHONEX phonetic encoder.
//!
//! Pins the two-step algorithm (Revised Romanization → Soundex-family
//! 4-character key) at concrete inputs. When either of the underlying
//! tables shifts, these tests flip red — the intent is that any table
//! change is a deliberate act, not an accidental drift.

use stringcheese_ko::phonetic::{KoreanPhonex, KoreanPhonexAdapter, revised_romanization};
use stringcheese_lang::LanguagePhoneticEncoder;

// -----------------------------------------------------------------
// Revised Romanization — reference values.
// -----------------------------------------------------------------

fn r(s: &str) -> String {
    revised_romanization(s)
}

#[test]
fn rr_country_name_hanguk() {
    // 한 (ᄒ+ᅡ+ᆫ = han) + 국 (ᄀ+ᅮ+ᆨ = guk).
    assert_eq!(r("한국"), "hanguk");
}

#[test]
fn rr_capital_seoul() {
    // 서 (ᄉ+ᅥ = seo) + 울 (ᄋ+ᅮ+ᆯ = ul).
    assert_eq!(r("서울"), "seoul");
}

#[test]
fn rr_kimchi() {
    // 김 (ᄀ+ᅵ+ᆷ = gim) + 치 (ᄎ+ᅵ = chi).
    assert_eq!(r("김치"), "gimchi");
}

#[test]
fn rr_hangul() {
    // 한 (han) + 글 (ᄀ+ᅳ+ᆯ = geul).
    assert_eq!(r("한글"), "hangeul");
}

#[test]
fn rr_double_consonant_onset_kk() {
    // 까 = ᄁ + ᅡ = "kka".
    assert_eq!(r("까"), "kka");
}

#[test]
fn rr_double_consonant_final_kk() {
    // 밖 = ᄇ + ᅡ + ᆩ = "bakk".
    assert_eq!(r("밖"), "bakk");
}

#[test]
fn rr_null_onset_is_silent() {
    // ᄋ in the initial slot is a placeholder — no consonant emitted.
    assert_eq!(r("아"), "a"); // ᄋ + ᅡ
    assert_eq!(r("이"), "i"); // ᄋ + ᅵ
}

#[test]
fn rr_final_ng_from_ieung() {
    // 강 = ᄀ + ᅡ + ᆼ = "gang". Final ᄋ is a real /ŋ/ consonant.
    assert_eq!(r("강"), "gang");
}

#[test]
fn rr_final_g_reads_as_k() {
    // 국 = ᄀ + ᅮ + ᆨ = "guk". Final ᄀ reads as `k`, not `g`.
    assert_eq!(r("국"), "guk");
}

#[test]
fn rr_final_b_reads_as_p() {
    assert_eq!(r("밥"), "bap");
}

#[test]
fn rr_final_d_reads_as_t() {
    assert_eq!(r("곧"), "got");
}

#[test]
fn rr_final_r_reads_as_l() {
    assert_eq!(r("물"), "mul");
}

#[test]
fn rr_diphthong_wae() {
    assert_eq!(r("왜"), "wae");
}

#[test]
fn rr_pass_through_ascii_and_empty() {
    assert_eq!(r(""), "");
    assert_eq!(r("hello"), "hello");
    assert_eq!(r("hello 서울"), "hello seoul");
}

// -----------------------------------------------------------------
// PHONEX — reference values.
// -----------------------------------------------------------------

fn p(w: &str) -> String {
    KoreanPhonex.encode(w).expect("non-empty input encodes")
}

#[test]
fn phonex_seoul() {
    // "seoul" → S(seed), E O U (vowel drops), L(4) → "S4" → "S400".
    assert_eq!(p("서울"), "S400");
}

#[test]
fn phonex_hanguk() {
    // "hanguk" → H(seed), A(reset), N(5), G(2), U(reset), K(2) → "H522".
    assert_eq!(p("한국"), "H522");
}

#[test]
fn phonex_key_shape_is_letter_plus_three_digits() {
    for w in ["한국", "서울", "김치", "한글", "강남", "부산", "인천"] {
        let key = p(w);
        assert_eq!(key.len(), 4, "key {key:?} for {w:?} not 4 chars");
        assert!(
            key.chars().next().unwrap().is_ascii_uppercase(),
            "first char of {key:?} not uppercase",
        );
        assert!(
            key[1..].bytes().all(|b| b.is_ascii_digit()),
            "trailing {:?} not all digits",
            &key[1..],
        );
    }
}

#[test]
fn phonex_returns_none_for_empty_or_letterless() {
    assert!(KoreanPhonex.encode("").is_none());
    assert!(KoreanPhonex.encode("   ").is_none());
    assert!(KoreanPhonex.encode("...").is_none());
    assert!(KoreanPhonex.encode("123").is_none());
}

// -----------------------------------------------------------------
// Adapter.
// -----------------------------------------------------------------

#[test]
fn adapter_name_is_phonex_ko() {
    assert_eq!(KoreanPhonexAdapter.name(), "phonex-ko");
}

#[test]
fn adapter_wraps_phonex_output_in_primary_only_tuple() {
    let (primary, alt) = KoreanPhonexAdapter.encode("서울").expect("서울 encodes");
    assert_eq!(primary, "S400");
    assert!(alt.is_none());
}

#[test]
fn adapter_returns_none_on_letterless_input() {
    assert!(KoreanPhonexAdapter.encode("").is_none());
    assert!(KoreanPhonexAdapter.encode("...").is_none());
    assert!(KoreanPhonexAdapter.encode("123").is_none());
}
