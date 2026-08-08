//! PHONEX-Romanian reference input/output pairs.
//!
//! Common Romanian surnames (`Popescu`, `Ionescu`, `Constantinescu`,
//! `Radu`, `Dumitrescu`, …) plus a handful of additional entries that
//! exercise every preprocessing rule (`ch → k` before front vowel,
//! `gh → g` before front vowel, `ph → f`, cedilla folding, silent
//! intervocalic `h`) and every duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_ro::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_ro::RomanianPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Romanian key).
const PAIRS: &[(&str, &str)] = &[
    // Task-required Romanian surnames — traced by hand.
    //
    // Popescu: P O P E S K U → P(seed,last=1), O(reset), P(1),
    //   E(reset), S(7), K(2), U(reset) → P, 1, 7, 2 → "P172"
    ("Popescu", "P172"),
    // Ionescu: I O N E S K U → I(seed,last=0), O(reset), N(5),
    //   E(reset), S(7), K(2), U(reset) → I, 5, 7, 2 → "I572"
    ("Ionescu", "I572"),
    // Radu: R A D U → R(seed,last=6), A(reset), D(3), U(reset)
    //   → R, 3 → "R300"
    ("Radu", "R300"),
    // Vlad: V L A D → V(seed,last=1), L(4), A(reset), D(3)
    //   → V, 4, 3 → "V430"
    ("Vlad", "V430"),
    // Marin: M A R I N → M(seed,last=5), A(reset), R(6), I(reset),
    //   N(5) → M, 6, 5 → "M650"
    ("Marin", "M650"),
    // Preprocessing: cedilla → comma-below fold. Both spellings
    // must produce the same key.
    // Ţară (cedilla ţ) vs. Țară (comma-below ț) → both fold to T
    //   → T A R A → T(seed,last=3), A(reset), R(6), A(reset)
    //   → T, 6 → "T600"
    ("țară", "T600"),
    ("ţară", "T600"),
    // Diacritic folding: brânză.
    //   Preprocess: B R A N Z A → B(seed,last=1), R(6), A(reset),
    //   N(5), Z(7), A(reset) → B, 6, 5, 7 → "B657"
    ("brânză", "B657"),
    // CH before front vowel → K.
    //   "chibrit" — CH+I → K, then IBRIT. Preprocess: K I B R I T
    //   → K(seed,last=2), I(reset), B(1), R(6), I(reset), T(3)
    //   → K, 1, 6, 3 → "K163"
    ("chibrit", "K163"),
    // GH before front vowel → G.
    //   "ghid" — GH+I → G, then ID. Preprocess: G I D
    //   → G(seed,last=2), I(reset), D(3) → "G300"
    ("ghid", "G300"),
    // PH → F. Loan-word rule.
    //   "Philip" — PH → F, then ILIP. Preprocess: F I L I P
    //   → F(seed,last=1), I(reset), L(4), I(reset), P(1)
    //   → F, 4, 1 → "F410"
    ("Philip", "F410"),
    // Silent intervocalic H.
    //   "Mihai" — H is intervocalic, dropped. Preprocess: M I A I
    //   → M(seed,last=5), I(reset), A(reset), I(reset) → "M000"
    ("Mihai", "M000"),
    // Word-initial H kept as seed.
    //   "Horia" — Preprocess: H O R I A
    //   → H(seed,last=0), O(reset), R(6), I(reset), A(reset)
    //   → H, 6 → "H600"
    ("Horia", "H600"),
    // Long consonant run.
    //   "Constantinescu" — C O N S T A N T I N E S K U
    //     → C(seed,last=2), O(reset), N(5), S(7), T(3), A(reset),
    //       N(5), T(3), I(reset), N(5), E(reset), S(7), K(2), U(reset)
    //     → C, 5, 7, 3 → "C573"
    ("Constantinescu", "C573"),
    // Dumitrescu.
    //   D U M I T R E S K U → D(seed,last=3), U(reset), M(5),
    //   I(reset), T(3), R(6), E(reset), S(7), K(2), U(reset)
    //   → D, 5, 3, 6 → "D536"
    ("Dumitrescu", "D536"),
    // Duplicate consonants collapse.
    //   "Anna" → A N N A → A(seed,last=0), N(5), N(dup), A(reset)
    //   → A, 5 → "A500"
    ("Anna", "A500"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = RomanianPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-RO({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Romanian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn diacritic_folded_variants_produce_the_same_key() {
    // Romanian diacritics fold to their base letter and therefore
    // encode identically to the un-accented spelling (except that
    // `ș`/`ț` fold to `S`/`T` — different classes from a plain
    // vowel).
    for (accented, plain) in [
        ("brânză", "branza"),
        ("mămăligă", "mamaliga"),
        ("învățător", "invatator"),
    ] {
        let a = RomanianPhonex.encode(accented).unwrap();
        let b = RomanianPhonex.encode(plain).unwrap();
        assert_eq!(a, b, "PHONEX-RO({accented:?}) != PHONEX-RO({plain:?})");
    }
}

#[test]
fn cedilla_and_comma_below_produce_the_same_key() {
    // The signature Romanian fold — `ş`/`ţ` (legacy cedilla) and
    // `ș`/`ț` (modern comma-below) both fold to `S`/`T` at
    // preprocessing and therefore encode identically.
    for (cedilla, comma_below) in [("ţară", "țară"), ("eşti", "ești"), ("Grigoraş", "Grigoraș")]
    {
        let a = RomanianPhonex.encode(cedilla).unwrap();
        let b = RomanianPhonex.encode(comma_below).unwrap();
        assert_eq!(
            a, b,
            "PHONEX-RO({cedilla:?}) = {a:?} != PHONEX-RO({comma_below:?}) = {b:?}"
        );
    }
}
