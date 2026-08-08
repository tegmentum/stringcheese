//! Light Punjabi stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_pa::stemmer`] module docs). The
//! stemmer strips one suffix per call, longest-match-wins, with the
//! over-strip safeguard rolling back any strip that would leave
//! fewer than 2 characters (scalars — Gurmukhi letters are 3 bytes
//! each, so "2 characters" is 6 bytes).

extern crate alloc;

use stringcheese_pa::LightPunjabiStemmer;

/// Reference pairs. 15+ pairs covering the stemmer's rule categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Plural markers.
    // -----------------------------------------------------------------
    ("ਘਰਾਂ", "ਘਰ"),  // houses — strip ਾਂ
    ("ਬੱਚਿਆਂ", "ਬੱਚ"), // children (obl pl) — strip ਿਆਂ
    ("ਕੁੜੀਆਂ", "ਕੁੜ"), // girls — strip ੀਆਂ (normalizes with singular)
    ("ਮੁੰਡਿਆਂ", "ਮੁੰਡ"), // boys (obl pl) — strip ਿਆਂ
    // -----------------------------------------------------------------
    // Case markers.
    // -----------------------------------------------------------------
    ("ਮੁੰਡੇ", "ਮੁੰਡ"),  // of the boy / boys direct — strip ੇ
    ("ਕੁੜੀ", "ਕੁੜ"), // girl (fem sg) — strip ੀ
    // -----------------------------------------------------------------
    // Imperfective participle endings.
    // -----------------------------------------------------------------
    ("ਬੋਲਦਾ", "ਬੋਲ"), // speaks, masc sg — strip ਦਾ
    ("ਬੋਲਦੀ", "ਬੋਲ"), // speaks, fem sg — strip ਦੀ
    ("ਬੋਲਦੇ", "ਬੋਲ"),  // speak, masc pl — strip ਦੇ
    // -----------------------------------------------------------------
    // Perfective / aorist endings.
    // -----------------------------------------------------------------
    ("ਬੋਲਿਆ", "ਬੋਲ"), // spoke, 3sg-m — strip ਿਆ
    ("ਬੋਲੀ", "ਬੋਲ"),  // spoke, 3sg-f — strip ੀ
    ("ਬੋਲੇ", "ਬੋਲ"),   // spoke, 3pl-m — strip ੇ
    ("ਬੋਲੀਆਂ", "ਬੋਲ"), // spoke, 3pl-f — strip ੀਆਂ
    // -----------------------------------------------------------------
    // Longest-match wins: ੀਆਂ beats bare ੀ or ਾਂ.
    // -----------------------------------------------------------------
    ("ਕੁੜੀਆਂ", "ਕੁੜ"),
    // -----------------------------------------------------------------
    // Bare noun — no suffix, pass through.
    // -----------------------------------------------------------------
    ("ਪੰਜਾਬ", "ਪੰਜਾਬ"),
    ("ਘਰ", "ਘਰ"),
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("ਦਾ", "ਦਾ"), // 2-char suffix alone — stripping leaves 0, guarded
    ("ਹਾਂ", "ਹਾਂ"), // stripping ਾਂ leaves 1 scalar → guarded
    ("ੇ", "ੇ"),     // 1-scalar input — length short-circuit
    // -----------------------------------------------------------------
    // Tippi / bindi / addak never touched on their own.
    // -----------------------------------------------------------------
    ("ਹੈਂ", "ਹੈਂ"),     // trailing bindi not in suffix table
    ("ਪੱਕਾ", "ਪੱਕਾ"), // stem ends in ਾ but no ਦਾ prefix to trigger match
    // -----------------------------------------------------------------
    // Non-Punjabi — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_punjabi_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightPunjabiStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightPunjabiStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Punjabi stemmer reference pair(s) disagreed:\n{}",
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
