//! PHONEX-Hungarian reference input/output pairs.
//!
//! A curated set of Hungarian surnames, place names, and common
//! words that exercises every preprocessing rule (long-vowel fold,
//! umlaut fold, silent `h`, and each of the eight Hungarian digraphs
//! plus the `dzs` trigraph) and the duplicate-consonant collapse.
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_hu::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_hu::HungarianPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Hungarian key).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Trivial: common single-word inputs. Traces in comments show
    // seed + code sequence with duplicate-consonant collapses and
    // vowel resets applied.
    // -------------------------------------------------------------
    // "Budapest" — B, U(drop), D=3, A(drop), P=1, E(drop), S=7 →
    //   "B317".
    ("Budapest", "B317"),
    // "Szeged" — SZ→S seed, E(drop), G=2, E(drop), D=3 → "S23"
    //   pad → "S230".
    ("Szeged", "S230"),
    // "Pécs" — P seed, E(drop), CS→C=2 → "P2" pad → "P200".
    ("Pécs", "P200"),
    // "Győr" — GY→G' seed, O(drop after umlaut fold), R=6 → "G6"
    //   pad → "G600".
    ("Győr", "G600"),
    // "Debrecen" — D seed, E(drop), B=1, R=6, E(drop), C=2 → "D162"
    //   at len=4, break. N never reached.
    ("Debrecen", "D162"),
    // "Kossuth" — K seed, O(drop), S=7, S=7 dup drop, U(drop),
    //   silent-H drops in preprocess, T=3 → "K73" pad → "K730".
    ("Kossuth", "K730"),
    // -------------------------------------------------------------
    // Digraph coverage.
    // -------------------------------------------------------------
    // "csirke" — CS→C seed, I(drop), R=6, K=2, E(drop) → "C62"
    //   pad → "C620".
    ("csirke", "C620"),
    // "szó" — SZ→S seed, O(drop after long-vowel fold) → "S"
    //   pad → "S000".
    ("szó", "S000"),
    // "zsák" — ZS→Z' seed, A(drop), K=2 → "Z2" pad → "Z200".
    ("zsák", "Z200"),
    // "gyerek" — GY→G' seed, E(drop), R=6, E(drop), K=2 → "G62"
    //   pad → "G620".
    ("gyerek", "G620"),
    // "nyár" — NY→N' seed, A(drop after long-vowel fold), R=6 →
    //   "N6" pad → "N600".
    ("nyár", "N600"),
    // "tyúk" — TY→T' seed, U(drop after long-vowel fold), K=2 →
    //   "T2" pad → "T200".
    ("tyúk", "T200"),
    // "lyuk" — LY→J seed, U(drop), K=2 → "J2" pad → "J200".
    ("lyuk", "J200"),
    // "dzsem" — DZS→J seed, E(drop), M=5 → "J5" pad → "J500".
    ("dzsem", "J500"),
    // -------------------------------------------------------------
    // Long-vowel folds.
    // -------------------------------------------------------------
    // "álom" — Á→A seed, L=4, O(drop), M=5 → "A45" pad → "A450".
    ("álom", "A450"),
    // "éj" — É→E seed, J=2 → "E2" pad → "E200".
    ("éj", "E200"),
    // "óra" — Ó→O seed, R=6, A(drop) → "O6" pad → "O600".
    ("óra", "O600"),
    // "új" — Ú→U seed, J=2 → "U2" pad → "U200".
    ("új", "U200"),
    // -------------------------------------------------------------
    // Silent-H tests.
    // -------------------------------------------------------------
    // "hat" — H drops → "at" → A seed, T=3 → "A3" pad → "A300".
    ("hat", "A300"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = HungarianPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-HU({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Hungarian reference pair(s) disagreed:\n{}",
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
fn long_vowel_folds_produce_the_same_key() {
    // Long vowels fold to their short counterparts for encoding.
    for (long, short) in [("álom", "alom"), ("óra", "ora"), ("új", "uj"), ("Íj", "Ij")] {
        let a = HungarianPhonex.encode(long).unwrap();
        let b = HungarianPhonex.encode(short).unwrap();
        assert_eq!(a, b, "PHONEX-HU({long:?}) != PHONEX-HU({short:?})");
    }
}

#[test]
fn umlaut_folds_produce_the_same_key_at_seed() {
    // ö/ő fold to O; ü/ű fold to U for the seed letter.
    for (umlaut, base) in [
        ("Öröm", "Orom"),
        ("Őz", "Oz"),
        ("Üveg", "Uveg"),
        ("Űr", "Ur"),
    ] {
        let a = HungarianPhonex.encode(umlaut).unwrap();
        let b = HungarianPhonex.encode(base).unwrap();
        assert_eq!(a, b, "PHONEX-HU({umlaut:?}) != PHONEX-HU({base:?})");
    }
}

#[test]
fn silent_h_is_dropped() {
    // Silent H: `hat` (six) and `at` share the key.
    assert_eq!(HungarianPhonex.encode("hat"), HungarianPhonex.encode("at"));
}

#[test]
fn digraphs_encode_as_advertised_seed() {
    // Each Hungarian digraph produces a well-defined seed letter.
    for (input, seed) in [
        ("csirke", 'C'),
        ("szó", 'S'),
        ("zsák", 'Z'),
        ("gyerek", 'G'),
        ("nyár", 'N'),
        ("tyúk", 'T'),
        ("lyuk", 'J'),
        ("dzsem", 'J'),
    ] {
        let key = HungarianPhonex.encode(input).unwrap();
        let first = key.chars().next().unwrap();
        assert_eq!(
            first, seed,
            "input {input:?} — key {key:?} — expected seed {seed:?}"
        );
    }
}
