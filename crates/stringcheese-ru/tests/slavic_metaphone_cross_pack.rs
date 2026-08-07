//! Cross-pack Slavic-Metaphone equivalence: the same encoder wired
//! into three different language packs must produce the same key for
//! the same input.
//!
//! The [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//! encoder lives in `stringcheese-phonetic` and is exposed under each
//! Slavic pack's `_WITH_SLAVIC_METAPHONE` constant through an adapter
//! that wraps the same underlying algorithm. This test file exists to
//! guarantee those adapters agree — a regression in any pack's
//! adapter (an accidental option override, a stale copy of the
//! encoder, a divergent `LanguagePhoneticEncoder::name`) would show
//! up here first.
//!
//! Compiled only when the crate's `slavic-metaphone` Cargo feature is
//! enabled; without the feature the pack does not carry the
//! `RUSSIAN_WITH_SLAVIC_METAPHONE` constant and this file is a
//! no-op.

#![cfg(feature = "slavic-metaphone")]

extern crate alloc;

use stringcheese_lang::Language;
use stringcheese_ru::RUSSIAN_WITH_SLAVIC_METAPHONE;
use stringcheese_sr::SERBIAN_WITH_SLAVIC_METAPHONE;
use stringcheese_uk::UKRAINIAN_WITH_SLAVIC_METAPHONE;

/// Cross-Slavic pairs `(label, cyrillic, latin)` where every pack's
/// Slavic-Metaphone encoder must produce identical keys for both
/// spellings — and every pack must agree with the others.
///
/// The task-mandated pair `Чехов` ↔ `Chekhov` heads the list;
/// the rest exercise the encoder across the Cyrillic ↔ Latin script
/// boundary on names common across the Slavic language family.
const CROSS_PAIRS: &[(&str, &str, &str)] = &[
    // Task-mandated cross-pack pair.
    ("Chekhov", "Чехов", "Chekhov"),
    // Other cross-Slavic pairs that produce the same encoded key
    // across all three packs.
    ("Petar", "Петар", "Petar"),
    ("Ivan", "Иван", "Ivan"),
    ("Nikola", "Никола", "Nikola"),
    ("Milan", "Милан", "Milan"),
    ("Volkov", "Волков", "Volkov"),
    ("Bratislava", "Братислава", "Bratislava"),
];

/// All three packs return the same key for the same input under the
/// Slavic-Metaphone opt-in encoder.
#[test]
fn all_three_packs_agree_on_cross_slavic_pairs() {
    let ru = RUSSIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();
    let uk = UKRAINIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();
    let sr = SERBIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();

    let mut failures = alloc::vec::Vec::new();
    for &(label, cyr, lat) in CROSS_PAIRS {
        // Encode the Cyrillic spelling under every pack.
        let ru_cyr = ru.encode(cyr).map(|(k, _)| k);
        let uk_cyr = uk.encode(cyr).map(|(k, _)| k);
        let sr_cyr = sr.encode(cyr).map(|(k, _)| k);
        // Encode the Latin spelling under every pack.
        let ru_lat = ru.encode(lat).map(|(k, _)| k);
        let uk_lat = uk.encode(lat).map(|(k, _)| k);
        let sr_lat = sr.encode(lat).map(|(k, _)| k);

        // Every value must be Some and every value must match every
        // other value — the shared encoder is deterministic.
        let all = [
            ("ru.cyr", &ru_cyr),
            ("uk.cyr", &uk_cyr),
            ("sr.cyr", &sr_cyr),
            ("ru.lat", &ru_lat),
            ("uk.lat", &uk_lat),
            ("sr.lat", &sr_lat),
        ];
        for (name, val) in all {
            if val.is_none() {
                failures.push(alloc::format!("  {label}: {name} returned None for input"));
            }
        }
        // Compare against the reference (ru.cyr) — if all are Some,
        // they must all be equal.
        if let Some(reference) = &ru_cyr {
            for (name, val) in &all[1..] {
                if let Some(v) = val
                    && v != reference
                {
                    failures.push(alloc::format!(
                        "  {label}: {name} = {v:?} disagrees with ru.cyr = {reference:?}"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} cross-pack disagreement(s) across {} pair(s):\n{}",
        failures.len(),
        CROSS_PAIRS.len(),
        failures.join("\n")
    );
}

/// The task-mandated pair `Чехов` ↔ `Chekhov` must produce equivalent
/// keys under both `RUSSIAN_WITH_SLAVIC_METAPHONE` and
/// `UKRAINIAN_WITH_SLAVIC_METAPHONE`.
#[test]
fn task_mandated_chekhov_pair_agrees_across_ru_and_uk() {
    let ru = RUSSIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();
    let uk = UKRAINIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();

    let (ru_cyr, _) = ru.encode("Чехов").expect("ru encodes Чехов");
    let (ru_lat, _) = ru.encode("Chekhov").expect("ru encodes Chekhov");
    let (uk_cyr, _) = uk.encode("Чехов").expect("uk encodes Чехов");
    let (uk_lat, _) = uk.encode("Chekhov").expect("uk encodes Chekhov");

    assert_eq!(ru_cyr, ru_lat, "ru: Чехов = Chekhov");
    assert_eq!(uk_cyr, uk_lat, "uk: Чехов = Chekhov");
    assert_eq!(ru_cyr, uk_cyr, "ru and uk agree on Чехов");
    assert_eq!(ru_lat, uk_lat, "ru and uk agree on Chekhov");
}

#[test]
fn every_pack_reports_the_same_adapter_name() {
    let ru = RUSSIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();
    let uk = UKRAINIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();
    let sr = SERBIAN_WITH_SLAVIC_METAPHONE.phonetic_encoder().unwrap();

    assert_eq!(ru.name(), "slavic-metaphone-2026");
    assert_eq!(uk.name(), "slavic-metaphone-2026");
    assert_eq!(sr.name(), "slavic-metaphone-2026");
}

#[test]
fn cross_pack_pair_count_meets_task_minimum() {
    // The task requires at least one pair (Чехов ↔ Chekhov); the
    // reference set carries several more to guard the cross-pack
    // wiring against regressions.
    assert!(
        CROSS_PAIRS.len() >= 5,
        "cross-pack pair count {} is below the 5-pair floor",
        CROSS_PAIRS.len()
    );
}
