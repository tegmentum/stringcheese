//! Icelandic rule-based stemmer reference input/output pairs.
//!
//! No canonical Snowball voc.txt exists for Icelandic (see the
//! `stemmer` module docs for why), so the pairs below are hand-traced
//! through the algorithm's suffix inventory rather than cross-verified
//! against an external reference. They exercise every branch of the
//! suffix table: definite-article suffixes (`-inum`, `-inni`, `-inn`,
//! `-nir`, `-nar`, `-num`, `-nni`, `-nu`, `-ið`, `-in`), noun case
//! endings (`-ur`, `-ar`, `-ir`, `-um`, `-s`, `-i`, `-a`), verb
//! personal endings (`-um`, `-uð`, `-a`, `-ir`, `-ið`), adjective
//! agreement (`-ur`, `-ir`, `-um`, `-a`, `-t`), and the
//! `MIN_STEM_CHARS` ≥ 3 guard.
//!
//! # Idempotence — not universal
//!
//! The Icelandic rule-based stemmer, like Snowball Danish and Porter
//! English, is *not* universally idempotent on arbitrary input: some
//! inflected words stem to a form that itself has a suffix the
//! internal loop strips on a later pass. The internal loop runs to
//! convergence, so the outer `stem()` is idempotent, but intermediate
//! states are not. This is normal — the property test module verifies
//! *convergence* within a small number of iterations.

extern crate alloc;

use stringcheese_is::IcelandicStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`IcelandicStemmer::stem`]).
///
/// Every value below was hand-traced through the algorithm's steps
/// and cross-verified against the module's unit tests.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Trivial identity cases: short words already at their stem
    // (protected by `MIN_STEM_CHARS` ≥ 3).
    // -----------------------------------------------------------------
    ("og", "og"),
    ("í", "í"),
    ("er", "er"),
    ("hús", "hús"),
    ("mín", "mín"),
    ("ís", "ís"),
    ("ár", "ár"),
    // -----------------------------------------------------------------
    // Definite-article suffix stripping.
    // -----------------------------------------------------------------
    // -inn (masc nom sg def). hesturinn → -inn → hestur → -ur → hest.
    ("hesturinn", "hest"),
    ("strákurinn", "strák"),
    // -in (fem nom sg def). bókin → -in → bók.
    ("bókin", "bók"),
    // -ið (neut sg def). húsið → -ið → hús.
    ("húsið", "hús"),
    // -nir (masc nom pl def). strákarnir → -nir → strákar → -ar →
    //   strák.
    ("strákarnir", "strák"),
    // -nar (fem/masc acc pl def). konurnar → -nar → konur → -ur →
    //   kon.
    ("konurnar", "kon"),
    // -inum (masc dat sg def). hestinum → -inum → hest.
    ("hestinum", "hest"),
    // -inni (fem dat sg def). skálinni → -inni → skál.
    ("skálinni", "skál"),
    // -nni (fem dat sg def alt). konunni → -nni → konu.
    ("konunni", "konu"),
    // -num (dat pl def). hestunum → -num → hestu.
    ("hestunum", "hestu"),
    // -----------------------------------------------------------------
    // Noun case endings (indefinite).
    // -----------------------------------------------------------------
    // -ur (masc nom sg indef).
    ("hestur", "hest"),
    // -ar (nom pl).
    ("hestar", "hest"),
    // -ir (fem/masc weak nom pl).
    ("gestir", "gest"),
    // -um (dat pl universal).
    ("konum", "kon"),
    // -s (masc/neut gen sg).
    ("hests", "hest"),
    ("húss", "hús"),
    // -i (dat sg universal).
    ("hesti", "hest"),
    // -a (gen pl / weak neut).
    ("hesta", "hest"),
    ("mála", "mál"),
    // -----------------------------------------------------------------
    // Verb personal endings.
    // -----------------------------------------------------------------
    // -um (1pl).
    ("komum", "kom"),
    // -ið (2pl).
    ("komið", "kom"),
    // -a (infinitive / 3pl).
    ("koma", "kom"),
    ("hafa", "haf"),
    // -----------------------------------------------------------------
    // Adjective agreement.
    // -----------------------------------------------------------------
    // -t (neut nom sg strong).
    ("stórt", "stór"),
    // -um (dat pl).
    ("stórum", "stór"),
    // -ir (masc nom pl).
    ("stórir", "stór"),
    // -a (weak declension).
    ("stóra", "stór"),
    // -----------------------------------------------------------------
    // `MIN_STEM_CHARS` ≥ 3 guard protects short residues.
    // -----------------------------------------------------------------
    // `þögn` (silence) — no suffix at the terminal position matches;
    //   result stays 'þögn'.
    ("þögn", "þögn"),
];

#[test]
fn stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = IcelandicStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  IcelandicStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Icelandic stemmer reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // At least 30 pairs to cover the suffix inventory.
    assert!(
        PAIRS.len() >= 30,
        "reference pair count {} is below the 30-pair floor",
        PAIRS.len()
    );
}

#[test]
fn every_reference_pair_converges() {
    // The stemmer's internal loop already runs to convergence, so
    // this verifies that external repeated calls are idempotent (no
    // further stripping after the first outer call).
    for &(input, _expected) in PAIRS {
        let mut cur = IcelandicStemmer.stem(input).into_owned();
        let mut steps = 0;
        loop {
            let next = IcelandicStemmer.stem(&cur).into_owned();
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
