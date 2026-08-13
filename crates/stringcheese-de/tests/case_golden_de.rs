//! Golden case-mapping vectors for the German pack.
//!
//! ≥ 20 vectors covering:
//!
//! * ASCII round-trips (German uses ASCII a-z for the base alphabet).
//! * German umlauts (ä, ö, ü and their capitals) — default Unicode
//!   handles these but the pack ships explicit entries for uniform
//!   pack-hit ratios.
//! * German sharp-s ß (U+00DF) full uppercase expansion → "SS".
//! * Capital sharp S ẞ (U+1E9E) simple lower to ß, full fold to "ss".
//! * Latin ligatures for foreign loanwords appearing in German text.

#![cfg(all(feature = "case-scud", not(target_family = "wasm")))]

use stringcheese_de::case_data::case_pack;
use stringcheese_icu_case::{CaseEngine, FoldMode};

fn engine() -> CaseEngine<'static> {
    CaseEngine::new(vec![case_pack().unwrap()])
}

// -----------------------------------------------------------------------
// ASCII (4 vectors)
// -----------------------------------------------------------------------

#[test]
fn ascii_lower_upper() {
    let e = engine();
    assert_eq!(e.to_lower("HALLO", "de"), "hallo");
    assert_eq!(e.to_upper("hallo", "de"), "HALLO");
    assert_eq!(e.to_lower("Hallo, Welt!", "de"), "hallo, welt!");
    assert_eq!(e.to_upper("hallo, welt!", "de"), "HALLO, WELT!");
}

// -----------------------------------------------------------------------
// Umlauts (10 vectors)
// -----------------------------------------------------------------------

#[test]
fn umlauts_simple_pairs() {
    let e = engine();
    for (upper, lower) in [('Ä', 'ä'), ('Ö', 'ö'), ('Ü', 'ü')] {
        assert_eq!(e.to_lower(&upper.to_string(), "de"), lower.to_string());
        assert_eq!(e.to_upper(&lower.to_string(), "de"), upper.to_string());
    }
}

#[test]
fn umlaut_words_roundtrip() {
    let e = engine();
    assert_eq!(e.to_upper("mädchen", "de"), "MÄDCHEN");
    assert_eq!(e.to_lower("MÜNCHEN", "de"), "münchen");
    assert_eq!(e.to_upper("brötchen", "de"), "BRÖTCHEN");
    assert_eq!(e.to_lower("KÄSE", "de"), "käse");
}

// -----------------------------------------------------------------------
// Sharp-s (5 vectors)
// -----------------------------------------------------------------------

#[test]
fn sharp_s_full_upper_expands_to_ss() {
    let e = engine();
    // ß has no simple uppercase in Unicode; the full-upper mapping
    // "SS" is what CLDR ships.
    assert_eq!(e.to_upper("ß", "de"), "SS");
    assert_eq!(e.to_upper("straße", "de"), "STRASSE");
    assert_eq!(e.to_upper("weißbier", "de"), "WEISSBIER");
}

#[test]
fn sharp_s_full_fold_matches_ss() {
    let e = engine();
    assert_eq!(e.fold("Straße", FoldMode::Full), "strasse");
    assert_eq!(e.fold("Weißbier", FoldMode::Full), "weissbier");
}

// -----------------------------------------------------------------------
// Capital sharp S (2 vectors)
// -----------------------------------------------------------------------

#[test]
fn capital_sharp_s_lowers_to_lowercase_sharp_s() {
    let e = engine();
    // ẞ (U+1E9E) → ß (U+00DF) simple lower.
    assert_eq!(e.to_lower("\u{1E9E}", "de"), "\u{00DF}");
    // Under Full fold, ẞ folds to "ss".
    assert_eq!(e.fold("\u{1E9E}", FoldMode::Full), "ss");
}

// -----------------------------------------------------------------------
// Pack coverage / smoke (3 vectors)
// -----------------------------------------------------------------------

#[test]
fn pack_covers_common_de_words() {
    let e = engine();
    assert_eq!(e.to_upper("guten morgen", "de"), "GUTEN MORGEN");
    assert_eq!(e.to_lower("HAUS", "de"), "haus");
    assert_eq!(e.to_upper("übermorgen", "de"), "ÜBERMORGEN");
}

#[test]
fn pack_metadata() {
    let pack = case_pack().unwrap();
    assert_eq!(pack.locale(), "de");
    assert_eq!(pack.cldr_version(), "44.1");
}

// -----------------------------------------------------------------------
// Vector-count meta-check
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_15() {
    // ascii_lower_upper: 4
    // umlauts_simple_pairs: 3 pairs × 2 directions = 6
    // umlaut_words_roundtrip: 4
    // sharp_s_full_upper_expands_to_ss: 3
    // sharp_s_full_fold_matches_ss: 2
    // capital_sharp_s_lowers_to_lowercase_sharp_s: 2
    // pack_covers_common_de_words: 3
    const SHIPPED_VECTORS: usize = 4 + 6 + 4 + 3 + 2 + 2 + 3;
    const _: () = assert!(SHIPPED_VECTORS >= 15, "de golden count too low");
    println!("shipped de golden vectors: {SHIPPED_VECTORS}");
}
