//! Cross-Slavic Metaphone reference pairs for the Serbian pack.
//!
//! The pairs below assert that
//! [`SERBIAN_WITH_SLAVIC_METAPHONE`](stringcheese_sr::SERBIAN_WITH_SLAVIC_METAPHONE)
//! hashes Serbian Cyrillic and Serbian Latin renderings to the same
//! [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone) key —
//! the whole point of enabling the shared cross-Slavic encoder in the
//! Serbian pack (on top of the bijective Cyrillic ↔ Latin
//! transliteration the default `SerbianLatin` encoder already
//! provides).
//!
//! Compiled only when the crate's `slavic-metaphone` Cargo feature is
//! enabled; without the feature the pack does not carry the
//! `SERBIAN_WITH_SLAVIC_METAPHONE` constant and this test file is a
//! no-op.

#![cfg(feature = "slavic-metaphone")]

extern crate alloc;

use stringcheese_lang::Language;
use stringcheese_sr::SERBIAN_WITH_SLAVIC_METAPHONE;

/// Reference pairs `(label, cyrillic, latin)` that must produce the
/// same Slavic-Metaphone key under the Serbian pack's opt-in encoder.
///
/// Chosen from common Serbian names and places whose Cyrillic
/// (Vukovica) and Latin (Gaj) spellings are both in wide use. The
/// bijective script conversion means most pairs already collapse
/// through the default `SerbianLatin` encoder — this test asserts
/// the cross-Slavic encoder also collapses them (plus the Latin
/// forms of Russian / Ukrainian names common in the Balkans).
const CROSS_SCRIPT_PAIRS: &[(&str, &str, &str)] = &[
    // Task-mandated pair: Чехов ↔ Chekhov (Russian surname commonly
    // written in Serbian sources under either script).
    ("Chekhov", "Чехов", "Chekhov"),
    // Belgrade — the pack's flagship dual-script example.
    ("Belgrade", "Београд", "Beograd"),
    // Serbian personal names in both scripts.
    ("Nikola", "Никола", "Nikola"),
    ("Petar", "Петар", "Petar"),
    ("Milan", "Милан", "Milan"),
    // Serbian surname exercising `č`/`ч` collapse (both in class C).
    ("Vukovic", "Вуковић", "Vuković"),
    // Postalveolar affricate exercise: `dž` ↔ `џ` (class C).
    ("Djordje", "Ђорђе", "Đorđe"),
    // Common place-name — cross-script + vowel-drop skeleton.
    ("NoviSad", "Нови Сад", "Novi Sad"),
];

#[test]
fn cross_script_pairs_produce_matching_keys() {
    let enc = SERBIAN_WITH_SLAVIC_METAPHONE
        .phonetic_encoder()
        .expect("Serbian slavic-metaphone pack ships a phonetic encoder");
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
