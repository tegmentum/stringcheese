//! PHONEX-Finnish reference input/output pairs.
//!
//! A curated set of Finnish place names, surnames, and common words
//! that exercise every preprocessing rule (long-consonant collapse,
//! long-vowel collapse, `ä → a` / `ö → o` / `å → o` fold, `y` treated
//! as a vowel) and every duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_fi::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_fi::FinnishPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Finnish key).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Capital cities and place names — exercises the seed-letter
    // preservation and the consonant classification table.
    // -------------------------------------------------------------
    ("Helsinki", "H475"), // H seed, L(4), S(7), N(5) → K would be 5th.
    ("Tampere", "T516"), // T seed, M(5), P(1) but wait — trace: T seed last=3. A reset. M(5) push. P(1) push. E reset. R(6) push → T516.
    ("Turku", "T620"),   // T seed. U reset. R(6) push. K(2) push. U reset → T62 → T620.
    ("Oulu", "O400"),    // O seed. U reset. L(4) push. U reset → O400.
    ("Espoo", "E710"),   // E seed last=0. S(7) push. P(1) push. Long-vowel oo → o via collapse.
    ("Vantaa", "V530"),  // V seed. A reset. N(5) push. T(3) push. Long aa collapses.
    ("Pori", "P600"),    // P seed. O reset. R(6) push. I reset → P600.
    ("Kotka", "K320"),   // K seed. O reset. T(3) push. K(2) push. A reset → K320.
    ("Kuopio", "K100"),  // K seed. U reset. O reset. P(1) push. I reset. O reset → K100.
    // -------------------------------------------------------------
    // Finnish special-vowel folds. `Jyväskylä` uses both `y` (vowel)
    // and `ä`.
    // -------------------------------------------------------------
    ("Jyväskylä", "J172"), // J seed. Y=vowel reset. V(1) push. Ä→A reset. S(7) push. K(2) push → J172.
    ("Åland", "O453"),     // Å→O seed. L(4) push. A reset. N(5) push. D(3) push → O453.
    ("Hämäläinen", "H545"),
    // -------------------------------------------------------------
    // Long-consonant collapse — a signature Finnish PHONEX
    // equivalence: `tuli` (fire) and `tulli` (customs) share a key.
    // -------------------------------------------------------------
    ("tuli", "T400"),
    ("tulli", "T400"),
    ("kukka", "K200"), // kk → k, so kukka and kuka share.
    ("kuka", "K200"),
    // -------------------------------------------------------------
    // Long-vowel collapse — `maa` (land) collapses to `ma`.
    // -------------------------------------------------------------
    ("maa", "M000"),
    // -------------------------------------------------------------
    // Common Finnish surnames — the top-frequency `-nen` surnames.
    // -------------------------------------------------------------
    ("Virtanen", "V635"),
    ("Korhonen", "K655"),
    ("Nieminen", "N555"),
    ("Mäkinen", "M255"),
    // -------------------------------------------------------------
    // Common brand / cultural names.
    // -------------------------------------------------------------
    ("Nokia", "N200"),
    ("Marimekko", "M652"),
    ("Sibelius", "S147"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = FinnishPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-FI({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Finnish reference pair(s) disagreed:\n{}",
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
fn long_consonants_and_vowels_produce_equivalent_keys() {
    // Signature Finnish PHONEX equivalence: geminate consonants
    // collapse.
    for (single, gemin) in [
        ("tuli", "tulli"),
        ("kuka", "kukka"),
        ("mato", "matto"),
        ("sata", "satta"),
    ] {
        let a = FinnishPhonex.encode(single).unwrap();
        let b = FinnishPhonex.encode(gemin).unwrap();
        assert_eq!(
            a, b,
            "PHONEX-FI({single:?}) = {a:?} != PHONEX-FI({gemin:?}) = {b:?}"
        );
    }
    // Long vowels collapse too.
    for (short, long) in [("maa", "ma"), ("puu", "pu"), ("työ", "tö")] {
        let a = FinnishPhonex.encode(short).unwrap();
        let b = FinnishPhonex.encode(long).unwrap();
        assert_eq!(
            a, b,
            "PHONEX-FI({short:?}) = {a:?} != PHONEX-FI({long:?}) = {b:?}"
        );
    }
}

#[test]
fn a_ring_folds_to_o() {
    // Finnish `å` is a Swedish letter (retained for loanwords /
    // Swedish-origin names) and folds to `o` for phonetic-key
    // purposes.
    let with_ring = FinnishPhonex.encode("Åland").unwrap();
    let with_o = FinnishPhonex.encode("Oland").unwrap();
    assert_eq!(with_ring, with_o);
}

#[test]
fn front_vowels_fold_to_back_vowels_in_key() {
    // ä → a, ö → o, but since vowels are dropped by the Soundex step
    // anyway (all vowels get sentinel `0`), the fold matters only for
    // the seed letter. Words differing only in ä/a or ö/o past the
    // seed produce identical keys.
    for (with_umlaut, without) in [("kirjä", "kirja"), ("työ", "tyo"), ("Mäkinen", "Makinen")] {
        let a = FinnishPhonex.encode(with_umlaut).unwrap();
        let b = FinnishPhonex.encode(without).unwrap();
        assert_eq!(
            a, b,
            "PHONEX-FI({with_umlaut:?}) = {a:?} != PHONEX-FI({without:?}) = {b:?}"
        );
    }
}

#[test]
fn y_is_treated_as_a_vowel() {
    // Finnish `y` is a front rounded vowel /y/, not a palatal glide
    // /j/. It classifies as a vowel and gets the sentinel `0` code,
    // meaning it resets `last_code` in the Soundex step — the same
    // behavior as `i`.
    let with_y = FinnishPhonex.encode("kyky").unwrap();
    let with_i = FinnishPhonex.encode("kiki").unwrap();
    assert_eq!(with_y, with_i, "y should encode like i (both are vowels)");
}
