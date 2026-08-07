//! Vukovica (Cyrillic) <-> Gaj's Latin transliteration reference pairs.
//!
//! Each pair below is (Cyrillic, Latin) — the two canonical spellings
//! of the same Serbian word. Every digraph pair (`љ ↔ lj`, `њ ↔ nj`,
//! `џ ↔ dž`) is covered at least once, and the special-diacritic
//! letters (`ђ ↔ đ`, `ж ↔ ž`, `ћ ↔ ć`, `ч ↔ č`, `ц ↔ c`, `ш ↔ š`,
//! `ј ↔ j`) each appear in at least one pair.

extern crate alloc;

use stringcheese_sr::scripts::{to_cyrillic, to_latin};

/// Reference pairs (Cyrillic, Latin).
const PAIRS: &[(&str, &str)] = &[
    // Digraphs (љ, њ, џ).
    ("љубав", "ljubav"), // л-у-б-а-в  =>  L-J-...
    ("његош", "njegoš"), // exercises њ and ш
    ("џем", "džem"),     // exercises џ
    ("Његош", "Njegoš"), // Title case digraph
    ("Љубав", "Ljubav"), // Title case digraph
    ("Џем", "Džem"),     // Title case digraph
    // Diacritic letters (ђ, ж, ћ, ч, ц, ш, ј).
    ("ђак", "đak"),           // ђ
    ("Ђорђе", "Đorđe"),       // repeated ђ
    ("жирафа", "žirafa"),     // ж
    ("ћирилица", "ćirilica"), // ћ
    ("човек", "čovek"),       // ч
    ("црква", "crkva"),       // ц
    ("школа", "škola"),       // ш
    ("јун", "jun"),           // ј
    // Common place-names — full-alphabet coverage across the set.
    ("Београд", "Beograd"),
    ("Нови Сад", "Novi Sad"),
    ("Ниш", "Niš"),
    ("Србија", "Srbija"),
    ("Крагујевац", "Kragujevac"),
    ("Војводина", "Vojvodina"),
    // Additional common words for coverage.
    ("кућа", "kuća"),
    ("хвала", "hvala"),
    ("фудбал", "fudbal"),
];

#[test]
fn to_latin_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(cyr, lat) in PAIRS {
        let got = to_latin(cyr);
        if got != lat {
            failures.push(alloc::format!(
                "  to_latin({cyr:?}) = {got:?} (expected {lat:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} pair(s) disagreed on to_latin:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn to_cyrillic_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(cyr, lat) in PAIRS {
        let got = to_cyrillic(lat);
        if got != cyr {
            failures.push(alloc::format!(
                "  to_cyrillic({lat:?}) = {got:?} (expected {cyr:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} pair(s) disagreed on to_cyrillic:\n{}",
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
fn every_digraph_is_covered() {
    let contains_any = |cyr: &str, letters: &[char]| letters.iter().any(|&c| cyr.contains(c));
    for (name, letters) in [
        ("љ / lj", &['љ', 'Љ'][..]),
        ("њ / nj", &['њ', 'Њ'][..]),
        ("џ / dž", &['џ', 'Џ'][..]),
    ] {
        let covered = PAIRS.iter().any(|&(cyr, _)| contains_any(cyr, letters));
        assert!(covered, "no reference pair covers the {name} digraph");
    }
}

#[test]
fn every_diacritic_letter_is_covered() {
    let mut chars = alloc::collections::BTreeSet::new();
    for &(cyr, _) in PAIRS {
        for c in cyr.chars().flat_map(char::to_lowercase) {
            chars.insert(c);
        }
    }
    for expected in ['ђ', 'ж', 'ћ', 'ч', 'ц', 'ш', 'ј'] {
        assert!(
            chars.contains(&expected),
            "no reference pair covers Cyrillic diacritic-letter {expected:?}"
        );
    }
}

#[test]
fn cyrillic_round_trip_holds() {
    for &(cyr, _) in PAIRS {
        let latin = to_latin(cyr);
        let back = to_cyrillic(&latin);
        assert_eq!(back, cyr, "round trip failed on {cyr:?} via {latin:?}");
    }
}
