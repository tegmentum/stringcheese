//! Golden collation vectors for the Turkish pack.
//!
//! ≥ 20 assertions covering primary/secondary/tertiary strength
//! distinctions, the German ß expansion the pack ships for
//! uniform composed-engine behaviour, and the CLDR-conformant
//! Turkish alphabet ordering — including the primary-distinct
//! dotless-ı / dotted-i tailoring shipped via
//! `SECT_PRIMARY_OVERRIDES`.
//!
//! # Primary-distinct ı / i (landed)
//!
//! Turkish's alphabetical order interleaves `... h ı i j ...`.
//! The tr collation pack ships primary-weight overrides for the
//! full Turkish lowercase alphabet; the `CollationEngine` picks
//! them up and ranks characters by their tabled weight rather
//! than DUCET root. Under this tailoring `ı < i` at primary,
//! matching Turkish dictionary conventions.

#![cfg(all(feature = "collation-scud", not(target_family = "wasm")))]

use core::cmp::Ordering;

use stringcheese_icu_collation::{CollationEngine, CollationStrength};
use stringcheese_tr::collation_data::collation_pack;

fn engine() -> CollationEngine<'static> {
    CollationEngine::new(vec![collation_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Basic ordering (10 assertions)
// -----------------------------------------------------------------------

#[test]
fn ascii_word_ordering() {
    let e = engine();
    for (a, b, expected) in [
        ("araba", "bebek", Ordering::Less),
        ("bebek", "araba", Ordering::Greater),
        ("bebek", "bebek", Ordering::Equal),
        ("cadde", "cami", Ordering::Less),
        ("dede", "dedem", Ordering::Less),
    ] {
        assert_eq!(
            e.compare(a, b, "tr", CollationStrength::Tertiary),
            expected,
            "tertiary ordering for ({a:?}, {b:?})"
        );
    }
}

#[test]
fn primary_folds_case() {
    let e = engine();
    for (a, b) in [
        ("araba", "ARABA"),
        ("bebek", "BEBEK"),
        ("kalem", "KALEM"),
        ("masa", "MASA"),
        ("sandalye", "SANDALYE"),
    ] {
        assert_eq!(
            e.compare(a, b, "tr", CollationStrength::Primary),
            Ordering::Equal,
            "primary should fold case for ({a:?}, {b:?})"
        );
    }
}

// -----------------------------------------------------------------------
// Turkish special letters — primary-override dictionary ordering
// (7 assertions)
// -----------------------------------------------------------------------

#[test]
fn turkish_special_letters_dictionary_order() {
    let e = engine();
    // Under the primary-override tailoring, Turkish letters sort
    // in strict dictionary order.
    for (a, b, expected) in [
        ("caz", "cim", Ordering::Less),
        ("çadır", "çam", Ordering::Less), // ç < ç ties, then a < a ties, then d < m at pos 2
        ("gemi", "göl", Ordering::Less),  // g < g ties, then e (150) < ö (280)
        ("hafta", "ıp", Ordering::Less),  // h (190) < ı (200)
        ("ıp", "ip", Ordering::Less),     // ı (200) < i (210)
        ("iyi", "jest", Ordering::Less),  // i (210) < j (220)
    ] {
        let ord = e.compare(a, b, "tr", CollationStrength::Primary);
        assert_eq!(ord, expected, "primary order for ({a:?}, {b:?})");
    }
    // Non-ASCII case-fold now works at primary under the override
    // table's ASCII-lowercase-then-lookup rule — ç folds to itself,
    // and uppercase Ç folds to lowercase ç (both have primary
    // weight 130 in the tr pack).
    assert_eq!(
        e.compare("çocuk", "ÇOCUK", "tr", CollationStrength::Primary),
        Ordering::Equal,
        "non-ASCII primary case-fold lands via SECT_PRIMARY_OVERRIDES",
    );
}

// -----------------------------------------------------------------------
// German ß expansion via the tr pack (2 assertions)
// -----------------------------------------------------------------------

#[test]
fn sharp_s_expansion_via_tr_pack() {
    let e = engine();
    assert_eq!(
        e.compare("Straße", "Strasse", "tr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("STRAẞE", "STRASSE", "tr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Primary-distinct ı / i (landed via SECT_PRIMARY_OVERRIDES) — 8 assertions
// -----------------------------------------------------------------------

#[test]
fn primary_distinct_dotless_i() {
    let e = engine();
    // Under the tr pack's primary-override table, `ı` (dotless-i,
    // primary weight 200) sorts strictly between `h` (190) and `i`
    // (210). This is the CLDR-conformant Turkish ordering.
    assert_eq!(
        e.compare("h", "ı", "tr", CollationStrength::Primary),
        Ordering::Less,
    );
    assert_eq!(
        e.compare("ı", "i", "tr", CollationStrength::Primary),
        Ordering::Less,
    );
    assert_eq!(
        e.compare("h", "i", "tr", CollationStrength::Primary),
        Ordering::Less,
    );
    // Whole words follow the same rule.
    assert_eq!(
        e.compare("hız", "ip", "tr", CollationStrength::Primary),
        Ordering::Less,
    );
    assert_eq!(
        e.compare("ıp", "ip", "tr", CollationStrength::Primary),
        Ordering::Less,
    );
    // Case-fold too — I lowercases to i (210), ı stays 200.
    assert_eq!(
        e.compare("I", "i", "tr", CollationStrength::Primary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("ı", "I", "tr", CollationStrength::Primary),
        Ordering::Less,
    );
    // Turkish alphabet as a sorted sequence.
    let mut letters = vec!["j", "i", "ı", "h", "g", "ğ"];
    letters.sort_by(|a, b| e.compare(a, b, "tr", CollationStrength::Primary));
    assert_eq!(letters, vec!["g", "ğ", "h", "ı", "i", "j"]);
}

// -----------------------------------------------------------------------
// Cross-strength antisymmetry (9 assertions)
// -----------------------------------------------------------------------

#[test]
fn ordering_is_antisymmetric() {
    let e = engine();
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in [("araba", "bebek"), ("kalem", "masa"), ("sandalye", "araba")] {
            let ab = e.compare(a, b, "tr", strength);
            let ba = e.compare(b, a, "tr", strength);
            assert_eq!(
                ab,
                ba.reverse(),
                "antisymmetry ({a:?}, {b:?}, {strength:?})"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Sort key consistency (9 assertions)
// -----------------------------------------------------------------------

#[test]
fn sort_key_matches_compare() {
    let e = engine();
    let pairs = [("araba", "bebek"), ("kalem", "masa"), ("Straße", "Strasse")];
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in pairs {
            let ka = e.sort_key(a, "tr", strength);
            let kb = e.sort_key(b, "tr", strength);
            let key_ord = ka.cmp(&kb);
            let cmp_ord = e.compare(a, b, "tr", strength);
            assert_eq!(
                key_ord, cmp_ord,
                "sort_key vs compare disagreed for ({a:?}, {b:?}, {strength:?})"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Vector-count sanity
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_20() {
    // - ascii_word_ordering:                        5
    // - primary_folds_case:                         5
    // - turkish_special_letters_dictionary_order:   7
    // - sharp_s_expansion_via_tr_pack:              2
    // - primary_distinct_dotless_i:                 8
    // - ordering_is_antisymmetric:                  3 * 3 = 9
    // - sort_key_matches_compare:                   3 * 3 = 9
    // Total:                                       45
    const SHIPPED_VECTORS: usize = 5 + 5 + 7 + 2 + 8 + 9 + 9;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "tr collation golden vector count fell below Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped tr collation golden vectors: {SHIPPED_VECTORS}");
}
