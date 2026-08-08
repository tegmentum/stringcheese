//! Snowball-family Greek stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_el::snowball`], covering the long-compound step
//! (superlatives, comparatives, mediopassive verbs, passive aorist,
//! abstract-noun -οτητα, passive-participle -μενος), the medium step
//! (nominal / adjectival case endings, common verb endings), and the
//! bare-final-vowel step. The list intentionally stays on well-behaved
//! inflections — the shipped stemmer is a hand-audited subset of the
//! full canonical Snowball spec (see the crate module docs for the
//! deferred-parity note).
//!
//! # Deferred: full-corpus cross-verification
//!
//! The Snowball project distributes `voc.txt` / `output.txt` with tens
//! of thousands of pairs. Embedding a subset here keeps compile times
//! and the test binary size sane; full-corpus cross-verification is a
//! follow-up wave.

extern crate alloc;

use stringcheese_el::GreekSnowball;

/// Reference pairs (input, expected stem after one call to
/// [`GreekSnowball::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Adjective endings — masculine / feminine / neuter / plural
    // forms all collapse to the same short stem.
    // -------------------------------------------------------------
    ("καλός", "καλ"),
    ("καλή", "καλ"),
    ("καλό", "καλ"),
    ("καλοί", "καλ"),
    ("καλές", "καλ"),
    ("καλά", "καλ"),
    // -------------------------------------------------------------
    // Noun -ος paradigm (masculine second declension).
    // -------------------------------------------------------------
    ("κόσμος", "κοσμ"),
    ("κόσμου", "κοσμ"),
    ("κόσμοι", "κοσμ"),
    ("κόσμων", "κοσμ"),
    ("κόσμους", "κοσμ"),
    // -------------------------------------------------------------
    // Noun -α paradigm (feminine first declension).
    // -------------------------------------------------------------
    ("γάτα", "γατ"),
    ("γάτας", "γατ"),
    ("γάτες", "γατ"),
    ("γατών", "γατ"),
    // -------------------------------------------------------------
    // Noun -η paradigm (feminine first declension, -η variant).
    // -------------------------------------------------------------
    ("πόλη", "πολ"),
    ("πόλης", "πολ"),
    // -------------------------------------------------------------
    // Noun -ι paradigm (neuter).
    // -------------------------------------------------------------
    ("σπίτι", "σπιτ"),
    // -------------------------------------------------------------
    // Verb -ω present paradigm.
    // -------------------------------------------------------------
    ("γράφω", "γραφ"),
    ("γράφεις", "γραφ"),
    ("γράφει", "γραφ"),
    ("γράφουμε", "γραφ"),
    ("γράφετε", "γραφ"),
    ("γράφουν", "γραφ"),
    // -------------------------------------------------------------
    // Long-compound step — passive participle, superlative,
    // comparative, abstract-noun -οτητα.
    // -------------------------------------------------------------
    ("γραμμένος", "γραμ"),       // -μενος (5) strips as a compound
    ("ωραιότερος", "ωρα"),       // -οτερος (6) then bare -ι strips
    ("ελευθεροτητα", "ελευθερ"), // -οτητα (5) strips as a compound
    // -------------------------------------------------------------
    // Full-alphabet exercises: proper nouns, common vocabulary.
    // -------------------------------------------------------------
    ("άνθρωπος", "ανθρωπ"), // -ος strips
    ("άνθρωποι", "ανθρωπ"), // -οι strips
    ("γυναίκα", "γυναικ"),  // -α strips (bare vowel)
    ("χώρα", "χωρ"),        // -α strips (bare vowel)
    ("χώρες", "χωρ"),       // -ες strips
    // -------------------------------------------------------------
    // Accent + final-sigma preprocessing: both spellings produce the
    // same stem.
    // -------------------------------------------------------------
    ("κοσμοσ", "κοσμ"), // no accent, no final sigma
    ("Κόσμος", "κοσμ"), // capitalized + accented + final sigma
    ("κόσμοσ", "κοσμ"), // accented, no final sigma
];

#[test]
fn snowball_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = GreekSnowball.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Greek stem({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Greek reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 25 pairs.
    assert!(
        PAIRS.len() >= 25,
        "reference pair count {} is below the 25-pair floor",
        PAIRS.len()
    );
}

#[test]
fn accented_and_unaccented_variants_stem_identically() {
    // The canonical demonstration that the accent-fold at
    // preprocessing time makes `καλός` and `καλος` stem the same way.
    let with_accent = GreekSnowball.stem("καλός").into_owned();
    let without_accent = GreekSnowball.stem("καλοσ").into_owned();
    assert_eq!(with_accent, without_accent);
}

#[test]
fn final_and_non_final_sigma_stem_identically() {
    // Same, for the final-sigma fold.
    let with_final = GreekSnowball.stem("κόσμος").into_owned();
    let with_nonfinal = GreekSnowball.stem("κόσμοσ").into_owned();
    assert_eq!(with_final, with_nonfinal);
}
