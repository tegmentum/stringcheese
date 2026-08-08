//! Light Marathi stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_mr::stemmer`] module docs). The
//! stemmer's internal loop iterates to convergence, so stacked
//! suffixes (an agglutinative case marker plus its underlying
//! linking vowel, or a plural marker plus a case marker) reduce in
//! a single external call.
//!
//! Marathi's agglutinative morphology makes the stemmer genuinely
//! useful — unlike Hindi, where postpositions are separate tokens
//! that the tokenizer has already split off, Marathi surface words
//! carry their case suffixes attached, so the stemmer's suffix table
//! sees them.

extern crate alloc;

use stringcheese_mr::LightMarathiStemmer;

/// Reference pairs. 20+ pairs covering the stemmer's rule categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Agglutinative case markers.
    // -----------------------------------------------------------------
    ("घराला", "घर"),  // to the house (dative)
    ("मुलाला", "मुल"),  // to the boy (dative)
    ("मुलाने", "मुल"),   // by the boy (instrumental)
    ("मुलाचा", "मुल"),  // of the boy (genitive m. sg.)
    ("मुलाची", "मुल"),  // of the boy (genitive f. sg.)
    ("मुलाचे", "मुल"),   // of the boy (genitive n. sg. / m. pl.)
    ("घरामध्ये", "घर"), // in the house (locative -मध्ये)
    ("घराहून", "घर"),  // from the house (ablative)
    // -----------------------------------------------------------------
    // Verb personal endings — present habitual.
    // -----------------------------------------------------------------
    ("बोलतो", "बोल"),  // I/he speak(s), m.
    ("बोलते", "बोल"),   // I/she speak(s), f./n.
    ("बोलतोस", "बोल"), // you speak, m. informal
    ("बोलतात", "बोल"), // they speak
    // -----------------------------------------------------------------
    // Verb personal endings — aorist / perfective.
    // -----------------------------------------------------------------
    ("बोलला", "बोल"),  // he spoke (3sg-m aorist)
    ("बोलली", "बोल"),  // she spoke (3sg-f aorist)
    ("बोलले", "बोल"),   // it spoke / they-m spoke (3sg-n / 3pl-m)
    ("बोलल्या", "बोल"), // they-f spoke (3pl-f aorist -ल्या)
    // -----------------------------------------------------------------
    // Infinitive.
    // -----------------------------------------------------------------
    ("करणे", "कर"), // to do
    // -----------------------------------------------------------------
    // Plural markers and stacked suffixes — convergence loop.
    // -----------------------------------------------------------------
    ("घरां", "घर"),    // houses (oblique plural, strip -ां)
    ("घरांना", "घर"),  // to the houses (plural + dative — stacked)
    ("मुलांच्या", "मुल"), // of the boys (genitive with oblique-pl agreement)
    // -----------------------------------------------------------------
    // Bare noun — no suffix, pass through.
    // -----------------------------------------------------------------
    ("पुस्तक", "पुस्तक"), // book (no suffix)
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("ला", "ला"), // suffix alone — stripping ला leaves 0 chars, guarded
    ("त", "त"),   // single-scalar alone — length short-circuit
    // -----------------------------------------------------------------
    // Non-Devanagari — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_marathi_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightMarathiStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightMarathiStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Marathi stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for 15+ pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}
