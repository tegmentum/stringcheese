//! Light Hebrew stemmer reference input/output pairs.
//!
//! The pairs below trace the light-stemmer's two-pass prefix/suffix
//! stripping (one prefix strip, one suffix strip, longest-match-wins
//! in each pass, with the over-strip safeguard rolling back any strip
//! that would leave fewer than 2 characters).
//!
//! Normalization is *not* applied to the reference inputs — the
//! stemmer is being tested in isolation, so every input is already in
//! its canonical unpointed form (no niqqud, no cantillation).

extern crate alloc;

use stringcheese_he::LightHebrewStemmer;

/// 24 reference pairs.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Prefix stripping only.
    // -----------------------------------------------------------------
    ("הספר", "ספר"),  // ha- + book
    ("והספר", "ספר"), // wa-ha- + book
    ("בהספר", "ספר"), // ba-ha- + book
    ("כהספר", "ספר"), // ka-ha- + book
    ("להספר", "ספר"), // la-ha- + book
    ("שהספר", "ספר"), // sha-ha- + book
    ("וספר", "ספר"),  // and- + book
    ("בספר", "ספר"),  // in- + book
    ("לספר", "ספר"),  // to- + book
    ("שספר", "ספר"),  // that- + book
    // -----------------------------------------------------------------
    // Suffix stripping only.
    // -----------------------------------------------------------------
    ("ספרים", "ספר"), // book + -im (masc plural)
    ("ילדות", "ילד"), // child + -ot (fem plural)
    ("ילדה", "ילד"),  // child + -h (fem sg)
    ("ספרי", "ספר"),  // book + -i (my)
    ("ספרו", "ספר"),  // book + -w (his)
    ("ספרנו", "ספר"), // book + -nu (our)
    ("אמרתי", "אמר"), // said + -ti (I-past). See stemmer.rs's
    // "ambiguous single-letter prefix" note for
    // why `אמרתי` and not `כתבתי` — the light
    // stemmer over-strips the `כ` in the latter.
    ("ספרהם", "ספר"), // book + -hem (their-m)
    // -----------------------------------------------------------------
    // Combined prefix + suffix stripping.
    // -----------------------------------------------------------------
    ("הילדים", "ילד"),  // ha- + child + -im
    ("והילדות", "ילד"), // wa-ha- + child + -ot
    ("ובספרים", "ספר"), // wa-b- + book + -im
    // -----------------------------------------------------------------
    // Words too short / no affix — pass through unchanged. All chosen
    // to NOT begin with a prefix letter (`ה ו ב כ ל מ ש`) so the
    // ambiguous-prefix over-strip does not fire; see stemmer.rs's
    // module-level "ambiguous single-letter prefix" note.
    // -----------------------------------------------------------------
    ("ספר", "ספר"),
    ("אור", "אור"),
    ("יום", "יום"),
    ("ה", "ה"),
    // -----------------------------------------------------------------
    // Non-Hebrew input — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_hebrew_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightHebrewStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightHebrewStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} light-Hebrew reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "at least 15 pairs". Verify we're above
    // that floor.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}
