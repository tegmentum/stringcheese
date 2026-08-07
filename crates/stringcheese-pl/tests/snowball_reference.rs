//! Polish light-stemmer reference input/output pairs.
//!
//! The pairs below were hand-traced through the light suffix-stripping
//! algorithm documented in [`stringcheese_pl::snowball`] on 30+ common
//! Polish words drawn from noun / adjective / verb / adverb paradigms.
//! Each expected value was verified by walking the RV computation and
//! the longest-eligible-suffix table entry that fires.
//!
//! # Deferred: full-corpus cross-verification
//!
//! There is **no stable canonical Snowball Polish** to be
//! parity-with — the Snowball project has experimental Polish sources
//! but no shipped `voc.txt` / `output.txt` reference-pair the way
//! Russian, French, German, Spanish, and Dutch have. This stemmer's
//! output is defined by its algorithm and the pairs shipped here.
//! Full-corpus cross-verification against a Polish lexicon (`morfologik`
//! or `pymorfeusz`) is a follow-up.
//!
//! # Idempotence — not universal
//!
//! Like every light suffix-stripping stemmer, Polish stemming is *not*
//! universally idempotent on arbitrary input: some inflected words
//! stem to a form that itself has a suffix the algorithm strips on a
//! second pass. This is normal — the property test module verifies
//! *convergence* within a small number of iterations, not per-call
//! idempotence.

extern crate alloc;

use stringcheese_pl::PolishSnowball;

/// Reference pairs (input, expected stem after one call to
/// [`PolishSnowball::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Trivial identity cases: short words / no matching suffix.
    // -----------------------------------------------------------------
    ("", ""),
    ("do", "do"),
    ("kot", "kot"),
    // -----------------------------------------------------------------
    // Noun plural / case endings.
    // -----------------------------------------------------------------
    // książki: -ki (2 chars) beats -i (1 char) at globally longest
    //   match → "książ". RV=3.
    ("książki", "książ"),
    // domami: -ami strip → "dom". RV=2.
    ("domami", "dom"),
    // domów: -ów strip → "dom". RV=2.
    ("domów", "dom"),
    // psach: -ach strip → "p" would violate min-stem (1 char). So -a
    //   also fails to keep min-stem ≥ 2. So -ch is not in the table.
    //   Only -h at position 4 not in table. -a at position 4, stem=3
    //   ≥ 2. -a is 1 char; strip yields "psac" (4 chars). Wait no
    //   the word "psach" is 5 chars. Let me re-think.
    //   psach = p s a c h (5 chars). RV = 3.
    //   -ach at position 2, 2 >= 3? No, 2 < 3. Skip.
    //   -ch not in table.
    //   -h not in table.
    //   Try smaller: no suffix ends "psach" that matches. So stems to
    //   itself.
    ("psach", "psach"),
    // studentach: 10 chars. RV: s non-v, t non-v, u v at 2. RV=3.
    //   -ach at position 7, 7 >= 3. Stem 7 ≥ 2. Strip → "student".
    ("studentach", "student"),
    // stołami: 7 chars. RV: s non-v, t non-v, o v at 2. RV=3.
    //   -ami at position 4, 4 >= 3. Stem 4 ≥ 2. Strip → "stoł".
    ("stołami", "stoł"),
    // -----------------------------------------------------------------
    // Adjective long-form endings.
    // -----------------------------------------------------------------
    // dobrego: -ego strip → "dobr". RV=2.
    ("dobrego", "dobr"),
    // dobremu: -emu strip → "dobr". RV=2.
    ("dobremu", "dobr"),
    // dobrymi: -ymi strip → "dobr". RV=2.
    ("dobrymi", "dobr"),
    // dobrych: -ych strip → "dobr". RV=2.
    ("dobrych", "dobr"),
    // -----------------------------------------------------------------
    // Adjective comparative.
    // -----------------------------------------------------------------
    // starsza: -sza strip → "star". RV=3.
    ("starsza", "star"),
    // starszy: -szy strip → "star". RV=3.
    ("starszy", "star"),
    // starsze: -sze strip → "star". RV=3.
    ("starsze", "star"),
    // -----------------------------------------------------------------
    // Verb infinitive.
    // -----------------------------------------------------------------
    // czytać: -ać strip → "czyt". RV=3.
    ("czytać", "czyt"),
    // mówić: 5 chars. RV: m non-v, ó v at 1. RV=2. -ić not in table
    //   directly, but -ć at position 4, 4 >= 2. Strip → "mówi".
    //   Wait, -ć is a 1-char suffix. Stem 4 chars ≥ 2. Strip. → "mówi".
    ("mówić", "mówi"),
    // pisać: -ać strip → "pis". RV=3.
    ("pisać", "pis"),
    // -----------------------------------------------------------------
    // Verb present tense.
    // -----------------------------------------------------------------
    // czytam: -am strip → "czyt". RV=3.
    ("czytam", "czyt"),
    // czytamy: -amy strip → "czyt". RV=3.
    ("czytamy", "czyt"),
    // czytasz: -asz strip → "czyt". RV=3.
    ("czytasz", "czyt"),
    // czytacie: -cie strip. RV=3. -cie at position 5, 5 >= 3. Stem 5 ≥ 2.
    //   Strip → "czyta".
    ("czytacie", "czyta"),
    // -----------------------------------------------------------------
    // Verb past tense.
    // -----------------------------------------------------------------
    // czytaliśmy: -liśmy strip → "czyta". RV=3.
    ("czytaliśmy", "czyta"),
    // czytaliście: -liście strip → "czyta". RV=3.
    ("czytaliście", "czyta"),
    // czytałem: -ałem strip → "czyt". RV=3.
    ("czytałem", "czyt"),
    // czytała: -ała not in table. -ła strip → "czyta". Wait, -ała
    //   would be table entry ['a','ł','a']? Not in table. -ła at
    //   position 5. RV=3. Stem 5 ≥ 2. Strip → "czyta".
    ("czytała", "czyta"),
    // -----------------------------------------------------------------
    // Nominal -anie / -enie / -ość.
    // -----------------------------------------------------------------
    // malowanie: -anie strip → "malow". RV=2.
    ("malowanie", "malow"),
    // pisanie: 7 chars. RV: p non-v, i v at 1. RV=2. -anie at position
    //   3, 3 >= 2. Stem 3 ≥ 2. Strip → "pis".
    ("pisanie", "pis"),
    // ważność: 7 chars. RV: w non-v, a v at 1. RV=2. -ość at position
    //   4, 4 >= 2. Stem 4 ≥ 2. Strip → "ważn".
    ("ważność", "ważn"),
    // -----------------------------------------------------------------
    // Diminutive.
    // -----------------------------------------------------------------
    // łóżko: -ko strip → "łóż". RV=2.
    ("łóżko", "łóż"),
    // domek: -ek strip → "dom". RV=2. -ek at position 3, 3 >= 2. Stem
    //   3 ≥ 2. Strip → "dom".
    ("domek", "dom"),
    // -----------------------------------------------------------------
    // Words that stem to themselves under the algorithm (no rule
    // matches). Confirms the algorithm doesn't over-stem short or
    // atypical inputs.
    // -----------------------------------------------------------------
    // pies: 4 chars. RV: p non-v, i v at 1. RV=2. Word ends in 's'.
    //   No table entry ends 'es' that fires (no -es entry). No -s
    //   single. Stems to itself.
    ("pies", "pies"),
    // ma: 2 chars ≤ 2 early return. Stems to itself.
    ("ma", "ma"),
    // -----------------------------------------------------------------
    // Polish diacritics preserved in the stem.
    // -----------------------------------------------------------------
    // książek: 7 chars. RV: k non-v, s non-v, i v at 2. RV=3. -ek at
    //   position 5, 5 >= 3. Stem 5 ≥ 2. Strip → "książ".
    ("książek", "książ"),
    // mężczyzna: 9 chars. RV: m non-v, ę v at 1. RV=2. -na at position
    //   7 (2 chars). 7 >= 2. Stem 7 ≥ 2. Strip → "mężczyz".
    ("mężczyzna", "mężczyz"),
];

#[test]
fn snowball_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = PolishSnowball.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Snowball({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Polish reference pair(s) disagreed:\n{}",
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
    // The Polish light stemmer is not universally idempotent on
    // arbitrary input (see the module doc), but every input in the
    // reference table converges to a fixed point within a small number
    // of iterations.
    for &(input, _expected) in PAIRS {
        let mut cur = PolishSnowball.stem(input).into_owned();
        let mut steps = 0;
        loop {
            let next = PolishSnowball.stem(&cur).into_owned();
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
