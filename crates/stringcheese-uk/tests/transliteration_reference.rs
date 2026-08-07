//! GOST 7.79 System B (Ukrainian adaptation) Cyrillic → Latin
//! transliteration reference pairs.
//!
//! The pairs below cover every letter of the modern Ukrainian
//! alphabet (33 letters) at least once, plus a handful of common
//! place-names and personal names that exercise the Ukrainian-
//! specific divergences from Russian GOST-B (`г → h` vs `ґ → g`,
//! `є → ye`, `ї → yi`, `и → y`, `х → kh`, `щ → shch`). The mapping
//! is the one documented in [`stringcheese_uk::phonetic`].

extern crate alloc;

use stringcheese_uk::UkrainianGost779B;

/// Reference pairs (input, expected output). Every letter of the
/// modern Ukrainian alphabet (а б в г ґ д е є ж з и і ї й к л м н о п
/// р с т у ф х ц ч ш щ ь ю я — 33 letters) is exercised at least once
/// across the set.
const PAIRS: &[(&str, &str)] = &[
    // ---------------------------------------------------------------
    // Common place-names — exercise the core letter set plus the
    // Ukrainian-specific ї / і / ь.
    // ---------------------------------------------------------------
    ("Київ", "kyyiv"),
    ("Львів", "l'viv"),
    ("Харків", "kharkiv"),
    ("Україна", "ukrayina"),
    ("Одеса", "odesa"),
    ("Дніпро", "dnipro"),
    ("Полтава", "poltava"),
    ("Житомир", "zhytomyr"),
    // ---------------------------------------------------------------
    // Ukrainian-specific letters — ґ / є / ї and the g/h divergence.
    // ---------------------------------------------------------------
    ("Ґудзик", "gudzyk"),
    ("Ґанок", "ganok"),
    ("гора", "hora"),
    ("Європа", "yevropa"),
    ("Їжак", "yizhak"),
    // ---------------------------------------------------------------
    // Consonant digraphs — щ (shch), ч (ch), ц (cz).
    // ---------------------------------------------------------------
    ("Щастя", "shchastya"),
    ("Чорне", "chorne"),
    ("Цукор", "czukor"),
    // ---------------------------------------------------------------
    // Personal name — ю → yu, і → i, й → j.
    // ---------------------------------------------------------------
    ("Юрій", "yurij"),
    // ---------------------------------------------------------------
    // Trailing soft sign — ь → '.
    // ---------------------------------------------------------------
    ("Мить", "myt'"),
    // ---------------------------------------------------------------
    // Coverage-completion words to reach the last few letters.
    // ---------------------------------------------------------------
    ("брат", "brat"),            // б
    ("шапка", "shapka"),         // ш (щ digraph contains sh but ш itself needs a hit)
    ("Ф'юзеляж", "f'yuzelyazh"), // ф + non-Cyrillic ' pass-through
];

#[test]
fn transliteration_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = UkrainianGost779B.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  UkrainianGost779B({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Ukrainian GOST 7.79-B reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 15 pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn every_ukrainian_letter_is_covered() {
    // Walk the full Ukrainian alphabet (33 lowercase letters). Every
    // one should appear in the lowercase form of at least one
    // reference input.
    const ALPHABET: &[char] = &[
        'а', 'б', 'в', 'г', 'ґ', 'д', 'е', 'є', 'ж', 'з', 'и', 'і', 'ї', 'й', 'к', 'л', 'м', 'н',
        'о', 'п', 'р', 'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ь', 'ю', 'я',
    ];
    for &letter in ALPHABET {
        let mut found = false;
        for &(input, _expected) in PAIRS {
            let lower: alloc::string::String = input.chars().flat_map(char::to_lowercase).collect();
            if lower.contains(letter) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "letter {letter:?} is not exercised by any reference pair"
        );
    }
}
