//! PHONEX-Amharic reference input/output pairs.
//!
//! A curated set of Amharic words that exercise the two-stage
//! encoder: first the Ge'ez → BGN/PCGN-style Latin transliteration
//! (per-syllable consonant + vowel decomposition via
//! [`stringcheese_am::geez::decompose`]), then the Soundex-shape
//! 4-character reduction (ASCII fold, vowel reset, consonant-class
//! classification).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_am::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_am::AmharicPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Amharic key).
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Single-syllable inputs.
    // -----------------------------------------------------------------
    // ሀ (h family, order 0) → "he" → strip apostrophe? no, no apostrophe here.
    // fold → "HE". H is a vowel (class 0), so seed = H, last=0.
    // E is a vowel → reset. Pad → "H000".
    ("ሀ", "H000"),
    // መ (m family, order 0) → "me" → "ME" → M seed, code_of(M)=5, last=5.
    // E vowel → reset. Pad → "M000".
    ("መ", "M000"),
    // ለ (l family, order 0) → "le" → "LE" → L seed, L is L=4, last=4.
    // E vowel → reset. Pad → "L000".
    ("ለ", "L000"),
    // -----------------------------------------------------------------
    // Multi-syllable words.
    // -----------------------------------------------------------------
    // አማርኛ = አ + ማ + ር + ኛ → "'e" + "ma" + "r" + "Na" = "'emarNa"
    // Strip non-letters: "EMARNA".
    // E seed. M code=5 push → "E5". A vow reset. R code=6 push
    // → "E56". N code=5 push → "E565". A vow reset. Length 4 done.
    ("አማርኛ", "E565"),
    // ኢትዮጵያ = ኢ + ት + ዮ + ጵ + ያ → "'i" + "t" + "yo" + "P" + "ya"
    //   = "'ityoPya". Strip non-letters: "ITYOPYA".
    // I seed, last=0. T code=3 push → "I3". Y is vow (H/Y both
    // classified as vowels in this reducer's `_ => 0` default),
    // reset last=0. O vow reset. P code=1 push → "I31". Y vow reset.
    // A vow reset. Length 3, pad → "I310".
    ("ኢትዮጵያ", "I310"),
    // እኔ = እ + ኔ → order 5 of ' family = "'" (empty vowel) plus
    //   order 4 of n family = "nie" → "'nie".
    // Strip non-letters: "NIE". N seed, last=5. I vow reset. E vow reset.
    // Pad → "N000".
    ("እኔ", "N000"),
    // ኢትዮጵ (short of Ethiopia) = ኢ + ት + ዮ + ጵ → "'ityoP".
    // Strip: "ITYOP". I seed, last=0. T code=3 push → "I3". Y vow
    // reset. O vow reset. P code=1 push → "I31". Length 3, pad → "I310".
    ("ኢትዮጵ", "I310"),
    // ግን = ግ + ን → order 5 of g family = "g" + order 5 of n family = "n"
    //   → "gn". Strip: "GN". G seed, code_of(G)=2, last=2. N code=5 push
    //   → "G5". Length 2, pad → "G500".
    ("ግን", "G500"),
    // ነው = ነ + ው → "ne" + "w" → "new". Strip: "NEW".
    // N seed, last=5. E vow reset. W code=1 push → "N1". Pad → "N100".
    ("ነው", "N100"),
    // ሰላም = ሰ + ላ + ም → "se" + "la" + "m" → "selam". Strip: "SELAM".
    // S seed, last=7. E vow reset. L code=4 push → "S4". A vow reset.
    // M code=5 push → "S45". Length 3, pad → "S450".
    ("ሰላም", "S450"),
    // ልጅ = ል + ጅ → order 5 of l family = "l" + order 5 of j family = "j"
    //   → "lj". Strip: "LJ". L seed, last=4. J code=2 push → "L2".
    //   Pad → "L200".
    ("ልጅ", "L200"),
    // ቤት = ቤ + ት → order 4 of b family = "bie" + order 5 of t family
    //   = "t" → "biet". Strip: "BIET". B seed, code_of(B)=1, last=1.
    //   I vow reset. E vow reset. T code=3 push → "B3". Length 2, pad
    //   → "B300".
    ("ቤት", "B300"),
    // ወይ = ወ + ይ → order 0 of w family = "we" + order 5 of y family = "y"
    //   → "wey". Strip: "WEY". W seed, code_of(W)=1, last=1. E vow reset.
    //   Y is a vowel (class 0), reset. Pad → "W000".
    ("ወይ", "W000"),
    // ዘመን = ዘ + መ + ን → "ze" + "me" + "n" → "zemen". Strip: "ZEMEN".
    // Z seed, code_of(Z)=7, last=7. E vow reset. M code=5 push → "Z5".
    // E vow reset. N code=5 push? Wait — after E, last_code was reset
    // to 0. So next N is code=5, != 0, push → "Z55"? No — wait, the
    // rule is `if code == last_code { continue }`, so after M pushed
    // last_code=5, then E reset to 0, then N=5 != 0 pushes. Result
    // "Z55" is wrong: after E resets last_code=0, and after N=5 is
    // pushed last_code=5. So output = "Z55"? Actually output is
    // ["Z", "5", "5"] which is "Z55", length 3. Pad to "Z550".
    ("ዘመን", "Z550"),
    // ጊዜ = ጊ + ዜ → order 2 of g family = "gi" + order 4 of z family
    //   = "zie" → "gizie". Strip: "GIZIE". G seed, code_of(G)=2, last=2.
    //   I vow reset. Z code=7 push → "G7". I vow reset. E vow reset.
    //   Pad → "G700".
    ("ጊዜ", "G700"),
    // -----------------------------------------------------------------
    // Non-Amharic direct-encode still works (adapter checks for
    // Amharic content separately, but direct AmharicPhonex.encode
    // runs on transliterated input which for pure ASCII is the input
    // unchanged).
    //
    // hello → HELLO → H seed, E vow reset, L code=4 push → "H4",
    // L dup skip, O vow reset. Pad → "H400".
    // -----------------------------------------------------------------
    ("hello", "H400"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = AmharicPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-AM({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Amharic reference pair(s) disagreed:\n{}",
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
fn vowel_orders_fold_together_under_phonex() {
    // ሀ (order 0, hä) and ሆ (order 6, ho) both fold under phonex —
    // both produce keys where H is the seed and the vowel resets the
    // duplicate-collapse state.
    let a = AmharicPhonex.encode("ሀ").unwrap();
    let b = AmharicPhonex.encode("ሆ").unwrap();
    assert_eq!(a, b, "H-family single syllables should share a key");
    assert_eq!(a, "H000");
}
