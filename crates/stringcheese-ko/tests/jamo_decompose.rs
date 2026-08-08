//! Exhaustive Hangul jamo decomposition / composition tests.
//!
//! The tests here exercise the algorithmic formulas in `crate::jamo`
//! at both boundary syllables (the smoke-test lookups the module docs
//! reference) and — the interesting property — on **every single one**
//! of the 11172 precomposed Hangul syllables in U+AC00..=U+D7A3. The
//! closed-form formulas make it tractable to enumerate the entire
//! block in the test.

use stringcheese_ko::jamo::{
    L_BASE, S_BASE, S_COUNT, T_BASE, V_BASE, compose_jamo, decompose_syllable,
    is_precomposed_syllable,
};

// -----------------------------------------------------------------
// Boundary cases from the task spec.
// -----------------------------------------------------------------

#[test]
fn ga_decomposes_to_giyeok_a_no_final() {
    // `가` (U+AC00, first syllable) = ᄀ + ᅡ, no final.
    let (l, v, t) = decompose_syllable('가').expect("가 is a Hangul syllable");
    assert_eq!(l, '\u{1100}'); // ᄀ
    assert_eq!(v, '\u{1161}'); // ᅡ
    assert!(t.is_none());
}

#[test]
fn han_decomposes_to_hieuh_a_nieun() {
    // `한` (U+D55C) = ᄒ + ᅡ + ᆫ.
    let (l, v, t) = decompose_syllable('한').expect("한 is a Hangul syllable");
    assert_eq!(l, '\u{1112}'); // ᄒ
    assert_eq!(v, '\u{1161}'); // ᅡ
    assert_eq!(t, Some('\u{11AB}')); // ᆫ
}

#[test]
fn last_syllable_hih_decomposes_correctly() {
    // `힣` (U+D7A3, last syllable) = ᄒ + ᅵ + ᇂ.
    let (l, v, t) = decompose_syllable('힣').expect("힣 is a Hangul syllable");
    assert_eq!(l, '\u{1112}'); // ᄒ
    assert_eq!(v, '\u{1175}'); // ᅵ
    assert_eq!(t, Some('\u{11C2}')); // ᇂ
}

// -----------------------------------------------------------------
// Non-syllable inputs.
// -----------------------------------------------------------------

#[test]
fn decompose_returns_none_for_non_syllables() {
    assert!(decompose_syllable('A').is_none());
    assert!(decompose_syllable(' ').is_none());
    assert!(decompose_syllable('\0').is_none());
    // A Hangul compatibility jamo (U+3131 ㄱ) — not a syllable.
    assert!(decompose_syllable('\u{3131}').is_none());
    // A conjoining jamo (U+1100 ᄀ) — not a syllable either; it *is*
    // a jamo but the block containing precomposed syllables starts
    // higher up.
    assert!(decompose_syllable('\u{1100}').is_none());
    // Boundary: just below the block.
    assert!(decompose_syllable('\u{ABFF}').is_none());
    // Boundary: just above the block.
    assert!(decompose_syllable('\u{D7A4}').is_none());
}

#[test]
fn is_precomposed_syllable_matches_range_endpoints() {
    assert!(is_precomposed_syllable('가'));
    assert!(is_precomposed_syllable('힣'));
    assert!(!is_precomposed_syllable('\u{ABFF}'));
    assert!(!is_precomposed_syllable('\u{D7A4}'));
}

// -----------------------------------------------------------------
// Exhaustive round-trip: every one of the 11172 syllables round-trips.
// -----------------------------------------------------------------

#[test]
fn every_syllable_round_trips() {
    for offset in 0..S_COUNT {
        let cp = S_BASE + offset;
        let c = char::from_u32(cp).expect("Hangul syllable range is valid Unicode");
        let (l, v, t) =
            decompose_syllable(c).unwrap_or_else(|| panic!("no decomposition for U+{cp:04X}"));
        // Structural checks: L in [0x1100, 0x1112], V in [0x1161,
        // 0x1175], T (when present) in [0x11A8, 0x11C2].
        let l_cp = l as u32;
        let v_cp = v as u32;
        assert!(
            (L_BASE..L_BASE + 19).contains(&l_cp),
            "L jamo U+{l_cp:04X} out of range for syllable U+{cp:04X}",
        );
        assert!(
            (V_BASE..V_BASE + 21).contains(&v_cp),
            "V jamo U+{v_cp:04X} out of range for syllable U+{cp:04X}",
        );
        if let Some(tc) = t {
            let t_cp = tc as u32;
            assert!(
                (T_BASE + 1..T_BASE + 28).contains(&t_cp),
                "T jamo U+{t_cp:04X} out of range for syllable U+{cp:04X}",
            );
        }
        let composed = compose_jamo(l, v, t).expect("valid jamos compose");
        assert_eq!(
            composed, c,
            "round-trip mismatch for U+{cp:04X}: got U+{:04X}",
            composed as u32,
        );
    }
}

// -----------------------------------------------------------------
// Compose input validation.
// -----------------------------------------------------------------

#[test]
fn compose_rejects_invalid_jamos() {
    // L out of range (below).
    assert!(compose_jamo('\u{10FF}', '\u{1161}', None).is_none());
    // L out of range (above).
    assert!(compose_jamo('\u{1113}', '\u{1161}', None).is_none());
    // V out of range (below).
    assert!(compose_jamo('\u{1100}', '\u{1160}', None).is_none());
    // V out of range (above).
    assert!(compose_jamo('\u{1100}', '\u{1176}', None).is_none());
    // T sentinel U+11A7 is not a legal T input (use `None` for "no
    // final" instead).
    assert!(compose_jamo('\u{1100}', '\u{1161}', Some('\u{11A7}')).is_none());
    // T out of range (above).
    assert!(compose_jamo('\u{1100}', '\u{1161}', Some('\u{11C3}')).is_none());
}

#[test]
fn compose_produces_valid_syllable_when_inputs_are_valid() {
    // ᄒ + ᅡ + ᆫ → 한 (U+D55C).
    let s = compose_jamo('\u{1112}', '\u{1161}', Some('\u{11AB}')).expect("valid jamos");
    assert_eq!(s as u32, 0xD55C);
    assert!(is_precomposed_syllable(s));
}
