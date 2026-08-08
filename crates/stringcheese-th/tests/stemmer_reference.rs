//! Thai stemmer reference input/output pairs.
//!
//! Each pair below has been traced by hand through the stemmer's
//! near-identity rule set (see [`stringcheese_th::stemmer`] module
//! docs). The stemmer either strips one of five nominalizer / agent
//! prefixes (`การ- ความ- ผู้- นัก- เครื่อง-`), folds exact
//! word-level reduplication (`XX → X`), or returns the input
//! unchanged.
//!
//! Thai has no inflection: no case, no plural, no verb-tense
//! endings. The stemmer is deliberately conservative — everything
//! else passes through as identity.

extern crate alloc;

use stringcheese_th::ThaiStemmer;

/// Reference pairs. 15+ pairs covering the stemmer's rule categories.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Verbal nominalizer การ-.
    // -----------------------------------------------------------------
    ("การเรียน", "เรียน"), // learning → to learn
    ("การพูด", "พูด"),     // speaking → to speak
    ("การกิน", "กิน"),     // eating → to eat
    // -----------------------------------------------------------------
    // Abstract nominalizer ความ-.
    // -----------------------------------------------------------------
    ("ความสุข", "สุข"), // happiness → happy
    ("ความรัก", "รัก"), // love → to love
    ("ความดี", "ดี"),   // goodness → good
    // -----------------------------------------------------------------
    // Agent prefix ผู้-.
    // -----------------------------------------------------------------
    ("ผู้พูด", "พูด"),   // speaker → to speak
    ("ผู้ชาย", "ชาย"), // man (male person) → male
    // -----------------------------------------------------------------
    // Practitioner prefix นัก-.
    // -----------------------------------------------------------------
    ("นักเรียน", "เรียน"), // student → to learn
    ("นักกีฬา", "กีฬา"),   // athlete → sport
    // -----------------------------------------------------------------
    // Instrument prefix เครื่อง-.
    // -----------------------------------------------------------------
    ("เครื่องบิน", "บิน"), // airplane → to fly
    ("เครื่องดื่ม", "ดื่ม"), // beverage → to drink
    // -----------------------------------------------------------------
    // Word-level reduplication (XX → X).
    // -----------------------------------------------------------------
    ("เด็กเด็ก", "เด็ก"), // children → child
    ("ดีดี", "ดี"),       // very good → good
    // -----------------------------------------------------------------
    // Bare content words — identity.
    // -----------------------------------------------------------------
    ("สวัสดี", "สวัสดี"),     // hello
    ("ประเทศ", "ประเทศ"), // country
    ("ไทย", "ไทย"),       // Thai
    // -----------------------------------------------------------------
    // Non-Thai — identity.
    // -----------------------------------------------------------------
    ("hello", "hello"),
    // -----------------------------------------------------------------
    // Over-strip guard.
    // -----------------------------------------------------------------
    ("ผู้", "ผู้"), // prefix alone — stripping leaves 0 chars, guarded
];

#[test]
fn thai_stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = ThaiStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  ThaiStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Thai stemmer reference pair(s) disagreed:\n{}",
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
