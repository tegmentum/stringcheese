//! PHONEX reference input/output pairs.
//!
//! Every French surname listed in the task spec (Dubois, Martin,
//! Bernard, Petit, Robert, Richard, Durand, Moreau, Laurent, Simon)
//! plus a handful of additional surnames that exercise every
//! preprocessing rule and every duplicate-collapse edge case.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_fr::phonetic`] — see there for the
//! classification table.

extern crate alloc;

use stringcheese_fr::Phonex;

/// Reference pairs (input, expected 4-char PHONEX key).
const PAIRS: &[(&str, &str)] = &[
    // Task-required surnames — hand-traced against the module-level
    // algorithm.
    ("Dubois", "D180"),
    ("Martin", "M635"),
    ("Bernard", "B656"),
    ("Petit", "P330"),
    ("Robert", "R163"),
    ("Richard", "R863"),
    ("Durand", "D653"),
    ("Moreau", "M600"),
    ("Laurent", "L653"),
    ("Simon", "S550"),
    // Additional surnames exercising digraph substitutions and
    // duplicate collapse.
    ("Dupont", "D153"),
    ("Rousseau", "R800"),
    ("Legrand", "L765"),
    ("Fournier", "F656"),
    ("Girard", "G663"),
    ("Bonnet", "B530"),
    ("Marchand", "M685"),
    ("Champagne", "X515"),
    ("Philippe", "F410"),
    ("Quentin", "K535"),
    // Names with accents — accent-fold to the same key as unaccented.
    ("François", "F658"),
    ("Éric", "E620"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = Phonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 10 pairs. Verify we're above
    // that.
    assert!(
        PAIRS.len() >= 10,
        "reference pair count {} is below the 10-pair floor",
        PAIRS.len()
    );
}

#[test]
fn vowel_accent_folded_variants_produce_the_same_key() {
    // Vowel accents fold to their base letter and therefore encode
    // identically to the unaccented spelling.
    //
    // `ç` is deliberately excluded from this equivalence: the shipped
    // encoder folds `ç` to `S` (its French pronunciation) rather than
    // to plain `C`, so `François` and `Francois` differ — as they
    // should, since the two spellings encode different French sounds.
    // A `Francois` written without the cedilla would code the middle
    // consonant as hard `C` (code 2), which is *not* the same sound
    // as the `ç` in `François`.
    for (accented, ascii) in [
        ("Éric", "Eric"),
        ("Hélène", "Helene"),
        ("Séverine", "Severine"),
        ("André", "Andre"),
        ("Bénédict", "Benedict"),
    ] {
        let a = Phonex.encode(accented).unwrap();
        let b = Phonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX({accented:?}) != PHONEX({ascii:?})");
    }
}
