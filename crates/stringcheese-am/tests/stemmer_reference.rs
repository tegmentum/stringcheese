//! Light Amharic stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_am::stemmer`] module docs). The
//! stemmer iterates to convergence, longest-match-wins per pass,
//! with the over-strip safeguard rolling back any strip that would
//! leave fewer than 2 characters (scalars — Ge'ez letters are 3
//! bytes each, so "2 characters" is 6 bytes for pure-Amharic input).

extern crate alloc;

use stringcheese_am::LightAmharicStemmer;

/// Reference pairs. 15+ pairs covering the stemmer's rule
/// categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Definite article.
    // -----------------------------------------------------------------
    ("ቤትው", "ቤት"), // the house — strip -ው
    ("ልጅዋ", "ልጅ"), // the daughter — strip -ዋ
    // -----------------------------------------------------------------
    // Plural.
    // -----------------------------------------------------------------
    ("ልጅኦች", "ልጅ"),       // children — strip -ኦች
    ("ቤትኦች", "ቤት"),       // houses — strip -ኦች
    ("ኢትዮጵያኦች", "ኢትዮጵያ"), // Ethiopians — strip -ኦች
    // -----------------------------------------------------------------
    // Possessive suffixes.
    // -----------------------------------------------------------------
    ("ቤትዬ", "ቤት"),  // my house — strip -ዬ
    ("ቤትችን", "ቤት"), // our house — strip -ችን
    ("ቤትችሁ", "ቤት"), // your (pl.) house — strip -ችሁ
    ("ቤትችው", "ቤት"), // their house — strip -ችው
    // -----------------------------------------------------------------
    // Object suffixes.
    // -----------------------------------------------------------------
    ("ቤትኣቸው", "ቤት"), // ... them — strip -ኣቸው (3-scalar, longest)
    ("አየኝ", "አየ"),   // ... saw me — strip -ኝ
    // -----------------------------------------------------------------
    // Longest-match wins: -ኣቸው (3-scalar) beats -ችው (2-scalar) beats
    // bare -ው. All three would in principle match the tail of the
    // -ኣቸው-ended word, but only the 3-scalar entry strips it cleanly.
    // -----------------------------------------------------------------
    ("አማርኛኣቸው", "አማርኛ"),
    // -----------------------------------------------------------------
    // Iterate-to-convergence: stacked plural + possessive.
    // -----------------------------------------------------------------
    ("ልጅኦችችን", "ልጅ"), // synthetic plural + our → strip -ችን then -ኦች
    // -----------------------------------------------------------------
    // Bare noun — no suffix, pass through.
    // -----------------------------------------------------------------
    ("አማርኛ", "አማርኛ"),
    ("ኢትዮጵያ", "ኢትዮጵያ"),
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("ው", "ው"),   // single-scalar alone — length short-circuit
    ("ችን", "ችን"), // suffix alone — stripping leaves 0 chars, guarded
    // -----------------------------------------------------------------
    // Non-Amharic — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_amharic_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightAmharicStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightAmharicStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Amharic stemmer reference pair(s) disagreed:\n{}",
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
