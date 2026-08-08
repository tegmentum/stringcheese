//! PHONEX-Belarusian reference input/output pairs.
//!
//! A curated set of Belarusian place names, surnames, and common
//! words that exercises every preprocessing rule (short-u `ў → W`,
//! digraph rewrites `дж → J` and `дз → Z`, soft-sign drop, apostrophe
//! drop) and the Soundex-shape encoding pass (seed retention, vowel
//! drop, duplicate-consonant collapse, four-character padding).
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_be::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_be::BelarusianPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Belarusian key).
const PAIRS: &[(&str, &str)] = &[
    // -------------------------------------------------------------
    // Common place names.
    // -------------------------------------------------------------
    // "Мінск" — M seed last=5, І vow reset, N=5 push, S=7 push,
    //   K=2 push → out=[M,5,7,2] len=4 break → "M572".
    ("Мінск", "M572"),
    // "Гродна" — G seed last=2, R=6 push, O vow reset, D=3 push,
    //   N=5 push, A vow reset → out=[G,6,3,5] → "G635".
    ("Гродна", "G635"),
    // "Магілёў" — M seed last=5, A vow reset, G=2 push, І vow reset,
    //   L=4 push, Ё vow reset, Ў=1 push → out=[M,2,4,1] → "M241".
    ("Магілёў", "M241"),
    // "Віцебск" — V seed last=1, І vow reset, C=7 push, E vow reset,
    //   B=1 push, S=7 push, K=2? out=[V,7,1,7] len=4 break → "V717".
    ("Віцебск", "V717"),
    // -------------------------------------------------------------
    // Short-u ў tests — ў folds to W in the labial class (1).
    // -------------------------------------------------------------
    // "аўтар" — A seed last=0, Ў→W=1 push, T=3 push, A vow reset,
    //   R=6 push → out=[A,1,3,6] → "A136".
    ("аўтар", "A136"),
    // "праўда" — P seed last=1, R=6 push, A vow reset, Ў→W=1 push,
    //   D=3 push → out=[P,6,1,3] → "P613".
    ("праўда", "P613"),
    // -------------------------------------------------------------
    // Digraph tests: дж → J (class 7), дз → Z (class 7). The digraph
    // is a single grapheme in the key, so `падзея` gets exactly one
    // class-7 code, not two.
    // -------------------------------------------------------------
    // "падзея" — P seed last=1, A vow reset, ДЗ→Z=7 push, E vow
    //   reset, A vow reset → out=[P,7] pad → "P700".
    ("падзея", "P700"),
    // "джэм" — ДЖ→J seed last=7, E vow reset, M=5 push → out=[J,5]
    //   pad → "J500".
    ("джэм", "J500"),
    // "джаз" — ДЖ→J seed last=7, A vow reset, Z=7 push → out=[J,7]
    //   pad → "J700".
    ("джаз", "J700"),
    // "дождж" — D seed last=3, O vow reset, ДЖ→J=7 push (dж is
    //   digraph at positions 3,4) → out=[D,7] pad → "D700".
    // Trace: д-о-ж-д-ж. At i=0 (д), next is 'о' → no digraph, fold
    //   'д' → D. i=1 (о), fold → O. i=2 (ж), fold → J. i=3 (д), next
    //   is 'ж' → digraph → J. So preprocess=DOJJ.
    // Encode: D seed last=3, O vow reset last=0, J=7 push last=7,
    //   J=7 dup drop → out=[D,7] pad → "D700".
    ("дождж", "D700"),
    // -------------------------------------------------------------
    // Soft sign is dropped by preprocess (not by the encoding pass).
    // The pair below shows `путь` and `пут` producing the same key.
    // -------------------------------------------------------------
    // "путь" — P seed, U vow, T=3 push, Ь drop → out=[P,3] pad
    //   → "P300".
    ("путь", "P300"),
    // "пут" — same key.
    ("пут", "P300"),
    // -------------------------------------------------------------
    // Apostrophe is dropped by preprocess (not a letter).
    // -------------------------------------------------------------
    // "аб'ект" — A seed, B=1 push, apostrophe drop, E vow reset,
    //   K=2 push, T=3 push → out=[A,1,2,3] → "A123".
    ("аб'ект", "A123"),
    // -------------------------------------------------------------
    // Common words touching each classification class.
    // -------------------------------------------------------------
    // "сябра" — S seed last=7, Я vow reset, B=1 push, R=6 push,
    //   A vow reset → out=[S,1,6] pad → "S160".
    ("сябра", "S160"),
    // "год" — G seed last=2, O vow reset, D=3 push → out=[G,3]
    //   pad → "G300".
    ("год", "G300"),
    // "яшчэ" — Y seed last=2 (й → Y), — wait, я not й. Я → A. So
    //   A seed last=0, ...
    // Actually 'я' folds to 'a' (vowel), so Y seed is wrong.
    //   Trace: я-ш-ч-э → A X Q E (in ASCII placeholders).
    //   A seed last=0, X=7 push, Q=7 dup drop, E vow reset →
    //   out=[A,7] pad → "A700".
    ("яшчэ", "A700"),
    // "малако" (milk) — M seed last=5, A vow reset, L=4 push,
    //   A vow reset, K=2 push, O vow reset → out=[M,4,2] pad
    //   → "M420".
    ("малако", "M420"),
    // "халодны" (cold) — H seed last=2, A vow reset, L=4 push,
    //   O vow reset, D=3 push, N=5 push, Y vow reset (і/ы → I vow)
    //   → out=[H,4,3,5] → "H435".
    // Wait ы folds to 'i' — vowel. So it's vow reset, not push.
    // Trace preprocess: х-а-л-о-д-н-ы → H A L O D N I
    // Encode: H seed last=2, A vow, L=4 push last=4, O vow, D=3
    //   push last=3, N=5 push last=5 out=[H,4,3,5] len=4 break.
    ("халодны", "H435"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = BelarusianPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-BE({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Belarusian reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 15 pairs; verify we're above
    // that.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn soft_sign_drops_produce_the_same_key() {
    // The soft sign is dropped by preprocess, so `путь` and `пут`
    // encode identically.
    assert_eq!(
        BelarusianPhonex.encode("путь"),
        BelarusianPhonex.encode("пут")
    );
}

#[test]
fn dz_digraph_is_one_grapheme_in_the_key() {
    // "падзея" — one class-7 code from ДЗ, not two — so the key is
    // "P700" not "P770" or similar.
    let key = BelarusianPhonex.encode("падзея").unwrap();
    assert_eq!(key, "P700");
}

#[test]
fn dj_digraph_takes_seed_when_word_initial() {
    // "джэм" — ДЖ→J takes the seed slot; the key starts with J.
    let key = BelarusianPhonex.encode("джэм").unwrap();
    assert!(key.starts_with('J'), "expected J-seed, got {key:?}");
}

#[test]
fn short_u_encodes_as_labial_class() {
    // Ў folds to W, class 1 alongside B/P/F/V. In "аўтар" the ў is
    // interior and produces the '1' digit.
    let key = BelarusianPhonex.encode("аўтар").unwrap();
    assert_eq!(key.chars().nth(1), Some('1'));
}

#[test]
fn case_invariant() {
    // Upper and lower encode to the same key.
    for w in ["Мінск", "Гродна", "аўтар", "джэм", "падзея"] {
        let lower: String = w.chars().flat_map(char::to_lowercase).collect();
        let upper: String = w.chars().flat_map(char::to_uppercase).collect();
        assert_eq!(
            BelarusianPhonex.encode(&lower),
            BelarusianPhonex.encode(&upper),
            "PHONEX-BE({lower:?}) != PHONEX-BE({upper:?})"
        );
    }
}
