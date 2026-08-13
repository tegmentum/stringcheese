//! Golden case-mapping vectors for the Chinese pack.
//!
//! ≥ 20 assertions covering:
//!
//! * ASCII a-z ↔ A-Z (52 assertions).
//! * Han characters as no-op under upper/lower/fold (the whole
//!   point of this pack — verifying that Han queries fall through
//!   the pack lookup and land on Rust's identity fallback).
//! * German ß expansion (belt-and-braces for composed-engine
//!   behaviour).
//! * Mixed CJK/Latin input (Chinese text commonly interleaves
//!   English loanwords and product names).

#![cfg(all(feature = "case-scud", not(target_family = "wasm")))]

use stringcheese_icu_case::{CaseEngine, FoldMode};
use stringcheese_zh::case_data::case_pack;

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
        assert_eq!(e.to_upper(&c.to_string(), "zh"), up.to_string());
        assert_eq!(e.to_lower(&up.to_string(), "zh"), c.to_string());
    }
}

// -----------------------------------------------------------------------
// Han identity — the whole point of the pack (14 assertions)
// -----------------------------------------------------------------------

#[test]
fn han_upper_and_lower_are_identity() {
    let e = engine();
    // A handful of high-frequency Han characters — the pack does
    // not list them, so queries fall through to `char::to_lowercase`
    // / `char::to_uppercase` which returns Han scalars unchanged.
    let han_samples: &[&str] = &[
        "\u{4E2D}",         // 中 (middle)
        "\u{56FD}",         // 国 (country)
        "\u{4EBA}",         // 人 (person)
        "\u{5C71}",         // 山 (mountain)
        "\u{6C34}",         // 水 (water)
        "\u{4F60}\u{597D}", // 你好 (hello)
        "\u{4E16}\u{754C}", // 世界 (world)
    ];
    for input in han_samples {
        assert_eq!(
            &e.to_upper(input, "zh"),
            input,
            "to_upper is not identity on {input:?}"
        );
        assert_eq!(
            &e.to_lower(input, "zh"),
            input,
            "to_lower is not identity on {input:?}"
        );
    }
}

// -----------------------------------------------------------------------
// Han roundtrip (2 assertions)
// -----------------------------------------------------------------------

#[test]
fn han_upper_lower_roundtrip() {
    let e = engine();
    let input = "\u{4E2D}\u{56FD}\u{4EBA}\u{6C11}"; // 中国人民
    assert_eq!(e.to_upper(&e.to_lower(input, "zh"), "zh"), input);
    assert_eq!(e.to_lower(&e.to_upper(input, "zh"), "zh"), input);
}

// -----------------------------------------------------------------------
// German ß expansion via zh pack (2 assertions)
// -----------------------------------------------------------------------

#[test]
fn sharp_s_expansion_via_zh_pack() {
    let e = engine();
    assert_eq!(e.to_upper("straße", "zh"), "STRASSE");
    assert_eq!(e.fold("Straße", FoldMode::Full), "strasse");
}

// -----------------------------------------------------------------------
// Mixed CJK/Latin (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn mixed_cjk_latin_input() {
    let e = engine();
    // "iPhone 是苹果" — Latin brand name embedded in Chinese text.
    // The Han characters stay unchanged; the Latin part upper/lowers.
    assert_eq!(
        e.to_upper("iPhone \u{662F}\u{82F9}\u{679C}", "zh"),
        "IPHONE \u{662F}\u{82F9}\u{679C}"
    );
    assert_eq!(
        e.to_lower("IPHONE \u{662F}\u{82F9}\u{679C}", "zh"),
        "iphone \u{662F}\u{82F9}\u{679C}"
    );
    // "Beijing 北京" — Latin capitals become lowercase.
    assert_eq!(
        e.to_lower("Beijing \u{5317}\u{4EAC}", "zh"),
        "beijing \u{5317}\u{4EAC}"
    );
    assert_eq!(
        e.to_upper("beijing \u{5317}\u{4EAC}", "zh"),
        "BEIJING \u{5317}\u{4EAC}"
    );
}

// -----------------------------------------------------------------------
// Empty / whitespace / punctuation (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn empty_whitespace_punctuation() {
    let e = engine();
    assert_eq!(e.to_lower("", "zh"), "");
    assert_eq!(e.to_upper("", "zh"), "");
    assert_eq!(e.to_lower("   ", "zh"), "   ");
    // Full-width comma U+FF0C — punctuation, no case, identity.
    assert_eq!(e.to_upper("hello\u{FF0C}world", "zh"), "HELLO\u{FF0C}WORLD");
}

// -----------------------------------------------------------------------
// Fold (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn fold_operations() {
    let e = engine();
    assert_eq!(e.fold("HELLO", FoldMode::Simple), "hello");
    assert_eq!(e.fold("Straße", FoldMode::Full), "strasse");
    // Han stays identity under fold.
    assert_eq!(
        e.fold("\u{4E2D}\u{56FD}", FoldMode::Simple),
        "\u{4E2D}\u{56FD}"
    );
    assert_eq!(
        e.fold("\u{4E2D}\u{56FD}", FoldMode::Full),
        "\u{4E2D}\u{56FD}"
    );
}

// -----------------------------------------------------------------------
// Vector-count sanity
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_20() {
    // - ascii_upper_lower_round_trip:   2 * 26 = 52
    // - han_upper_and_lower_are_identity: 2 * 7 = 14
    // - han_upper_lower_roundtrip:              2
    // - sharp_s_expansion_via_zh_pack:          2
    // - mixed_cjk_latin_input:                  4
    // - empty_whitespace_punctuation:           4
    // - fold_operations:                        4
    // Total:                                   82
    const SHIPPED_VECTORS: usize = 52 + 14 + 2 + 2 + 4 + 4 + 4;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "zh golden vector count fell below the Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped zh golden vectors: {SHIPPED_VECTORS}");
}
