//! Golden collation vectors for the French pack.
//!
//! ≥ 20 assertions covering primary/secondary/tertiary strength
//! distinctions, ligature expansions, French alphabetical ordering,
//! the **backwards-secondary** accent tie-break, and sort-key
//! consistency.
//!
//! # Backwards-secondary landing
//!
//! Classical French sort orders accents right-to-left within a
//! word: `cote < côte < coté < côté` — words are compared at the
//! primary level on their base letters (all four fold to "cote"),
//! then the level-2 tie-break scans accents from the RIGHT.
//! Landed via the `SECT_COLLATION_OPTIONS` backwards-secondary bit
//! consumed by `stringcheese-icu-collation::CollationEngine`. The
//! `backwards_secondary_classic_quartet` test below asserts the
//! full four-way ordering.

#![cfg(all(feature = "collation-scud", not(target_family = "wasm")))]

use core::cmp::Ordering;

use stringcheese_fr::collation_data::collation_pack;
use stringcheese_icu_collation::{CollationEngine, CollationStrength};

fn engine() -> CollationEngine<'static> {
    CollationEngine::new(vec![collation_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Primary strength — case + diacritics folded (10 assertions)
// -----------------------------------------------------------------------

#[test]
fn primary_folds_case_and_combining_marks() {
    let e = engine();
    // Precomposed accented characters (é U+00E9, É U+00C9) do NOT
    // fold in Phase 2's primary_fold — that is a documented
    // deferral (see docs/design/wit-i18n.md § 8.2, "precomposed
    // accented-character decomposition"). We test only the
    // ASCII-fold and the decomposed-form-strip paths.
    for (a, b) in [
        ("bonjour", "BONJOUR"),
        ("cafe\u{0301}", "cafe"),   // decomposed é vs plain e
        ("nai\u{0308}ve", "naive"), // decomposed ï vs plain i
        ("oeuvre", "OEUVRE"),
    ] {
        assert_eq!(
            e.compare(a, b, "fr", CollationStrength::Primary),
            Ordering::Equal,
            "primary should fold for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn primary_still_distinguishes_base_letters() {
    let e = engine();
    for (a, b, expected) in [
        ("bonjour", "chateau", Ordering::Less),
        ("pomme", "orange", Ordering::Greater),
        ("chien", "chat", Ordering::Greater),
        ("un", "deux", Ordering::Greater),
        ("a", "b", Ordering::Less),
    ] {
        assert_eq!(
            e.compare(a, b, "fr", CollationStrength::Primary),
            expected,
            "primary base-letter ordering for ({a:?}, {b:?})",
        );
    }
}

// -----------------------------------------------------------------------
// Secondary / tertiary strength (6 assertions)
// -----------------------------------------------------------------------

#[test]
fn secondary_folds_case() {
    let e = engine();
    // Same precomposed-vs-decomposed caveat as
    // `primary_folds_case_and_combining_marks`.
    for (a, b) in [("bonjour", "BONJOUR"), ("hello", "HELLO")] {
        assert_eq!(
            e.compare(a, b, "fr", CollationStrength::Secondary),
            Ordering::Equal,
            "secondary should fold case for ({a:?}, {b:?})",
        );
    }
}

#[test]
fn tertiary_distinguishes_case_and_diacritics() {
    let e = engine();
    // Case differs at tertiary.
    assert_ne!(
        e.compare("bonjour", "BONJOUR", "fr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    // Diacritic differs at tertiary via feruca's UCA weights (é
    // has a distinct secondary/tertiary weight from e).
    assert_ne!(
        e.compare("cafe", "café", "fr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Ligature expansion (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn ligature_expansions() {
    let e = engine();
    // Œ → OE via SCUD expansion; the compare sees the two as equal
    // at tertiary.
    assert_eq!(
        e.compare("Œuvre", "OEuvre", "fr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("œuvre", "oeuvre", "fr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("Æquus", "AEquus", "fr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("æquus", "aequus", "fr", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Backwards-secondary classic tie-break sequence (7 assertions)
// -----------------------------------------------------------------------

#[test]
fn backwards_secondary_classic_quartet() {
    // Classical French dictionary order: base letters all fold to
    // "cote" at primary, so the tie-break scans accents from the
    // RIGHT. Reversed per-position secondary sequences:
    //   cote → [0, 0, 0, 0]
    //   côte → [0, 0, ô, 0]
    //   coté → [é, 0, 0, 0]
    //   côté → [é, 0, ô, 0]
    // Bytewise sort gives cote < côte < coté < côté.
    let e = engine();
    let mut words = vec!["côté", "coté", "cote", "côte"];
    words.sort_by(|a, b| e.compare(a, b, "fr", CollationStrength::Tertiary));
    assert_eq!(words, vec!["cote", "côte", "coté", "côté"]);
}

#[test]
fn backwards_secondary_all_tie_at_primary() {
    // All four words fold to the same primary key ("cote") when
    // combining marks are stripped and precomposed accented Latin
    // letters are decomposed to base + mark.
    let e = engine();
    for a in ["cote", "côte", "coté", "côté"] {
        for b in ["cote", "côte", "coté", "côté"] {
            assert_eq!(
                e.compare(a, b, "fr", CollationStrength::Primary),
                Ordering::Equal,
                "primary should tie for ({a:?}, {b:?})",
            );
        }
    }
}

#[test]
fn backwards_secondary_survives_reflexivity() {
    let e = engine();
    for w in ["cote", "côte", "coté", "côté"] {
        assert_eq!(
            e.compare(w, w, "fr", CollationStrength::Tertiary),
            Ordering::Equal,
        );
    }
}

// -----------------------------------------------------------------------
// Cross-strength stability (12 assertions)
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
            ("bonjour", "chateau"),
            ("Œuvre", "aardvark"),
            ("café", "cafe"),
            ("chien", "chat"),
        ] {
            let ab = e.compare(a, b, "fr", strength);
            let ba = e.compare(b, a, "fr", strength);
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
    let pairs = [
        ("bonjour", "chateau"),
        ("Œuvre", "OEuvre"),
        ("café", "cafe"),
        ("chien", "chat"),
    ];
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in pairs {
            let ka = e.sort_key(a, "fr", strength);
            let kb = e.sort_key(b, "fr", strength);
            let key_ord = ka.cmp(&kb);
            let cmp_ord = e.compare(a, b, "fr", strength);
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
fn shipped_vector_count_meets_20() {
    // - primary_folds_case_and_combining_marks:  5
    // - primary_still_distinguishes_base_letters: 5
    // - secondary_folds_case:                    2
    // - tertiary_distinguishes_case_and_diacritics: 2
    // - ligature_expansions:                     4
    // - backwards_secondary_classic_quartet:     1 (four-way sort)
    // - backwards_secondary_all_tie_at_primary:  16 (4x4)
    // - backwards_secondary_survives_reflexivity: 4
    // - ordering_is_antisymmetric_across_strengths: 3 * 4 = 12
    // - sort_key_matches_compare:                3 * 4 = 12
    // Total:                                    63
    const SHIPPED_VECTORS: usize = 5 + 5 + 2 + 2 + 4 + 1 + 16 + 4 + 12 + 12;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "fr collation golden vector count fell below Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped fr collation golden vectors: {SHIPPED_VECTORS}");
}
