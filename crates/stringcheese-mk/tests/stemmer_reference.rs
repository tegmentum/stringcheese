//! Macedonian stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_mk::stemmer`], covering the three-way definite-
//! article step (Macedonian's signature feature), the plural cascade,
//! verb personal endings, and the final bare-vowel strip.
//!
//! # Three-way article coverage
//!
//! At least 6 pairs exercise the definite-article step directly across
//! the three proximity series (proximal `-ов`/`-ва`/`-во`/`-ве`,
//! medial `-от`/`-та`/`-то`/`-те`, distal `-он`/`-на`/`-но`/`-не`).
//! The article must strip before the other suffix cascades so that
//! `градот`, `градов`, and `градон` all collapse to the same stem as
//! `град`.
//!
//! # Design note
//!
//! Macedonian's palatal alternations (`к` / `ц`, `г` / `з`, `х` / `с`)
//! are not reversed by the light stemmer — see the module docs for the
//! rationale.

extern crate alloc;

use stringcheese_mk::MacedonianStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`MacedonianStemmer::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // MEDIAL DEFINITE-ARTICLE — the neutral, most-common series.
    // -------------------------------------------------------------
    ("градот", "град"),    // masc medial -от
    ("книгата", "книг"),   // fem medial -та, then bare -а
    ("детето", "дете"),    // neut medial -то (bare -е is NOT stripped)
    ("градовите", "град"), // plural medial -те, then plural -ови
    ("книгите", "книг"),   // plural medial -те, then bare -и
    // -------------------------------------------------------------
    // PROXIMAL DEFINITE-ARTICLE — distinctly Macedonian (Bulgarian has
    // no such series).
    // -------------------------------------------------------------
    ("градов", "град"),    // masc proximal -ов
    ("книгава", "книг"),   // fem proximal -ва, then bare -а
    ("детево", "дете"),    // neut proximal -во
    ("градовиве", "град"), // plural proximal -ве, then plural -ови
    // -------------------------------------------------------------
    // DISTAL DEFINITE-ARTICLE — likewise distinctly Macedonian.
    // -------------------------------------------------------------
    ("градон", "град"),    // masc distal -он
    ("книгана", "книг"),   // fem distal -на, then bare -а
    ("детено", "дете"),    // neut distal -но
    ("градовине", "град"), // plural distal -не, then plural -ови
    // -------------------------------------------------------------
    // Bare noun / adjective / final-vowel step.
    // -------------------------------------------------------------
    ("книга", "книг"), // final -а strip
    ("нови", "нов"),   // final -и strip
    ("ново", "нов"),   // final -о strip
    ("нова", "нов"),   // final -а strip (adj fem)
    ("град", "град"),  // consonant-ending, R1 protects the stem
    // -------------------------------------------------------------
    // Plural cascade.
    // -------------------------------------------------------------
    ("градови", "град"), // masc plural of monosyllabic root -ови
    ("лебови", "леб"),   // masc plural -ови on another monosyllabic root
    ("коњеви", "коњ"),   // masc plural -еви after a soft consonant (њ)
    // -------------------------------------------------------------
    // Verb inflection — present, aorist.
    // -------------------------------------------------------------
    ("правам", "прав"),  // 1sg present: -ам
    ("праваш", "прав"),  // 2sg present: -аш
    ("правав", "прав"),  // 1sg aorist: -ав
    ("правиме", "прав"), // 1pl present: -ме verb strip then bare -и
    ("правите", "прав"), // 2pl present: -те article strip then bare -и
    // -------------------------------------------------------------
    // Macedonian-specific letters carry through.
    // -------------------------------------------------------------
    ("куќа", "куќ"),     // fem noun with ќ; -а strips
    ("ѓавол", "ѓавол"),  // masc noun with ѓ; consonant-ending
    ("њива", "њив"),     // fem noun with њ; -а strips
    ("ѕвезда", "ѕвезд"), // fem noun with ѕ; -а strips
    ("џин", "џин"),      // masc noun with џ; consonant-ending
    // -------------------------------------------------------------
    // Short-word protection — R1 blocks over-stripping.
    // -------------------------------------------------------------
    ("сум", "сум"), // R1 = 3 (end); no suffix eligible
    ("не", "не"),   // R1 = 2 (end); no suffix eligible
];

#[test]
fn stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = MacedonianStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  MacedonianStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Macedonian stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task-family precedent asks for at least 30 pairs.
    assert!(
        PAIRS.len() >= 30,
        "reference pair count {} is below the 30-pair floor",
        PAIRS.len()
    );
}

#[test]
fn three_way_article_pairs_meet_the_task_floor() {
    // The task spec asks for at least 6 pairs demonstrating three-way
    // definite-article stripping (2 per proximity series minimum).
    let proximal: usize = PAIRS
        .iter()
        .filter(|&&(input, _)| {
            input.ends_with("ов")
                || input.ends_with("ва")
                || input.ends_with("во")
                || input.ends_with("ве")
                || input.ends_with("иве")
        })
        .count();
    let medial: usize = PAIRS
        .iter()
        .filter(|&&(input, _)| {
            input.ends_with("от")
                || input.ends_with("та")
                || input.ends_with("то")
                || input.ends_with("те")
                || input.ends_with("ите")
        })
        .count();
    let distal: usize = PAIRS
        .iter()
        .filter(|&&(input, _)| {
            input.ends_with("он")
                || input.ends_with("на")
                || input.ends_with("но")
                || input.ends_with("не")
                || input.ends_with("ине")
        })
        .count();
    assert!(
        proximal >= 2 && medial >= 2 && distal >= 2,
        "three-way article coverage below floor: \
         proximal={proximal}, medial={medial}, distal={distal}"
    );
}

#[test]
fn all_three_proximity_articles_collapse_to_bare_stem() {
    // The canonical Macedonian demonstration: proximal, medial, and
    // distal articled forms all stem to the same string as the bare
    // form.
    let bare = MacedonianStemmer.stem("град").into_owned();
    assert_eq!(MacedonianStemmer.stem("градов").into_owned(), bare);
    assert_eq!(MacedonianStemmer.stem("градот").into_owned(), bare);
    assert_eq!(MacedonianStemmer.stem("градон").into_owned(), bare);
}
