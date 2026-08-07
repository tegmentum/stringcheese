//! PHONEX-Spanish reference input/output pairs.
//!
//! Every Spanish surname listed in the task spec (García, Martínez,
//! López, Sánchez, Rodríguez, González, Pérez, Fernández, Torres,
//! Ramírez) plus a handful of additional surnames that exercise every
//! preprocessing rule and every duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_es::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_es::SpanishPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Spanish key).
const PAIRS: &[(&str, &str)] = &[
    // Task-required surnames — hand-traced against the module-level
    // algorithm.
    //
    // García: G, A(0), R(6), C(2), I(0), A(0) → G, 6, 2 → G620
    ("García", "G620"),
    // Martínez: M, A(0), R(6), T(3), I(0), N(5), E(0), Z→S(7) → M635 (cap)
    ("Martínez", "M635"),
    // López: L, O(0), P(1), E(0), Z→S(7) → L170
    ("López", "L170"),
    // Sánchez: S, A(0), N(5), CH→X(7), E(0), Z→S(dup 7 dropped after vowel? — Z→S is 7, same as prev)
    //   Wait let me re-trace: preprocess "SÁNCHEZ": Á→A, CH→X, Z→S
    //   → "SANXES". Encode: S(seed, last=7), A(0/reset), N(5), X(7), E(0/reset), S(7)
    //   → S, 5, 7, 7 — but last two 7s... after X(7) we set last=7. Then E resets last=0.
    //     Then S(7) != 0, push. So S, 5, 7, 7 → "S577"
    ("Sánchez", "S577"),
    // Rodríguez: preprocess "RODRÍGUEZ" → "RODRIGUES" (Í→I, Z→S)
    //   Encode: R(seed, last=6), O(0/reset), D(3), R(6), I(0/reset), G(2), U(0/reset), E(0), S(7)
    //   → R, 3, 6, 2, 7 — cap at 4 → "R362"
    ("Rodríguez", "R362"),
    // González: preprocess "GONZÁLEZ" → "GONSALES" (Z→S ×2, Á→A)
    //   Encode: G(seed, last=2), O(0/reset), N(5), S(7), A(0/reset), L(4), E(0/reset), S(7)
    //   → G, 5, 7, 4, 7 — cap → "G574"
    ("González", "G574"),
    // Pérez: preprocess "PÉREZ" → "PERES" (É→E, Z→S)
    //   Encode: P(seed, last=1), E(0/reset), R(6), E(0/reset), S(7)
    //   → P, 6, 7 → "P670"
    ("Pérez", "P670"),
    // Fernández: preprocess "FERNÁNDEZ" → "FERNANDES" (Á→A, Z→S)
    //   Encode: F(seed, last=1), E(0/reset), R(6), N(5), A(0/reset), N(5), D(3), E(0/reset), S(7)
    //   → F, 6, 5, 5, 3, 7 — dupes: N(5) then A(reset), then N(5) is not dup (last was reset).
    //     Actually: F(seed,last=1), E(reset,last=0), R(6,push,last=6), N(5,push,last=5),
    //     A(reset,last=0), N(5,push,last=5), D(3,push,last=3), E(reset,last=0), S(7,push,last=7)
    //     → F 6 5 5 3 7 — cap 3 digits → "F655"
    ("Fernández", "F655"),
    // Torres: preprocess "TORRES" → RR→R → "TORES"
    //   Encode: T(seed,last=3), O(reset), R(6), E(reset), S(7) → T, 6, 7 → "T670"
    ("Torres", "T670"),
    // Ramírez: preprocess "RAMÍREZ" → "RAMIRES" (Í→I, Z→S)
    //   Encode: R(seed,last=6), A(reset), M(5), I(reset), R(6), E(reset), S(7)
    //   → R, 5, 6, 7 → "R567"
    ("Ramírez", "R567"),
    // Additional surnames exercising digraph substitutions.
    //
    // Villa: V→B, I(reset), LL→L, A(reset) → B, 4 → "B400"
    ("Villa", "B400"),
    // Muñoz: Ñ→N, Z→S → "MUNOS"
    //   Encode: M(seed,last=5), U(reset), N(5), O(reset), S(7) → M, 5, 7 → "M570"
    ("Muñoz", "M570"),
    // Chávez: CH→X, Á→A, V→B, Z→S → "XABES"
    //   Encode: X(seed,last=7), A(reset), B(1), E(reset), S(7) → X, 1, 7 → "X170"
    ("Chávez", "X170"),
    // Ortiz: Z→S → "ORTIS"
    //   Encode: O(seed,last=0), R(6), T(3), I(reset), S(7) → O, 6, 3, 7 → "O637"
    ("Ortiz", "O637"),
    // Herrera: H drop → "ERRERA" → RR→R → "ERERA"
    //   Encode: E(seed,last=0), R(6), E(reset), R(6), A(reset) → E, 6, 6 — dup?
    //     After E(reset), last=0. R(6): push. last=6. E(reset), last=0. R(6): push, last=6. A(reset).
    //     → E, 6, 6 → "E660"
    ("Herrera", "E660"),
    // Aguilar: A(seed), G(2), U(reset), I(reset), L(4), A(reset), R(6) → A, 2, 4, 6 → "A246"
    ("Aguilar", "A246"),
    // Jiménez: J(seed,last=2), I(reset), M(5), É→E(reset), N(5), E(reset), Z→S(7)
    //   → J, 5, 5, 7 — after M(5)/E(reset)/N(5): the second N is 5 but reset in between → both pushed.
    //   Actually: J(seed,last=2), I(reset,last=0), M(5,push,last=5), E(reset,last=0),
    //     N(5,push,last=5), E(reset,last=0), Z→S(7,push,last=7) → "J557"
    ("Jiménez", "J557"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = SpanishPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-ES({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Spanish reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 15 pairs. Verify we're above
    // that.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn vowel_accent_folded_variants_produce_the_same_key() {
    // Accented vowels fold to their base letter and therefore encode
    // identically to the unaccented spelling.
    for (accented, ascii) in [
        ("García", "Garcia"),
        ("Martínez", "Martinez"),
        ("López", "Lopez"),
        ("Sánchez", "Sanchez"),
        ("Pérez", "Perez"),
        ("Rodríguez", "Rodriguez"),
        ("Ramírez", "Ramirez"),
    ] {
        let a = SpanishPhonex.encode(accented).unwrap();
        let b = SpanishPhonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX-ES({accented:?}) != PHONEX-ES({ascii:?})");
    }
}

#[test]
fn v_b_z_s_mergers_produce_the_same_key() {
    // Betacismo: `v` and `b` are the same phoneme in Spanish.
    assert_eq!(
        SpanishPhonex.encode("Vera"),
        SpanishPhonex.encode("Bera"),
        "V-B merger failed"
    );
    // Seseo: `z` and `s` collapse.
    assert_eq!(
        SpanishPhonex.encode("Zapata"),
        SpanishPhonex.encode("Sapata"),
        "Z-S merger failed"
    );
}
