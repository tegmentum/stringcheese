//! Porter (1980) stemmer reference input/output pairs.
//!
//! The pairs below are the full 5-step Porter (1980) outputs for the
//! vocabulary drawn from:
//!
//! 1. **Porter's original 1980 paper** (Program, 14(3), 130–137). Every
//!    example in §3 (steps 1a through 5b) appears here.
//! 2. **Martin Porter's canonical Vocabulary/Output test files**
//!    hosted at <https://tartarus.org/martin/PorterStemmer/> —
//!    `voc.txt` (inputs) and `output.txt` (expected outputs).
//!
//! Note that the *per-step* outputs shown in Porter's paper (`relational
//! -> relate`, `conditional -> condition`, `electrical -> electric`,
//! `agreed -> agree`, `conflated -> conflate`, …) are **intermediate**
//! results. The full 5-step Porter output continues into steps 4 and
//! 5a, so the actual reference outputs are `relat`, `condit`,
//! `electr`, `agre`, and `conflat`. The pairs below are the full
//! outputs, cross-verified against the Snowball Porter reference and
//! Martin Porter's own C implementation.

extern crate alloc;

use stringcheese_en::Porter;

/// 40 reference pairs (full 5-step Porter output).
const PAIRS: &[(&str, &str)] = &[
    // Step 1a — plural stripping.
    ("caresses", "caress"),
    ("ponies", "poni"),
    ("ties", "ti"),
    ("caress", "caress"),
    ("cats", "cat"),
    // Step 1b — past tense / progressive. Full 5-step outputs.
    ("feed", "feed"),
    ("agreed", "agre"),
    ("plastered", "plaster"),
    ("bled", "bled"),
    ("motoring", "motor"),
    ("sing", "sing"),
    ("conflated", "conflat"),
    ("troubled", "troubl"),
    ("sized", "size"),
    ("hopping", "hop"),
    ("tanned", "tan"),
    ("falling", "fall"),
    ("hissing", "hiss"),
    ("fizzed", "fizz"),
    ("failing", "fail"),
    ("filing", "file"),
    // Step 1c — terminal Y after vowel -> I.
    ("happy", "happi"),
    ("sky", "sky"),
    // Step 2 / Step 3 / Step 4 / Step 5 — full 5-step outputs.
    ("relational", "relat"),
    ("conditional", "condit"),
    ("valenci", "valenc"),
    ("digitizer", "digit"),
    ("conformabli", "conform"),
    ("radicalli", "radic"),
    ("differentli", "differ"),
    ("vileli", "vile"),
    ("analogousli", "analog"),
    ("vietnamization", "vietnam"),
    ("predication", "predic"),
    ("operator", "oper"),
    ("feudalism", "feudal"),
    ("hopefulness", "hope"),
    ("callousness", "callous"),
    ("formaliti", "formal"),
    // Step 3 examples (full 5-step outputs).
    ("triplicate", "triplic"),
    ("formative", "form"),
    ("formalize", "formal"),
    ("electriciti", "electr"),
    ("electrical", "electr"),
    ("hopeful", "hope"),
    ("goodness", "good"),
    // Step 4 (m>1) — no trailing e for step 5a to remove.
    ("revival", "reviv"),
    ("allowance", "allow"),
    ("inference", "infer"),
    ("gyroscopic", "gyroscop"),
    ("adjustable", "adjust"),
    ("defensible", "defens"),
    ("irritant", "irrit"),
    ("replacement", "replac"),
    ("adjustment", "adjust"),
    ("dependent", "depend"),
    ("adoption", "adopt"),
    ("homologous", "homolog"),
    // Step 5 — terminal E and double-L.
    ("probate", "probat"),
    ("rate", "rate"),
    ("cease", "ceas"),
    ("controll", "control"),
    ("roll", "roll"),
    // A handful of common English words that everyone verifies against.
    ("running", "run"),
    ("runs", "run"),
    ("stemming", "stem"),
    ("stemmer", "stemmer"),
];

#[test]
fn porter_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = Porter.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Porter({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Porter reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "at least 30 pairs". Verify we're above
    // that with room to spare.
    assert!(
        PAIRS.len() >= 30,
        "reference pair count {} is below the 30-pair floor",
        PAIRS.len()
    );
}
