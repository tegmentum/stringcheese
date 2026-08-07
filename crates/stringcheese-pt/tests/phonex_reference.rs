//! PHONEX-Portuguese reference input/output pairs.
//!
//! A curated set of Portuguese surnames and common words that exercise
//! every preprocessing rule (LH → L, NH → N, CH → X, QU → K, RR → R,
//! Ç → S, H silent, V → B, Z → S, accent folding) and every
//! duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_pt::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_pt::PortuguesePhonex;

/// Reference pairs (input, expected 4-char PHONEX-Portuguese key).
const PAIRS: &[(&str, &str)] = &[
    // Silva: S(seed,last=7), I(reset), L(4), B(1), A(reset) → S, 4, 1 → "S410"
    ("Silva", "S410"),
    // Santos: S(seed,last=7), A(reset), N(5), T(3), O(reset), S(7) → S, 5, 3, 7 → "S537"
    ("Santos", "S537"),
    // Oliveira: O(seed,last=0), L(4), I(reset), B(1) (V→B), E(reset), I(reset), R(6), A(reset)
    //   → O, 4, 1, 6 → "O416"
    ("Oliveira", "O416"),
    // Souza: preprocess "SOUSA" (Z→S) → S(seed,last=7), O(reset), U(reset), S(7,push,last=7),
    //   A(reset) → S, 7 — but S is the seed which was 7, and the next 7 is a dup... let me
    //   re-trace: S(seed,last=7), O(0/reset,last=0), U(0/reset,last=0), S(7,push,last=7),
    //   A(0/reset,last=0) → out = "S7" → pad → "S700"
    ("Souza", "S700"),
    // Rodrigues: R(seed,last=6), O(reset), D(3), R(6), I(reset), G(2), U(reset), E(reset),
    //   S(7) → R, 3, 6, 2, 7 — cap 4 → "R362"
    ("Rodrigues", "R362"),
    // Pereira: P(seed,last=1), E(reset), R(6), E(reset), I(reset), R(6), A(reset) → P, 6, 6
    //   — after R(6) last=6, then E(reset) resets last=0, then R(6) is not dup (last was 0), push.
    //   → P, 6, 6 → "P660"
    ("Pereira", "P660"),
    // Almeida: A(seed,last=0), L(4), M(5), E(reset), I(reset), D(3), A(reset) → A, 4, 5, 3 → "A453"
    ("Almeida", "A453"),
    // Costa: C(seed,last=2), O(reset), S(7), T(3), A(reset) → C, 7, 3 → "C730"
    ("Costa", "C730"),
    // Ferreira: preprocess RR→R → "FEREIRA". F(seed,last=1), E(reset), R(6), E(reset), I(reset),
    //   R(6), A(reset). After R(6) last=6, E(reset), R(6) — not dup → push. → F, 6, 6 → "F660"
    ("Ferreira", "F660"),
    // Carvalho: preprocess LH→L → "CARBALO" (V→B). C(seed,last=2), A(reset), R(6), B(1),
    //   A(reset), L(4), O(reset) → C, 6, 1, 4 → "C614"
    ("Carvalho", "C614"),
    // Coelho: preprocess LH→L → "COELO". C(seed,last=2), O(reset), E(reset), L(4), O(reset)
    //   → C, 4 → "C400"
    ("Coelho", "C400"),
    // Chaves: preprocess CH→X, V→B → "XABES". X(seed,last=7), A(reset), B(1), E(reset), S(7)
    //   → X, 1, 7 → "X170"
    ("Chaves", "X170"),
    // Queiroz: preprocess QU→K, Z→S → "KEIROS". K(seed,last=2), E(reset), I(reset), R(6),
    //   O(reset), S(7) → K, 6, 7 → "K670"
    ("Queiroz", "K670"),
    // Nogueira: preprocess QU→K (NO ‘q’... it's just QU inside — "NOKEIRA"). Wait —
    //   "NOGUEIRA" has "GU" not "QU". "GU" isn't a digraph substitution; it stays.
    //   Preprocess: "NOGUEIRA". N(seed,last=5), O(reset), G(2), U(reset), E(reset), I(reset),
    //   R(6), A(reset) → N, 2, 6 → "N260"
    ("Nogueira", "N260"),
    // João: preprocess Ã→A → "JOAO". J(seed,last=2), O(reset), A(reset), O(reset) → J → "J000"
    ("João", "J000"),
    // Cação (silly test word using ç and ã): preprocess Ç→S, Ã→A → "CASAO". C(seed,last=2),
    //   A(reset), S(7), A(reset), O(reset) → C, 7 → "C700"
    ("Cação", "C700"),
    // Vieira: preprocess V→B → "BIEIRA". B(seed,last=1), I(reset), E(reset), I(reset), R(6),
    //   A(reset) → B, 6 → "B600"
    ("Vieira", "B600"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = PortuguesePhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-PT({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Portuguese reference pair(s) disagreed:\n{}",
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
        ("São", "Sao"),
        ("João", "Joao"),
        // `Coração` mixes cedilla (`ç → S`) with the tilde (`ã → A`) —
        // the ASCII equivalent for encoding must pre-fold the cedilla
        // to `s`, not leave it as `c` (which encodes as class 2).
        ("Coração", "Corasao"),
        ("Vovô", "Vovo"),
    ] {
        let a = PortuguesePhonex.encode(accented).unwrap();
        let b = PortuguesePhonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX-PT({accented:?}) != PHONEX-PT({ascii:?})");
    }
}

#[test]
fn digraph_and_merger_equivalences() {
    // LH → L (palatal lateral).
    assert_eq!(
        PortuguesePhonex.encode("Filho"),
        PortuguesePhonex.encode("Filo"),
        "LH-L merger failed"
    );
    // NH → N (palatal nasal).
    assert_eq!(
        PortuguesePhonex.encode("Ninho"),
        PortuguesePhonex.encode("Nino"),
        "NH-N merger failed"
    );
    // V-B merger.
    assert_eq!(
        PortuguesePhonex.encode("Vieira"),
        PortuguesePhonex.encode("Bieira"),
        "V-B merger failed"
    );
    // Z-S merger.
    assert_eq!(
        PortuguesePhonex.encode("Souza"),
        PortuguesePhonex.encode("Sousa"),
        "Z-S merger failed"
    );
    // Ç-S merger.
    assert_eq!(
        PortuguesePhonex.encode("Cação"),
        PortuguesePhonex.encode("Casao"),
        "Ç-S merger failed"
    );
    // Silent H.
    assert_eq!(
        PortuguesePhonex.encode("Henrique"),
        PortuguesePhonex.encode("Enrique"),
        "Silent-H fold failed"
    );
}
