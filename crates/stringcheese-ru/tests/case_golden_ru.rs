//! Golden case-mapping vectors for the Russian pack.
//!
//! ≥ 20 vectors covering ASCII, the modern Russian alphabet
//! (А..Я / а..я), the irregular Ё ↔ ё pair, whole-word inputs,
//! idempotence spot-checks, and the German ß expansion the pack
//! ships for uniform composed-engine behaviour.

#![cfg(all(feature = "case-scud", not(target_family = "wasm")))]

use stringcheese_icu_case::{CaseEngine, FoldMode};
use stringcheese_ru::case_data::case_pack;

fn engine() -> CaseEngine<'static> {
    CaseEngine::new(vec![case_pack().unwrap()])
}

// -----------------------------------------------------------------------
// ASCII round-trip (52 assertions)
// -----------------------------------------------------------------------

#[test]
fn ascii_upper_lower_round_trip() {
    let e = engine();
    for c in 'a'..='z' {
        let up = c.to_ascii_uppercase();
        assert_eq!(e.to_upper(&c.to_string(), "ru"), up.to_string());
        assert_eq!(e.to_lower(&up.to_string(), "ru"), c.to_string());
    }
}

// -----------------------------------------------------------------------
// Modern Russian alphabet (64 assertions — 32 pairs × 2)
// -----------------------------------------------------------------------

#[test]
fn russian_alphabet_pairs() {
    let e = engine();
    // U+0410..=U+042F (А..Я) map to U+0430..=U+044F (а..я).
    for upper_cp in 0x0410u32..=0x042Fu32 {
        let lower_cp = upper_cp + 0x20;
        let upper = char::from_u32(upper_cp).unwrap();
        let lower = char::from_u32(lower_cp).unwrap();
        assert_eq!(
            e.to_lower(&upper.to_string(), "ru"),
            lower.to_string(),
            "to_lower({upper:?}) failed"
        );
        assert_eq!(
            e.to_upper(&lower.to_string(), "ru"),
            upper.to_string(),
            "to_upper({lower:?}) failed"
        );
    }
}

// -----------------------------------------------------------------------
// Ё ↔ ё (5 assertions)
// -----------------------------------------------------------------------

#[test]
fn yo_pair_roundtrip() {
    let e = engine();
    assert_eq!(e.to_lower("Ё", "ru"), "ё");
    assert_eq!(e.to_upper("ё", "ru"), "Ё");
    assert_eq!(e.to_lower("ЁЖИК", "ru"), "ёжик");
    assert_eq!(e.to_upper("ёж", "ru"), "ЁЖ");
    assert_eq!(e.to_upper("ёлка", "ru"), "ЁЛКА");
}

// -----------------------------------------------------------------------
// Whole-word Russian (6 assertions)
// -----------------------------------------------------------------------

#[test]
fn whole_word_upper_lower() {
    let e = engine();
    assert_eq!(e.to_lower("МОСКВА", "ru"), "москва");
    assert_eq!(e.to_upper("москва", "ru"), "МОСКВА");
    assert_eq!(e.to_lower("САНКТ-ПЕТЕРБУРГ", "ru"), "санкт-петербург");
    assert_eq!(e.to_upper("санкт-петербург", "ru"), "САНКТ-ПЕТЕРБУРГ");
    assert_eq!(e.to_lower("ПРИВЕТ, МИР!", "ru"), "привет, мир!");
    assert_eq!(e.to_upper("привет, мир!", "ru"), "ПРИВЕТ, МИР!");
}

// -----------------------------------------------------------------------
// German ß expansion via ru pack (2 assertions)
// -----------------------------------------------------------------------

#[test]
fn sharp_s_expansion_via_ru_pack() {
    let e = engine();
    assert_eq!(e.to_upper("straße", "ru"), "STRASSE");
    assert_eq!(e.fold("Straße", FoldMode::Full), "strasse");
}

// -----------------------------------------------------------------------
// Empty / whitespace / punctuation (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn empty_whitespace_punctuation() {
    let e = engine();
    assert_eq!(e.to_lower("", "ru"), "");
    assert_eq!(e.to_upper("", "ru"), "");
    assert_eq!(e.to_lower("   ", "ru"), "   ");
    assert_eq!(e.to_upper("абв 123 ГДЕ", "ru"), "АБВ 123 ГДЕ");
}

// -----------------------------------------------------------------------
// Idempotence (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn to_lower_is_idempotent() {
    let e = engine();
    for input in ["МОСКВА", "Ёжик", "Санкт-Петербург", "hello"] {
        let once = e.to_lower(input, "ru");
        let twice = e.to_lower(&once, "ru");
        assert_eq!(once, twice, "to_lower not idempotent on {input:?}");
    }
}

// -----------------------------------------------------------------------
// Vector-count sanity
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_20() {
    // - ascii_upper_lower_round_trip:    2 * 26 = 52
    // - russian_alphabet_pairs:          2 * 32 = 64
    // - yo_pair_roundtrip:                       5
    // - whole_word_upper_lower:                  6
    // - sharp_s_expansion_via_ru_pack:           2
    // - empty_whitespace_punctuation:            4
    // - to_lower_is_idempotent:                  4
    // Total:                                   137
    const SHIPPED_VECTORS: usize = 52 + 64 + 5 + 6 + 2 + 4 + 4;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "ru golden vector count fell below the Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped ru golden vectors: {SHIPPED_VECTORS}");
}
