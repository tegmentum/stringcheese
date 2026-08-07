//! Snowball Portuguese stemmer reference input/output pairs.
//!
//! The pairs below are drawn from the Snowball project's canonical
//! `voc.txt` / `output.txt` reference files distributed with the
//! [Portuguese stemmer][ref], cross-verified by hand-tracing every
//! rule path exercised. This test embeds ~35 pairs — enough to walk
//! every step's happy path (regions R1/R2/RV, prelude/postlude nasal
//! placeholder, standard suffix removal, verb suffix, trailing-`i`
//! after `c`, residual suffix cleanup, residual form) — while keeping
//! the test file small enough to hand-audit.
//!
//! [ref]: https://snowballstem.org/algorithms/portuguese/stemmer.html
//!
//! # Deferred: full-corpus cross-verification
//!
//! The Snowball project distributes `voc.txt` with tens of thousands
//! of test pairs. Embedding a subset here keeps compile times and the
//! test binary size sane; full-corpus cross-verification against every
//! pair is a follow-up wave and would run under a feature-gated
//! `snowball-fullcorpus` cfg reading the vocab files at build time.
//!
//! # Idempotence — not universal
//!
//! Snowball Portuguese, like Porter English and Snowball French /
//! Spanish, is *not* universally idempotent on arbitrary input: some
//! inflected words stem to a form that itself has a suffix the
//! algorithm strips on a second pass. This is normal — the property
//! test module verifies *convergence* within a small number of
//! iterations, not per-call idempotence.

extern crate alloc;

use stringcheese_pt::PortugueseSnowball;

/// Reference pairs (input, expected stem after one call to
/// [`PortugueseSnowball::stem`]).
///
/// Every value below was hand-traced through the algorithm's steps
/// and cross-verified against the module's unit tests.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Trivial identity cases: short words already at their stem.
    // -----------------------------------------------------------------
    ("o", "o"),
    ("a", "a"),
    ("um", "um"),
    // -----------------------------------------------------------------
    // Step 4 residual: plain nouns with -o / -a / -os endings.
    // -----------------------------------------------------------------
    ("menino", "menin"),
    ("menina", "menin"),
    ("meninos", "menin"),
    ("meninas", "menin"),
    ("casa", "cas"),
    ("casas", "cas"),
    ("livro", "livr"),
    ("livros", "livr"),
    // -----------------------------------------------------------------
    // Step 1 group A: -ismo / -ismos.
    // -----------------------------------------------------------------
    ("analfabetismo", "analfabet"),
    // -----------------------------------------------------------------
    // Step 1 group A: -ação (encoded as aca~o after prelude).
    // -----------------------------------------------------------------
    // `formação` is *too short* to trigger the -ação rule: R2=6 but
    // the suffix starts at position 4. Step 4 strips only the final
    // `o`, and the postlude reassembles the nasal. Interesting
    // documented behaviour of the algorithm — short -ação nouns
    // stem to their nasal form, not to the bare root.
    ("formação", "formaçã"),
    ("desmitificação", "desmitific"),
    // -----------------------------------------------------------------
    // Step 1 group B: -logia → replace with log.
    // -----------------------------------------------------------------
    ("biologia", "biolog"),
    // -----------------------------------------------------------------
    // Step 1 group C: -ução → replace with u.
    // -----------------------------------------------------------------
    ("revolução", "revolu"),
    // -----------------------------------------------------------------
    // Step 1 group D: -ência → replace with ente. Only fires when the
    // suffix is in R2; `preferência` (long enough) qualifies.
    // -----------------------------------------------------------------
    ("preferência", "preferent"),
    // -----------------------------------------------------------------
    // Step 1 group E: -amente → delete if in R1.
    // -----------------------------------------------------------------
    ("rapidamente", "rapid"),
    // -----------------------------------------------------------------
    // Step 1 group F: -mente → delete if in R2.
    // -----------------------------------------------------------------
    ("claramente", "clar"),
    // -----------------------------------------------------------------
    // Step 1 group G: -idade → delete if in R2.
    // -----------------------------------------------------------------
    ("felicidade", "felic"),
    // -----------------------------------------------------------------
    // Step 1 group H: -ivo → delete if in R2.
    // -----------------------------------------------------------------
    ("abusivo", "abus"),
    // -----------------------------------------------------------------
    // Step 1 group I: -eira → replace with -ir.
    // -----------------------------------------------------------------
    ("costureira", "costureir"),
    ("costureiras", "costureir"),
    // -----------------------------------------------------------------
    // Step 2 verb suffixes: -ar / -er / -ir infinitives.
    // -----------------------------------------------------------------
    ("falar", "fal"),
    ("perder", "perd"),
    ("posar", "pos"),
    // -----------------------------------------------------------------
    // Step 2 verb suffixes: -ando / -endo / -indo gerunds.
    // -----------------------------------------------------------------
    ("falando", "fal"),
    // -----------------------------------------------------------------
    // Step 2 verb suffixes: -ada / -ida past participles fem.
    // -----------------------------------------------------------------
    ("noticiada", "notic"),
    ("consagrada", "consagr"),
    // -----------------------------------------------------------------
    // Step 2 verb suffixes: -am (3pl present).
    // -----------------------------------------------------------------
    ("prometem", "promet"),
    ("subornavam", "suborn"),
    // -----------------------------------------------------------------
    // Step 2 verb suffixes: -asse (past subjunctive).
    // -----------------------------------------------------------------
    ("colocasse", "coloc"),
    // -----------------------------------------------------------------
    // Step 2 verb suffixes: -ou (3sg preterite).
    // -----------------------------------------------------------------
    ("readaptou", "readapt"),
    // -----------------------------------------------------------------
    // Step 2 verb suffixes: -er where RV allows.
    // -----------------------------------------------------------------
    ("laser", "las"),
    // -----------------------------------------------------------------
    // Step 4 residual: -a alone on a stem that survives step 1 + 2.
    // -----------------------------------------------------------------
    ("tremenda", "tremend"),
    ("estrelas", "estrel"),
    ("mordomias", "mordom"),
    // -----------------------------------------------------------------
    // Step 5 residual form: trailing -e delete in RV.
    // -----------------------------------------------------------------
    ("ventre", "ventr"),
    // -----------------------------------------------------------------
    // Words that stem to themselves under the algorithm (no rule
    // matches). Confirms the algorithm doesn't over-stem short or
    // atypical inputs.
    // -----------------------------------------------------------------
    ("honolulu", "honolulu"),
    ("autor", "autor"),
];

#[test]
fn snowball_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = PortugueseSnowball.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  Snowball({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Snowball reference pair(s) disagreed:\n{}",
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
    // Snowball Portuguese is not universally idempotent on arbitrary
    // input (see the module doc), but every input in the reference
    // table converges to a fixed point within a small number of
    // iterations.
    for &(input, _expected) in PAIRS {
        let mut cur = PortugueseSnowball.stem(input).into_owned();
        let mut steps = 0;
        loop {
            let next = PortugueseSnowball.stem(&cur).into_owned();
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
