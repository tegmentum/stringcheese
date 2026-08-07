//! Light Persian stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_fa::stemmer`] module docs). The
//! stemmer strips one nominal suffix (optionally preceded by a ZWNJ)
//! per call, longest-match-wins, with the over-strip safeguard
//! rolling back any strip that would leave fewer than 2 characters.
//!
//! Normalization is *not* applied to the reference inputs — the
//! stemmer is being tested in isolation, so every input is already in
//! a canonical Persian spelling (Persian yeh / kaf, no tatweel).

extern crate alloc;

use stringcheese_fa::LightPersianStemmer;

/// 20 reference pairs.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Plural / plural + ezafeh — with and without ZWNJ.
    // -----------------------------------------------------------------
    ("کتاب\u{200C}ها", "کتاب"),  // book + ZWNJ + plural
    ("کتابها", "کتاب"),          // book + plural (no ZWNJ)
    ("کتاب\u{200C}های", "کتاب"), // book + ZWNJ + plural + ezafeh
    ("کتابهای", "کتاب"),         // book + plural + ezafeh (no ZWNJ)
    // -----------------------------------------------------------------
    // Comparative / superlative — longest-match wins.
    // -----------------------------------------------------------------
    ("بزرگتر", "بزرگ"),           // big + comparative
    ("بزرگ\u{200C}تر", "بزرگ"),   // big + ZWNJ + comparative
    ("بزرگترین", "بزرگ"),         // big + superlative
    ("بزرگ\u{200C}ترین", "بزرگ"), // big + ZWNJ + superlative
    ("خوبترین", "خوب"),           // good + superlative
    // -----------------------------------------------------------------
    // Possessive clitics.
    // -----------------------------------------------------------------
    ("کتابام", "کتاب"),  // book + 1sg possessive
    ("کتابای", "کتاب"),  // book + 2sg possessive
    ("کتاباش", "کتاب"),  // book + 3sg possessive
    ("کتابمان", "کتاب"), // book + 1pl possessive
    ("کتابتان", "کتاب"), // book + 2pl possessive
    ("کتابشان", "کتاب"), // book + 3pl possessive
    // -----------------------------------------------------------------
    // No affix — pass through unchanged.
    // -----------------------------------------------------------------
    ("کتاب", "کتاب"),
    ("خانه", "خانه"),
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("ها", "ها"), // suffix alone — stripping would leave 0 chars, guarded
    ("ک", "ک"),   // shorter than 2 chars — length short-circuit
    // -----------------------------------------------------------------
    // Non-Persian input — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_persian_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightPersianStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightPersianStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Persian stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "15+ pairs".
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}
