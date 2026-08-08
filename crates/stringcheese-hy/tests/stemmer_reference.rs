//! Armenian stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_hy::stemmer`], covering the postposed definite
//! article, the seven Eastern Armenian singular case suffixes, the
//! plural markers (monosyllabic-base `-եր` and polysyllabic-base
//! `-ներ`), the plural + case combinations, and the aorist personal
//! endings. The list intentionally stays on well-behaved
//! inflections — the shipped stemmer is a hand-audited longest-match
//! suffix stripper that iterates to convergence (see the crate
//! module docs).
//!
//! # Deferred: full-corpus cross-verification
//!
//! Armenian has no canonical Snowball-family test corpus; a
//! future wave could cross-verify against a published academic
//! Armenian IR corpus.

extern crate alloc;

use stringcheese_hy::ArmenianStemmer;

/// Reference pairs (input, expected stem after
/// [`ArmenianStemmer::stem`] runs to convergence).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Postposed definite article.
    // -------------------------------------------------------------
    ("մայրը", "մայր"), // mother-def → mother
    ("հայրը", "հայր"), // father-def → father
    ("տղան", "տղա"),   // boy-def (post-vowel form -ն) → boy
    ("գիրքը", "գիրք"), // book-def → book
    // -------------------------------------------------------------
    // Genitive singular -ի.
    // -------------------------------------------------------------
    ("գրքի", "գրք"),   // of book
    ("մարդի", "մարդ"), // of person
    // -------------------------------------------------------------
    // Dative singular -ին.
    // -------------------------------------------------------------
    ("մարդին", "մարդ"), // to person
    ("գրքին", "գրք"),   // to book
    // -------------------------------------------------------------
    // Ablative singular -ից.
    // -------------------------------------------------------------
    ("գրքից", "գրք"),   // from book
    ("մարդից", "մարդ"), // from person
    // -------------------------------------------------------------
    // Instrumental singular -ով.
    // -------------------------------------------------------------
    ("գրիչով", "գրիչ"), // with pen
    ("գրքով", "գրք"),   // with book
    // -------------------------------------------------------------
    // Locative singular -ում.
    // -------------------------------------------------------------
    ("քաղաքում", "քաղաք"), // in city
    // `տանում` (in house) — first strips `-ում` to `տան`, then the
    // iterated pass strips `-ն` (the "after-vowel" definite article
    // rule fires because `ա` precedes) to leave `տա`. This is a
    // characteristic over-strip of a light suffix stripper without
    // a lexicon that could tell `տան` is itself a stem; documented
    // here as the expected behaviour of the iterated cascade.
    ("տանում", "տա"),
    // -------------------------------------------------------------
    // Plural markers.
    // -------------------------------------------------------------
    ("գրքեր", "գրք"),    // books (monosyllabic base -եր)
    ("մարդներ", "մարդ"), // people (polysyllabic base -ներ)
    // -------------------------------------------------------------
    // Plural + case combinations.
    // -------------------------------------------------------------
    ("գրքերի", "գրք"),        // pl.gen (monosyllabic)
    ("մարդների", "մարդ"),     // pl.gen (polysyllabic)
    ("գրքերով", "գրք"),       // pl.ins (monosyllabic)
    ("գրքերից", "գրք"),       // pl.abl (monosyllabic)
    ("գրքերին", "գրք"),       // pl.dat (monosyllabic)
    ("քաղաքներում", "քաղաք"), // pl.loc (polysyllabic)
    // -------------------------------------------------------------
    // Aorist personal endings.
    // -------------------------------------------------------------
    ("սիրեցի", "սիր"),   // I loved (1sg)
    ("սիրեցիր", "սիր"),  // you loved (2sg)
    ("սիրեց", "սիր"),    // he/she loved (3sg)
    ("սիրեցինք", "սիր"), // we loved (1pl)
    ("սիրեցիք", "սիր"),  // you-pl loved (2pl)
    ("սիրեցին", "սիր"),  // they loved (3pl)
    // -------------------------------------------------------------
    // Case + case-fold: uppercase input stems to the same result.
    // -------------------------------------------------------------
    ("ՄԱՅՐԸ", "մայր"),
];

#[test]
fn stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = ArmenianStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Armenian stem({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Armenian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // Enough pairs to cover every table category.
    assert!(
        PAIRS.len() >= 25,
        "reference pair count {} is below the 25-pair floor",
        PAIRS.len()
    );
}

#[test]
fn eu_and_ligature_stem_identically() {
    // The `եւ → և` two-letter-to-ligature normalization means both
    // spellings stem to the same result. Bare `եւ` / `և` are 2 / 1
    // chars, both below the strip threshold, so the stems are the
    // normalized forms themselves.
    let with_two = ArmenianStemmer.stem("եւ").into_owned();
    let with_lig = ArmenianStemmer.stem("և").into_owned();
    assert_eq!(with_two, with_lig);
}
