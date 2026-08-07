//! Snowball Bulgarian stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_bg::snowball`], covering the definite-article step
//! (Bulgarian's signature feature), the plural cascade, verb / l-
//! participle endings, and the final bare-vowel strip.
//!
//! # Definite-article coverage
//!
//! At least 5 pairs exercise the definite-article step directly —
//! the article is a suffix (`-ът`, `-ят`, `-та`, `-то`, `-те`, and
//! the long-adjective `-ият` / `-ата` / `-ото` / `-ите` variants),
//! and it must strip before the other suffix cascades so that
//! `книгата` and `книга` collapse to the same stem.
//!
//! # Design note
//!
//! Bulgarian's palatal alternations (`к` / `ц`, `г` / `з`, `х` / `с`,
//! stressed `я` / unstressed `е`) are not reversed by the light
//! stemmer — see the module docs for the rationale. Reference pairs
//! that would require palatal restoration to line up (`ръка` /
//! `ръце`, `голям` / `големи`) are absent from this set.

extern crate alloc;

use stringcheese_bg::BulgarianSnowball;

/// Reference pairs (input, expected stem after one call to
/// [`BulgarianSnowball::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // DEFINITE-ARTICLE step — Bulgarian's signature morphological
    // feature. The article is a noun-suffix, not a separate word, so
    // an articled surface form (`книгата`) must collapse to the same
    // stem as the bare form (`книга`).
    // -------------------------------------------------------------
    ("книгата", "книг"),     // fem noun + -та article; -ата → -а → книг
    ("човекът", "човек"),    // masc full definite subject -ът
    ("учителят", "учител"),  // masc soft-noun full definite -ят
    ("детето", "дет"),       // neut noun definite -то, then -е final
    ("столовете", "стол"),   // plural definite -те then plural -ове
    ("градовете", "град"),   // same pattern as столовете
    ("красивата", "красив"), // long-adjective fem definite -ата
    ("красивото", "красив"), // long-adjective neut definite -ото
    ("красивите", "красив"), // long-adjective plural definite -ите
    ("новата", "нов"),       // long-adjective fem definite -ата
    ("новите", "нов"),       // long-adjective plural definite -ите
    ("книгите", "книг"),     // plural noun definite -ите
    // -------------------------------------------------------------
    // Bare noun / adjective / final-vowel step.
    // -------------------------------------------------------------
    ("книга", "книг"), // final -а strip
    ("нови", "нов"),   // final -и strip
    ("ново", "нов"),   // final -о strip
    ("стол", "стол"),  // consonant-ending, R1 protects the stem
    // -------------------------------------------------------------
    // Plural cascade.
    // -------------------------------------------------------------
    ("столове", "стол"), // masc plural of monosyllabic root
    ("градове", "град"), // same shape
    // -------------------------------------------------------------
    // Verb inflection — present, aorist, imperfect.
    // -------------------------------------------------------------
    ("правя", "прав"),    // 1sg present: final -я strip
    ("правим", "прав"),   // 1pl present: -им
    ("правят", "прав"),   // 3pl present: -ят
    ("правите", "прав"),  // 2pl present / plural definite -ите (same shape)
    ("четох", "чет"),     // aorist 1sg: -ох
    ("четеш", "чет"),     // present 2sg: -еш
    ("четете", "чет"),    // present 2pl: -ете
    ("четат", "чет"),     // present 3pl: -ат
    ("четеше", "чет"),    // imperfect 3sg: -еше
    ("правеха", "прав"),  // imperfect 3pl: -еха
    ("правехме", "прав"), // imperfect 1pl: -ехме
    // -------------------------------------------------------------
    // l-participle. (The masc bare `-л` is deliberately absent from
    // the verb table — it would over-strip noun stems like `учител`
    // — so `казал` is left intact. `-ла` / `-ло` / `-ли` cover the
    // remaining forms.)
    // -------------------------------------------------------------
    ("казала", "каз"), // fem l-participle -ла, then final -а
    ("казало", "каз"), // neut l-participle -ло, then final -а
    ("казали", "каз"), // pl l-participle -ли, then final -а
    // -------------------------------------------------------------
    // Bulgarian-specific: ъ is a vowel, not a hard sign.
    // -------------------------------------------------------------
    ("път", "път"),  // ъ is a vowel; short word preserved by R1
    ("къща", "къщ"), // ъ is a vowel; -а strips in R1
];

#[test]
fn snowball_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = BulgarianSnowball.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  BulgarianSnowball({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Bulgarian Snowball reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 30 pairs. Verify we're above
    // that.
    assert!(
        PAIRS.len() >= 30,
        "reference pair count {} is below the 30-pair floor",
        PAIRS.len()
    );
}

#[test]
fn definite_article_pairs_meet_the_task_floor() {
    // The task spec asks for at least 5 pairs demonstrating definite-
    // article stripping. Count the pairs whose input carries an
    // article suffix (`-ът`, `-ят`, `-та`, `-то`, `-те`, or the
    // long-form `-ият`, `-ия`, `-ата`, `-ото`, `-ите`).
    let article_pairs: usize = PAIRS
        .iter()
        .filter(|&&(input, _)| {
            input.ends_with("ът")
                || input.ends_with("ят")
                || input.ends_with("та")
                || input.ends_with("то")
                || input.ends_with("те")
                || input.ends_with("ият")
                || input.ends_with("ата")
                || input.ends_with("ото")
                || input.ends_with("ите")
        })
        .count();
    assert!(
        article_pairs >= 5,
        "only {article_pairs} of {} pairs exercise a definite-article \
         suffix (task floor is 5)",
        PAIRS.len()
    );
}

#[test]
fn articled_and_bare_forms_stem_identically() {
    // The canonical Bulgarian demonstration: a noun's articled form
    // and its bare form stem to the same string. This is what the
    // article step exists to guarantee.
    let bare = BulgarianSnowball.stem("книга").into_owned();
    let articled = BulgarianSnowball.stem("книгата").into_owned();
    assert_eq!(bare, articled);
}
