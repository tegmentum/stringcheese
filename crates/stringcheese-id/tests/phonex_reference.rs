//! PHONEX-Indonesian reference input/output pairs.
//!
//! Hand-traced against the module algorithm in
//! [`stringcheese_id::phonetic`]. Coverage: the four Indonesian
//! native digraphs (`ny → N`, `ng → G`, `sy → S`, `kh → K`), silent
//! `H`, common city names, common family names, common words, and
//! the empty-input `None` boundary.

extern crate alloc;

use stringcheese_id::IndonesianPhonex;

/// Reference pairs (input, expected 4-char PHONEX-ID key).
const PAIRS: &[(&str, &str)] = &[
    // Common cities.
    ("Jakarta", "J263"),
    ("Bandung", "B532"),
    ("Surabaya", "S610"),
    ("Medan", "M350"),
    // Common family names.
    // W(seed last=1). I reset. B(1) push. O reset. W(1) push
    // (vowel reset cleared the dup-collapse tracker) → "W11" → pad
    // → "W110".
    ("Wibowo", "W110"),
    ("Sutanto", "S353"),
    ("Halim", "A450"), // H dropped: "ALIM" → A seed, L push, I reset, M push → A45 → A450.
    // Common words — vowel patterns.
    ("makan", "M250"),
    ("minum", "M550"),
    ("rumah", "R500"), // H dropped: "RUMA".
    ("buku", "B200"),
    ("jalan", "J450"),
    // Native digraphs.
    ("nyanyi", "N500"), // both ny → N.
    ("bunga", "B200"),  // ng → G, encodes as class 2.
    ("syarat", "S630"), // sy → S.
    ("khusus", "K770"), // kh → K.
    ("akhir", "A260"),  // kh mid-word.
    // Silent H boundary.
    ("hotel", "O340"),
];

#[test]
fn phonex_id_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = IndonesianPhonex
            .encode(input)
            .expect("non-empty input encodes");
        if got != expected {
            failures.push(alloc::format!(
                "  IndonesianPhonex({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-ID reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn empty_input_yields_none() {
    assert!(IndonesianPhonex.encode("").is_none());
    assert!(IndonesianPhonex.encode("   ").is_none());
    assert!(IndonesianPhonex.encode("!!!").is_none());
}

#[test]
fn keys_are_always_four_chars_when_present() {
    for &(input, _) in PAIRS {
        let key = IndonesianPhonex.encode(input).expect("non-empty encodes");
        assert_eq!(
            key.chars().count(),
            4,
            "key {key:?} for {input:?} not 4 chars"
        );
    }
}

#[test]
fn case_insensitive_on_ascii() {
    for &(input, expected) in PAIRS {
        let upper = input.to_ascii_uppercase();
        let lower = input.to_ascii_lowercase();
        let key_upper = IndonesianPhonex.encode(&upper).expect("encodes");
        let key_lower = IndonesianPhonex.encode(&lower).expect("encodes");
        assert_eq!(
            key_upper, expected,
            "uppercase {upper:?} produced {key_upper:?}, expected {expected:?}"
        );
        assert_eq!(
            key_lower, expected,
            "lowercase {lower:?} produced {key_lower:?}, expected {expected:?}"
        );
    }
}
