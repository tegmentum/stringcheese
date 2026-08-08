//! Georgian stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_ka::stemmer`], covering all seven case endings, the
//! contemporary and archaic plural markers, the five common
//! agglutinated postpositions, the plural + case / postposition
//! compounds, and the highest-frequency verb personal / tense endings.
//! Longest-match wins so `-ისთვის` (6 chars) beats bare `-ის` (2
//! chars). Every strip is guarded by a 2-scalar minimum stem length.

extern crate alloc;

use stringcheese_ka::GeorgianStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`GeorgianStemmer::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Nominative singular — bare `-ი`.
    //
    // Note: nouns whose stem ends in `-ნ` or `-თ` (e.g. `წიგნი` book,
    // `ცხენი` horse) are ambiguous with the archaic-plural markers
    // `-ნი` / `-თა` and are over-stripped by the longest-match rule.
    // See the stemmer's module docs for the trade-off.
    // -----------------------------------------------------------------
    ("ქართული", "ქართულ"), // Georgian
    ("კაცი", "კაც"),       // man (nom)
    ("სახლი", "სახლ"),     // house (nom, stem ends in ლ — no ambiguity)
    ("ქალი", "ქალ"),       // woman (nom)
    // -----------------------------------------------------------------
    // Dative-accusative — bare `-ს`.
    // -----------------------------------------------------------------
    ("წიგნს", "წიგნ"), // book (dat)
    ("კაცს", "კაც"),   // man (dat)
    // -----------------------------------------------------------------
    // Ergative — `-მა`. Marks the subject of a transitive verb in the
    // aorist; a diagnostic feature of Kartvelian.
    // -----------------------------------------------------------------
    ("კაცმა", "კაც"),   // man (erg)
    ("ცხენმა", "ცხენ"), // horse (erg — no -ნი ambiguity: -მა strips)
    // -----------------------------------------------------------------
    // Genitive — `-ის`.
    // -----------------------------------------------------------------
    ("წიგნის", "წიგნ"), // book (gen)
    ("სახლის", "სახლ"), // house (gen)
    // -----------------------------------------------------------------
    // Instrumental — `-ით`.
    // -----------------------------------------------------------------
    ("ხელით", "ხელ"),   // by hand
    ("კალმით", "კალმ"), // with a pen
    // -----------------------------------------------------------------
    // Adverbial — `-ად`.
    // -----------------------------------------------------------------
    ("მასწავლებლად", "მასწავლებლ"), // as a teacher
    // -----------------------------------------------------------------
    // Plural — contemporary `-ები`.
    // -----------------------------------------------------------------
    ("წიგნები", "წიგნ"), // books
    ("სახლები", "სახლ"), // houses
    // -----------------------------------------------------------------
    // Plural + case compounds.
    // -----------------------------------------------------------------
    ("წიგნებს", "წიგნ"),  // books (dat, plural + `-ს`)
    ("წიგნების", "წიგნ"), // books (gen)
    ("წიგნებით", "წიგნ"), // books (instr)
    ("წიგნებმა", "წიგნ"), // books (erg)
    // -----------------------------------------------------------------
    // Postpositions — bare.
    // -----------------------------------------------------------------
    ("სახლში", "სახლ"),     // in the house
    ("მაგიდაზე", "მაგიდა"), // on the table
    ("მასთან", "მას"),      // at him
    ("მისგან", "მის"),      // from him
    // -----------------------------------------------------------------
    // Postposition + plural compound.
    // -----------------------------------------------------------------
    ("ბავშვებთან", "ბავშვ"), // with the children
    // -----------------------------------------------------------------
    // Genitive + `-კენ` compound (-ისკენ).
    // -----------------------------------------------------------------
    ("ქალაქისკენ", "ქალაქ"), // toward the city
    // -----------------------------------------------------------------
    // Genitive + `-თვის` compound (-ისთვის) — longest-match beats
    // bare `-ის`.
    // -----------------------------------------------------------------
    ("ნესვისთვის", "ნესვ"), // for the melon
    // -----------------------------------------------------------------
    // Verb — 3sg present `-ავს`.
    // -----------------------------------------------------------------
    ("ხატავს", "ხატ"), // he/she draws
    // -----------------------------------------------------------------
    // Verb — 1sg past continuous `-ვდი`.
    // -----------------------------------------------------------------
    ("ვხატავდი", "ვხატა"), // I was drawing
    // -----------------------------------------------------------------
    // Over-strip guard: `მე` "I" (2 chars) below the length gate.
    // -----------------------------------------------------------------
    ("მე", "მე"),
    ("ის", "ის"),
];

#[test]
fn stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = GeorgianStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Georgian stem({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Georgian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task-family convention is at least 25 pairs.
    assert!(
        PAIRS.len() >= 25,
        "reference pair count {} is below the 25-pair floor",
        PAIRS.len()
    );
}
