//! PHONEX-Norwegian reference input/output pairs.
//!
//! A curated set of Norwegian surnames and common words that exercise
//! every preprocessing rule (`skj → S`, `sk` before front vowel → `S`,
//! `kj → C`, `k` before front vowel → `C`, `ch → S`, silent `H`,
//! Norwegian-vowel fold `å → O` / `æ → E` / `ø → E`, diaeresis / acute
//! fold) and every duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_no::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_no::NorwegianPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Norwegian key).
const PAIRS: &[(&str, &str)] = &[
    // Hansen: silent H drops → ANSEN. A(seed,last=0), N(5), S(7), E
    //   reset, N(5) → A, 5, 7, 5 → "A575"
    ("Hansen", "A575"),
    // Olsen: O(seed,last=0), L(4), S(7), E reset, N(5) → O, 4, 7, 5
    //   → "O475"
    ("Olsen", "O475"),
    // Larsen: L(seed,last=4), A reset, R(6), S(7), E reset, N(5) →
    //   L, 6, 7, 5 → "L675"
    ("Larsen", "L675"),
    // Berg: B(seed,last=1), E reset, R(6), G(2) → B, 6, 2 → "B620"
    ("Berg", "B620"),
    // Skjære (magpie): SKJ → S, then Æ → E, R, E. → "SERE".
    //   S(seed,last=7), E reset, R(6), E reset → S, 6 → "S600"
    ("Skjære", "S600"),
    // Ski: SK before I → S. Then I. → "SI".
    //   S(seed,last=7), I reset → S → "S000"
    ("Ski", "S000"),
    // Skål (cheers): SK before Å (Å folds to O — a back vowel from
    //   PHONEX's perspective; the digraph check uses the already-
    //   folded ASCII letter O, which is NOT in the front-vowel set).
    //   So SK stays split. Å → O. → "SKOL".
    //   S(seed,last=7), K(2), O reset, L(4) → S, 2, 4 → "S240"
    ("Skål", "S240"),
    // Kjøre (drive): KJ → C, then Ø → E, R, E. → "CERE".
    //   C(seed,last=2), E reset, R(6), E reset → C, 6 → "C600"
    ("Kjøre", "C600"),
    // Kino (cinema): K before I → C, then I, N, O. → "CINO".
    //   C(seed,last=2), I reset, N(5), O reset → C, 5 → "C500"
    ("Kino", "C500"),
    // Kake (cake): K before A (back) stays K, A, K before E (front) → C,
    //   E. → "KACE". K(seed,last=2), A reset last=0, C code=2 (not dup
    //   because A reset), push, E reset → K, 2 → "K200"
    ("Kake", "K200"),
    // Chef (loan): CH → S, then E, F. → "SEF".
    //   S(seed,last=7), E reset, F(1) → S, 1 → "S100"
    ("Chef", "S100"),
    // Nilsen: N(seed,last=5), I reset, L(4), S(7), E reset, N(5) →
    //   N, 4, 7, 5 → "N475"
    ("Nilsen", "N475"),
    // Andersen: A(seed,last=0), N(5), D(3), E reset, R(6), S(7), E
    //   reset, N(5) → A, 5, 3, 6 — cap 4 → "A536"
    ("Andersen", "A536"),
    // Pedersen: P(seed,last=1), E reset, D(3), E reset, R(6), S(7),
    //   E reset, N(5) → P, 3, 6, 7 — cap 4 → "P367"
    ("Pedersen", "P367"),
    // Johansen: J(seed,last=2), O reset, H drops, A reset, N(5),
    //   S(7), E reset, N(5) → J, 5, 7, 5 → "J575"
    ("Johansen", "J575"),
    // Bjørn: B(seed,last=1), J(2), Ø → E reset, R(6), N(5) →
    //   B, 2, 6, 5 → "B265"
    ("Bjørn", "B265"),
    // Håkon: H drops → ÅKON. Å → O. → "OKON".
    //   O(seed,last=0), K(2), O reset, N(5) → O, 2, 5 → "O250"
    ("Håkon", "O250"),
    // Være (to be): V(seed,last=1), Æ → E reset, R(6), E reset →
    //   V, 6 → "V600"
    ("Være", "V600"),
    // Ørn (eagle): Ø → E as seed. E(seed,last=0), R(6), N(5) →
    //   E, 6, 5 → "E650"
    ("Ørn", "E650"),
    // Fisk (fish): F(seed,last=1), I reset, S(7), K(2) → F, 7, 2 →
    //   "F720"
    ("Fisk", "F720"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = NorwegianPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-NO({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Norwegian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 15 pairs. Verify we're above
    // that with room to spare.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn norwegian_vowel_folded_variants_produce_the_same_key() {
    // Å folds to O; Æ and Ø fold to E; so accented variants encode
    // identically to their ASCII fold.
    for (accented, ascii) in [
        ("Håkon", "Hokon"),
        ("Være", "Vere"),
        ("Ørn", "Ern"),
        ("Café", "Cafe"),
    ] {
        let a = NorwegianPhonex.encode(accented).unwrap();
        let b = NorwegianPhonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX-NO({accented:?}) != PHONEX-NO({ascii:?})");
    }
}

#[test]
fn digraph_and_merger_equivalences() {
    // SKJ collapses to sibilant S — matches SJ- pronunciation in
    // words where the same phonetic cluster is spelled `sj`.
    //   skjære → SKJ→S, ÆRE → SERE → S(7), E reset, R(6), E reset →
    //     "S600"
    //   sjære (imagined spelling variant) → S(7), J(2), ÆRE → SJERE →
    //     S seed last=7, J code=2 push, Æ→E reset, R(6) push, E reset
    //     → "S26" cap → "S260"
    //   These do NOT match — the SKJ collapse is only for SKJ, not
    //   SJ. Test the more meaningful merger: KJ merges with K-before-
    //   front-vowel (both encode as C).
    //     kjøre: C, Ø→E reset, R(6), E reset → C(seed), 6 → "C600"
    //     kere:   K before E → C, E reset, R(6), E reset → C(seed), 6
    //             → "C600"
    assert_eq!(
        NorwegianPhonex.encode("kjøre"),
        NorwegianPhonex.encode("kere"),
        "KJ-K-before-front-vowel merger failed"
    );
    // Silent H drop: `Hansen` and `Ansen` share the key.
    assert_eq!(
        NorwegianPhonex.encode("Hansen"),
        NorwegianPhonex.encode("Ansen"),
        "Silent-H fold failed"
    );
    // CH → S: `Chef` and `Sef` share the key.
    assert_eq!(
        NorwegianPhonex.encode("Chef"),
        NorwegianPhonex.encode("Sef"),
        "CH-S merger failed"
    );
    // Æ / Ø / Å folds — vowel identity check.
    assert_eq!(
        NorwegianPhonex.encode("På"),
        NorwegianPhonex.encode("Po"),
        "Å-O fold failed"
    );
}
