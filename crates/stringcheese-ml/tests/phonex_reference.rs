//! PHONEX-Malayalam reference input/output pairs.
//!
//! A curated set of Malayalam words that exercise the two-stage
//! encoder: first the ISO 15919 transliteration (schwa-preserving,
//! retroflex-under-dot, alveolar-under-bar, retroflex-approximant
//! `ḻ`, and chillu-letter suppression), then the Soundex-shape
//! 4-character reduction (ASCII fold, vowel reset, consonant-class
//! classification). The output format is standard Soundex-shape:
//! **letter seed followed by numeric class codes**, e.g. "N500"
//! (N-seed for ÑĀN, then N=5 and two padding `0`s).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_ml::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_ml::MalayalamPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Malayalam key).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Simple bare consonants + inherent schwa. The vowel resets the
    // duplicate-collapse state but is not itself pushed to the code
    // output (only the seed letter is).
    // -----------------------------------------------------------------
    // ക → "ka" → K seed, A vow reset → pad "K000".
    ("ക", "K000"),
    // ത → "ta" → T seed, A vow reset → pad "T000".
    ("ത", "T000"),
    // പ → "pa" → P seed, A vow reset → pad "P000".
    ("പ", "P000"),
    // -----------------------------------------------------------------
    // Independent vowels — the seed is the vowel itself.
    // -----------------------------------------------------------------
    // അ → "a" → A seed → pad "A000".
    ("അ", "A000"),
    // ഇ → "i" → I seed → pad "I000".
    ("ഇ", "I000"),
    // ഉ → "u" → U seed → pad "U000".
    ("ഉ", "U000"),
    // -----------------------------------------------------------------
    // Malayalam words / common names.
    // -----------------------------------------------------------------
    // മലയാളം → "malayāḷaṁ" → MALAYALAM (ā → A, ḷ → L, ṁ → M).
    //   M seed last=5; A vow; L code=4 push '4', out="M4" last=4;
    //   A vow; Y code=0 (Y not classed); A vow; L code=4 push '4',
    //   out="M44" last=4; A vow; M code=5 push '5', out="M445" → done.
    ("മലയാളം", "M445"),
    // ഞാൻ → "ñān" → NAN → N seed last=5; A vow; N code=5 push '5',
    //   out="N5" last=5 → pad "N500".
    ("ഞാൻ", "N500"),
    // അവർ → "avar" → AVAR → A seed last=0; V code=1 push '1',
    //   out="A1" last=1; A vow; R code=6 push '6', out="A16" last=6
    //   → pad "A160".
    ("അവർ", "A160"),
    // ഇത് → "it" → IT → I seed last=0; T code=3 push '3', out="I3"
    //   last=3 → pad "I300".
    ("ഇത്", "I300"),
    // രാമൻ → "rāman" → RAMAN → R seed last=6; A vow last=0; M code=5
    //   push '5', out="R5" last=5; A vow last=0; N code=5 ≠ last=0
    //   push '5', out="R55" last=5. Pad "R550".
    ("രാമൻ", "R550"),
    // വീട് → "vīṭ" → VIT → V seed last=1; I vow; T code=3 push '3',
    //   out="V3" last=3 → pad "V300".
    ("വീട്", "V300"),
    // സത്യം → "satyaṁ" → SATYAM → S seed last=7; A vow; T code=3
    //   push '3', out="S3" last=3; Y vow-like last=0; A vow; M code=5
    //   push '5', out="S35" last=5 → pad "S350".
    ("സത്യം", "S350"),
    // പഠിക്കുന്നു → "paṭhikkunnu" → PATHIKKUNNU → P seed last=1;
    //   A vow; T code=3 push '3', out="P3" last=3; H not classed
    //   code=0 last=0; I vow; K code=2 push '2', out="P32" last=2;
    //   K dup; U vow; N code=5 push '5', out="P325" last=5, len=4 break.
    //   → "P325".
    ("പഠിക്കുന്നു", "P325"),
    // -----------------------------------------------------------------
    // Chillu-letter fold — each chillu shares its base's phonex code.
    // -----------------------------------------------------------------
    ("ൻ", "N000"), // chillu n → "n" → N000
    ("ർ", "R000"), // chillu r → "r" → R000
    ("ൽ", "L000"), // chillu l → "l" → L000
    ("ൾ", "L000"), // chillu ḷ → "ḷ" → L000
    ("ൺ", "N000"), // chillu ṇ → "ṇ" → N000
    // -----------------------------------------------------------------
    // Retroflex fold — ട → T, ണ → N.
    // -----------------------------------------------------------------
    ("ട", "T000"), // ṭa → T seed, A vow reset → "T000"
    ("ണ", "N000"), // ṇa → N seed, A vow reset → "N000"
    // -----------------------------------------------------------------
    // Alveolar fold — റ → R.
    // -----------------------------------------------------------------
    ("റ", "R000"), // ṟa → R seed, A vow reset → "R000"
    // -----------------------------------------------------------------
    // Retroflex L fold — ഴ → L, ള → L, ല → L.
    // -----------------------------------------------------------------
    ("ല", "L000"), // la → L seed, A vow reset → "L000"
    ("ള", "L000"), // ḷa → L seed, A vow reset → "L000"
    ("ഴ", "L000"), // ḻa → L seed, A vow reset → "L000"
    // -----------------------------------------------------------------
    // Non-Malayalam pass-through — the ISO 15919 encoder leaves ASCII
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
        let got = MalayalamPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-ML({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Malayalam reference pair(s) disagreed:\n{}",
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
    // Retroflex ട and dental ത both encode to T under fold_letter.
    // ട → "ṭa" → TA → T000. ത → "ta" → TA → T000. Same key.
    assert_eq!(MalayalamPhonex.encode("ട"), MalayalamPhonex.encode("ത"));
}

#[test]
fn retroflex_and_dental_nasals_produce_the_same_key() {
    // ണ (ṇa) and ന (na) both fold to N.
    assert_eq!(MalayalamPhonex.encode("ണ"), MalayalamPhonex.encode("ന"));
}

#[test]
fn all_l_variants_produce_the_same_key() {
    // ല (la), ള (ḷa), ഴ (ḻa) all fold to L.
    let a = MalayalamPhonex.encode("ല").unwrap();
    let b = MalayalamPhonex.encode("ള").unwrap();
    let c = MalayalamPhonex.encode("ഴ").unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn chillu_and_base_consonant_produce_the_same_key() {
    // Word-level equivalence: അൻ (a + chillu n) and അന (a + na)
    // both fold to A + N → A500.
    assert_eq!(MalayalamPhonex.encode("അൻ"), MalayalamPhonex.encode("അന"));
    // Word-level: രാമൻ (rāman) and രാമന (rāmana) share the phonex
    // key because vowel-reset collapses the trailing schwa.
    assert_eq!(MalayalamPhonex.encode("രാമൻ"), MalayalamPhonex.encode("രാമന"));
}

#[test]
fn long_vowel_folds_produce_the_same_key() {
    // Long vowel matra ā and short schwa a both fold to A → vowel
    // reset. So ക and കാ differ only in the presence of ā, which
    // doesn't matter to the phonex code.
    assert_eq!(MalayalamPhonex.encode("ക"), MalayalamPhonex.encode("കാ"));
}
