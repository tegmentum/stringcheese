//! GOST 7.79 System B Cyrillic → Latin transliteration reference pairs.
//!
//! The pairs below cover every letter of the modern Russian alphabet
//! at least once, plus a handful of common place-names and personal
//! names. The mapping is the one documented in
//! [`stringcheese_ru::phonetic`] — a context-free, ASCII-only
//! variant of GOST 7.79 System B.

extern crate alloc;

use stringcheese_ru::RussianGost779B;

/// Reference pairs (input, expected output). Every letter of the
/// modern Russian alphabet (а б в г д е ё ж з и й к л м н о п р с т у ф
/// х ц ч ш щ ъ ы ь э ю я — 33 letters) is exercised at least once
/// across the set.
const PAIRS: &[(&str, &str)] = &[
    // ---------------------------------------------------------------
    // Common place-names — exercise a, б, в, г, д, е, ж, з, и, к, л,
    // м, н, о, п, р, с, т, у, ф, х, я.
    // ---------------------------------------------------------------
    ("Москва", "moskva"),  // м о с к в а
    ("Россия", "rossiya"), // р о с с и я
    ("Санкт-Петербург", "sankt-peterburg"),
    ("Владивосток", "vladivostok"),
    ("Новосибирск", "novosibirsk"),
    ("Волга", "volga"),
    ("Урал", "ural"),
    ("Байкал", "bajkal"), // exercises й → j
    // ---------------------------------------------------------------
    // Common names — exercise the remaining letters and the digraphs.
    // ---------------------------------------------------------------
    ("Пушкин", "pushkin"),  // ш → sh
    ("Чехов", "chexov"),    // ч → ch, х → x
    ("Толстой", "tolstoj"), // ой → oj
    ("Достоевский", "dostoevskij"),
    ("Хабаровск", "xabarovsk"), // exercises х → x
    // ---------------------------------------------------------------
    // ё, щ, ъ, ы, ь, э, ю exercises.
    // ---------------------------------------------------------------
    ("Ёж", "yozh"),          // ё → yo, ж → zh
    ("щи", "shhi"),          // щ → shh
    ("подъезд", "pod''ezd"), // ъ → ''
    ("сын", "syn"),          // ы → y
    ("конь", "kon'"),        // ь → '
    ("это", "e'to"),         // э → e'
    ("юг", "yug"),           // ю → yu
    ("цирк", "czirk"),       // ц → cz
    // ---------------------------------------------------------------
    // Full-alphabet coverage-check words.
    // ---------------------------------------------------------------
    ("гений", "genij"),    // г, е, н, и, й
    ("фильм", "fil'm"),    // ф, и, л, ь, м
    ("объект", "ob''ekt"), // о, б, ъ, е, к, т
];

#[test]
fn transliteration_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = RussianGost779B.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  GOST779B({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} GOST 7.79-B reference pair(s) disagreed:\n{}",
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
fn every_modern_russian_letter_is_covered() {
    // Walk the full modern Russian alphabet (33 lowercase letters).
    // Every one should appear in the lowercase form of at least one
    // reference input.
    const ALPHABET: &[char] = &[
        'а', 'б', 'в', 'г', 'д', 'е', 'ё', 'ж', 'з', 'и', 'й', 'к', 'л', 'м', 'н', 'о', 'п', 'р',
        'с', 'т', 'у', 'ф', 'х', 'ц', 'ч', 'ш', 'щ', 'ъ', 'ы', 'ь', 'э', 'ю', 'я',
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
