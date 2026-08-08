//! PHONEX-Icelandic reference input/output pairs.
//!
//! A curated set of Icelandic surnames and common words that exercise
//! every preprocessing rule: `þ → th`, `ð → dh`, `æ → ae`, `ö → oe`
//! digraph expansions; `hv → kv` historical cluster fold; silent `h`;
//! long-vowel accent folds (`á é í ó ú ý`); and duplicate-collapse
//! edge cases.
//!
//! The expected values are computed against the module-level
//! algorithm documented in [`stringcheese_is::phonetic`] — see there
//! for the classification table.

extern crate alloc;

use stringcheese_is::IcelandicPhonex;

/// Reference pairs (input, expected 4-char PHONEX-Icelandic key).
const PAIRS: &[(&str, &str)] = &[
    // Þór (Thor) — Þ→TH, then TH→T (silent H drop), Ó→O, R.
    //   T(seed,last=3), O reset, R(6) → "T600"
    ("Þór", "T600"),
    // Björn — B, J, Ö→OE, R, N.
    //   B(seed,last=1), J(2), O reset, E reset, R(6), N(5) → "B265"
    ("Björn", "B265"),
    // Æsir — Æ→AE, S, I, R.
    //   A(seed,last=0), E reset, S(7), I reset, R(6) → "A760"
    ("Æsir", "A760"),
    // góður (good, masc nom sg) — G, Ó→O, Ð→DH → D, H drop, U, R.
    //   G(seed,last=2), O reset, D(3), U reset, R(6) → "G360"
    ("góður", "G360"),
    // hvíta (white, fem acc sg) — HV→KV, Í→I, T, A.
    //   K(seed,last=2), V(1), I reset, T(3), A reset → "K130"
    ("hvíta", "K130"),
    // hafa (to have) — H drop, A, F, A.
    //   A(seed,last=0), F(1), A reset → "A100"
    ("hafa", "A100"),
    // hestur (horse) — H drop, E, S, T, U, R.
    //   E(seed,last=0), S(7), T(3), U reset, R(6) → "E736"
    ("hestur", "E736"),
    // bók (book) — B, Ó→O, K.
    //   B(seed,last=1), O reset, K(2) → "B200"
    ("bók", "B200"),
    // kona (woman) — K, O, N, A.
    //   K(seed,last=2), O reset, N(5), A reset → "K500"
    ("kona", "K500"),
    // vera (to be) — V, E, R, A.
    //   V(seed,last=1), E reset, R(6), A reset → "V600"
    ("vera", "V600"),
    // Jónsson (patronym) — J, Ó→O, N, S, S, O, N.
    //   J(seed,last=2), O reset, N(5), S(7), S dup, O reset, N(5)
    //   → "J575"
    ("Jónsson", "J575"),
    // Sigurðsson (patronym) — S, I, G, U, R, Ð→DH → D, H drop, S,
    //   S, O, N. — capped to 4 chars.
    //   S(seed,last=7), I reset, G(2), U reset, R(6), D(3) → "S263"
    ("Sigurðsson", "S263"),
    // Ísland (Iceland) — Í→I, S, L, A, N, D.
    //   I(seed,last=0), S(7), L(4), A reset, N(5), D(3) → "I745"
    //   capped
    ("Ísland", "I745"),
    // Halldór (male name) — H drop, A, L, L, D, Ó→O, R.
    //   A(seed,last=0), L(4), L dup, D(3), O reset, R(6) → "A436"
    ("Halldór", "A436"),
    // Reykjavík (capital city) — R, E, Y, K, J, A, V, Í→I, K.
    //   R(seed,last=6), E reset, Y reset, K(2), J dup, A reset,
    //   V(1), I reset, K(2) → "R212"
    ("Reykjavík", "R212"),
    // þjóð (nation) — Þ→TH → T, H drop, J, Ó→O, Ð→DH → D, H drop.
    //   T(seed,last=3), J(2), O reset, D(3) → "T23" pad → "T230"
    ("þjóð", "T230"),
];

#[test]
fn phonex_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = IcelandicPhonex.encode(input).unwrap_or_default();
        if got != expected {
            failures.push(alloc::format!(
                "  PHONEX-IS({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} PHONEX-Icelandic reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // Verify we ship at least 15 reference pairs.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

#[test]
fn digraph_expansions_produce_expected_merges() {
    // Þ → TH: 'Þór' and 'Thor' share the key.
    assert_eq!(
        IcelandicPhonex.encode("Þór"),
        IcelandicPhonex.encode("Thor"),
        "Þ→TH expansion failed"
    );
    // Æ → AE: 'Æsir' and 'Aesir' share the key.
    assert_eq!(
        IcelandicPhonex.encode("Æsir"),
        IcelandicPhonex.encode("Aesir"),
        "Æ→AE expansion failed"
    );
    // Ö → OE: 'Björn' and 'Bjoern' share the key.
    assert_eq!(
        IcelandicPhonex.encode("Björn"),
        IcelandicPhonex.encode("Bjoern"),
        "Ö→OE expansion failed"
    );
}

#[test]
fn hv_kv_merger_collapses() {
    // hv-/kv- merger: 'hvíta' and 'kvíta' share the key.
    assert_eq!(
        IcelandicPhonex.encode("hvíta"),
        IcelandicPhonex.encode("kvíta"),
        "hv-/kv- merger failed"
    );
}

#[test]
fn silent_h_drop() {
    // Silent H drop: 'Hafa' and 'Afa' share the key.
    assert_eq!(
        IcelandicPhonex.encode("Hafa"),
        IcelandicPhonex.encode("Afa"),
        "Silent-H drop failed"
    );
}

#[test]
fn long_vowel_folded_variants_produce_the_same_key() {
    // Long-vowel accents fold to base letters; accented and plain
    // spellings encode identically.
    for (accented, ascii) in [
        ("ára", "ara"),
        ("ís", "is"),
        ("Ýr", "Yr"),
        ("bók", "bok"),
        ("þú", "tu"),
    ] {
        let a = IcelandicPhonex.encode(accented).unwrap();
        let b = IcelandicPhonex.encode(ascii).unwrap();
        assert_eq!(a, b, "PHONEX-IS({accented:?}) != PHONEX-IS({ascii:?})");
    }
}
