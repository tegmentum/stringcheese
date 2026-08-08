//! PHONEX-Thai reference input/output pairs.
//!
//! A curated set of Thai words that exercise the two-stage encoder:
//! first the Royal-Thai-Romanization-family consonant fold (with
//! vowel marks / tone marks / signs dropped), then the Soundex-shape
//! 4-character reduction (seed letter, class digits, duplicate
//! collapse, zero pad).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_th::phonetic`] — see there
//! for the family and classification tables.

extern crate alloc;

use stringcheese_th::ThaiPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Thai key).
///
/// Traces:
/// - ก → K → "K000".
/// - กา → K (า drops) → "K000".
/// - ก่า → K (่ and า drop) → "K000".
/// - ทท → TT → collapse T → "T000".
/// - ตต → TT → "T000".
/// - สวัสดี → S W (ั drops) S D (ี drops) → SWSD.
///   Trace: S seed, W code=1 push → "S1"; S code=7 push → "S17";
///   D code=3 push → "S173". Truncated at length 4.
/// - ประเทศ → P R (ะ drops) (เ drops) T (ศ → S). Trace: P seed,
///   R code=6 push → "P6"; T code=3 push → "P63"; S code=7 push
///   → "P637".
/// - ไทย → (ไ drops) T Y (Y is vowel class 0). Trace: T seed;
///   Y code=0 vowel reset; end → "T" pad → "T000".
/// - ฉัน → C (ั drops) N. Trace: C seed, N code=5 → "C5" pad → "C500".
/// - นัก → N (ั drops) K. Trace: N seed, K code=2 → "N2" pad → "N200".
/// - หมาย → H M A Y (H seed, M code=5, A code=0 reset, Y code=0
///   reset). "H5" pad → "H500".
/// - รัก → R (ั drops) K. Trace: R seed, K code=2 → "R2" pad → "R200".
/// - ครับ → K R (ั drops) B. Trace: K seed, R code=6 push → "K6";
///   B code=1 push → "K61" pad → "K610".
/// - ค่ะ → K (่ drops) (ะ drops). Trace: K seed, no more → "K000".
/// - ผู้ → P (ู drops) (้ drops). Trace: P seed → "P000".
/// - ซ, ศ, ษ, ส all → S → "S000".
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Bare consonants + inherent vowel drop.
    // -----------------------------------------------------------------
    ("ก", "K000"),
    ("กา", "K000"),
    ("ก่า", "K000"),
    // -----------------------------------------------------------------
    // Consonant-family folds.
    // -----------------------------------------------------------------
    ("ทท", "T000"), // TT collapse
    ("ตต", "T000"), // same fold
    ("ซ", "S000"),  // so so → S
    ("ศ", "S000"),  // so sala → S
    ("ษ", "S000"),  // so ruesi → S
    ("ส", "S000"),  // so sua → S
    // -----------------------------------------------------------------
    // Common Thai words.
    // -----------------------------------------------------------------
    ("สวัสดี", "S173"),   // S W S D
    ("ประเทศ", "P637"), // P R T S
    ("ไทย", "T000"),    // T Y (Y is class 0)
    ("ฉัน", "C500"),     // C N
    ("นัก", "N200"),     // N K
    ("รัก", "R200"),     // R K
    ("ครับ", "K610"),    // K R B
    ("ค่ะ", "K000"),     // K
    ("ผู้", "P000"),      // P
    // -----------------------------------------------------------------
    // Non-Thai input still runs through fold_thai_to_ascii — ASCII
    // letters uppercase and pass through.
    // -----------------------------------------------------------------
    ("hello", "H400"), // H seed, E vow, L push=4, L dup drop, O vow → "H4" pad → "H400".
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = ThaiPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-TH({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Thai reference pair(s) disagreed:\n{}",
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
fn sibilants_produce_the_same_key() {
    // ซ, ศ, ษ, ส all fold to S.
    let a = ThaiPhonex.encode("ซ").unwrap();
    let b = ThaiPhonex.encode("ศ").unwrap();
    let c = ThaiPhonex.encode("ษ").unwrap();
    let d = ThaiPhonex.encode("ส").unwrap();
    assert_eq!(a, b);
    assert_eq!(b, c);
    assert_eq!(c, d);
}

#[test]
fn dentals_produce_the_same_key() {
    // ท and ต both fold to T.
    let a = ThaiPhonex.encode("ท").unwrap();
    let b = ThaiPhonex.encode("ต").unwrap();
    assert_eq!(a, b);
}

#[test]
fn tone_marks_are_ignored() {
    // ก, ก่, ก้, ก๊, ก๋ all encode identically — tone marks drop.
    let base = ThaiPhonex.encode("ก").unwrap();
    for tone in ["ก่", "ก้", "ก๊", "ก๋"] {
        assert_eq!(
            ThaiPhonex.encode(tone).unwrap(),
            base,
            "tone-mark variant {tone:?} should equal base key {base:?}",
        );
    }
}
