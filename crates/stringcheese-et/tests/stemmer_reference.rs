//! Estonian stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_et::stemmer`], covering the productive suffix
//! categories: case endings (illative, inessive, elative, allative,
//! adessive, ablative, translative, terminative, essive, abessive,
//! comitative), plural markers, common verb inflections, and the
//! diminutive `-ke` / `-kene`.
//!
//! # Reading the pairs
//!
//! Estonian agglutinates — a single orthographic word carries case
//! and number suffixes stacked on the stem. The comment on each pair
//! spells out the morphology; the expected output is the surface
//! stem after one call to the stemmer (which strips at most one
//! suffix per call).
//!
//! # Non-goals
//!
//! - **Full-vocabulary cross-verification.** Estonian has no
//!   Snowball-equivalent `voc.txt` / `output.txt` reference files;
//!   embedding a hand-audited subset here covers every suffix
//!   category's happy path.
//! - **Lexicon-driven consonant-gradation reversal.** The shipped
//!   stemmer does not reverse `raamat` ↔ `raamatu`, `laps` ↔ `lapse`
//!   — those alternations require a lexicon.

extern crate alloc;

use stringcheese_et::EstonianStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`EstonianStemmer::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // `maja` "house" — every productive case-inflected form
    // collapses to `maja`.
    // -------------------------------------------------------------
    ("maja", "maja"),
    ("majas", "maja"),   // inessive `-s` "in the house"
    ("majale", "maja"),  // allative `-le` "onto the house"
    ("majaga", "maja"),  // comitative `-ga` "with the house"
    ("majata", "maja"),  // abessive `-ta` "without the house"
    ("majaks", "maja"),  // translative `-ks` "becoming a house"
    ("majasse", "maja"), // illative `-sse` "into the house"
    ("majast", "maja"),  // elative `-st` "out of the house"
    ("majani", "maja"),  // terminative `-ni` "up to the house"
    ("majana", "maja"),  // essive `-na` "as a house"
    ("majad", "maja"),   // plural nominative `-d` "houses"
    // -------------------------------------------------------------
    // `kool` "school" — u-final vs. consonant-final stem contrast.
    // -------------------------------------------------------------
    ("kool", "kool"),
    ("koolis", "kooli"),
    ("koolile", "kooli"),
    ("koolist", "kooli"),
    // -------------------------------------------------------------
    // `kass` "cat" — the noun-plural `-id` vs. verb-past `-sid`
    // disambiguation lives here: `kassid` is unambiguously the
    // partitive plural, and the stemmer's vowel-preceding constraint
    // on `-sid` prevents the past-tense misparse (`kass` doesn't end
    // in a vowel).
    // -------------------------------------------------------------
    ("kass", "kass"),
    ("kassid", "kass"),
    // -------------------------------------------------------------
    // `raamat` "book" — plural markers.
    // -------------------------------------------------------------
    ("raamat", "raamat"),
    ("raamatuid", "raamatu"), // partitive plural `-id`
    ("raamatute", "raamatu"), // genitive plural `-te`
    // -------------------------------------------------------------
    // Diminutive `-ke` / `-kene` on `linnu` "bird".
    // -------------------------------------------------------------
    ("linnuke", "linnu"),
    ("linnukene", "linnu"),
    // -------------------------------------------------------------
    // Verb inflections — `kõndima` "to walk" family.
    // -------------------------------------------------------------
    ("kõnnivad", "kõnni"), // 3pl present `-vad`
    ("kõndinud", "kõndi"), // past active participle `-nud`
    // -------------------------------------------------------------
    // Verb past-tense `-sid` — vowel-preceding context fires.
    // -------------------------------------------------------------
    ("käisid", "käi"), // 2sg past of käima "to go"
    // -------------------------------------------------------------
    // Front-vowel / õ / ü / ä words — the pipeline preserves
    // diacritic-bearing stems.
    // -------------------------------------------------------------
    ("külas", "küla"),      // inessive of küla "village"
    ("külale", "küla"),     // allative of küla
    ("õpetaja", "õpetaja"), // "teacher" — no productive suffix
    // -------------------------------------------------------------
    // Short base words (< 3 chars) — early-return path.
    // -------------------------------------------------------------
    ("on", "on"),
    ("ei", "ei"),
    ("ma", "ma"),
    // -------------------------------------------------------------
    // 3-char base words — protected by min-stem floors from
    // over-stripping.
    // -------------------------------------------------------------
    ("see", "see"),
    ("too", "too"),
    ("kes", "kes"), // single-char `-s` blocked (min-stem-3)
];

#[test]
fn stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = EstonianStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  stem({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Estonian stemmer reference pair(s) disagreed:\n{}",
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
fn every_reference_pair_converges() {
    // The Estonian stemmer strips at most one suffix per call, so
    // convergence should happen within a small number of iterations.
    for &(input, _expected) in PAIRS {
        let mut cur = EstonianStemmer.stem(input).into_owned();
        let mut steps = 0;
        loop {
            let next = EstonianStemmer.stem(&cur).into_owned();
            if next == cur {
                break;
            }
            cur = next;
            steps += 1;
            assert!(
                steps <= 5,
                "stem did not converge in 5 iterations for input {input:?} (last = {cur:?})"
            );
        }
    }
}

#[test]
fn case_paradigm_collapses_to_one_stem() {
    // The flagship demonstration: every productive case-inflected
    // form of `maja` "house" folds to the same equivalence-class key.
    let paradigm = [
        "maja", "majas", "majale", "majaga", "majata", "majaks", "majasse", "majast", "majani",
        "majana",
    ];
    let stems: alloc::collections::BTreeSet<_> = paradigm
        .iter()
        .map(|w| EstonianStemmer.stem(w).into_owned())
        .collect();
    assert_eq!(
        stems.len(),
        1,
        "case paradigm of maja produced {} distinct stems: {stems:?}",
        stems.len()
    );
    assert!(stems.contains("maja"));
}

#[test]
fn no_vowel_harmony_variants() {
    // Estonian has no vowel harmony — unlike Finnish, the suffix
    // table lists each suffix exactly once. This is a sanity check
    // that the pack does not accidentally require a harmony variant.
    // Both back-vowel and front-vowel stems share the same `-s`
    // inessive suffix.
    assert_eq!(EstonianStemmer.stem("majas").into_owned(), "maja"); // back
    assert_eq!(EstonianStemmer.stem("külas").into_owned(), "küla"); // front
}
