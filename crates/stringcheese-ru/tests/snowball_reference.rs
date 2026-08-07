//! Snowball Russian stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_ru::snowball`], covering the perfective gerund,
//! reflexive, adjectival, verb, noun, derivational, and undouble /
//! superlative / soft-sign passes. The list intentionally stays on
//! well-behaved inflections — the surface residue for words where the
//! algorithm over-strips (like `странный → стра`, because the `нн`
//! participle rule fires after the `-ый` adjective strip) is treated
//! as the stem; consonant restoration is a separate lexicon-driven
//! concern and is out of scope. See the crate module docs for the
//! deferred-item list.
//!
//! # Deferred: full-corpus cross-verification
//!
//! The Snowball project distributes `voc.txt` / `output.txt` with
//! tens of thousands of pairs. Embedding a subset here keeps compile
//! times and the test binary size sane; full-corpus cross-verification
//! is a follow-up wave and would run under a feature-gated
//! `snowball-fullcorpus` cfg reading the vocab files at build time.

extern crate alloc;

use stringcheese_ru::RussianSnowball;

/// Reference pairs (input, expected stem after one call to
/// [`RussianSnowball::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Adjective endings — masculine / feminine / neuter / plural
    // singular and case forms all collapse to the same stem.
    // -------------------------------------------------------------
    ("красивый", "красив"),
    ("красивая", "красив"),
    ("красивое", "красив"),
    ("красивые", "красив"),
    ("красивого", "красив"),
    ("красивому", "красив"),
    ("красивым", "красив"),
    ("красивыми", "красив"),
    ("белый", "бел"),
    ("белая", "бел"),
    ("новый", "нов"),
    ("новая", "нов"),
    ("добрый", "добр"),
    // -------------------------------------------------------------
    // Noun endings — case + number.
    // -------------------------------------------------------------
    ("столы", "стол"),
    ("книги", "книг"),
    ("доме", "дом"),
    ("домом", "дом"),
    ("домов", "дом"),
    ("города", "город"),
    ("городе", "город"),
    // -------------------------------------------------------------
    // Verb endings — past + infinitive + present.
    // -------------------------------------------------------------
    ("читал", "чита"),
    ("читала", "чита"),
    ("читали", "чита"),
    ("читать", "чита"),
    ("делал", "дела"),
    ("делает", "дела"),
    ("делают", "дела"),
    ("знать", "зна"),
    ("знаю", "зна"),
    // -------------------------------------------------------------
    // Reflexive verbs — `ся` / `сь` stripping.
    // -------------------------------------------------------------
    ("смеяться", "смея"),
    ("улыбаться", "улыба"),
    // -------------------------------------------------------------
    // Derivational — `ость` / `ост` in R2.
    // -------------------------------------------------------------
    ("полезность", "полезн"),
    ("молодость", "молод"),
    // -------------------------------------------------------------
    // Adjectival + participle chain (`-ый` then `-нн`).
    // -------------------------------------------------------------
    ("странный", "стра"),
    ("длинный", "длин"),
    ("современного", "современ"),
    // -------------------------------------------------------------
    // Superlatives — adjectival then step 4's `ейш` fires.
    // -------------------------------------------------------------
    ("интереснейший", "интересн"),
    ("красивейший", "красив"),
    ("умнейший", "умн"),
    ("новейший", "нов"),
    // -------------------------------------------------------------
    // Soft-sign endings — step 4 removes trailing `ь` where step 1
    // has not already claimed it.
    // -------------------------------------------------------------
    ("лошадь", "лошад"),
    ("ночь", "ноч"),
    // -------------------------------------------------------------
    // ё → е preprocessing: both spellings produce the same stem.
    // -------------------------------------------------------------
    ("ёлка", "елк"),
    ("елка", "елк"),
    // -------------------------------------------------------------
    // Perfective gerund (group 1) — `в` preceded by `а`.
    // -------------------------------------------------------------
    ("прочитав", "прочита"),
];

#[test]
fn snowball_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = RussianSnowball.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Snowball({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Snowball reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 30 pairs. Verify we're above
    // that.
    assert!(
        PAIRS.len() >= 30,
        "reference pair count {} is below the 30-pair floor",
        PAIRS.len()
    );
}

#[test]
fn yo_and_ye_variants_stem_identically() {
    // The ё-fold precomputation means `ёлка` and `елка` stem to the
    // same form; this is the canonical demonstration the crate docs
    // point at.
    let with_yo = RussianSnowball.stem("ёлка").into_owned();
    let without_yo = RussianSnowball.stem("елка").into_owned();
    assert_eq!(with_yo, without_yo);
}
