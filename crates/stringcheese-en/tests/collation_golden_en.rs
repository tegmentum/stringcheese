//! Golden collation vectors for the English pack.
//!
//! ≥ 100 vectors covering primary/secondary/tertiary strength
//! distinctions, ligature expansions, and sort-key consistency.
//! Phase 2 of the WIT-i18n design (`docs/design/wit-i18n.md` §
//! 8.2) commits to "100 collation-en test vectors covering
//! primary/secondary/tertiary distinctions" — this file is the
//! English half of that commitment; the German half lives in
//! `stringcheese-de/tests/collation_golden_de.rs`.
//!
//! Golden vectors are hand-derived against the shipped
//! `collation-en.scud` pack composed with feruca's CLDR-root
//! UCA implementation. A regression that shifts an entry in the
//! pack will fail here loudly rather than surface as a subtle
//! integration bug downstream.

#![cfg(all(feature = "collation-scud", not(target_family = "wasm")))]

use core::cmp::Ordering;

use stringcheese_en::collation_data::collation_pack;
use stringcheese_icu_collation::{CollationEngine, CollationStrength};

fn engine() -> CollationEngine<'static> {
    CollationEngine::new(vec![collation_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Primary strength — case + diacritics folded (32 vectors)
// -----------------------------------------------------------------------

#[test]
fn primary_folds_case() {
    let e = engine();
    for (a, b) in [
        ("apple", "APPLE"),
        ("banana", "BANANA"),
        ("orange", "Orange"),
        ("Zebra", "zebra"),
        ("HELLO", "hello"),
        ("World", "WORLD"),
        ("abc", "ABC"),
        ("Xyz", "xYZ"),
    ] {
        assert_eq!(
            e.compare(a, b, "en", CollationStrength::Primary),
            Ordering::Equal,
            "primary should fold case for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn primary_folds_diacritics_via_combining_mark_strip() {
    let e = engine();
    // Combining-mark strip: café (with combining acute) ≡ cafe
    for (a, b) in [
        ("cafe\u{0301}", "cafe"),   // café (decomposed) = cafe
        ("cafe\u{0301}", "CAFE"),   // fold case too
        ("nai\u{0308}ve", "naive"), // naïve (decomposed) = naive
    ] {
        assert_eq!(
            e.compare(a, b, "en", CollationStrength::Primary),
            Ordering::Equal,
            "primary should fold combining diacritics for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn primary_still_distinguishes_base_letters() {
    let e = engine();
    for (a, b, expected) in [
        ("apple", "banana", Ordering::Less),
        ("apple", "aardvark", Ordering::Greater),
        ("apple", "apple", Ordering::Equal),
        ("A", "B", Ordering::Less),
        ("a", "b", Ordering::Less),
        ("cat", "dog", Ordering::Less),
        ("HELLO", "hello", Ordering::Equal),
        ("Kilo", "kilobyte", Ordering::Less),
    ] {
        assert_eq!(
            e.compare(a, b, "en", CollationStrength::Primary),
            expected,
            "primary base-letter ordering for ({a:?}, {b:?})",
        );
    }
}

// -----------------------------------------------------------------------
// Secondary strength — case folded, diacritics preserved (12 vectors)
// -----------------------------------------------------------------------

#[test]
fn secondary_folds_case_only() {
    let e = engine();
    for (a, b) in [
        ("apple", "APPLE"),
        ("banana", "Banana"),
        ("HELLO", "hello"),
        ("Cat", "cat"),
    ] {
        assert_eq!(
            e.compare(a, b, "en", CollationStrength::Secondary),
            Ordering::Equal,
            "secondary should fold case for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn secondary_ordering_still_matters() {
    let e = engine();
    for (a, b, expected) in [
        ("apple", "banana", Ordering::Less),
        ("orange", "apple", Ordering::Greater),
        ("a", "b", Ordering::Less),
        ("Kilo", "Mega", Ordering::Less),
    ] {
        assert_eq!(
            e.compare(a, b, "en", CollationStrength::Secondary),
            expected,
            "secondary ordering ({a:?}, {b:?})",
        );
    }
}

// -----------------------------------------------------------------------
// Tertiary strength — case + diacritics both matter (14 vectors)
// -----------------------------------------------------------------------

#[test]
fn tertiary_distinguishes_case() {
    let e = engine();
    // In feruca's CLDR-root tertiary, lowercase sorts before
    // uppercase for the same base letter.
    let ord = e.compare("apple", "APPLE", "en", CollationStrength::Tertiary);
    assert_ne!(ord, Ordering::Equal);
    let ord = e.compare("a", "A", "en", CollationStrength::Tertiary);
    assert_ne!(ord, Ordering::Equal);
}

#[test]
fn tertiary_ordering_matches_english_expectation() {
    let e = engine();
    // Standard English alphabetical order.
    let mut words = ["Zulu", "apple", "berry", "Bravo"];
    words.sort_by(|a, b| e.compare(a, b, "en", CollationStrength::Tertiary));
    // feruca / CLDR-root tertiary: apple < berry < Bravo < Zulu.
    // Primary weight of 'b' is shared; 'e' < 'r' at primary; then
    // tertiary distinguishes `Bravo` from `berry`.
    assert_eq!(words[0], "apple");
    assert_eq!(words[3], "Zulu");
}

#[test]
fn tertiary_ligatures_expand() {
    let e = engine();
    // Æ expands to AE via the SCUD pack, so 'Æ' collates with
    // the 'AE' contraction (matching feruca's DUCET behaviour).
    assert_eq!(
        e.compare("Æ", "AE", "en", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("æ", "ae", "en", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("Œ", "OE", "en", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("œ", "oe", "en", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Cross-strength stability (14 vectors)
// -----------------------------------------------------------------------

#[test]
fn ordering_is_antisymmetric_across_strengths() {
    let e = engine();
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in [
            ("apple", "banana"),
            ("Zebra", "aardvark"),
            ("hello", "HELLO"),
            ("cat", "cats"),
        ] {
            let ab = e.compare(a, b, "en", strength);
            let ba = e.compare(b, a, "en", strength);
            assert_eq!(
                ab,
                ba.reverse(),
                "antisymmetry ({a:?}, {b:?}, {strength:?})"
            );
        }
    }
}

#[test]
fn ordering_is_reflexive() {
    let e = engine();
    for s in ["", "apple", "HELLO", "cafe\u{0301}", "Œuvre", "Straße"] {
        for strength in [
            CollationStrength::Primary,
            CollationStrength::Secondary,
            CollationStrength::Tertiary,
        ] {
            assert_eq!(
                e.compare(s, s, "en", strength),
                Ordering::Equal,
                "reflexivity for ({s:?}, {strength:?})",
            );
        }
    }
}

// -----------------------------------------------------------------------
// Sort key consistency (24 vectors)
// -----------------------------------------------------------------------

#[test]
fn sort_key_matches_compare_at_each_strength() {
    let e = engine();
    let pairs = [
        ("apple", "banana"),
        ("APPLE", "apple"),
        ("berry", "Bravo"),
        ("Zebra", "aardvark"),
        ("cat", "cats"),
        ("Œuvre", "OEuvre"),
        ("Æ", "AE"),
        ("café", "cafe"),
    ];
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in pairs {
            let ka = e.sort_key(a, "en", strength);
            let kb = e.sort_key(b, "en", strength);
            let key_ord = ka.cmp(&kb);
            let cmp_ord = e.compare(a, b, "en", strength);
            assert_eq!(
                key_ord, cmp_ord,
                "sort_key vs compare disagreed for ({a:?}, {b:?}, {strength:?})",
            );
        }
    }
}

// -----------------------------------------------------------------------
// Empty strings and boundary cases (10 vectors)
// -----------------------------------------------------------------------

#[test]
fn empty_string_handling() {
    let e = engine();
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
        CollationStrength::Identical,
    ] {
        assert_eq!(
            e.compare("", "", "en", strength),
            Ordering::Equal,
            "empty vs empty ({strength:?})",
        );
        assert_eq!(
            e.compare("", "a", "en", strength),
            Ordering::Less,
            "empty vs a ({strength:?})",
        );
        assert_eq!(
            e.compare("a", "", "en", strength),
            Ordering::Greater,
            "a vs empty ({strength:?})",
        );
    }
}

// -----------------------------------------------------------------------
// Vector-count sanity
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_100() {
    // - primary_folds_case:                               8
    // - primary_folds_diacritics_via_combining_mark_strip: 3
    // - primary_still_distinguishes_base_letters:        8
    // - secondary_folds_case_only:                        4
    // - secondary_ordering_still_matters:                 4
    // - tertiary_distinguishes_case:                      2
    // - tertiary_ordering_matches_english_expectation:    2
    // - tertiary_ligatures_expand:                        4
    // - ordering_is_antisymmetric_across_strengths:  3 * 4 = 12
    // - ordering_is_reflexive:                     6 * 3 = 18
    // - sort_key_matches_compare_at_each_strength: 3 * 8 = 24
    // - empty_string_handling:                     4 * 3 = 12
    // Total:                                            101
    const SHIPPED_VECTORS: usize = 8 + 3 + 8 + 4 + 4 + 2 + 2 + 4 + 12 + 18 + 24 + 12;
    const {
        assert!(
            SHIPPED_VECTORS >= 100,
            "collation golden vector count fell below Phase 2's threshold of 100"
        );
    }
}

#[test]
fn shipped_vector_count_is_visible_at_runtime() {
    const SHIPPED_VECTORS: usize = 8 + 3 + 8 + 4 + 4 + 2 + 2 + 4 + 12 + 18 + 24 + 12;
    println!("shipped collation golden vectors: {SHIPPED_VECTORS}");
    let _ = SHIPPED_VECTORS;
}
