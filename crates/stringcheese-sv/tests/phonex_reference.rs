//! PHONEX-Swedish reference input/output pairs.
//!
//! A curated set of Swedish surnames and common words that exercise
//! every preprocessing rule (`SJ → S`, `STJ / SKJ → S`, `SCH → S`,
//! `SK` + front vowel → `S`, `TJ / KJ → C`, `K` + front vowel → `C`,
//! `CH → S`, plus the vowel folds `å → o`, `ä → e`, `ö → e`) and every
//! duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_sv::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_sv::SwedishPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Swedish key).
const PAIRS: &[(&str, &str)] = &[
    // Andersson: A(seed,last=0), N(5), D(3), E(reset), R(6), S(7),
    //   S(dup drop), O(reset), N(5) → A, 5, 3, 6 (cap 4) → "A536".
    ("Andersson", "A536"),
    // Johansson: J(seed,last=2), O(reset), H(class 0 reset),
    //   A(reset), N(5), S(7), S(dup drop), O(reset), N(5) → J, 5, 7,
    //   5 → "J575".
    ("Johansson", "J575"),
    // Karlsson: K(seed,last=2), A(reset), R(6), L(4), S(7), S(dup
    //   drop), O(reset), N(5) → K, 6, 4, 7 (cap 4) → "K647".
    ("Karlsson", "K647"),
    // Nilsson: N(seed,last=5), I(reset), L(4), S(7), S(dup drop),
    //   O(reset), N(5) → N, 4, 7, 5 → "N475".
    ("Nilsson", "N475"),
    // Eriksson: E(seed,last=0), R(6), I(reset), K(2), S(7), S(dup
    //   drop), O(reset), N(5) → E, 6, 2, 7 → "E627".
    ("Eriksson", "E627"),
    // Larsson: L(seed,last=4), A(reset), R(6), S(7), S(dup drop),
    //   O(reset), N(5) → L, 6, 7, 5 → "L675".
    ("Larsson", "L675"),
    // Olsson: O(seed,last=0), L(4), S(7), S(dup drop), O(reset),
    //   N(5) → O, 4, 7, 5 → "O475".
    ("Olsson", "O475"),
    // Persson: P(seed,last=1), E(reset), R(6), S(7), S(dup drop),
    //   O(reset), N(5) → P, 6, 7, 5 → "P675".
    ("Persson", "P675"),
    // Sjöberg: preprocess SJ → S, then Ö → E, then B, E, R, G. So
    //   "SEBERG". Seed 'S' last=7. E reset. B code=1 push → "S1"
    //   last=1. E reset. R code=6 push → "S16" last=6. G code=2 push
    //   → "S162" last=2. → "S162".
    ("Sjöberg", "S162"),
    // Sjögren: SJ + Ö + G + R + E + N → SEGREN. Seed 'S' last=7. E
    //   reset. G code=2 push → "S2" last=2. R code=6 push → "S26" last=6.
    //   E reset. N code=5 push → "S265" last=5. → "S265".
    ("Sjögren", "S265"),
    // Bergström: B(seed,last=1), E(reset), R(6), G(2), S(7), T(3),
    //   R(6), Ö→E(reset), M(5) → B, 6, 2, 7 (cap 4) → "B627".
    ("Bergström", "B627"),
    // Håkan: Å→O. Preprocess "HOKAN". H seed last=0 (class 0).
    //   O reset. K code=2 push → "H2" last=2. A reset. N code=5
    //   push → "H25" last=5. → "H25" pad → "H250".
    ("Håkan", "H250"),
    // Åke: preprocess Å → O → "OKE". Seed 'O' last=0. K code=2 push
    //   → "OK" → "O2" last=2. E reset. → "O2" pad → "O200".
    ("Åke", "O200"),
    // Öberg: preprocess Ö → E → "EBERG". Seed 'E' last=0. B code=1
    //   push → "E1" last=1. E reset. R code=6 push → "E16" last=6.
    //   G code=2 push → "E162" last=2. → "E162".
    ("Öberg", "E162"),
    // Tjänare: preprocess TJ → C, then Ä → E, then N, A, R, E:
    //   "CENARE". Seed 'C' last=2. E reset. N code=5 push → "C5"
    //   last=5. A reset. R code=6 push → "C56" last=6. E reset. →
    //   "C56" pad → "C560".
    ("Tjänare", "C560"),
    // Köpa: preprocess Ö→E, K before E (front) → C: "CEPA". Seed 'C'
    //   last=2. E reset. P code=1 push → "C1" last=1. A reset. →
    //   "C1" pad → "C100".
    ("Köpa", "C100"),
    // Skede: preprocess Ä→..., SK before E (front, after E fold no
    //   change) → S. Then E, D, E: "SEDE". Seed 'S' last=7. E reset.
    //   D code=3 push → "S3" last=3. E reset. → "S3" pad → "S300".
    ("Skede", "S300"),
    // Skola: SK before O (back) — no collapse, K encodes as class 2.
    //   Preprocess "SKOLA". Seed 'S' last=7. K code=2 push → "S2"
    //   last=2. O reset. L code=4 push → "S24" last=4. A reset. →
    //   "S24" pad → "S240".
    ("Skola", "S240"),
    // Choklad: CH → S. Preprocess "SOKLAD". Seed 'S' last=7. O reset.
    //   K code=2 push → "S2" last=2. L code=4 push → "S24" last=4.
    //   A reset. D code=3 push → "S243" last=3. → "S243".
    ("Choklad", "S243"),
    // Schmidt: SCH → S. Preprocess "SMIDT". Seed 'S' last=7. M code=5
    //   push → "S5" last=5. I reset. D code=3 push → "S53" last=3.
    //   T code=3 dup drop. → "S53" pad → "S530".
    ("Schmidt", "S530"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = SwedishPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-SV({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Swedish reference pair(s) disagreed:\n{}",
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
fn vowel_accent_folded_variants_produce_the_same_key() {
    // Accented vowels fold to their base letter and therefore encode
    // identically to the equivalent unaccented spelling.
    for (accented, ascii) in [
        ("Håkan", "Hokan"),
        ("Öberg", "Eberg"),
        ("Bergström", "Bergstrem"),
    ] {
        let a = SwedishPhonex.encode(accented).unwrap();
        let b = SwedishPhonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX-SV({accented:?}) != PHONEX-SV({ascii:?})");
    }
}

#[test]
fn digraph_and_merger_equivalences() {
    let encode = |w: &str| SwedishPhonex.encode(w).unwrap();

    // SJ, STJ, SKJ all collapse to `S` — the sj-sound.
    assert_eq!(encode("sjuk"), encode("stjuk"), "SJ / STJ collapse failed");
    assert_eq!(encode("sjuk"), encode("skjuk"), "SJ / SKJ collapse failed");

    // TJ / KJ both encode as `C` — the tj-sound.
    assert_eq!(encode("tjuv"), encode("kjuv"), "TJ / KJ collapse failed");

    // K before a front vowel encodes as C.
    assert_eq!(
        encode("köpa"),
        encode("cöpa"),
        "K-front-vowel / C collapse failed"
    );

    // CH → S. `choklad` and `sjoklad` (contrived) share a phonetic
    // prefix.
    assert_eq!(
        encode("choklad"),
        encode("sjoklad"),
        "CH / SJ collapse failed"
    );
}
