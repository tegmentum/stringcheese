//! Belarusian light stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_be::stemmer`], covering the reflexive pass and the
//! main suffix cascade across noun / adjective / verb inflection
//! paradigms, plus the trailing soft-sign strip.
//!
//! # Design note
//!
//! There is no canonical Snowball Belarusian to compare against — see
//! the module docs for the rationale. The reference pairs here are
//! the algorithm's own contract; a change that flips any pair should
//! flip the corresponding test red so the pack's behaviour is visible
//! in review.

extern crate alloc;

use stringcheese_be::BelarusianStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`BelarusianStemmer::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Adjective endings — masculine / feminine / neuter / plural and
    // their case forms all collapse to the same stem.
    // -------------------------------------------------------------
    ("красівы", "красів"),
    ("красівая", "красів"),
    ("красівае", "красів"),
    ("красівыя", "красів"),
    ("красівага", "красів"),
    ("красіваму", "красів"),
    ("красівых", "красів"),
    ("красівым", "красів"),
    ("новы", "нов"),
    ("новая", "нов"),
    ("новае", "нов"),
    ("новыя", "нов"),
    // -------------------------------------------------------------
    // Noun endings — case + number.
    // -------------------------------------------------------------
    ("сталы", "стал"),
    ("сябра", "сябр"),
    ("сталамі", "стал"),
    ("сталах", "стал"),
    ("сталом", "стал"),
    // -------------------------------------------------------------
    // The load-bearing genitive-plural / past-tense disambiguation:
    // -оў (2 chars) beats -ў (1 char) under the globally longest
    // match discipline.
    // -------------------------------------------------------------
    ("садоў", "сад"),
    // -------------------------------------------------------------
    // Verb endings — infinitive + past + present.
    // -------------------------------------------------------------
    ("чытаць", "чыта"),
    ("чытаў", "чыта"),
    ("чытала", "чыта"),
    ("чыталі", "чыта"),
    ("чытаюць", "чыта"),
    ("чытаеш", "чыта"),
    // -------------------------------------------------------------
    // Reflexive verbs — -ся stripped, then the main pass fires on
    // what remains.
    // -------------------------------------------------------------
    ("чытаўся", "чыта"),
    // -------------------------------------------------------------
    // Trailing soft sign — the main pass leaves the trailing ь to
    // the final rule (no infinitive -ць matches here).
    // -------------------------------------------------------------
    ("путь", "пут"),
    // -------------------------------------------------------------
    // Bare vowel -я strips from a stem containing Belarusian э.
    // -------------------------------------------------------------
    ("ідэя", "ідэ"),
    // -------------------------------------------------------------
    // Belarusian-specific letters survive when the word ends in a
    // consonant that no suffix in the table matches.
    // -------------------------------------------------------------
    ("год", "год"),
    ("аўтар", "аўтар"),
];

#[test]
fn light_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = BelarusianStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  BelarusianStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Belarusian stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 25 pairs; verify we're above
    // that.
    assert!(
        PAIRS.len() >= 25,
        "reference pair count {} is below the 25-pair floor",
        PAIRS.len()
    );
}

#[test]
fn belarusian_specific_letters_are_exercised() {
    // The reference set touches Belarusian-specific letters (ў, і, ы,
    // э) at least once — either in the input or in the expected stem
    // — so a regression in the letter handling shows up as a
    // reference-pair failure.
    let joined: String = PAIRS
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .collect::<alloc::vec::Vec<_>>()
        .join("");
    for c in ['ў', 'і', 'ы', 'э'] {
        assert!(
            joined.contains(c),
            "Belarusian letter {c:?} is not exercised by any reference pair"
        );
    }
}
