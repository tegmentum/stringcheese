//! PHONEX-Bengali reference input/output pairs.
//!
//! A curated set of Bengali words that exercise the two-stage
//! encoder: first the ISO 15919 transliteration
//! (schwa-preserving, retroflex-under-dot, sibilant-diacritic),
//! then the Soundex-shape 4-character reduction (ASCII fold, vowel
//! reset, consonant-class classification).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_bn::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_bn::BengaliPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Bengali key).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Simple bare consonants + inherent schwa.
    // -----------------------------------------------------------------
    // ক → "ka" → "K000" (K seed, A vow reset, pad).
    ("ক", "K000"),
    // গ → "ga" → "G000" (G in class 2 seed, A vow reset, pad).
    ("গ", "G000"),
    // -----------------------------------------------------------------
    // Independent vowels — the seed is the vowel itself.
    // -----------------------------------------------------------------
    // অ → "a" → "A000" (A seed, pad).
    ("অ", "A000"),
    // ই → "i" → "I000" (I seed, pad).
    ("ই", "I000"),
    // -----------------------------------------------------------------
    // Bengali names / common words.
    // -----------------------------------------------------------------
    // রাম → "rāma" → RAMA → R seed, A vow, M code=5, A vow reset → "R5" → "R500".
    ("রাম", "R500"),
    // কমল → "kamala" → KAMALA → K seed, A vow, M code=5, A vow,
    //   L code=4, A vow reset → "K54" → "K540".
    ("কমল", "K540"),
    // সত্য → "satya" → SATYA → S seed, A vow, T code=3, Y vow reset,
    //   A vow reset → "S3" → "S300".
    ("সত্য", "S300"),
    // নমস্তে → "namaste" → NAMASTE → N seed, A vow, M code=5, A vow,
    //   S code=7, T code=3, E vow reset → "N573" → "N573".
    ("নমস্তে", "N573"),
    // বাংলা → "bāṁlā" → BAMLA → B seed, A vow, M code=5, L code=4,
    //   A vow reset → "B54" → "B540".
    ("বাংলা", "B540"),
    // বই → "bai" → BAI → B seed, A vow, I vow → "B" → "B000".
    ("বই", "B000"),
    // -----------------------------------------------------------------
    // Retroflex under-dot fold — ট → T.
    // -----------------------------------------------------------------
    // ট → "ṭa" → TA → T seed, A vow reset → "T" → "T000".
    ("ট", "T000"),
    // -----------------------------------------------------------------
    // Sibilant fold — শ → S, ষ → S, স → S.
    // -----------------------------------------------------------------
    ("শ", "S000"), // ś → S → "S000"
    ("ষ", "S000"), // ṣ → S → "S000"
    ("স", "S000"), // s → S → "S000"
    // -----------------------------------------------------------------
    // ধর্ম → "dharma" → DHARMA → D seed, H drops? No — H is not
    //   dropped in this encoder (unlike Czech's silent-H). D seed, H
    //   is class 0 vowel-reset, A vow, R code=6, M code=5, A vow reset
    //   → "D65" → "D650".
    // Actually H in phonex Soundex is class 0. Let me trace:
    //   D H A R M A → D seed last=3. H code=0 (vowel-like reset).
    //   A code=0 reset. R code=6 push → "D6" last=6. M code=5 push
    //   → "D65" last=5. A code=0 reset. Pad → "D650".
    // -----------------------------------------------------------------
    ("ধর্ম", "D650"),
    // -----------------------------------------------------------------
    // Digit — passes through as ASCII, non-letter so filtered.
    // ২০২৬ → "2026". After fold_to_ascii_upper, empty. So None.
    // Instead include a mixed example.
    // -----------------------------------------------------------------
    // চাঁদ → "cām̐da" → CAMDA → C seed, A vow, M code=5, D code=3,
    //   A vow reset → "C53" → "C530".
    ("চাঁদ", "C530"),
    // -----------------------------------------------------------------
    // Non-Bengali should still work (adapter checks for Bengali content
    // separately, but direct BengaliPhonex.encode runs on transliterated
    // input which for pure ASCII is the input unchanged).
    // hello → HELLO → H seed, E vow, L code=4, L dup drop, O vow reset
    //   → "H4" → "H400".
    // -----------------------------------------------------------------
    ("hello", "H400"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = BengaliPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-BN({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Bengali reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 15 pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn retroflex_fold_produces_the_same_key() {
    // Retroflex ট and dental ত both encode to T under fold_letter.
    // ট → "ṭa" → TA → T000. ত → "ta" → TA → T000. Same key.
    assert_eq!(BengaliPhonex.encode("ট"), BengaliPhonex.encode("ত"));
}

#[test]
fn sibilant_folds_produce_the_same_key() {
    // শ, ষ, স all fold to S.
    let a = BengaliPhonex.encode("শ").unwrap();
    let b = BengaliPhonex.encode("ষ").unwrap();
    let c = BengaliPhonex.encode("স").unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn long_vowel_folds_produce_the_same_key() {
    // Long vowel matra ā and short schwa a both fold to A → vowel
    // reset. So রম and রাম differ only in the presence of ā, which
    // doesn't matter to the phonex code.
    assert_eq!(BengaliPhonex.encode("রম"), BengaliPhonex.encode("রাম"));
}
