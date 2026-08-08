//! Light Hindi stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_hi::stemmer`] module docs). The
//! stemmer strips one suffix per call, longest-match-wins, with the
//! over-strip safeguard rolling back any strip that would leave
//! fewer than 2 characters (scalars — Devanagari letters are 3 bytes
//! each, so "2 characters" is typically 6 bytes).
//!
//! Normalization is *not* applied to the reference inputs — the
//! stemmer is being tested in isolation, so every input is in its
//! surface-form Devanagari spelling.

extern crate alloc;

use stringcheese_hi::LightHindiStemmer;

/// Reference pairs. 20+ pairs covering the stemmer's rule categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Gender / number markers — the most common Hindi suffixes.
    // -----------------------------------------------------------------
    ("लड़का", "लड़क"), // boy (m. sg.) — strip ा
    ("लड़की", "लड़क"), // girl (f. sg.) — strip ी
    ("लड़के", "लड़क"),  // boys (m. pl. / oblique) — strip े
    ("लड़कों", "लड़क"), // boys (oblique plural) — strip ों (beats bare े)
    ("बच्चा", "बच्च"), // child (m. sg.)
    ("बच्ची", "बच्च"), // child (f. sg.)
    ("बच्चे", "बच्च"),  // children (m. pl.)
    ("बच्चों", "बच्च"), // children (oblique)
    // -----------------------------------------------------------------
    // Verb tense markers.
    // -----------------------------------------------------------------
    ("जाता", "जा"), // goes (m. sg. habitual) — strip ता
    ("जाती", "जा"), // goes (f. sg. habitual) — strip ती
    ("जाते", "जा"),  // go (m. pl. habitual) — strip ते
    // "आता" (comes, m. sg. habitual) is आ + त + ा — the ता suffix
    // matches but the stem "आ" is 1 char and the min-stem-2 guard
    // rolls it back; the fallback ा match leaves "आत" (2 chars),
    // which meets the guard. Documented as an over-conservative
    // over-strip.
    ("आता", "आत"),
    ("आती", "आत"),
    ("खाया", "खा"), // ate (m. sg. perfective) — strip या
    // -----------------------------------------------------------------
    // Longest-match wins.
    // -----------------------------------------------------------------
    ("घरों", "घर"), // houses (oblique) — ों beats е
    // -----------------------------------------------------------------
    // Bare noun — no suffix, pass through.
    // -----------------------------------------------------------------
    ("किताब", "किताब"), // book (no suffix)
    ("पानी", "पान"),    // water — strip ी? actually stripping ी would
    // leave 3 chars पान which is >= 2. ✓
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("का", "का"), // postposition alone — stripping का would leave 0 chars, guarded
    ("ी", "ी"),   // single-scalar matra alone — length short-circuit
    // -----------------------------------------------------------------
    // Non-Devanagari — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_hindi_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightHindiStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightHindiStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Hindi stemmer reference pair(s) disagreed:\n{}",
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
