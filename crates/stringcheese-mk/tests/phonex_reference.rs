//! PHONEX-Macedonian reference input/output pairs.
//!
//! A curated set of Macedonian words that exercises every Macedonian-
//! specific letter (`ѓ`, `ќ`, `љ`, `њ`, `џ`, `ѕ`, `ј`) and the
//! duplicate-consonant collapse.
//!
//! The expected values are computed against the module-level algorithm
//! documented in [`stringcheese_mk::phonetic`] — see there for the
//! classification table. Traces are inlined per pair.

extern crate alloc;

use stringcheese_mk::MacedonianPhonex;

/// Reference pairs (input, expected 4-char-count PHONEX-Macedonian
/// key). The key's first scalar is the Cyrillic seed letter (2 bytes in
/// UTF-8); the trailing three characters are ASCII digits.
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Core Cyrillic — no Macedonian-specific letters. Verifies the
    // shape of the encoder on the plain letter set.
    // -------------------------------------------------------------
    // "град" — г seed, р=6, а(drop), д=3 → "г63" pad → "г630".
    ("град", "г630"),
    // "книга" — к seed, н=5, и(drop), г=2, а(drop) → "к52" pad → "к520".
    ("книга", "к520"),
    // "мир" — м seed, и(drop), р=6 → "м6" pad → "м600".
    ("мир", "м600"),
    // -------------------------------------------------------------
    // Macedonian-specific ѓ, ќ (palatal stops → class 2).
    // -------------------------------------------------------------
    // "ѓавол" — ѓ seed, а(drop), в=1, о(drop), л=4 → "ѓ14" pad → "ѓ140".
    ("ѓавол", "ѓ140"),
    // "куќа" — к seed, у(drop), ќ=2, а(drop) → "к2" pad → "к200".
    ("куќа", "к200"),
    // -------------------------------------------------------------
    // Macedonian-specific љ, њ (palatal liquid / nasal).
    // -------------------------------------------------------------
    // "љубов" — љ seed last=4, у(vow reset last=0), б=1 push,
    //   о(vow reset last=0), в=1 push (fresh — vowel reset the state
    //   between the two labials) → "љ11" pad → "љ110".
    ("љубов", "љ110"),
    // "коњ" — к seed, о(drop), њ=5 → "к5" pad → "к500".
    ("коњ", "к500"),
    // "њива" — њ seed, и(drop), в=1, а(drop) → "њ1" pad → "њ100".
    ("њива", "њ100"),
    // -------------------------------------------------------------
    // Macedonian-specific ѕ, џ (dz, dʒ affricates → class 7).
    // -------------------------------------------------------------
    // "ѕвезда" — ѕ seed, в=1, е(drop), з=7, д=3, а(drop) → "ѕ173"
    //   at len=4, break.
    ("ѕвезда", "ѕ173"),
    // "џин" — џ seed, и(drop), н=5 → "џ5" pad → "џ500".
    ("џин", "џ500"),
    // -------------------------------------------------------------
    // Macedonian-specific ј (palatal glide → class 2).
    // -------------------------------------------------------------
    // "јас" — ј seed, а(drop), с=7 → "ј7" pad → "ј700".
    ("јас", "ј700"),
    // "Скопје" — с seed, к=2, о(drop), п=1, ј=2 → "с212" at len=4.
    ("Скопје", "с212"),
    // -------------------------------------------------------------
    // Case-insensitivity and duplicate collapse.
    // -------------------------------------------------------------
    // "СКОПЈЕ" — same as lowercase; see above.
    ("СКОПЈЕ", "с212"),
    // "асс" — а seed, с=7, с dup drop → "а7" pad → "а700".
    ("асс", "а700"),
    // "аба" — а seed, б=1, а(drop, reset) → "а1" pad → "а100".
    ("аба", "а100"),
    // -------------------------------------------------------------
    // Short inputs pad to 4-char-count.
    // -------------------------------------------------------------
    ("а", "а000"),
    ("не", "н000"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = MacedonianPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-MK({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Macedonian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task-family precedent asks for at least 15 pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn every_macedonian_specific_letter_is_exercised() {
    // Walk the seven Macedonian-specific letters — each should appear
    // in at least one lowercase reference input.
    const SPECIALS: &[char] = &['ѓ', 'ќ', 'љ', 'њ', 'џ', 'ѕ', 'ј'];
    for &letter in SPECIALS {
        let mut found = false;
        for &(input, _) in PAIRS {
            let lower: alloc::string::String = input.chars().flat_map(char::to_lowercase).collect();
            if lower.contains(letter) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "Macedonian-specific letter {letter:?} is not exercised by any reference pair"
        );
    }
}

#[test]
fn case_folded_inputs_produce_the_same_key() {
    // The encoder lowercases first — uppercase / mixed-case inputs
    // produce the same key as the lowercase original.
    for (mixed, lower) in [("Скопје", "скопје"), ("ЈАС", "јас"), ("КНИГА", "книга")]
    {
        let a = MacedonianPhonex.encode(mixed).unwrap();
        let b = MacedonianPhonex.encode(lower).unwrap();
        assert_eq!(a, b, "PHONEX-MK({mixed:?}) != PHONEX-MK({lower:?})");
    }
}
