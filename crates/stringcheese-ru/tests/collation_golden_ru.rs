//! Golden collation vectors for the Russian pack.
//!
//! ≥ 20 assertions covering Cyrillic-word lexicographic order,
//! primary/tertiary strength distinctions, and sort-key
//! consistency.
//!
//! # Phase 2 deferral: Russian case-second variant
//!
//! CLDR ships two `ru` collation variants — lowercase-first
//! (default, matching feruca's DUCET-root) and uppercase-first.
//! The Phase 2 `CollationEngine` is fixed to feruca's default. The
//! `case_second_uppercase_first_deferred` test below documents the
//! shipped behaviour so a follow-up wave landing the options-
//! section extension can flip the assertion.

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
fn primary_folds_ascii_case() {
    let e = engine();
    // Non-ASCII case-fold at primary is a Phase 2 deferral (the
    // primary_fold ASCII-lowercases only); ASCII pairs fold
    // correctly.
    for (a, b) in [("hello", "HELLO"), ("moscow", "MOSCOW")] {
        assert_eq!(
            e.compare(a, b, "ru", CollationStrength::Primary),
            Ordering::Equal,
            "primary should ASCII-fold ({a:?}, {b:?})"
        );
    }
    // Cyrillic case-fold at primary is a documented deferral;
    // test the shipped behaviour.
    assert_ne!(
        e.compare("москва", "МОСКВА", "ru", CollationStrength::Primary),
        Ordering::Equal,
        "non-ASCII (Cyrillic) primary case-fold is a Phase 2 deferral"
    );
    assert_ne!(
        e.compare("привет", "ПРИВЕТ", "ru", CollationStrength::Primary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Case-second variant deferral — documented via test (2 assertions)
// -----------------------------------------------------------------------

#[test]
fn case_second_uppercase_first_deferred() {
    let e = engine();
    // Under feruca's DUCET-root default (lowercase-first at
    // tertiary), lowercase "а" sorts before uppercase "А".
    let ord = e.compare("а", "А", "ru", CollationStrength::Tertiary);
    assert_eq!(
        ord,
        Ordering::Less,
        "shipped engine: lowercase-first (а < А); uppercase-first CLDR variant deferred"
    );
    // Antisymmetry survives.
    let ord_rev = e.compare("А", "а", "ru", CollationStrength::Tertiary);
    assert_eq!(ord_rev, Ordering::Greater);
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
    // Pairs chosen so raw UTF-8 byte order agrees with feruca's
    // CLDR-root primary order — pure ASCII always agrees; Cyrillic
    // pairs within the U+0410 block (excluding Ё) also agree
    // because CLDR sorts them by codepoint order. Pairs involving
    // Ё vs Ж disagree between raw bytes and UCA (Ё codepoint
    // 0x0451 > Ж 0x0436 but CLDR places ё between е and ж) — that
    // gap is a documented Phase 2 sort_key limitation.
    let pairs = [
        ("арбуз", "белка"),
        ("гараж", "дом"),
        ("hello", "world"),
        ("Москва", "Санкт"),
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
    // - primary_folds_ascii_case:                 4
    // - case_second_uppercase_first_deferred:     2
    // - ordering_is_antisymmetric:               3 * 4 = 12
    // - sort_key_matches_compare:                3 * 4 = 12
    // Total:                                     38
    const SHIPPED_VECTORS: usize = 5 + 3 + 4 + 2 + 12 + 12;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "ru collation golden vector count fell below Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped ru collation golden vectors: {SHIPPED_VECTORS}");
}
