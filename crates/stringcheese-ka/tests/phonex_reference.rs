//! PHONEX-Georgian reference input/output pairs.
//!
//! A curated set of Georgian words that exercise the two-stage
//! encoder: first the ISO 9984 Georgian -> Latin transliteration
//! (with the ejective apostrophe folded away), then the Soundex-shape
//! 4-character reduction (ASCII fold, vowel reset, consonant-class
//! classification).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_ka::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_ka::GeorgianPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Georgian key).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Bare consonants — seed + zero-padding.
    // -----------------------------------------------------------------
    ("ბ", "B000"), // ban -> B
    ("ს", "S000"), // san -> S
    ("რ", "R000"), // rae -> R
    ("მ", "M000"), // man -> M
    // -----------------------------------------------------------------
    // Ejective / aspirate pairs fold to the same PHONEX class after
    // the ISO 9984 apostrophe drops.
    // -----------------------------------------------------------------
    ("ტ", "T000"), // t' (ejective) -> t -> T (class 3)
    ("თ", "T000"), // t (aspirate)  -> t -> T (class 3)
    ("კ", "K000"), // k' (ejective) -> k -> K (class 2)
    ("ქ", "K000"), // k (aspirate)  -> k -> K (class 2)
    // -----------------------------------------------------------------
    // Common names / place-names.
    // -----------------------------------------------------------------
    // თბილისი "Tbilisi": TBILISI -> T seed, B=1, I vow, L=4, I vow,
    // S=7, I vow -> "T147".
    ("თბილისი", "T147"),
    // ქართული "Georgian": KARTULI -> K seed, A vow, R=6, T=3, U vow,
    // L=4, I vow -> "K634".
    ("ქართული", "K634"),
    // ქალაქი "city": KALAKI -> K seed, A vow, L=4, A vow, K=2, I vow
    // -> "K42" -> "K420".
    ("ქალაქი", "K420"),
    // მამა "father": MAMA -> M seed, A vow, M=5, A vow -> "M5" -> "M500".
    ("მამა", "M500"),
    // დედა "mother": DEDA -> D seed, E vow, D=3 (pushed after vowel
    // reset), A vow -> "D3" -> "D300".
    ("დედა", "D300"),
    // ცხენი "horse": TSKHENI -> T seed, S=7, K=2, H vow, E vow,
    // N=5, I vow -> "T725".
    ("ცხენი", "T725"),
    // შავი "black": SHAVI -> S seed, H vow, A vow, V=1, I vow ->
    // "S1" -> "S100".
    ("შავი", "S100"),
    // ღამე "night": GHAME -> G seed, H vow, A vow, M=5, E vow ->
    // "G5" -> "G500".
    ("ღამე", "G500"),
    // ბაბუა "grandfather": BABUA -> B seed, A vow, B=1 (pushed
    // after vowel reset), U vow, A vow -> "B1" -> "B100".
    ("ბაბუა", "B100"),
    // გამარჯობა "hello": GAMARJOBA -> G seed, A vow, M=5, A vow,
    // R=6, J=2, O vow, B=1 (truncated at 4) -> "G562" (stops at
    // out.len()==4).
    ("გამარჯობა", "G562"),
    // -----------------------------------------------------------------
    // Empty / non-Georgian.
    // -----------------------------------------------------------------
    // Direct GeorgianPhonex.encode on a bare ASCII word runs its
    // Latin-uppercase fold; the adapter guards against no-Georgian
    // input separately.
    // -----------------------------------------------------------------
    ("hello", "H400"), // H seed, E vow, L=4, L dup drop, O vow -> "H4" -> "H400"
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = GeorgianPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-KA({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Georgian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // Task-family convention: at least 15 pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn ejective_and_aspirate_pairs_produce_the_same_key() {
    // ტ (t' ejective) and თ (t aspirate) both fold to Latin `t` and
    // produce the same phonex class-3 key.
    assert_eq!(GeorgianPhonex.encode("ტ"), GeorgianPhonex.encode("თ"));
    // კ (k' ejective) and ქ (k aspirate) both fold to `k` -> class 2.
    assert_eq!(GeorgianPhonex.encode("კ"), GeorgianPhonex.encode("ქ"));
    // პ (p' ejective) and ფ (p aspirate) both fold to `p` -> class 1.
    assert_eq!(GeorgianPhonex.encode("პ"), GeorgianPhonex.encode("ფ"));
    // წ (ts' ejective) and ც (ts aspirate) both fold to `ts` ->
    // class 3 + class 7.
    assert_eq!(GeorgianPhonex.encode("წ"), GeorgianPhonex.encode("ც"));
    // ჭ (ch' ejective) and ჩ (ch aspirate) both fold to `ch` ->
    // class 2 (C).
    assert_eq!(GeorgianPhonex.encode("ჭ"), GeorgianPhonex.encode("ჩ"));
}
