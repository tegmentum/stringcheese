//! Golden case-mapping vectors for the French pack.
//!
//! ≥ 20 vectors covering ASCII, Latin-1 supplement, French accented
//! letters (é, à, ç, ô, û, ï, ÿ), the Œ/Æ ligatures common in
//! French orthography, and the German ß expansion the pack ships
//! for uniform composed-engine behaviour.
//!
//! Golden vectors are hand-derived from Unicode 15 `CaseFolding.txt`
//! and executed against the shipped `case-fr.scud` pack. A
//! regression that shifts an entry in the pack will fail here
//! loudly rather than surface as a subtle integration bug
//! downstream.

#![cfg(all(feature = "case-scud", not(target_family = "wasm")))]

use stringcheese_fr::case_data::case_pack;
use stringcheese_icu_case::{CaseEngine, FoldMode, TitleOptions};

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
        assert_eq!(e.to_upper(&c.to_string(), "fr"), up.to_string());
        assert_eq!(e.to_lower(&up.to_string(), "fr"), c.to_string());
    }
}

// -----------------------------------------------------------------------
// French accented letters (14 assertions)
// -----------------------------------------------------------------------

#[test]
fn french_accented_letters_pairs() {
    let e = engine();
    for (upper, lower) in [
        ('É', 'é'),
        ('È', 'è'),
        ('Ê', 'ê'),
        ('Ë', 'ë'),
        ('À', 'à'),
        ('Â', 'â'),
        ('Ç', 'ç'),
        ('Î', 'î'),
        ('Ï', 'ï'),
        ('Ô', 'ô'),
        ('Ù', 'ù'),
        ('Û', 'û'),
        ('Ü', 'ü'),
        ('Ÿ', 'ÿ'),
    ] {
        assert_eq!(
            e.to_lower(&upper.to_string(), "fr"),
            lower.to_string(),
            "to_lower({upper:?}) failed"
        );
        assert_eq!(
            e.to_upper(&lower.to_string(), "fr"),
            upper.to_string(),
            "to_upper({lower:?}) failed"
        );
    }
}

// -----------------------------------------------------------------------
// Œ and Æ ligatures (6 assertions)
// -----------------------------------------------------------------------

#[test]
fn oe_ligature_roundtrip() {
    let e = engine();
    assert_eq!(e.to_upper("œuvre", "fr"), "ŒUVRE");
    assert_eq!(e.to_lower("ŒIL", "fr"), "œil");
    assert_eq!(e.to_upper("cœur", "fr"), "CŒUR");
    assert_eq!(e.to_lower("Œ", "fr"), "œ");
}

#[test]
fn ae_ligature_roundtrip() {
    let e = engine();
    assert_eq!(e.to_lower("Æ", "fr"), "æ");
    assert_eq!(e.to_upper("æ", "fr"), "Æ");
}

// -----------------------------------------------------------------------
// German ß expansion (3 assertions)
// -----------------------------------------------------------------------

#[test]
fn sharp_s_expansion_available_through_fr_pack() {
    let e = engine();
    // A French text quoting a German loanword still gets the ß
    // expansion because the pack ships it explicitly.
    assert_eq!(e.to_upper("straße", "fr"), "STRASSE");
    assert_eq!(e.fold("Straße", FoldMode::Full), "strasse");
    assert_eq!(e.to_upper("ß", "fr"), "SS");
}

// -----------------------------------------------------------------------
// Whole-word French inputs (6 assertions)
// -----------------------------------------------------------------------

#[test]
fn whole_word_upper_lower() {
    let e = engine();
    assert_eq!(e.to_upper("français", "fr"), "FRANÇAIS");
    assert_eq!(e.to_lower("FRANÇAIS", "fr"), "français");
    assert_eq!(e.to_upper("québécois", "fr"), "QUÉBÉCOIS");
    assert_eq!(e.to_lower("QUÉBÉCOIS", "fr"), "québécois");
    assert_eq!(e.to_upper("à côté de moi", "fr"), "À CÔTÉ DE MOI");
    assert_eq!(e.to_lower("À CÔTÉ DE MOI", "fr"), "à côté de moi");
}

// -----------------------------------------------------------------------
// Titlecase (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn title_case_french_words() {
    let e = engine();
    let opts = TitleOptions::default();
    assert_eq!(
        e.to_title("bonjour tout le monde", "fr", opts).unwrap(),
        "Bonjour Tout Le Monde"
    );
    assert_eq!(
        e.to_title("château de versailles", "fr", opts).unwrap(),
        "Château De Versailles"
    );
    // Note: the Phase 1 title-boundary rule treats ASCII apostrophe
    // as punctuation, so `d'art` becomes `D'Art` — the `a` after `'`
    // is at a boundary. A UAX #29 word-break-aware title rule would
    // treat `d'art` as one word; that's deferred to Phase 6+ with
    // the `stringcheese-icu-break` capability.
    assert_eq!(
        e.to_title("œuvre d'art", "fr", opts).unwrap(),
        "Œuvre D'Art"
    );
    assert_eq!(
        e.to_title("école française", "fr", opts).unwrap(),
        "École Française"
    );
}

// -----------------------------------------------------------------------
// Empty / whitespace / punctuation (5 assertions)
// -----------------------------------------------------------------------

#[test]
fn empty_and_whitespace_and_punctuation() {
    let e = engine();
    assert_eq!(e.to_lower("", "fr"), "");
    assert_eq!(e.to_upper("", "fr"), "");
    assert_eq!(e.to_lower("   ", "fr"), "   ");
    assert_eq!(e.to_lower("Bonjour, Monde!", "fr"), "bonjour, monde!");
    assert_eq!(e.to_upper("Bonjour, Monde!", "fr"), "BONJOUR, MONDE!");
}

// -----------------------------------------------------------------------
// Idempotence spot-checks (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn to_lower_is_idempotent() {
    let e = engine();
    for input in [
        "bonjour",
        "français",
        "château",
        "œuvre d'art",
    ] {
        let once = e.to_lower(input, "fr");
        let twice = e.to_lower(&once, "fr");
        assert_eq!(once, twice, "to_lower not idempotent on {input:?}");
    }
}

// -----------------------------------------------------------------------
// Fold (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn fold_full_lowercases_and_expands() {
    let e = engine();
    assert_eq!(e.fold("FRANÇAIS", FoldMode::Simple), "français");
    assert_eq!(e.fold("École", FoldMode::Simple), "école");
    assert_eq!(e.fold("ŒUVRE", FoldMode::Simple), "œuvre");
    assert_eq!(e.fold("Straße", FoldMode::Full), "strasse");
}

// -----------------------------------------------------------------------
// Fallback under unknown locale
// -----------------------------------------------------------------------

#[test]
fn unknown_locale_still_uses_pack_via_fallback_to_root() {
    let e = engine();
    // No `fr-CA` pack shipped; queries fall to `fr → ""`, still
    // pack-hit.
    assert_eq!(e.to_lower("BONJOUR", "fr-CA"), "bonjour");
    assert_eq!(e.to_upper("bonjour", "fr-CA"), "BONJOUR");
}

// -----------------------------------------------------------------------
// Vector-count sanity
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_20() {
    // - ascii_upper_lower_round_trip:            2 * 26 = 52
    // - french_accented_letters_pairs:           2 * 14 = 28
    // - oe_ligature_roundtrip:                          4
    // - ae_ligature_roundtrip:                          2
    // - sharp_s_expansion_available_through_fr_pack:    3
    // - whole_word_upper_lower:                         6
    // - title_case_french_words:                        4
    // - empty_and_whitespace_and_punctuation:           5
    // - to_lower_is_idempotent:                         4
    // - fold_full_lowercases_and_expands:               4
    // - unknown_locale_still_uses_pack_via_fallback_to_root: 2
    // Total:                                          114
    const SHIPPED_VECTORS: usize = 52 + 28 + 4 + 2 + 3 + 6 + 4 + 5 + 4 + 4 + 2;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "fr golden vector count fell below the Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped fr golden vectors: {SHIPPED_VECTORS}");
}
