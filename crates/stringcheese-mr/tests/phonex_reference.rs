//! PHONEX-Marathi reference input/output pairs.
//!
//! A curated set of Marathi words that exercise the two-stage
//! encoder: first the ISO 15919 transliteration (schwa-preserving,
//! retroflex-under-dot, sibilant-diacritic, Marathi-specific
//! `ḷ`/`ṟ`), then the Soundex-shape 4-character reduction (ASCII
//! fold, vowel reset, consonant-class classification).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_mr::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_mr::MarathiPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Marathi key).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Simple bare consonants + inherent schwa.
    // -----------------------------------------------------------------
    // क → "ka" → "K000" (K seed, A vow reset, pad).
    ("क", "K000"),
    // ग → "ga" → "G000" (G in class 2 seed, A vow reset, pad).
    ("ग", "G000"),
    // -----------------------------------------------------------------
    // Independent vowels — the seed is the vowel itself.
    // -----------------------------------------------------------------
    // अ → "a" → "A000" (A seed, pad).
    ("अ", "A000"),
    // इ → "i" → "I000" (I seed, pad).
    ("इ", "I000"),
    // -----------------------------------------------------------------
    // Marathi-specific letters — the two additions Standard Hindi
    // lacks.
    // -----------------------------------------------------------------
    // ळ → "ḷa" → LA → "L000" (retroflex-L folds to L).
    ("ळ", "L000"),
    // ऱ → "ṟa" → RA → "R000" (R-with-bar folds to R).
    ("ऱ", "R000"),
    // कमळ "lotus" — Marathi word using ळ. K seed, A vow, M code=5,
    //   A vow, L code=4, A vow reset → "K54" → "K540".
    ("कमळ", "K540"),
    // -----------------------------------------------------------------
    // Marathi names / common words.
    // -----------------------------------------------------------------
    // राम → "rāma" → RAMA → R seed, A vow, M code=5, A vow reset → "R500".
    ("राम", "R500"),
    // सत्य → "satya" → SATYA → S seed, A vow, T code=3, Y vow, A vow → "S3" → "S300".
    ("सत्य", "S300"),
    // नमस्ते → "namaste" → NAMASTE → N seed, A vow, M code=5, A vow,
    //   S code=7, T code=3, E vow reset → "N573".
    ("नमस्ते", "N573"),
    // मराठी → "marāṭhī" → MARATHI → M seed, A vow, R code=6, A vow,
    //   T code=3 (retroflex ṭ folds to T), H code=0 (vowel-like reset),
    //   I vow → "M63" → "M630".
    ("मराठी", "M630"),
    // पुस्तक "book" → "pustaka" → PUSTAKA → P seed, U vow, S code=7,
    //   T code=3, A vow, K code=2, A vow reset → "P732" → "P732".
    ("पुस्तक", "P732"),
    // -----------------------------------------------------------------
    // Retroflex under-dot fold — ट → T.
    // -----------------------------------------------------------------
    // ट → "ṭa" → TA → "T000".
    ("ट", "T000"),
    // -----------------------------------------------------------------
    // Sibilant fold — शा → S, ष → S, स → S.
    // -----------------------------------------------------------------
    ("श", "S000"), // ś → S
    ("ष", "S000"), // ṣ → S
    ("स", "S000"), // s → S
    // -----------------------------------------------------------------
    // धर्म → "dharma" → DHARMA → D seed last=3. H code=0 (vowel-like reset).
    //   A vow reset. R code=6 push → "D6". M code=5 push → "D65". A vow reset.
    //   Pad → "D650".
    // -----------------------------------------------------------------
    ("धर्म", "D650"),
    // -----------------------------------------------------------------
    // Non-Devanagari should still work at the PHONEX layer (adapter
    // filters by Devanagari-content, but bare MarathiPhonex.encode
    // runs on transliterated input which for pure ASCII is the input
    // unchanged).
    // hello → HELLO → H seed, E vow, L code=4, L dup drop, O vow reset
    //   → "H4" → "H400".
    // -----------------------------------------------------------------
    ("hello", "H400"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = MarathiPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-MR({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Marathi reference pair(s) disagreed:\n{}",
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
fn marathi_retroflex_l_folds_to_dental_l() {
    // ळ (retroflex ḷa) and ल (dental la) both fold to L under the
    // phonex-level ASCII fold. Same key.
    assert_eq!(MarathiPhonex.encode("ळ"), MarathiPhonex.encode("ल"));
}

#[test]
fn marathi_r_with_bar_folds_to_dental_r() {
    // ऱ (ṟa) and र (ra) both fold to R.
    assert_eq!(MarathiPhonex.encode("ऱ"), MarathiPhonex.encode("र"));
}

#[test]
fn retroflex_fold_produces_the_same_key() {
    // ट (ṭa) and त (ta) both fold to T.
    assert_eq!(MarathiPhonex.encode("ट"), MarathiPhonex.encode("त"));
}

#[test]
fn sibilant_folds_produce_the_same_key() {
    // श, ष, स all fold to S.
    let a = MarathiPhonex.encode("श").unwrap();
    let b = MarathiPhonex.encode("ष").unwrap();
    let c = MarathiPhonex.encode("स").unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn long_vowel_folds_produce_the_same_key() {
    // Long vowel matra ā and short schwa a both fold to A → vowel
    // reset. So रम and राम differ only in the presence of ā.
    assert_eq!(MarathiPhonex.encode("रम"), MarathiPhonex.encode("राम"));
}
