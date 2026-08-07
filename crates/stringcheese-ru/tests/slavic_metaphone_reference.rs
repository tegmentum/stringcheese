//! Cross-Slavic Metaphone reference pairs for the Russian pack.
//!
//! The pairs below assert that
//! [`RUSSIAN_WITH_SLAVIC_METAPHONE`](stringcheese_ru::RUSSIAN_WITH_SLAVIC_METAPHONE)
//! hashes Russian Cyrillic renderings and their Latin transliterations
//! to the same [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//! key — the whole point of enabling the shared cross-Slavic encoder
//! in the Russian pack.
//!
//! Compiled only when the crate's `slavic-metaphone` Cargo feature is
//! enabled; without the feature the pack does not carry the
//! `RUSSIAN_WITH_SLAVIC_METAPHONE` constant and this test file is a
//! no-op.

#![cfg(feature = "slavic-metaphone")]

extern crate alloc;

use stringcheese_lang::Language;
use stringcheese_ru::RUSSIAN_WITH_SLAVIC_METAPHONE;

/// Reference pairs `(label, cyrillic, latin)` that must produce the
/// same Slavic-Metaphone key under the Russian pack's opt-in encoder.
///
/// Chosen for well-established Russian ↔ Latin equivalences: common
/// place names and surnames where the Latin transliteration is
/// widely used. The set purposely mixes GOST-B / BGN-PCGN / passport
/// transliteration styles because the Slavic-Metaphone encoder
/// collapses across all of them.
const CROSS_SCRIPT_PAIRS: &[(&str, &str, &str)] = &[
    // Task-mandated pair: Чехов ↔ Chekhov (surname).
    ("Chekhov", "Чехов", "Chekhov"),
    // Common place-names — pronunciation-preserving Latin renderings.
    ("Moscow", "Москва", "Moskva"),
    ("Volga", "Волга", "Volga"),
    ("Novgorod", "Новгород", "Novgorod"),
    ("Peter", "Пётр", "Petr"),
    ("Petersburg", "Санкт-Петербург", "Sankt-Peterburg"),
    // The `kh` digraph on the Latin side, matching `х` on the
    // Cyrillic side — one of the encoder's targeted collapses.
    ("Kharkov", "Харьков", "Kharkov"),
    // A common surname that exercises the `sh` / `ш` collapse.
    ("Pushkin", "Пушкин", "Pushkin"),
    // A common surname that exercises the `ts` / `ц` collapse.
    ("Tsar", "царь", "tsar"),
    // Common noun where both Cyrillic and Latin start on a shared
    // consonant class (M in this case), exercising the same-class
    // skeleton after the vowel drop.
    ("Moloko", "Молоко", "Moloko"),
    // Common name — the `ch` digraph on the Latin side reads /tʃ/
    // (class C), matching Cyrillic `ч`.
    ("Ivan Chekhov", "Иван Чехов", "Ivan Chekhov"),
];

#[test]
fn cross_script_pairs_produce_matching_keys() {
    let enc = RUSSIAN_WITH_SLAVIC_METAPHONE
        .phonetic_encoder()
        .expect("Russian slavic-metaphone pack ships a phonetic encoder");
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
    // encoder against regressions in adjacent transliteration
    // conventions.
    assert!(
        CROSS_SCRIPT_PAIRS.len() >= 8,
        "cross-script pair count {} is below the 8-pair floor",
        CROSS_SCRIPT_PAIRS.len()
    );
}
