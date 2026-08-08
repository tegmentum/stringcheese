//! Light Malayalam stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_ml::stemmer`] module docs). The
//! stemmer strips one suffix per call, longest-match-wins, with the
//! over-strip safeguard rolling back any strip that would leave
//! fewer than 2 characters (scalars — Malayalam letters are 3 bytes
//! each, so "2 characters" is typically 6 bytes).

extern crate alloc;

use stringcheese_ml::LightMalayalamStemmer;

/// Reference pairs. 15+ pairs covering the stemmer's rule categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Plural markers.
    // -----------------------------------------------------------------
    ("പുസ്തകങ്ങൾ", "പുസ്തക"),  // books — strip -ങ്ങൾ
    ("വീടുകൾ", "വീടു"),     // houses — strip -കൾ
    ("ടീച്ചർമാർ", "ടീച്ചർ"), // teachers — strip -മാർ
    // -----------------------------------------------------------------
    // Case markers.
    // -----------------------------------------------------------------
    ("വീട്ടിൽ", "വീട്ട"), // in the house — strip -ിൽ
    ("രാമന്റെ", "രാമ"),    // of Rama — strip -ന്റെ
    ("അവനെ", "അവ"),     // him (accusative) — strip -നെ
    ("രാമന്", "രാമ"),      // to Rama (dative) — strip -ന്
    ("രാമനോട്", "രാമന"),   // to/with Rama (sociative) — strip -ോട്
    ("അവരുടെ", "അവര"),   // their — strip -ുടെ
    // -----------------------------------------------------------------
    // Verb endings.
    // -----------------------------------------------------------------
    ("പഠിക്കുന്നു", "പഠിക്ക"), // studies (present) — strip -ുന്നു
    ("പഠിക്കുന്ന", "പഠിക്ക"), // studying (adjectival) — strip -ുന്ന
    ("പഠിക്കുക", "പഠിക്ക"),  // to study (infinitive) — strip -ുക
    // -----------------------------------------------------------------
    // Emphatic / future particle.
    // -----------------------------------------------------------------
    ("രാമനും", "രാമന"), // Rama also — strip -ും
    // -----------------------------------------------------------------
    // Longest-match wins: -ുന്നു (5) beats -ുന്ന (4).
    // -----------------------------------------------------------------
    ("പഠിക്കുന്നു", "പഠിക്ക"),
    // -----------------------------------------------------------------
    // Chillu-ending words — bare chillu is NOT a suffix.
    // -----------------------------------------------------------------
    ("ഞാൻ", "ഞാൻ"),   // I — ends in chillu ൻ, not stripped
    ("അവർ", "അവർ"), // they — ends in chillu ർ, not stripped
    ("അവൾ", "അവൾ"), // she — ends in chillu ൾ, not stripped
    // -----------------------------------------------------------------
    // Bare nouns — no suffix, pass through.
    // -----------------------------------------------------------------
    ("മലയാളം", "മലയാളം"),
    ("രാമ", "രാമ"),
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("കൾ", "കൾ"), // suffix alone — stripping would leave 0 chars, guarded
    ("ുക", "ുക"),   // 2-scalar infinitive alone — guarded
    ("ോ", "ോ"),   // 1-scalar interrogative — MIN_STEM_CHARS short-circuit
    // -----------------------------------------------------------------
    // Non-Malayalam — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_malayalam_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightMalayalamStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightMalayalamStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Malayalam stemmer reference pair(s) disagreed:\n{}",
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
