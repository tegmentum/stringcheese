//! PHONEX-Tamil reference input/output pairs.
//!
//! A curated set of Tamil words that exercise the two-stage encoder:
//! first the ISO 15919 transliteration (schwa-preserving,
//! retroflex-under-dot, alveolar-under-bar, retroflex-approximant
//! `ḻ`), then the Soundex-shape 4-character reduction (ASCII fold,
//! vowel reset, consonant-class classification). The output format is
//! standard Soundex-shape: **letter seed followed by numeric class
//! codes**, e.g. "T540" (T-seed for TAMIḺ, then codes M=5, L=4, and
//! a padding `0`).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_ta::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_ta::TamilPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Tamil key).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Simple bare consonants + inherent schwa. The vowel resets the
    // duplicate-collapse state but is not itself pushed to the code
    // output (only the seed letter is).
    // -----------------------------------------------------------------
    // க → "ka" → K seed, A vow reset → pad "K000".
    ("க", "K000"),
    // த → "ta" → T seed, A vow reset → pad "T000".
    ("த", "T000"),
    // ப → "pa" → P seed, A vow reset → pad "P000".
    ("ப", "P000"),
    // -----------------------------------------------------------------
    // Independent vowels — the seed is the vowel itself.
    // -----------------------------------------------------------------
    // அ → "a" → A seed → pad "A000".
    ("அ", "A000"),
    // இ → "i" → I seed → pad "I000".
    ("இ", "I000"),
    // உ → "u" → U seed → pad "U000".
    ("உ", "U000"),
    // -----------------------------------------------------------------
    // Tamil words / common names.
    // -----------------------------------------------------------------
    // தமிழ் → "tamiḻ" → TAMIL (ḻ → L) → T seed, A vow reset,
    //   M code=5 push, I vow reset, L code=4 push → "T54" pad "T540".
    ("தமிழ்", "T540"),
    // நான் → "nāṉ" → NAN → N seed last=5; A vow last=0;
    //   N code=5 ≠ 0 push '5', out="N5" → pad "N500".
    ("நான்", "N500"),
    // அவர் → "avar" → AVAR → A seed last=0; V code=1 push '1',
    //   out="A1" last=1; A vow last=0; R code=6 push '6', out="A16"
    //   last=6 → pad "A160".
    ("அவர்", "A160"),
    // இது → "itu" → ITU → I seed last=0; T code=3 push '3',
    //   out="I3" last=3; U vow last=0 → pad "I300".
    ("இது", "I300"),
    // மாணவர் → "māṇavar" → MANAVAR → M seed last=5; A vow;
    //   N code=5 push '5', out="M5" last=5; A vow;
    //   V code=1 push '1', out="M51" last=1; A vow;
    //   R code=6 push '6', out="M516" last=6 → "M516".
    ("மாணவர்", "M516"),
    // வீடு → "vīṭu" → VITU → V seed last=1; I vow;
    //   T code=3 push '3', out="V3" last=3; U vow → pad "V300".
    ("வீடு", "V300"),
    // நன்றி → "naṉṟi" → NANRI → N seed last=5; A vow;
    //   N code=5 push '5', out="N5" last=5;
    //   R code=6 push '6', out="N56" last=6; I vow → pad "N560".
    ("நன்றி", "N560"),
    // பேசுகிறேன் → "pēcukiṟēṉ" → PECUKIREN → P seed last=1; E vow;
    //   C code=2 push '2', out="P2" last=2; U vow;
    //   K code=2 push '2', out="P22" last=2; I vow;
    //   R code=6 push '6', out="P226" last=6, break at 4 → "P226".
    ("பேசுகிறேன்", "P226"),
    // -----------------------------------------------------------------
    // Retroflex fold — ட → T, ண → N.
    // -----------------------------------------------------------------
    ("ட", "T000"), // ṭa → T seed, A vow reset → "T000"
    ("ண", "N000"), // ṇa → N seed, A vow reset → "N000"
    // -----------------------------------------------------------------
    // Alveolar fold — ன → N, ற → R.
    // -----------------------------------------------------------------
    ("ன", "N000"), // ṉa → N seed, A vow reset → "N000"
    ("ற", "R000"), // ṟa → R seed, A vow reset → "R000"
    // -----------------------------------------------------------------
    // Retroflex L fold — ழ → L, ள → L, ல → L.
    // -----------------------------------------------------------------
    ("ல", "L000"), // la → L seed, A vow reset → "L000"
    ("ள", "L000"), // ḷa → L seed, A vow reset → "L000"
    ("ழ", "L000"), // ḻa → L seed, A vow reset → "L000"
    // -----------------------------------------------------------------
    // Grantha loans.
    // -----------------------------------------------------------------
    ("ஜ", "J000"), // ja → J seed, A vow → "J000"
    ("ஷ", "S000"), // ṣa → S seed, A vow → "S000"
    ("ஸ", "S000"), // sa → S seed, A vow → "S000"
    ("ஹ", "H000"), // ha → H seed, A vow → "H000"
    // -----------------------------------------------------------------
    // Non-Tamil pass-through — the ISO 15919 encoder leaves ASCII
    // alone. hello → HELLO → H seed last=0 (H not in codes); E vow;
    // L code=4 push '4', out="H4" last=4; L dup drop; O vow → pad
    // "H400".
    // -----------------------------------------------------------------
    ("hello", "H400"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = TamilPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-TA({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Tamil reference pair(s) disagreed:\n{}",
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
    // Retroflex ட and dental த both encode to T under fold_letter.
    // ட → "ṭa" → TA → T000. த → "ta" → TA → T000. Same key.
    assert_eq!(TamilPhonex.encode("ட"), TamilPhonex.encode("த"));
}

#[test]
fn alveolar_and_dental_nasals_produce_the_same_key() {
    // ன (alveolar ṉa) and ந (dental na) both fold to N.
    assert_eq!(TamilPhonex.encode("ன"), TamilPhonex.encode("ந"));
}

#[test]
fn all_l_variants_produce_the_same_key() {
    // ல (la), ள (ḷa), ழ (ḻa) all fold to L.
    let a = TamilPhonex.encode("ல").unwrap();
    let b = TamilPhonex.encode("ள").unwrap();
    let c = TamilPhonex.encode("ழ").unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn long_vowel_folds_produce_the_same_key() {
    // Long vowel matra ā and short schwa a both fold to A → vowel
    // reset. So க and கா differ only in the presence of ā, which
    // doesn't matter to the phonex code.
    assert_eq!(TamilPhonex.encode("க"), TamilPhonex.encode("கா"));
}
