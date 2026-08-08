//! Devanagari → IAST transliteration reference input/output pairs.
//!
//! Coverage over all **33 classical Devanagari consonants** (velars,
//! palatals, retroflexes, dentals, labials, semivowels, sibilants,
//! and h), all **10 primary independent vowels** (`अ आ इ ई उ ऊ ए ऐ ओ
//! औ`), the virama-driven consonant clusters, the anusvara / visarga
//! combining marks, and the 10 Devanagari digits. Each pair is
//! either a bare letter, a real Hindi / Sanskrit word, or a
//! carefully-chosen minimal example that exercises a specific IAST
//! mapping.
//!
//! The IAST convention this pack follows is the **Sanskrit-style
//! explicit-schwa** rendering: base consonants carry the inherent
//! `a` unless overridden by a matra or the virama (see the
//! [`stringcheese_hi::phonetic`] module docs).

extern crate alloc;

use stringcheese_hi::HindiIast;

/// Reference pairs — 52 entries covering all 33 classical
/// consonants + 10 primary independent vowels + 5 real-word
/// examples + 2 combining marks + 2 digits.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // 10 independent vowels.
    // -----------------------------------------------------------------
    ("अ", "a"),
    ("आ", "ā"),
    ("इ", "i"),
    ("ई", "ī"),
    ("उ", "u"),
    ("ऊ", "ū"),
    ("ए", "e"),
    ("ऐ", "ai"),
    ("ओ", "o"),
    ("औ", "au"),
    // -----------------------------------------------------------------
    // 33 base consonants — each rendered with its inherent schwa.
    // Velars: क ख ग घ ङ.
    // -----------------------------------------------------------------
    ("क", "ka"),
    ("ख", "kha"),
    ("ग", "ga"),
    ("घ", "gha"),
    ("ङ", "ṅa"),
    // Palatals: च छ ज झ ञ.
    ("च", "ca"),
    ("छ", "cha"),
    ("ज", "ja"),
    ("झ", "jha"),
    ("ञ", "ña"),
    // Retroflexes: ट ठ ड ढ ण.
    ("ट", "ṭa"),
    ("ठ", "ṭha"),
    ("ड", "ḍa"),
    ("ढ", "ḍha"),
    ("ण", "ṇa"),
    // Dentals: त थ द ध न.
    ("त", "ta"),
    ("थ", "tha"),
    ("द", "da"),
    ("ध", "dha"),
    ("न", "na"),
    // Labials: प फ ब भ म.
    ("प", "pa"),
    ("फ", "pha"),
    ("ब", "ba"),
    ("भ", "bha"),
    ("म", "ma"),
    // Semivowels and liquids: य र ल व.
    ("य", "ya"),
    ("र", "ra"),
    ("ल", "la"),
    ("व", "va"),
    // Sibilants and h: श ष स ह.
    ("श", "śa"),
    ("ष", "ṣa"),
    ("स", "sa"),
    ("ह", "ha"),
    // -----------------------------------------------------------------
    // Words exercising virama-driven consonant clusters and matras.
    // -----------------------------------------------------------------
    ("राम", "rāma"),     // classic Sanskrit name
    ("सत्य", "satya"),    // truth — virama on त
    ("धर्म", "dharma"),   // duty — virama on र
    ("नमस्ते", "namaste"), // greeting
    ("कमल", "kamala"),   // lotus — schwas on all three
    // -----------------------------------------------------------------
    // Combining marks.
    // -----------------------------------------------------------------
    ("हैं", "haiṃ"), // "are" — anusvara after diphthong matra
    ("अः", "aḥ"),  // visarga
    // -----------------------------------------------------------------
    // Devanagari digits fold to ASCII.
    // -----------------------------------------------------------------
    ("२०२६", "2026"),
    ("०", "0"),
];

#[test]
fn iast_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = HindiIast.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  HindiIast({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} IAST reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "25+ pairs".
    assert!(
        PAIRS.len() >= 25,
        "reference pair count {} is below the 25-pair floor",
        PAIRS.len()
    );
}

#[test]
fn all_thirty_three_classical_consonants_are_covered() {
    // Walk the 33 classical Devanagari consonants (velars, palatals,
    // retroflexes, dentals, labials, semivowels, sibilants, h) and
    // verify each appears in the lowercase form of at least one
    // reference input.
    const CONSONANTS: &[char] = &[
        'क', 'ख', 'ग', 'घ', 'ङ', // velars
        'च', 'छ', 'ज', 'झ', 'ञ', // palatals
        'ट', 'ठ', 'ड', 'ढ', 'ण', // retroflexes
        'त', 'थ', 'द', 'ध', 'न', // dentals
        'प', 'फ', 'ब', 'भ', 'म', // labials
        'य', 'र', 'ल', 'व', // semivowels
        'श', 'ष', 'स', 'ह', // sibilants + h
    ];
    for &letter in CONSONANTS {
        let found = PAIRS.iter().any(|&(input, _)| input.contains(letter));
        assert!(
            found,
            "consonant {letter:?} is not exercised by any reference pair"
        );
    }
}

#[test]
fn all_ten_independent_vowels_are_covered() {
    const VOWELS: &[char] = &['अ', 'आ', 'इ', 'ई', 'उ', 'ऊ', 'ए', 'ऐ', 'ओ', 'औ'];
    for &v in VOWELS {
        let found = PAIRS.iter().any(|&(input, _)| input.contains(v));
        assert!(
            found,
            "independent vowel {v:?} is not exercised by any reference pair"
        );
    }
}
