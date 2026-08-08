//! ISO 843 Greek → Latin transliteration reference pairs.
//!
//! The pairs below cover every letter of the modern Greek alphabet at
//! least once, including the final-sigma variant `ς`, the accented
//! vowel forms `ά έ ή ί ό ύ ώ`, and the diaeresis-marked vowels
//! `ϊ ϋ`. The mapping is the one documented in
//! [`stringcheese_el::phonetic`] — the ISO 843 Type 1 transliteration
//! plus the context-sensitive `γγ → ng` double-gamma rule.

extern crate alloc;

use stringcheese_el::GreekIso843;

/// Reference pairs (input, expected output). Every letter of the
/// modern Greek alphabet (α β γ δ ε ζ η θ ι κ λ μ ν ξ ο π ρ σ/ς τ υ φ
/// χ ψ ω — 24 letters) is exercised at least once across the set.
const PAIRS: &[(&str, &str)] = &[
    // ---------------------------------------------------------------
    // Common place-names.
    // ---------------------------------------------------------------
    ("Αθήνα", "athina"),             // α θ η ν α
    ("Θεσσαλονίκη", "thessaloniki"), // θ ε σ σ α λ ο ν ι κ η
    ("Πάτρα", "patra"),
    ("Ηράκλειο", "irakleio"),
    ("Κρήτη", "kriti"),
    ("Ρόδος", "rodos"), // exercises final sigma
    // ---------------------------------------------------------------
    // Common names.
    // ---------------------------------------------------------------
    ("Γιώργος", "giorgos"),
    ("Δημήτρης", "dimitris"), // exercises δ, μ, τ, ρ
    ("Χρήστος", "christos"),  // χ → ch
    ("Ψαρός", "psaros"),      // ψ → ps
    ("Ξένη", "xeni"),         // ξ → x
    ("Ζωή", "zoi"),           // ζ → z
    ("Φοίβος", "foivos"),     // φ → f, β → v
    // ---------------------------------------------------------------
    // Final-sigma coverage — both spellings encode to the same ASCII.
    // ---------------------------------------------------------------
    ("κόσμος", "kosmos"), // final sigma
    ("κόσμοσ", "kosmos"), // non-final sigma
    // ---------------------------------------------------------------
    // Accented / diaeresis vowel coverage.
    // ---------------------------------------------------------------
    ("άκρη", "akri"),         // ά
    ("έχω", "echo"),          // έ, χ
    ("ήλιος", "ilios"),       // ή
    ("ίδιος", "idios"),       // ί
    ("όνομα", "onoma"),       // ό
    ("ύψος", "ypsos"),        // ύ, ψ
    ("ώρα", "ora"),           // ώ
    ("προϊόν", "proion"),     // ϊ, ό
    ("ταΰγετος", "taygetos"), // ΰ
    // ---------------------------------------------------------------
    // Digraphs and the γγ → ng double-gamma rule.
    // ---------------------------------------------------------------
    ("άγγελος", "angelos"), // γγ → ng
    ("αγγελία", "angelia"), // γγ → ng
    // ---------------------------------------------------------------
    // Full-alphabet coverage-check words.
    // ---------------------------------------------------------------
    ("θάλασσα", "thalassa"), // θ, λ, σ, σ
    ("χαρά", "chara"),       // χ, ρ, ά
    ("ξένος", "xenos"),      // ξ, ν
    ("ψάρι", "psari"),       // ψ
    ("ουρανός", "oyranos"),  // exercises bare υ inside the diphthong ου and again word-medially
];

#[test]
fn transliteration_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = GreekIso843.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  ISO843({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} ISO 843 reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 20 pairs covering all 24 letters
    // + final sigma + accented forms.
    assert!(
        PAIRS.len() >= 20,
        "reference pair count {} is below the 20-pair floor",
        PAIRS.len()
    );
}

#[test]
fn every_modern_greek_letter_is_covered() {
    // Walk the full modern Greek alphabet (24 lowercase base letters
    // plus the final-sigma positional variant). Every one should
    // appear in the lowercase form of at least one reference input.
    const ALPHABET: &[char] = &[
        'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ', 'σ',
        'ς', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
    ];
    for &letter in ALPHABET {
        let mut found = false;
        for &(input, _expected) in PAIRS {
            let lower: alloc::string::String = input.chars().flat_map(char::to_lowercase).collect();
            if lower.contains(letter) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "letter {letter:?} is not exercised by any reference pair"
        );
    }
}

#[test]
fn every_accented_vowel_is_covered() {
    // The accented vowels `ά έ ή ί ό ύ ώ` should each appear in at
    // least one reference input.
    for accent in ['ά', 'έ', 'ή', 'ί', 'ό', 'ύ', 'ώ'] {
        let mut found = false;
        for &(input, _expected) in PAIRS {
            if input.contains(accent) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "accented vowel {accent:?} is not exercised by any reference pair"
        );
    }
}
