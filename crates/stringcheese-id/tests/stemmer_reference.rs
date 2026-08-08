//! Nazief-Adriani Indonesian stemmer reference input/output pairs.
//!
//! The pairs below are hand-traced against the module algorithm in
//! [`stringcheese_id::stemmer`], covering the five-step cascade
//! (stopword short-circuit, particle suffix, possessive suffix,
//! derivational suffix, derivational prefix with `me-`/`pe-`
//! consonant restoration). The list intentionally stays on
//! well-behaved forms — a handful of documented over-strips (`pergi`
//! → `perg`, `mengirim` → `irim`) are captured in the "documented
//! over-stripping" section so a regression re-litigating those
//! trade-offs flips a red test rather than passing silently.
//!
//! # Deferred: dictionary-backed cross-verification
//!
//! Sastrawi ships a 30 000-word Indonesian root-word dictionary; a
//! full-corpus test would run the shipped stemmer against the
//! dictionary-backed reference. Embedding that dictionary here would
//! bloat the repo; a follow-up wave could add a feature-gated
//! `nazief-fullcorpus` cfg reading the vocab file at build time.

extern crate alloc;

use stringcheese_id::IndonesianStemmer;

/// Reference pairs (input, expected stem after one call to
/// [`IndonesianStemmer::stem`]).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Trivial identity: short words already at their stem.
    // -------------------------------------------------------------
    ("ada", "ada"),
    ("api", "api"),
    ("air", "air"),
    // -------------------------------------------------------------
    // Stopword short-circuit — even though `-an` looks strippable
    // from `bukan`, the stopword table intercepts.
    // -------------------------------------------------------------
    ("dan", "dan"),
    ("yang", "yang"),
    ("adalah", "adalah"),
    ("bukan", "bukan"),
    ("dengan", "dengan"),
    // -------------------------------------------------------------
    // Particle suffixes — step 2.
    // -------------------------------------------------------------
    ("bacalah", "baca"),
    ("siapakah", "siapa"),
    ("apapun", "apa"),
    // -------------------------------------------------------------
    // Possessive suffixes — step 3.
    // -------------------------------------------------------------
    ("bukuku", "buku"),
    ("rumahmu", "rumah"),
    ("namanya", "nama"),
    ("tanganku", "tangan"),
    // -------------------------------------------------------------
    // Derivational suffixes — step 4.
    // -------------------------------------------------------------
    ("makanan", "makan"),
    ("jalanan", "jalan"),
    ("bacakan", "baca"),
    ("berikan", "beri"),
    ("panjangi", "panjang"),
    // -------------------------------------------------------------
    // Non-assimilating prefixes — step 5 easy cases.
    // -------------------------------------------------------------
    ("dibaca", "baca"),
    ("ditulis", "tulis"),
    ("berjalan", "jalan"),
    ("berlari", "lari"),
    ("terbaik", "baik"),
    ("terjatuh", "jatuh"),
    ("ketua", "tua"),
    ("seorang", "orang"),
    // Special `bel-` allomorph of `ber-`.
    ("belajar", "ajar"),
    // -------------------------------------------------------------
    // Assimilating `me-` prefixes — with consonant restoration.
    // -------------------------------------------------------------
    ("membaca", "baca"),
    ("membeli", "beli"),
    ("memilih", "pilih"),
    ("menulis", "tulis"),
    ("menari", "tari"),
    ("mendengar", "dengar"),
    ("menjaga", "jaga"),
    ("mengambil", "ambil"),
    ("menyapu", "sapu"),
    ("melihat", "lihat"),
    // -------------------------------------------------------------
    // Assimilating `pe-` prefixes — agent nominalizer.
    // -------------------------------------------------------------
    ("pemilih", "pilih"),
    ("penulis", "tulis"),
    // -------------------------------------------------------------
    // Circumfixes — prefix + suffix combos fall out for free from
    // the ordered suffix-then-prefix strip.
    // -------------------------------------------------------------
    ("perbuatan", "buat"),
    ("kesatuan", "satu"),
    // -------------------------------------------------------------
    // Documented over-strips — the no-dictionary variant trades
    // dictionary confirmation for over-strips on a handful of
    // shape-ambiguous inputs. Locking these in as reference pairs
    // so any change to the algorithm's rules re-litigates them
    // explicitly.
    // -------------------------------------------------------------
    ("mati", "mat"),      // `-i` strip fires (no prefix guard for `ma-`).
    ("mengirim", "irim"), // `meng-` ambiguity resolved as "no elision".
    // -------------------------------------------------------------
    // Regression: `pergi` is a root ("go"). The commit-on-shape
    // rule for the 3-letter `per-`/`ber-`/`ter-` prefixes prevents
    // the bare `pe-`+`r` (sonorant) rule from firing when a longer
    // 3-letter prefix's shape is present but its residue is too
    // short. Without the commit, `pergi` would strip to `rgi`.
    // -------------------------------------------------------------
    ("pergi", "pergi"),
];

#[test]
fn stemmer_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = IndonesianStemmer.stem(input).into_owned();
        if got != expected {
            failures.push(alloc::format!(
                "  IndonesianStemmer({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Nazief-Adriani reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // Sanity: aim for at least 30 pairs to exercise every step's
    // happy path plus the documented over-strips.
    assert!(
        PAIRS.len() >= 30,
        "reference pair count {} is below the 30-pair floor",
        PAIRS.len()
    );
}

#[test]
fn every_reference_pair_converges() {
    // The shipped stemmer's rules fire at most once per step per
    // call, but IR pipelines may call stem() on already-stemmed
    // inputs. Verify each pair reaches a fixed point in ≤ 5
    // additional iterations.
    for &(input, _expected) in PAIRS {
        let mut cur = IndonesianStemmer.stem(input).into_owned();
        let mut steps = 0;
        loop {
            let next = IndonesianStemmer.stem(&cur).into_owned();
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
