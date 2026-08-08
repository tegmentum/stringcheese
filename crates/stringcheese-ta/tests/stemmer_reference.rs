//! Light Tamil stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! suffix table (see [`stringcheese_ta::stemmer`] module docs). The
//! stemmer strips one suffix per call, longest-match-wins, with the
//! over-strip safeguard rolling back any strip that would leave
//! fewer than 2 characters (scalars — Tamil letters are 3 bytes
//! each, so "2 characters" is typically 6 bytes).

extern crate alloc;

use stringcheese_ta::LightTamilStemmer;

/// Reference pairs. 15+ pairs covering the stemmer's rule categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Plural marker -கள்.
    // -----------------------------------------------------------------
    ("மாணவர்கள்", "மாணவர்"), // students — strip கள்
    // -----------------------------------------------------------------
    // Case markers (matra forms after consonants).
    // -----------------------------------------------------------------
    ("வீட்டில்", "வீட்ட"),     // in the house — strip ில்
    ("மரத்தால்", "மரத்த"),    // by the tree — strip ால்
    ("அவனுக்கு", "அவனுக்"), // to him — strip கு
    ("நண்பனின்", "நண்பன"),   // of the friend — strip ின்
    // -----------------------------------------------------------------
    // Verb personal endings (present).
    // -----------------------------------------------------------------
    ("பேசுகிறேன்", "பேசு"),  // I speak — strip கிறேன்
    ("பேசுகிறாய்", "பேசு"),   // you speak — strip கிறாய்
    ("பேசுகிறார்", "பேசு"),   // he/she polite present — strip கிறார்
    ("பேசுகிறோம்", "பேசு"),  // we speak — strip கிறோம்
    ("பேசுகிறார்கள்", "பேசு"), // they speak — strip கிறார்கள்
    ("பேசுகிறீர்கள்", "பேசு"), // you plural speak — strip கிறீர்கள்
    // -----------------------------------------------------------------
    // Longest-match wins: கிறார்கள் (9) beats bare கள் (3).
    // -----------------------------------------------------------------
    ("பேசுகிறார்கள்", "பேசு"),
    // -----------------------------------------------------------------
    // Bare nouns — no suffix, pass through.
    // -----------------------------------------------------------------
    ("தமிழ்", "தமிழ்"),
    ("நான்", "நான்"),
    // -----------------------------------------------------------------
    // Over-strip guard / short input.
    // -----------------------------------------------------------------
    ("கள்", "கள்"), // suffix alone — stripping would leave 0 chars, guarded
    ("கு", "கு"), // 2-scalar dative alone — guarded
    // "வரை" — the 3-scalar allative -வரை is guarded (0-char stem),
    // then falls through to the 1-char accusative matra -ை which
    // leaves 2 scalars ("வர"). The 2-scalar result passes the
    // MIN_STEM_CHARS floor exactly.
    ("வரை", "வர"),
    // -----------------------------------------------------------------
    // Non-Tamil — no-op.
    // -----------------------------------------------------------------
    ("hello", "hello"),
];

#[test]
fn light_tamil_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = LightTamilStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  LightTamilStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Tamil stemmer reference pair(s) disagreed:\n{}",
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
