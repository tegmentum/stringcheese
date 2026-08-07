//! Ukrainian light stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_uk::snowball`], covering the reflexive pass and the
//! main suffix cascade across noun / adjective / verb inflection
//! paradigms, plus the trailing soft-sign strip.
//!
//! # Design note
//!
//! There is no canonical Snowball Ukrainian to compare against — see
//! the module docs for the rationale. The reference pairs here are
//! the algorithm's own contract; a change that flips any pair should
//! flip the corresponding test red so the pack's behaviour is
//! visible in review.

extern crate alloc;

use stringcheese_uk::UkrainianSnowball;

/// Reference pairs (input, expected stem after one call to
/// [`UkrainianSnowball::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Adjective endings — masculine / feminine / neuter / plural
    // and their case forms all collapse to the same stem.
    // -------------------------------------------------------------
    ("красивий", "красив"),
    ("красива", "красив"),
    ("красиве", "красив"),
    ("красиві", "красив"),
    ("красивого", "красив"),
    ("красивому", "красив"),
    ("красивих", "красив"),
    ("красивими", "красив"),
    ("красивою", "красив"),
    ("новий", "нов"),
    ("нова", "нов"),
    ("нове", "нов"),
    ("нові", "нов"),
    // -------------------------------------------------------------
    // Comparative / superlative shapes — the -іший / -іших / -ішого
    // family strips in a single pass because it lives in the main
    // table at higher priority than the bare adjective endings.
    // -------------------------------------------------------------
    ("новіший", "нов"),
    ("гарніший", "гарн"),
    ("гарніші", "гарн"),
    // -------------------------------------------------------------
    // Noun endings — case + number.
    // -------------------------------------------------------------
    ("столи", "стол"),
    ("стола", "стол"),
    ("столу", "стол"),
    ("столом", "стол"),
    ("столами", "стол"),
    ("книги", "книг"),
    ("містом", "міст"),
    ("містах", "міст"),
    // -------------------------------------------------------------
    // The load-bearing genitive-plural / past-tense disambiguation:
    // -ів (2 chars) beats -в (1 char) under the globally longest
    // match discipline.
    // -------------------------------------------------------------
    ("будинків", "будинк"),
    // -------------------------------------------------------------
    // Verb endings — infinitive + past + present.
    // -------------------------------------------------------------
    ("читати", "чита"),
    ("читав", "чита"),
    ("читала", "чита"),
    ("читали", "чита"),
    ("читають", "чита"),
    ("читаємо", "чита"),
    ("читаєте", "чита"),
    ("читаєш", "чита"),
    // -------------------------------------------------------------
    // Reflexive verbs — -ся / -сь stripped, then the main pass
    // fires on what remains.
    // -------------------------------------------------------------
    ("сміятися", "смія"),
    ("усміхатися", "усміха"),
    // -------------------------------------------------------------
    // Trailing soft sign — the main pass here strips -ть, so the
    // trailing-ь rule finds nothing left to strip.
    // -------------------------------------------------------------
    ("мить", "ми"),
    // -------------------------------------------------------------
    // Ukrainian-specific letters survive when the word ends in a
    // consonant that no suffix in the table matches.
    // -------------------------------------------------------------
    ("рік", "рік"),
    ("їжак", "їжак"),
    ("ґудзик", "ґудзик"),
];

#[test]
fn light_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = UkrainianSnowball.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  UkrainianSnowball({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Ukrainian stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 25 pairs. Verify we're above
    // that.
    assert!(
        PAIRS.len() >= 25,
        "reference pair count {} is below the 25-pair floor",
        PAIRS.len()
    );
}

#[test]
fn ukrainian_specific_letters_are_exercised() {
    // The reference set touches every Ukrainian-specific letter
    // (ґ, є, і, ї) at least once — either in the input or in the
    // expected stem — so a regression in the letter handling shows
    // up as a reference-pair failure.
    let joined: String = PAIRS
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .collect::<alloc::vec::Vec<_>>()
        .join("");
    for c in ['ґ', 'є', 'і', 'ї'] {
        assert!(
            joined.contains(c),
            "Ukrainian-specific letter {c:?} is not exercised by any reference pair"
        );
    }
}
