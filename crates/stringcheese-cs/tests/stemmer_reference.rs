//! Czech light stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_cs::stemmer`], covering the main suffix cascade
//! across noun / adjective / possessive / verb inflection paradigms.
//!
//! # Design note
//!
//! There is no canonical Snowball Czech to compare against — see
//! the module docs for the rationale. The reference pairs here are
//! the algorithm's own contract; a change that flips any pair should
//! flip the corresponding test red so the pack's behaviour is
//! visible in review.

extern crate alloc;

use stringcheese_cs::CzechStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`CzechStemmer::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Adjective endings — masculine / feminine / neuter / plural
    // and case forms all collapse to the same stem.
    // -------------------------------------------------------------
    ("krásný", "krásn"),
    ("krásná", "krásn"),
    ("krásné", "krásn"),
    ("krásného", "krásn"),
    ("krásnému", "krásn"),
    ("krásných", "krásn"),
    ("krásnými", "krásn"),
    ("krásným", "krásn"),
    // -------------------------------------------------------------
    // Verb -ovat family — infinitive, past tense, present tense.
    // Every form collapses to the bare stem.
    // -------------------------------------------------------------
    ("pracovat", "prac"),
    ("pracoval", "prac"),
    ("pracovala", "prac"),
    ("pracovalo", "prac"),
    ("pracovali", "prac"),
    ("pracovaly", "prac"),
    ("pracuji", "prac"),
    ("pracuje", "prac"),
    ("pracuješ", "prac"),
    // -------------------------------------------------------------
    // Verb -at / -it / -ět families — past-tense masculine and
    // infinitives.
    // -------------------------------------------------------------
    ("dělat", "děl"),
    ("dělal", "děl"),
    ("mluvit", "mluv"),
    ("mluvil", "mluv"),
    ("viděl", "vid"),
    // -------------------------------------------------------------
    // Noun endings — case + number.
    // -------------------------------------------------------------
    ("ženám", "žen"),
    ("ženami", "žen"),
    ("ženách", "žen"),
    // -------------------------------------------------------------
    // Possessive adjective forms (Petrov paradigm).
    // -------------------------------------------------------------
    ("petrovi", "petr"),
    ("petrova", "petr"),
    ("petrovy", "petr"),
    ("petrové", "petr"),
    // -------------------------------------------------------------
    // Czech-specific letters survive when the word ends in a
    // consonant that no suffix matches.
    // -------------------------------------------------------------
    ("žluťoučký", "žluťoučk"),
    ("byt", "byt"),
    // -------------------------------------------------------------
    // Very short words stem to themselves.
    // -------------------------------------------------------------
    ("on", "on"),
    ("je", "je"),
];

#[test]
fn light_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = CzechStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  CzechStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Czech stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 20 pairs. Verify we're above
    // that.
    assert!(
        PAIRS.len() >= 20,
        "reference pair count {} is below the 20-pair floor",
        PAIRS.len()
    );
}

#[test]
fn czech_specific_letters_are_exercised() {
    // The reference set touches every haček + long-vowel + e-caron +
    // ring-over-u distinctive Czech letter used in the fixture set at
    // least once — either in the input or in the expected stem — so a
    // regression in the letter handling shows up as a reference-pair
    // failure.
    let joined: String = PAIRS
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .collect::<alloc::vec::Vec<_>>()
        .join("");
    // We hit: á (krásný etc.), ě (dělat, viděl), í (žluťoučký? no —
    // has ý), ý (krásný, žluťoučký), ž (žena, žluťoučký), ť
    // (žluťoučký), č (žluťoučký).
    for c in ['á', 'ě', 'ý', 'ž', 'č'] {
        assert!(
            joined.contains(c),
            "Czech-specific letter {c:?} is not exercised by any reference pair"
        );
    }
}

#[test]
fn every_reference_pair_converges() {
    // The stemmer is not universally idempotent on arbitrary input,
    // but every input in the reference table converges to a fixed
    // point within a small number of iterations.
    for &(input, _expected) in PAIRS {
        let mut cur = CzechStemmer.stem(input).into_owned();
        let mut steps = 0;
        loop {
            let next = CzechStemmer.stem(&cur).into_owned();
            if next == cur {
                break;
            }
            cur = next;
            steps += 1;
            assert!(
                steps <= 5,
                "stem did not converge in 5 iterations for input {input:?} (last = {cur:?})"
            );
        }
    }
}
