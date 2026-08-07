//! Serbian light-stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_sr::snowball`], covering adjective/noun long-form
//! and short-form endings, masculine plural `-ov`- extension, feminine
//! `-a` / `-e` / `-ama` case forms, verb infinitives and L-participles,
//! and one `-ost` derivational ending. Both Cyrillic and Latin inputs
//! are exercised — Cyrillic inputs round-trip through the internal
//! Latin normalization.
//!
//! # Deferred: full-corpus cross-verification
//!
//! The Snowball project distributes `voc.txt` / `output.txt` for
//! Serbian with tens of thousands of pairs. Embedding a subset here
//! keeps compile times and the test binary size sane; full-corpus
//! cross-verification is a follow-up wave.

extern crate alloc;

use stringcheese_sr::SerbianSnowball;

/// Reference pairs (input, expected stem).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Adjectives — short and long forms collapse to the same stem.
    // -------------------------------------------------------------
    ("lepa", "lep"),
    ("lepe", "lep"),
    ("lepi", "lep"),
    ("lepo", "lep"),
    ("lepu", "lep"),
    ("lepim", "lep"),
    ("lepom", "lep"),
    ("velika", "velik"),
    ("veliko", "velik"),
    ("veliki", "velik"),
    // -------------------------------------------------------------
    // Nouns — masculine with `-ov` plural extension.
    // -------------------------------------------------------------
    ("grad", "grad"),
    ("grada", "grad"),
    ("gradu", "grad"),
    ("gradovi", "grad"),
    ("gradova", "grad"),
    ("gradovima", "grad"),
    // -------------------------------------------------------------
    // Nouns — feminine with `-a` / `-e` / `-ama` case forms.
    // -------------------------------------------------------------
    ("kuća", "kuć"),
    ("kuće", "kuć"),
    ("kuću", "kuć"),
    ("kućama", "kuć"),
    // -------------------------------------------------------------
    // Verbs — `-ati` infinitive, `-ao` / `-ala` L-participle.
    // -------------------------------------------------------------
    ("pisati", "pis"),
    ("pisao", "pis"),
    ("pisala", "pis"),
    ("pisali", "pis"),
    // -------------------------------------------------------------
    // Verbs — present tense fragment.
    // -------------------------------------------------------------
    ("radim", "rad"),
    ("radiš", "rad"),
    ("raditi", "rad"),
    ("radio", "rad"),
    // -------------------------------------------------------------
    // Cyrillic — same words, different script.
    // -------------------------------------------------------------
    ("лепа", "леп"),
    ("градови", "град"),
    ("писати", "пис"),
];

#[test]
fn stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = SerbianSnowball.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Serbian({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Serbian reference pair(s) disagreed:\n{}",
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
fn cyrillic_and_latin_stems_are_transliteration_equivalents() {
    // A Cyrillic input stemmed to a Cyrillic form should equal the
    // Latin input stemmed to a Latin form, up to transliteration.
    let latin = SerbianSnowball.stem("gradovi").into_owned();
    let cyrillic = SerbianSnowball.stem("градови").into_owned();
    assert_eq!(
        stringcheese_sr::scripts::to_cyrillic(&latin),
        cyrillic,
        "Latin stem transliterated should equal Cyrillic stem"
    );
}
