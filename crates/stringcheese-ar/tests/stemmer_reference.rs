//! Larkey ALP light10 stemmer reference input/output pairs.
//!
//! The pairs below trace the light10 algorithm as documented in Larkey,
//! Ballesteros, Connell 2002, *"Improving Stemming for Arabic
//! Information Retrieval: Light Stemming and Co-occurrence Analysis"*
//! (SIGIR 2002), and further re-implemented across Lucene's
//! `ArabicStemmer` and Snowball's `arabic_stemmer.sbl`. Each pair has
//! been traced by hand through the algorithm's two passes (one prefix
//! strip, one suffix strip, longest-match-wins in each pass, with the
//! over-strip safeguard rolling back any strip that would leave fewer
//! than 2 characters).
//!
//! Normalization is *not* applied to the reference inputs — the
//! stemmer is being tested in isolation, so every input is already in
//! its canonical unvoweled form (no harakat, plain alef, plain yeh).

extern crate alloc;

use stringcheese_ar::Light10;

/// 26 reference pairs.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Prefix stripping only.
    // -----------------------------------------------------------------
    ("الكتاب", "كتاب"),  // al- + book
    ("والكتاب", "كتاب"), // wa-al- + book
    ("فالكتاب", "كتاب"), // fa-al- + book
    ("بالكتاب", "كتاب"), // bi-al- + book
    ("كالكتاب", "كتاب"), // ka-al- + book
    ("وكتاب", "كتاب"),   // wa- + book
    // -----------------------------------------------------------------
    // Suffix stripping only.
    // -----------------------------------------------------------------
    ("كتابها", "كتاب"), // book + -ha (her book)
    ("كتابان", "كتاب"), // book + -an (dual)
    ("طالبات", "طالب"), // student(fem) + -at (fem plural)
    ("معلمون", "معلم"), // teacher + -un (masc plural nom)
    ("معلمين", "معلم"), // teacher + -in (masc plural acc/gen)
    ("كتابه", "كتاب"),  // book + -h (his book)
    ("كتابي", "كتاب"),  // book + -y (my book)
    ("طالبة", "طالب"),  // student + -h (teh marbuta)
    ("طالبية", "طالب"), // student + -yh (feminine adjective)
    // -----------------------------------------------------------------
    // Combined prefix + suffix stripping.
    // -----------------------------------------------------------------
    ("الطالبات", "طالب"),  // al- + student + -at
    ("والطالبات", "طالب"), // wa-al- + student + -at
    ("بالمعلمين", "معلم"), // bi-al- + teacher + -in
    ("والكتابان", "كتاب"), // wa-al- + book + -an (dual)
    // -----------------------------------------------------------------
    // Words too short / no affix — pass through unchanged.
    // -----------------------------------------------------------------
    ("كتاب", "كتاب"),
    ("طالب", "طالب"),
    ("علم", "علم"),
    // -----------------------------------------------------------------
    // Over-strip guard fires — original word preserved.
    // -----------------------------------------------------------------
    ("الن", "الن"), // "ال" strip would leave 1-char stem — rolled back
    ("و", "و"),     // shorter than 2 chars — length short-circuit
    ("ال", "ال"),   // stripping "ال" would leave "" — guarded
    // -----------------------------------------------------------------
    // Non-Arabic input — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light10_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = Light10.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Light10({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Larkey light10 reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "20+ pairs". Verify we're above that.
    assert!(
        PAIRS.len() >= 20,
        "reference pair count {} is below the 20-pair floor",
        PAIRS.len()
    );
}
