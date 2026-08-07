//! PHONEX-Dutch reference input/output pairs.
//!
//! A curated set of Dutch surnames and common words that exercise
//! every preprocessing rule (`IJ → I`, `SCH → SX`, `CH → G`, `RR → R`,
//! silent `H`, diaeresis / acute fold) and every duplicate-collapse
//! edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_nl::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_nl::DutchPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Dutch key).
const PAIRS: &[(&str, &str)] = &[
    // Jansen: J(seed,last=2), A(reset), N(5), S(7), E(reset), N(5)
    //   → J, 5, 7, 5 → "J575"
    ("Jansen", "J575"),
    // Bakker: B(seed,last=1), A(reset), K(2), K(dup drop), E(reset),
    //   R(6) → B, 2, 6 → "B260"
    ("Bakker", "B260"),
    // Visser: V(seed,last=1), I(reset), S(7), S(dup drop), E(reset),
    //   R(6) → V, 7, 6 → "V760"
    ("Visser", "V760"),
    // Vries: V(seed,last=1), R(6), I(reset), E(reset), S(7) → V, 6, 7
    //   → "V670"
    ("Vries", "V670"),
    // Hendrik: silent H drops → ENDRIK. E(seed,last=0), N(5), D(3),
    //   R(6), I(reset), K(2) → E, 5, 3, 6 — cap 4 → "E536"
    ("Hendrik", "E536"),
    // Schmidt: preprocess SCH → SX → "SXMIDT". S(seed,last=7), X(2),
    //   M(5), I(reset), D(3), T(dup? D was 3, T is 3, dup drop)
    //   → "S253" — wait, let me re-trace: S seed last=7. X code=2 push
    //   "S2" last=2. M code=5 push "S25" last=5. I reset last=0. D code=3
    //   push "S253" last=3. T code=3 dup drop. → "S253" pad → "S253".
    ("Schmidt", "S253"),
    // Schilder (painter): preprocess SCH → SX → "SXILDER". S seed last=7.
    //   X code=2 push "S2" last=2. I reset. L code=4 push "S24" last=4.
    //   D code=3 push "S243" last=3. E reset. R code=6 push cap 4 →
    //   "S243". Wait we already have 4 chars. Stop after "S243". → "S243"
    ("Schilder", "S243"),
    // Licht: L(seed,last=4), I(reset), CH→G(2), T(3) → L, 2, 3 → "L230"
    ("Licht", "L230"),
    // Nacht: N(seed,last=5), A(reset), CH→G(2), T(3) → N, 2, 3 → "N230"
    ("Nacht", "N230"),
    // Rijk: R(seed,last=6), IJ→I(reset), K(2) → R, 2 → "R200"
    ("Rijk", "R200"),
    // Mijn: M(seed,last=5), IJ→I(reset), N(5,push,last=5 since last was
    //   reset to 0) → "M5" → "M500"
    ("Mijn", "M500"),
    // Mulder: M(seed,last=5), U(reset), L(4), D(3), E(reset), R(6) → M,
    //   4, 3, 6 → "M436"
    ("Mulder", "M436"),
    // Peters: P(seed,last=1), E(reset), T(3), E(reset), R(6), S(7) → P,
    //   3, 6, 7 → "P367"
    ("Peters", "P367"),
    // Groot: G(seed,last=2), R(6), O(reset), O(reset), T(3) → G, 6, 3
    //   → "G630"
    ("Groot", "G630"),
    // Been (bone): B(seed,last=1), E(reset), E(reset), N(5) → B, 5
    //   → "B500"
    ("Been", "B500"),
    // Één (with diacritic): preprocess Één → EEN → E(seed,last=0),
    //   E(reset), N(5) → E, 5 → "E500"
    ("Één", "E500"),
    // Café: preprocess CAFÉ → CAFE. C(seed,last=2), A(reset), F(1),
    //   E(reset) → C, 1 → "C100"
    ("Café", "C100"),
    // Coöperatie: preprocess Coöperatie → COOPERATIE. C(seed,last=2),
    //   O(reset), O(reset), P(1), E(reset), R(6), A(reset), T(3),
    //   I(reset), E(reset) → C, 1, 6, 3 → "C163"
    ("Coöperatie", "C163"),
    // Kerkhof: K(seed,last=2), E(reset), R(6), K(2), H(drop), O(reset),
    //   F(1) → K, 6, 2, 1 → "K621"
    ("Kerkhof", "K621"),
    // Zwart: Z(seed,last=7), W(1), A(reset), R(6), T(3) → Z, 1, 6, 3
    //   → "Z163"
    ("Zwart", "Z163"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = DutchPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-NL({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Dutch reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 10 pairs. Verify we're above
    // that with room to spare.
    assert!(
        PAIRS.len() >= 10,
        "reference pair count {} is below the 10-pair floor",
        PAIRS.len()
    );
}

#[test]
fn vowel_accent_folded_variants_produce_the_same_key() {
    // Accented vowels fold to their base letter and therefore encode
    // identically to the unaccented spelling.
    for (accented, ascii) in [
        ("Één", "Een"),
        ("Café", "Cafe"),
        ("Coöperatie", "Cooperatie"),
        ("Idëën", "Ideen"),
    ] {
        let a = DutchPhonex.encode(accented).unwrap();
        let b = DutchPhonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX-NL({accented:?}) != PHONEX-NL({ascii:?})");
    }
}

#[test]
fn digraph_and_merger_equivalences() {
    // IJ → I collapse.
    assert_eq!(
        DutchPhonex.encode("mijn"),
        DutchPhonex.encode("min"),
        "IJ-I merger failed"
    );
    // CH → G equivalence: `licht` shares consonant skeleton with `ligt`.
    //   licht: L, I, CH→G, T → "L230".
    //   ligt: L, I, G, T → "L230" (I reset, G code=2, T code=3).
    assert_eq!(
        DutchPhonex.encode("licht"),
        DutchPhonex.encode("ligt"),
        "CH-G merger failed"
    );
    // Silent H drop: `Hendrik` and `Endrik` share the key.
    assert_eq!(
        DutchPhonex.encode("Hendrik"),
        DutchPhonex.encode("Endrik"),
        "Silent-H fold failed"
    );
    // RR → R idempotence (via digraph pass).
    assert_eq!(
        DutchPhonex.encode("Karren"),
        DutchPhonex.encode("Karen"),
        "RR-R merger failed"
    );
}
