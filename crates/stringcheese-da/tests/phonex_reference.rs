//! PHONEX-Danish reference input/output pairs.
//!
//! A curated set of Danish surnames and common words that exercise
//! every preprocessing rule (`sj → S`, `sk` before front vowel → `S`,
//! `ch → S`, silent `H`, Danish-vowel fold `å → O` / `æ → E` / `ø → E`,
//! diaeresis / acute fold) and every duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_da::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_da::DanishPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Danish key).
const PAIRS: &[(&str, &str)] = &[
    // Hansen: silent H drops → ANSEN. A(seed,last=0), N(5), S(7), E
    //   reset, N(5) → "A575"
    ("Hansen", "A575"),
    // Jensen: J(seed,last=2), E reset, N(5), S(7), E reset, N(5) →
    //   "J575"
    ("Jensen", "J575"),
    // Nielsen: N(seed,last=5), I reset, E reset, L(4), S(7), E reset,
    //   N(5) → "N475"
    ("Nielsen", "N475"),
    // Larsen: L(seed,last=4), A reset, R(6), S(7), E reset, N(5) →
    //   "L675"
    ("Larsen", "L675"),
    // Andersen: A(seed,last=0), N(5), D(3), E reset, R(6), S(7), E
    //   reset, N(5) — capped to 4 → "A536"
    ("Andersen", "A536"),
    // Pedersen: P(seed,last=1), E reset, D(3), E reset, R(6), S(7), E
    //   reset, N(5) — capped → "P367"
    ("Pedersen", "P367"),
    // Christensen: CH → S, then RISTENSEN. S(seed,last=7), R(6), I
    //   reset, S(7), T(3), E reset, N(5), S(7) — capped → "S673"
    ("Christensen", "S673"),
    // Berg: B(seed,last=1), E reset, R(6), G(2) → "B620"
    ("Berg", "B620"),
    // Sjæl (soul): SJ → S, then Æ → E, L. → "SEL".
    //   S(seed,last=7), E reset, L(4) → "S400"
    ("Sjæl", "S400"),
    // Ski: SK before I → S. Then I. → "SI".
    //   S(seed,last=7), I reset → "S000"
    ("Ski", "S000"),
    // Skål (cheers): SK before Å (Å folds to O — a back vowel from
    //   PHONEX's perspective; the digraph check uses the already-
    //   folded ASCII letter O, which is NOT in the front-vowel set).
    //   So SK stays split. Å → O. → "SKOL".
    //   S(seed,last=7), K(2), O reset, L(4) → "S240"
    ("Skål", "S240"),
    // Chef (loan): CH → S, then E, F. → "SEF".
    //   S(seed,last=7), E reset, F(1) → "S100"
    ("Chef", "S100"),
    // Bjørn: B(seed,last=1), J(2), Ø → E reset, R(6), N(5) → "B265"
    ("Bjørn", "B265"),
    // Håkon: H drops → ÅKON. Å → O. → "OKON".
    //   O(seed,last=0), K(2), O reset, N(5) → "O250"
    ("Håkon", "O250"),
    // Være (to be): V(seed,last=1), Æ → E reset, R(6), E reset →
    //   "V600"
    ("Være", "V600"),
    // Ørn (eagle): Ø → E as seed. E(seed,last=0), R(6), N(5) → "E650"
    ("Ørn", "E650"),
    // Fisk (fish): F(seed,last=1), I reset, S(7), K(2) → "F720"
    ("Fisk", "F720"),
    // Olsen: O(seed,last=0), L(4), S(7), E reset, N(5) → "O475"
    ("Olsen", "O475"),
    // Møller: M(seed,last=5), Ø → E reset, L(4), L dup drop, E reset,
    //   R(6) → "M46" pad → "M460"
    ("Møller", "M460"),
    // Rasmussen: R(seed,last=6), A reset, S(7), M(5), U reset, S(7),
    //   S dup, E reset, N(5) — capped → "R757"
    ("Rasmussen", "R757"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = DanishPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-DA({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Danish reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // Verify we ship at least 15 reference pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn danish_vowel_folded_variants_produce_the_same_key() {
    // Å folds to O; Æ and Ø fold to E; so accented variants encode
    // identically to their ASCII fold.
    for (accented, ascii) in [
        ("Håkon", "Hokon"),
        ("Være", "Vere"),
        ("Ørn", "Ern"),
        ("Café", "Cafe"),
    ] {
        let a = DanishPhonex.encode(accented).unwrap();
        let b = DanishPhonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX-DA({accented:?}) != PHONEX-DA({ascii:?})");
    }
}

#[test]
fn digraph_and_merger_equivalences() {
    // Silent H drop: `Hansen` and `Ansen` share the key.
    assert_eq!(
        DanishPhonex.encode("Hansen"),
        DanishPhonex.encode("Ansen"),
        "Silent-H fold failed"
    );
    // CH → S: `Chef` and `Sef` share the key.
    assert_eq!(
        DanishPhonex.encode("Chef"),
        DanishPhonex.encode("Sef"),
        "CH-S merger failed"
    );
    // Æ / Ø / Å folds — vowel identity check.
    assert_eq!(
        DanishPhonex.encode("På"),
        DanishPhonex.encode("Po"),
        "Å-O fold failed"
    );
}
