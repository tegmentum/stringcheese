//! GOST 7.79 System B (Bulgarian adaptation) Cyrillic → Latin
//! transliteration reference pairs.
//!
//! The pairs below cover every letter of the modern Bulgarian
//! alphabet (30 letters) at least once, plus a handful of common
//! place-names and personal names that exercise the Bulgarian-specific
//! divergences from Russian GOST-B (`щ → sht`, `ъ → a` because ъ is a
//! vowel not a hard sign, `х → h`, `ц → ts`). The mapping is the one
//! documented in [`stringcheese_bg::phonetic`].

extern crate alloc;

use stringcheese_bg::BulgarianGost779B;

/// Reference pairs (input, expected output). Every letter of the
/// modern Bulgarian alphabet (а б в г д е ж з и й к л м н о п р с т у
/// ф х ц ч ш щ ъ ь ю я — 30 letters) is exercised at least once
/// across the set.
const PAIRS: &[(&str, &str)] = &[
    // ---------------------------------------------------------------
    // Common place-names — exercise the core letter set.
    // ---------------------------------------------------------------
    ("София", "sofiya"),
    ("България", "balgariya"), // ъ → a (Bulgarian vowel, not hard sign)
    ("Пловдив", "plovdiv"),
    ("Варна", "varna"),
    ("Русе", "ruse"),
    ("Бургас", "burgas"),
    ("Велико Търново", "veliko tarnovo"), // space passes through
    // ---------------------------------------------------------------
    // Bulgarian-specific divergences from Russian GOST-B.
    // ---------------------------------------------------------------
    ("щастие", "shtastie"), // щ → sht (Russian would be shhastie)
    ("Пещера", "peshtera"), // щ → sht in a place-name
    ("път", "pat"),         // ъ → a; would be p'' in Russian
    ("храна", "hrana"),     // х → h (Russian would be xrana)
    ("цар", "tsar"),        // ц → ts (Russian would be czar)
    // ---------------------------------------------------------------
    // Digraphs and remaining single-letter mappings.
    // ---------------------------------------------------------------
    ("час", "chas"),       // ч → ch
    ("шапка", "shapka"),   // ш → sh
    ("жена", "zhena"),     // ж → zh
    ("юг", "yug"),         // ю → yu
    ("ябълка", "yabalka"), // я → ya (+ ъ → a inside)
    ("май", "maj"),        // й → j
    ("Гьоте", "g'ote"),    // ь → ' (soft sign, rare in Bulgarian)
    // ---------------------------------------------------------------
    // Full-alphabet coverage words.
    // ---------------------------------------------------------------
    ("градина", "gradina"), // г, р, а, д, и, н
    ("филм", "film"),       // ф, и, л, м
    ("окей", "okej"),       // о, к, е, й
    ("език", "ezik"),       // е, з, и, к (covers з)
];

#[test]
fn transliteration_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = BulgarianGost779B.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  BulgarianGost779B({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Bulgarian GOST 7.79-B reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 20 pairs.
    assert!(
        PAIRS.len() >= 20,
        "reference pair count {} is below the 20-pair floor",
        PAIRS.len()
    );
}

#[test]
fn every_bulgarian_letter_is_covered() {
    // Walk the full Bulgarian alphabet (30 lowercase letters). Every
    // one should appear in the lowercase form of at least one
    // reference input.
    const ALPHABET: &[char] = &[
        'а', 'б', 'в', 'г', 'д', 'е', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о', 'п', 'р', 'с',
        'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ъ', 'ь', 'ю', 'я',
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
