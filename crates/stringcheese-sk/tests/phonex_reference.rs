//! PHONEX-Slovak reference input/output pairs.
//!
//! A curated set of Slovak surnames and common words that exercise
//! every preprocessing rule (haček fold including Slovak-only `ľ`;
//! long-vowel fold; Slovak-only `ä → E`, `ô → O`, `ĺ → L`, `ŕ → R`;
//! `CH → X`; `RR → R`; silent `H`) and every duplicate-collapse edge
//! case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_sk::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_sk::SlovakPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Slovak key).
const PAIRS: &[(&str, &str)] = &[
    // Novák: N seed. O vow reset. V code=1 push → "N1". A vow reset.
    //   K code=2 push → "N12". Pad → "N120".
    ("Novák", "N120"),
    // Kováč: K seed. O vow reset. V code=1 push → "K1". A vow reset.
    //   C (from č fold) code=2 push → "K12". Pad → "K120".
    ("Kováč", "K120"),
    // Kráľ (Slovak-only ľ folds to L): K seed. R code=6 push → "K6".
    //   A vow reset. L code=4 push → "K64". Pad → "K640".
    ("Kráľ", "K640"),
    // Kôň (Slovak-only ô folds to O; ň folds to N): K seed. O vow
    //   reset. N code=5 push → "K5". Pad → "K500".
    ("Kôň", "K500"),
    // Späť (Slovak-only ä folds to E; ť folds to T): S seed. P code=1
    //   push → "S1". E vow reset. T code=3 push → "S13". Pad → "S130".
    ("Späť", "S130"),
    // Ďuriš (ď→D, š→S): D seed. U vow reset. R code=6 push → "D6".
    //   I vow reset. S code=7 push → "D67". Pad → "D670".
    ("Ďuriš", "D670"),
    // chlieb (CH→X): X seed. L code=4 push → "X4". I vow reset. E
    //   vow reset. B code=1 push → "X41". Pad → "X410".
    ("chlieb", "X410"),
    // Havran (H drops → AVRAN): A seed. V code=1 push → "A1". R
    //   code=6 push → "A16". A vow reset. N code=5 push → "A165".
    //   Cap → "A165".
    ("Havran", "A165"),
    // Novotný (ý→Y): N seed. O vow reset. V code=1 push → "N1". O
    //   vow reset. T code=3 push → "N13". N code=5 push → "N135".
    //   Y vow reset. → "N135".
    ("Novotný", "N135"),
    // Malý: M seed. A vow reset. L code=4 push → "M4". Y vow reset.
    //   Pad → "M400".
    ("Malý", "M400"),
    // Vôňa (ô→O, ň→N): V seed. O vow reset. N code=5 push → "V5".
    //   A vow reset. Pad → "V500".
    ("Vôňa", "V500"),
    // Stĺp (Slovak-only ĺ folds to L): S seed. T code=3 push → "S3".
    //   L code=4 push → "S34". P code=1 push → "S341". Pad → "S341".
    ("Stĺp", "S341"),
    // Vŕba (Slovak-only ŕ folds to R): V seed. R code=6 push → "V6".
    //   B code=1 push → "V61". A vow reset. Pad → "V610".
    ("Vŕba", "V610"),
    // Ľubica (Slovak-only ľ folds to L, so it's the seed letter):
    //   L seed. U vow reset. B code=1 push → "L1". I vow reset.
    //   C code=2 push → "L12". A vow reset. Pad → "L120".
    ("Ľubica", "L120"),
    // Žila (ž→Z): Z seed. I vow reset. L code=4 push → "Z4". A vow
    //   reset. Pad → "Z400".
    ("Žila", "Z400"),
    // Šarišský: S seed. A vow reset. R code=6 push → "S6". I vow
    //   reset. S code=7 push → "S67". S dup drop. K code=2 push →
    //   "S672". Y vow reset. → "S672".
    ("Šarišský", "S672"),
    // Horváth (H drops from start and end → ORVAT): O seed. R code=6
    //   push → "O6". V code=1 push → "O61". A vow reset. T code=3
    //   push → "O613". → "O613".
    ("Horváth", "O613"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = SlovakPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-SK({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Slovak reference pair(s) disagreed:\n{}",
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
fn hacek_folds_produce_the_same_key_as_base_letter() {
    // Haček-marked consonants fold to their base letter and therefore
    // encode identically to the unmarked spelling for Soundex-shape
    // classification purposes.
    for (with_hacek, ascii) in [
        ("žena", "zena"),
        ("šum", "sum"),
        ("deti", "deti"),
        ("kôň", "kon"),
        ("ľud", "lud"),
    ] {
        let a = SlovakPhonex.encode(with_hacek).unwrap();
        let b = SlovakPhonex.encode(ascii).unwrap();
        assert_eq!(
            a, b,
            "PHONEX-SK({with_hacek:?}) != PHONEX-SK({ascii:?}) (haček fold failed)"
        );
    }
}

#[test]
fn long_vowel_folds_produce_the_same_key() {
    // Long vowels fold to their short counterparts for encoding.
    for (long, short) in [("útok", "utok"), ("Íra", "Ira"), ("ýma", "yma")] {
        let a = SlovakPhonex.encode(long).unwrap();
        let b = SlovakPhonex.encode(short).unwrap();
        assert_eq!(a, b, "PHONEX-SK({long:?}) != PHONEX-SK({short:?})");
    }
}

#[test]
fn slovak_specific_letter_folds_produce_the_same_key() {
    // Slovak-only letter folds — `ä → E`, `ô → O`, `ľ → L`, `ĺ → L`,
    // `ŕ → R`. Each folds to its base placeholder and encodes
    // identically.
    for (slovak, base) in [
        ("späť", "spet"), // ä → E
        ("kôň", "kon"),   // ô → O
        ("kráľ", "kral"), // ľ → L
        ("stĺp", "stlp"), // ĺ → L
        ("vŕba", "vrba"), // ŕ → R
    ] {
        let a = SlovakPhonex.encode(slovak).unwrap();
        let b = SlovakPhonex.encode(base).unwrap();
        assert_eq!(a, b, "PHONEX-SK({slovak:?}) != PHONEX-SK({base:?})");
    }
}

#[test]
fn silent_h_is_dropped() {
    // Silent H: `hora` and `ora` share the key (both begin with 'O'
    // after H drops).
    assert_eq!(SlovakPhonex.encode("hora"), SlovakPhonex.encode("ora"));
}

#[test]
fn ch_digraph_merges_with_x() {
    // CH → X preprocessing: `chlieb` encodes to a key starting with X.
    let key = SlovakPhonex.encode("chlieb").unwrap();
    assert!(
        key.starts_with('X'),
        "chlieb should encode to X-prefixed key: {key}"
    );
}
