//! Cross-Slavic Metaphone reference pairs for the Ukrainian pack.
//!
//! The pairs below assert that
//! [`UKRAINIAN_WITH_SLAVIC_METAPHONE`](stringcheese_uk::UKRAINIAN_WITH_SLAVIC_METAPHONE)
//! hashes Ukrainian Cyrillic renderings and their Latin
//! transliterations to the same
//! [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone) key —
//! the whole point of enabling the shared cross-Slavic encoder in the
//! Ukrainian pack.
//!
//! Compiled only when the crate's `slavic-metaphone` Cargo feature is
//! enabled; without the feature the pack does not carry the
//! `UKRAINIAN_WITH_SLAVIC_METAPHONE` constant and this test file is a
//! no-op.

#![cfg(feature = "slavic-metaphone")]

extern crate alloc;

use stringcheese_lang::Language;
use stringcheese_uk::UKRAINIAN_WITH_SLAVIC_METAPHONE;

/// Reference pairs `(label, cyrillic, latin)` that must produce the
/// same Slavic-Metaphone key under the Ukrainian pack's opt-in
/// encoder.
///
/// Chosen for well-established Ukrainian ↔ Latin equivalences: common
/// place names and surnames where the Latin transliteration is
/// widely used. Includes the task-mandated Чехов ↔ Chekhov pair —
/// the shared cross-Slavic encoder must hash a name identically no
/// matter which pack encodes it.
const CROSS_SCRIPT_PAIRS: &[(&str, &str, &str)] = &[
    // Task-mandated pair: Чехов ↔ Chekhov (surname; borrows a
    // Russian surname but the Ukrainian pack's Slavic-Metaphone key
    // must still match the Latin transliteration).
    ("Chekhov", "Чехов", "Chekhov"),
    // Ukrainian-specific place-names.
    ("Kharkiv", "Харків", "Kharkiv"),
    // Common surname exercising `sh` ↔ `ш` and consonant collapse.
    ("Shevchenko", "Шевченко", "Shevchenko"),
    // Common surname with `ch` reading /tʃ/ on both sides.
    ("Chornobyl", "Чорнобиль", "Chornobyl"),
    // Common noun exercising the `kh` ↔ `х` digraph.
    ("Khata", "Хата", "Khata"),
    // A common name that exercises the `p`/`п` mapping and later
    // consonants after the vowel drop.
    ("Petro", "Петро", "Petro"),
    // Common surname — matches through the Slavic-Metaphone
    // vowel-drop rule.
    ("Bondarenko", "Бондаренко", "Bondarenko"),
    // Common name — matches through the Slavic-Metaphone consonant
    // skeleton after vowel drop.
    ("Melnyk", "Мельник", "Melnyk"),
];

#[test]
fn cross_script_pairs_produce_matching_keys() {
    let enc = UKRAINIAN_WITH_SLAVIC_METAPHONE
        .phonetic_encoder()
        .expect("Ukrainian slavic-metaphone pack ships a phonetic encoder");
    let mut failures = alloc::vec::Vec::new();
    for &(label, cyr, lat) in CROSS_SCRIPT_PAIRS {
        let Some((cyr_key, _)) = enc.encode(cyr) else {
            failures.push(alloc::format!(
                "  {label}: encoder returned None for Cyrillic {cyr:?}"
            ));
            continue;
        };
        let Some((lat_key, _)) = enc.encode(lat) else {
            failures.push(alloc::format!(
                "  {label}: encoder returned None for Latin {lat:?}"
            ));
            continue;
        };
        if cyr_key != lat_key {
            failures.push(alloc::format!(
                "  {label}: Cyrillic {cyr:?} = {cyr_key:?}, Latin {lat:?} = {lat_key:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cross-script pairs disagreed:\n{}",
        failures.len(),
        CROSS_SCRIPT_PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn cross_script_pair_count_meets_the_task_floor() {
    // The task-level floor is 1 mandated pair (Чехов ↔ Chekhov); the
    // reference set carries an order of magnitude more to guard the
    // encoder against regressions.
    assert!(
        CROSS_SCRIPT_PAIRS.len() >= 8,
        "cross-script pair count {} is below the 8-pair floor",
        CROSS_SCRIPT_PAIRS.len()
    );
}
