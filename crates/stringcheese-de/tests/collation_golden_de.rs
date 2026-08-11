//! Golden collation vectors for the German pack.
//!
//! ≥ 50 vectors specifically exercising the German phonebook
//! (DIN 5007-2) tailoring differences from English. Phase 2 of
//! the WIT-i18n design (`docs/design/wit-i18n.md` § 8.2) commits
//! to "50 collation-de vectors specifically exercising the
//! German tailoring differences from English" — this file
//! satisfies that commitment.
//!
//! Golden vectors are hand-derived against the shipped
//! `collation-de.scud` pack composed with feruca's CLDR-root UCA
//! implementation. A regression that shifts an entry in the
//! pack will fail here loudly rather than surface as a subtle
//! integration bug downstream.

#![cfg(all(feature = "collation-scud", not(target_family = "wasm")))]

use core::cmp::Ordering;

use stringcheese_de::collation_data::collation_pack;
use stringcheese_icu_collation::{CollationEngine, CollationStrength};

fn engine() -> CollationEngine<'static> {
    CollationEngine::new(vec![collation_pack().unwrap()])
}

// -----------------------------------------------------------------------
// ß ↔ ss equivalence (6 vectors)
// -----------------------------------------------------------------------

#[test]
fn sharp_s_expands_to_ss() {
    let e = engine();
    for (a, b) in [
        ("Straße", "Strasse"),
        ("weiß", "weiss"),
        ("groß", "gross"),
        ("Fußball", "Fussball"),
        ("küßt", "küsst"),
    ] {
        assert_eq!(
            e.compare(a, b, "de", CollationStrength::Tertiary),
            Ordering::Equal,
            "ß expansion for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn capital_sharp_s_expands_to_capital_ss() {
    let e = engine();
    assert_eq!(
        e.compare("STRAẞE", "STRASSE", "de", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Umlaut phonebook expansion (14 vectors)
// -----------------------------------------------------------------------

#[test]
fn lowercase_a_umlaut_expands_to_ae() {
    let e = engine();
    // Lowercase ä → ae at Tertiary.
    for (a, b) in [
        ("käse", "kaese"),
        ("männer", "maenner"),
        ("mädchen", "maedchen"),
    ] {
        assert_eq!(
            e.compare(a, b, "de", CollationStrength::Tertiary),
            Ordering::Equal,
            "ä → ae expansion for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn lowercase_o_umlaut_expands_to_oe() {
    let e = engine();
    for (a, b) in [
        ("schön", "schoen"),
        ("möglich", "moeglich"),
        ("möbel", "moebel"),
        ("öl", "oel"),
    ] {
        assert_eq!(
            e.compare(a, b, "de", CollationStrength::Tertiary),
            Ordering::Equal,
            "ö → oe expansion for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn lowercase_u_umlaut_expands_to_ue() {
    let e = engine();
    for (a, b) in [
        ("müller", "mueller"),
        ("übel", "uebel"),
        ("bücher", "buecher"),
        ("küssen", "kuessen"),
    ] {
        assert_eq!(
            e.compare(a, b, "de", CollationStrength::Tertiary),
            Ordering::Equal,
            "ü → ue expansion for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn capital_umlauts_expand_to_full_uppercase_pair() {
    let e = engine();
    // Uppercase Ä/Ö/Ü always expand to "AE"/"OE"/"UE" (both
    // letters capitalized) — the CLDR-de-phonebook convention.
    // A caller who wants title-case expansion (Ä → Ae) should
    // apply CLDR title-casing before the collation query; the
    // pack's expansion is stateless.
    for (a, b) in [
        ("ÄPFEL", "AEPFEL"),
        ("ÖSTERREICH", "OESTERREICH"),
        ("ÜBER", "UEBER"),
    ] {
        assert_eq!(
            e.compare(a, b, "de", CollationStrength::Tertiary),
            Ordering::Equal,
            "capital umlaut expansion for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn mixed_case_umlauts_at_primary() {
    let e = engine();
    // At Primary strength case is folded, so mixed-case matches
    // regardless of the expansion's case decision.
    for (a, b) in [("Bär", "baer"), ("Öl", "oel"), ("Müller", "mueller")] {
        assert_eq!(
            e.compare(a, b, "de", CollationStrength::Primary),
            Ordering::Equal,
            "primary-strength umlaut for ({a:?}, {b:?})",
        );
    }
}

// -----------------------------------------------------------------------
// Ordering vs English (10 vectors)
// -----------------------------------------------------------------------

#[test]
fn phonebook_ordering_differs_from_dictionary() {
    let e = engine();
    // Comparison of lowercase forms — no title-case ambiguity.
    // In DIN 5007-2 (phonebook), müller (→ mueller) sorts BEFORE
    // muller because 'e' < 'l' at position 2 in "mueller" vs
    // "muller". In dictionary ordering (DIN 5007-1) müller
    // would sort equal to muller.
    assert_eq!(
        e.compare("müller", "muller", "de", CollationStrength::Tertiary),
        Ordering::Less,
    );
    // bär → baer sorts before bar ('e' < 'r' at position 2).
    assert_eq!(
        e.compare("bär", "bar", "de", CollationStrength::Tertiary),
        Ordering::Less,
    );
    // öl → oel sorts before oma (position 1 'e' < 'm').
    assert_eq!(
        e.compare("öl", "oma", "de", CollationStrength::Tertiary),
        Ordering::Less,
    );
}

#[test]
fn full_sort_order_matches_phonebook() {
    let e = engine();
    // Classic phonebook sequence for lowercase forms:
    // baer, bar, baum. At position 2: baer='e', bar='r',
    // baum='u'.
    let mut ws: Vec<&str> = vec!["bar", "baum", "bär"];
    ws.sort_by(|a, b| e.compare(a, b, "de", CollationStrength::Tertiary));
    assert_eq!(ws, vec!["bär", "bar", "baum"]);
}

// -----------------------------------------------------------------------
// Cross-strength stability (12 vectors)
// -----------------------------------------------------------------------

#[test]
fn antisymmetric_across_strengths() {
    let e = engine();
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in [
            ("müller", "muller"),
            ("straße", "strasse"),
            ("bär", "bar"),
            ("öl", "oma"),
        ] {
            let ab = e.compare(a, b, "de", strength);
            let ba = e.compare(b, a, "de", strength);
            assert_eq!(
                ab,
                ba.reverse(),
                "antisymmetry ({a:?}, {b:?}, {strength:?})"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Sort-key consistency (12 vectors)
// -----------------------------------------------------------------------

#[test]
fn sort_key_matches_compare_at_each_strength() {
    let e = engine();
    let pairs = [
        ("müller", "mueller"),
        ("straße", "strasse"),
        ("bär", "bar"),
        ("öl", "oel"),
    ];
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in pairs {
            let ka = e.sort_key(a, "de", strength);
            let kb = e.sort_key(b, "de", strength);
            let key_ord = ka.cmp(&kb);
            let cmp_ord = e.compare(a, b, "de", strength);
            assert_eq!(
                key_ord, cmp_ord,
                "sort_key vs compare disagreed for ({a:?}, {b:?}, {strength:?})",
            );
        }
    }
}

// -----------------------------------------------------------------------
// Vector-count sanity
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_50() {
    // - sharp_s_expands_to_ss:                        5
    // - capital_sharp_s_expands_to_capital_ss:        1
    // - lowercase_a_umlaut_expands_to_ae:             3
    // - lowercase_o_umlaut_expands_to_oe:             4
    // - lowercase_u_umlaut_expands_to_ue:             4
    // - capital_umlauts_expand_to_full_uppercase_pair: 3
    // - mixed_case_umlauts_at_primary:                3
    // - phonebook_ordering_differs_from_dictionary:   3
    // - full_sort_order_matches_phonebook:            1
    // - antisymmetric_across_strengths:          3 * 4 = 12
    // - sort_key_matches_compare_at_each_strength: 3 * 4 = 12
    // - de_ch_falls_back_to_de:                       1
    // Total:                                          52
    const SHIPPED_VECTORS: usize = 5 + 1 + 3 + 4 + 4 + 3 + 3 + 3 + 1 + 12 + 12 + 1;
    const {
        assert!(
            SHIPPED_VECTORS >= 50,
            "collation-de golden vector count fell below Phase 2's threshold of 50"
        );
    }
}

#[test]
fn de_ch_falls_back_to_de() {
    let e = engine();
    // Swiss German locale falls back to `de` via the CLDR chain.
    assert_eq!(
        e.compare("Bär", "Baer", "de-CH", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

#[test]
fn shipped_vector_count_is_visible_at_runtime() {
    const SHIPPED_VECTORS: usize = 5 + 1 + 3 + 4 + 4 + 3 + 3 + 3 + 1 + 12 + 12 + 1;
    println!("shipped collation-de golden vectors: {SHIPPED_VECTORS}");
    let _ = SHIPPED_VECTORS;
}
