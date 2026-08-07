//! Slovak light stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_sk::stemmer`], covering the main suffix cascade
//! across noun / adjective / possessive / verb inflection paradigms.
//!
//! # Design note
//!
//! There is no canonical Snowball Slovak to compare against — see
//! the module docs for the rationale. The reference pairs here are
//! the algorithm's own contract; a change that flips any pair should
//! flip the corresponding test red so the pack's behaviour is
//! visible in review.

extern crate alloc;

use stringcheese_sk::SlovakStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`SlovakStemmer::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Adjective endings — masculine / feminine / neuter / plural
    // and case forms all collapse to the same stem.
    // -------------------------------------------------------------
    ("pekný", "pekn"),
    ("pekná", "pekn"),
    ("pekné", "pekn"),
    ("pekného", "pekn"),
    ("peknému", "pekn"),
    ("pekných", "pekn"),
    ("peknými", "pekn"),
    ("pekným", "pekn"),
    // -------------------------------------------------------------
    // Verb -ovať family — Slovak infinitive ends in `-ť` (not
    // Czech's `-t`), past tense, and Slovak-specific present-tense
    // paradigm `-ujem` / `-uješ` / `-uje` / `-ujeme` / `-ujete`
    // / `-ujú`. Every form collapses to the bare stem.
    // -------------------------------------------------------------
    ("pracovať", "prac"),
    ("pracoval", "prac"),
    ("pracovala", "prac"),
    ("pracovalo", "prac"),
    ("pracovali", "prac"),
    ("pracujem", "prac"),
    ("pracuješ", "prac"),
    ("pracuje", "prac"),
    ("pracujeme", "prac"),
    ("pracujete", "prac"),
    ("pracujú", "prac"),
    // -------------------------------------------------------------
    // Verb -iť family — Slovak infinitive.
    // -------------------------------------------------------------
    ("robiť", "rob"),
    ("hovoril", "hovor"),
    // -------------------------------------------------------------
    // Verb -ieť infinitive (Slovak-specific — Czech has -ět).
    // -------------------------------------------------------------
    ("vidieť", "vid"),
    // -------------------------------------------------------------
    // Verb -núť infinitive.
    // -------------------------------------------------------------
    ("napadnúť", "napad"),
    // -------------------------------------------------------------
    // Noun endings — case + number.
    // -------------------------------------------------------------
    ("ženám", "žen"),
    ("ženami", "žen"),
    ("ženách", "žen"),
    ("ženou", "žen"),
    // Slovak's `-om` instrumental (Czech has `-em`).
    ("pánom", "pán"),
    // -------------------------------------------------------------
    // Possessive adjective forms (Petrov paradigm).
    // -------------------------------------------------------------
    ("petrovi", "petr"),
    ("petrova", "petr"),
    ("petrovo", "petr"),
    ("petrove", "petr"),
    // -------------------------------------------------------------
    // Slovak-specific letters survive when the word ends in a
    // consonant that no suffix matches, or when the RV guard blocks
    // stripping of a sole-vowel ending.
    // -------------------------------------------------------------
    ("chudý", "chud"),
    ("žltý", "žltý"), // RV guard: sole final vowel — untouched.
    ("byt", "byt"),
    ("stĺp", "stĺp"), // Slovak-only ĺ preserved intact.
    ("kôň", "kôň"),   // Slovak-only ô + ň preserved intact.
    ("späť", "späť"), // Slovak-only ä + ť preserved intact.
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
        let got = SlovakStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  SlovakStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Slovak stemmer reference pair(s) disagreed:\n{}",
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
fn slovak_specific_letters_are_exercised() {
    // The reference set touches every haček + long-vowel +
    // Slovak-specific letter used in the fixture set at least once —
    // either in the input or in the expected stem — so a regression
    // in the letter handling shows up as a reference-pair failure.
    let joined: String = PAIRS
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .collect::<alloc::vec::Vec<_>>()
        .join("");
    // Slovak-specific letters used: ä (späť), ô (kôň), ľ (in tokenizer
    // fixtures — not required here), ĺ (stĺp), ť (many infinitives),
    // ý (pekný, žltý), ž (žena, žltý), č (?), ú (napadnúť).
    for c in ['á', 'ý', 'ž', 'ť', 'ú', 'ä', 'ô', 'ĺ'] {
        assert!(
            joined.contains(c),
            "Slovak-specific letter {c:?} is not exercised by any reference pair"
        );
    }
}

#[test]
fn slovak_infinitive_uses_t_with_hacek() {
    // The marquee Slovak/Czech divergence: Slovak infinitive is `-ť`,
    // not Czech's `-t`. Verify that the -ovať / -iť / -ieť / -núť
    // family strips cleanly.
    assert_eq!(
        SlovakStemmer.stem("pracovať").into_owned(),
        "prac",
        "Slovak -ovať infinitive must strip"
    );
    assert_eq!(
        SlovakStemmer.stem("robiť").into_owned(),
        "rob",
        "Slovak -iť infinitive must strip"
    );
    assert_eq!(
        SlovakStemmer.stem("vidieť").into_owned(),
        "vid",
        "Slovak -ieť infinitive must strip"
    );
    assert_eq!(
        SlovakStemmer.stem("napadnúť").into_owned(),
        "napad",
        "Slovak -núť infinitive must strip"
    );
}

#[test]
fn every_reference_pair_converges() {
    // The stemmer is not universally idempotent on arbitrary input,
    // but every input in the reference table converges to a fixed
    // point within a small number of iterations.
    for &(input, _expected) in PAIRS {
        let mut cur = SlovakStemmer.stem(input).into_owned();
        let mut steps = 0;
        loop {
            let next = SlovakStemmer.stem(&cur).into_owned();
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
