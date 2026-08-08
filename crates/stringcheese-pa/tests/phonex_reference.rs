//! PHONEX-Punjabi reference input/output pairs.
//!
//! A curated set of Punjabi words that exercise the two-stage
//! encoder: first the ISO 15919 transliteration (schwa-preserving,
//! retroflex-under-dot, sibilant-diacritic, addak-gemination,
//! tippi/bindi nasalization), then the tone-collapse pre-pass
//! (`gh → k`, `jh → c`, `ḍh → ṭ`, `dh → t`, `bh → p`) that folds
//! the historical voiced-aspirate letters to their voiceless-
//! unaspirated counterparts, then the Soundex-shape 4-character
//! reduction (ASCII fold, vowel reset, consonant-class classification).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_pa::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_pa::PunjabiPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Punjabi key).
///
/// Note: the seed is the first ASCII letter of the transliteration,
/// but every subsequent output position is a Soundex *digit* (not a
/// letter). So `ਕਰ` → "kara" → seed 'K', then R pushes its class code
/// '6' → "K6" pad → "K600" (never "KR00").
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Simple bare consonants + inherent schwa.
    // -----------------------------------------------------------------
    // ਕ → "ka" → K seed, A vow reset, pad → "K000".
    ("ਕ", "K000"),
    // ਗ → "ga" → G seed (class 2), A vow reset, pad → "G000".
    ("ਗ", "G000"),
    // -----------------------------------------------------------------
    // Independent vowels — the seed is the vowel itself.
    // -----------------------------------------------------------------
    // ਅ → "a" → A seed, pad → "A000".
    ("ਅ", "A000"),
    // ਇ → "i" → I seed, pad → "I000".
    ("ਇ", "I000"),
    // -----------------------------------------------------------------
    // Punjabi names / common words.
    // -----------------------------------------------------------------
    // ਪੰਜਾਬ → "paṁjāba" → P seed, A vow, M pushes '5', J pushes '2',
    //   A vow, B pushes '1' → "P521" (4 chars, break).
    ("ਪੰਜਾਬ", "P521"),
    // ਘਰ → "ghara" → tone-collapse "kara" → K seed, A vow, R pushes '6',
    //   A vow → "K6" pad → "K600".
    ("ਘਰ", "K600"),
    // ਕਰ → "kara" → K seed, A vow, R pushes '6' → "K600".
    ("ਕਰ", "K600"),
    // ਗੁਰੂ → "gurū" → G seed, U vow, R pushes '6', U vow → "G6" pad
    //   → "G600".
    ("ਗੁਰੂ", "G600"),
    // ਪੱਕਾ → "pakkā" → P seed (1), A vow, K pushes '2', K dup drop,
    //   A vow → "P2" pad → "P200".
    ("ਪੱਕਾ", "P200"),
    // ਰਾਮ → "rāma" → R seed, A vow, M pushes '5', A vow → "R5" pad
    //   → "R500".
    ("ਰਾਮ", "R500"),
    // ਸਿੰਘ (Singh) → "siṁgha" → tone-collapse "siṁka" → S seed, I vow,
    //   M pushes '5', K pushes '2', A vow → "S52" pad → "S520".
    ("ਸਿੰਘ", "S520"),
    // ਮੈਂ → "maim̐" → M seed, A vow, I vow, M pushes '5' → "M5" pad
    //   → "M500".
    ("ਮੈਂ", "M500"),
    // ਸਤਿ → "sati" → S seed, A vow, T pushes '3', I vow → "S3" pad
    //   → "S300".
    ("ਸਤਿ", "S300"),
    // -----------------------------------------------------------------
    // Retroflex under-dot fold — ਟ (ṭ) folds to T.
    // -----------------------------------------------------------------
    // ਟ → "ṭa" → T seed, A vow → "T000".
    ("ਟ", "T000"),
    // -----------------------------------------------------------------
    // Perso-Arabic loans.
    // -----------------------------------------------------------------
    // ਖ਼ਬਰ → "xabara" → X seed (class 2), A vow, B pushes '1', A vow,
    //   R pushes '6', A vow → "X16" pad → "X160".
    ("ਖ਼ਬਰ", "X160"),
    // ਜ਼ੋਰ → "zora" → Z seed (7), O vow, R pushes '6', A vow → "Z6"
    //   pad → "Z600".
    ("ਜ਼ੋਰ", "Z600"),
    // -----------------------------------------------------------------
    // Tone-bearing letters collapse — ਭਾਰਤ (Bhārat, India).
    // ਭਾਰਤ → "bhārata" → tone-collapse "pārata" → P seed (1), A vow,
    //   R pushes '6', A vow, T pushes '3', A vow → "P63" pad → "P630".
    // -----------------------------------------------------------------
    ("ਭਾਰਤ", "P630"),
    // -----------------------------------------------------------------
    // Non-Punjabi passes through the transliterator (direct
    // `PunjabiPhonex.encode` runs on transliterated input which for
    // pure ASCII is the input unchanged).
    // hello → HELLO → H seed, E vow, L pushes '4', L dup drop, O vow
    //   → "H4" pad → "H400".
    // -----------------------------------------------------------------
    ("hello", "H400"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = PunjabiPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-PA({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Punjabi reference pair(s) disagreed:\n{}",
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
fn tone_collapse_produces_the_same_key() {
    // ਘਰ (ghara → kara) and ਕਰ (kara) share the tone-collapsed key.
    assert_eq!(PunjabiPhonex.encode("ਘਰ"), PunjabiPhonex.encode("ਕਰ"));
    // ਭਾਰ (bhāra → pāra) and ਪਾਰ (pāra) share.
    assert_eq!(PunjabiPhonex.encode("ਭਾਰ"), PunjabiPhonex.encode("ਪਾਰ"));
    // ਧਰ (dhara → tara) and ਤਰ (tara) share.
    assert_eq!(PunjabiPhonex.encode("ਧਰ"), PunjabiPhonex.encode("ਤਰ"));
}

#[test]
fn retroflex_fold_produces_the_same_key() {
    // Retroflex ਟ and dental ਤ both encode to T under fold_letter.
    assert_eq!(PunjabiPhonex.encode("ਟ"), PunjabiPhonex.encode("ਤ"));
}

#[test]
fn long_vowel_folds_produce_the_same_key() {
    // Long vowel matra ā and short schwa a both fold to A → vowel
    // reset. So ਕਰ and ਕਾਰ differ only in the presence of ā, which
    // doesn't matter to the phonex code.
    assert_eq!(PunjabiPhonex.encode("ਕਰ"), PunjabiPhonex.encode("ਕਾਰ"));
}
