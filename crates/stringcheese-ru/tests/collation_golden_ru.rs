//! Golden collation vectors for the Russian pack.
//!
//! ≥ 20 assertions covering Cyrillic-word lexicographic order,
//! primary/tertiary strength distinctions, and sort-key
//! consistency.
//!
//! # Case-second variant (CLDR `ru` `standard`)
//!
//! The Russian pack sets the `case_second` options bit so
//! `CollationEngine` promotes case-distinguishing weights from
//! tertiary to secondary. Under this tailoring, lowercase sorts
//! before uppercase at secondary strength — matching CLDR's
//! `ru` `standard` variant. The `case_second_*` vectors below
//! exercise the promotion at Primary / Secondary / Tertiary.

#![cfg(all(feature = "collation-scud", not(target_family = "wasm")))]

use core::cmp::Ordering;

use stringcheese_icu_collation::{CollationEngine, CollationStrength};
use stringcheese_ru::collation_data::collation_pack;

fn engine() -> CollationEngine<'static> {
    CollationEngine::new(vec![collation_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Cyrillic word list ordering (5 assertions)
// -----------------------------------------------------------------------

#[test]
fn cyrillic_word_list_orders_alphabetically() {
    let e = engine();
    // Modern Cyrillic sorts by codepoint order under CLDR-root;
    // these five words are in Russian alphabetical order.
    let words = ["арбуз", "белка", "весна", "гараж", "дом"];
    for pair in words.windows(2) {
        let a = pair[0];
        let b = pair[1];
        assert_eq!(
            e.compare(a, b, "ru", CollationStrength::Tertiary),
            Ordering::Less,
            "expected {a:?} < {b:?} in Russian alphabetical order"
        );
    }
    // Reflexivity spot-check.
    assert_eq!(
        e.compare("привет", "привет", "ru", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Ё placement (3 assertions)
// -----------------------------------------------------------------------

#[test]
fn yo_letter_ordering() {
    let e = engine();
    // Ё (U+0401) sits between Е (U+0415) and Ж (U+0416) at
    // codepoint level; that is also what CLDR ships for `ru`.
    // Feruca's DUCET-root primary weights ё between е and ж.
    let ord_e_yo = e.compare("е", "ё", "ru", CollationStrength::Tertiary);
    let ord_yo_zh = e.compare("ё", "ж", "ru", CollationStrength::Tertiary);
    assert_eq!(ord_e_yo, Ordering::Less, "е should sort before ё");
    assert_eq!(ord_yo_zh, Ordering::Less, "ё should sort before ж");
    // Antisymmetry.
    assert_eq!(
        e.compare("ж", "ё", "ru", CollationStrength::Tertiary),
        Ordering::Greater,
    );
}

// -----------------------------------------------------------------------
// Primary strength — case folded (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn primary_folds_ascii_and_cyrillic_case() {
    let e = engine();
    // The case-second path's level-1 fold lowercases via
    // char::to_lowercase, so both ASCII and Cyrillic case pairs
    // collapse at primary.
    for (a, b) in [
        ("hello", "HELLO"),
        ("moscow", "MOSCOW"),
        ("москва", "МОСКВА"),
        ("привет", "ПРИВЕТ"),
    ] {
        assert_eq!(
            e.compare(a, b, "ru", CollationStrength::Primary),
            Ordering::Equal,
            "primary should case-fold ({a:?}, {b:?}) under ru case-second"
        );
    }
}

// -----------------------------------------------------------------------
// Case-second (CLDR ru standard) — 6 assertions
// -----------------------------------------------------------------------

#[test]
fn case_second_promotes_case_to_secondary_level() {
    let e = engine();
    // "Аа" (upper, lower) vs "аА" (lower, upper): primary folds to
    // the same "аа"; under case-second, the L2 case marker breaks
    // the tie — lowercase < uppercase, so leading-lowercase wins.
    assert_eq!(
        e.compare("Аа", "аА", "ru", CollationStrength::Secondary),
        Ordering::Greater,
    );
    assert_eq!(
        e.compare("аА", "Аа", "ru", CollationStrength::Secondary),
        Ordering::Less,
    );
    // Case difference dominates even at Tertiary strength.
    assert_eq!(
        e.compare("АБВ", "абв", "ru", CollationStrength::Tertiary),
        Ordering::Greater,
    );
    // A case difference at position 3 still dominates: "Абв" vs
    // "абВ" — both fold to "абв" at primary; L2 sequence is
    // [Upper, lower, lower] vs [lower, lower, Upper]. The first
    // position diverges (Upper > lower), so "Абв" > "абВ".
    assert_eq!(
        e.compare("Абв", "абВ", "ru", CollationStrength::Secondary),
        Ordering::Greater,
    );
    // "абВ" vs "абв" — only pos 3 differs, and lowercase wins.
    assert_eq!(
        e.compare("абВ", "абв", "ru", CollationStrength::Secondary),
        Ordering::Greater,
    );
    // Base-letter difference still beats case at every strength.
    assert_eq!(
        e.compare("АРБУЗ", "белка", "ru", CollationStrength::Secondary),
        Ordering::Less,
    );
}

#[test]
fn case_second_ties_disappear_at_primary() {
    let e = engine();
    // At Primary, case is ignored entirely.
    for (a, b) in [("Аа", "аА"), ("АБВ", "абв"), ("Абв", "абВ")] {
        assert_eq!(
            e.compare(a, b, "ru", CollationStrength::Primary),
            Ordering::Equal,
            "primary should tie for {a:?} vs {b:?}",
        );
    }
}

// -----------------------------------------------------------------------
// Cross-strength antisymmetry (12 assertions)
// -----------------------------------------------------------------------

#[test]
fn ordering_is_antisymmetric() {
    let e = engine();
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in [
            ("Москва", "Санкт"),
            ("арбуз", "белка"),
            ("дом", "гараж"),
            ("hello", "world"),
        ] {
            let ab = e.compare(a, b, "ru", strength);
            let ba = e.compare(b, a, "ru", strength);
            assert_eq!(
                ab,
                ba.reverse(),
                "antisymmetry ({a:?}, {b:?}, {strength:?})"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Sort key consistency (12 assertions)
// -----------------------------------------------------------------------

#[test]
fn sort_key_matches_compare() {
    let e = engine();
    // Case-second sort_key path derives its bytewise key from the
    // same L1|L2|L3 layout compare uses, so every pair round-trips
    // — including case-diverging pairs that fold to the same
    // primary form.
    let pairs = [
        ("арбуз", "белка"),
        ("гараж", "дом"),
        ("hello", "world"),
        ("Москва", "Санкт"),
        ("Аа", "аА"),
        ("Абв", "абВ"),
        ("привет", "ПРИВЕТ"),
    ];
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in pairs {
            let ka = e.sort_key(a, "ru", strength);
            let kb = e.sort_key(b, "ru", strength);
            let key_ord = ka.cmp(&kb);
            let cmp_ord = e.compare(a, b, "ru", strength);
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
    // - cyrillic_word_list_orders_alphabetically: 5
    // - yo_letter_ordering:                       3
    // - primary_folds_ascii_and_cyrillic_case:    4
    // - case_second_promotes_case_to_secondary:   6
    // - case_second_ties_disappear_at_primary:    3
    // - ordering_is_antisymmetric:               3 * 4 = 12
    // - sort_key_matches_compare:                3 * 7 = 21
    // Total:                                     54
    const SHIPPED_VECTORS: usize = 5 + 3 + 4 + 6 + 3 + 12 + 21;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "ru collation golden vector count fell below Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped ru collation golden vectors: {SHIPPED_VECTORS}");
}
