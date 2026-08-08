//! Algorithmic Hangul syllable ↔ jamo decomposition and composition.
//!
//! # Hangul syllables
//!
//! Modern Korean text is written with **precomposed Hangul syllables**
//! in the range U+AC00..=U+D7A3 (11172 code points). Each syllable is
//! deterministically composed of a *choseong* (initial consonant), a
//! *jungseong* (medial vowel), and an optional *jongseong* (final
//! consonant) — commonly labelled L, V, and T after Unicode's L/V/T
//! terminology in section 3.12 of the Unicode Standard.
//!
//! # Decomposition formula
//!
//! For a precomposed syllable with code point `C` in U+AC00..=U+D7A3:
//!
//! ```text
//! SIndex = C - 0xAC00
//! L = 0x1100 + SIndex / (21 * 28)
//! V = 0x1161 + (SIndex % (21 * 28)) / 28
//! T = 0x11A7 + SIndex % 28    (T == 0x11A7 means "no final")
//! ```
//!
//! where U+1100..=U+1112 are the 19 L (initial consonant) jamos,
//! U+1161..=U+1175 are the 21 V (medial vowel) jamos, and
//! U+11A8..=U+11C2 are the 27 T (final consonant) jamos (with the
//! sentinel U+11A7 reserved for the "no final" case). The formula
//! encodes 19 · 21 · 28 = 11172 syllables, which is exactly the size of
//! the U+AC00..=U+D7A3 block.
//!
//! # Composition formula (inverse)
//!
//! For jamos `L ∈ [0x1100, 0x1112]`, `V ∈ [0x1161, 0x1175]`, and
//! `T ∈ [0x11A7, 0x11C2]`:
//!
//! ```text
//! LIndex = L - 0x1100
//! VIndex = V - 0x1161
//! TIndex = T - 0x11A7
//! C = 0xAC00 + (LIndex * 21 + VIndex) * 28 + TIndex
//! ```
//!
//! When `T == 0x11A7` (no final), `TIndex == 0` and the syllable is a
//! valid U+AC00..=U+D7A3 code point with no jongseong.
//!
//! # Round-trip
//!
//! Composition is the exact inverse of decomposition:
//! `compose(decompose(s)) == s` for every syllable in U+AC00..=U+D7A3.
//! The `jamo_decompose` integration test enumerates all 11172 syllables
//! and asserts the round-trip.
//!
//! # Non-goals
//!
//! - **Compatibility jamos.** The Hangul Compatibility Jamo block
//!   (U+3130..=U+318F) is a legacy compatibility mapping used only for
//!   glyph rendering, not for text processing. This module works on the
//!   modern conjoining jamos (L/V/T at U+1100..=U+11FF) exclusively.
//!   Callers who ingest compatibility jamos should map them through the
//!   Unicode Standard's compatibility decomposition tables (NFKD) before
//!   handing text to this module — that mapping is out of scope here.
//! - **Old Hangul.** Old Hangul used jamos in U+A960..=U+A97C (Extension
//!   A) and U+D7B0..=U+D7FB (Extension B), plus non-standard L/V/T
//!   combinations. Not covered.
//! - **Unicode-standard NFD Hangul decomposition.** The Unicode Standard
//!   defines a canonical decomposition where a syllable with a final
//!   decomposes into three jamos (L, V, T) and a syllable without a
//!   final decomposes into two (L, V). This module's
//!   [`decompose_syllable`] returns the `(L, V, Option<T>)` triple form
//!   instead — an easier surface for downstream code that wants to
//!   pattern-match on the presence of a final consonant. A caller who
//!   wants strict NFD can flatten the triple themselves.

/// The Hangul Syllable block base — the first precomposed syllable
/// (`가` U+AC00) sits here.
pub const S_BASE: u32 = 0xAC00;

/// The Hangul L (initial consonant) jamo base — U+1100 is `ᄀ`.
pub const L_BASE: u32 = 0x1100;

/// The Hangul V (medial vowel) jamo base — U+1161 is `ᅡ`.
pub const V_BASE: u32 = 0x1161;

/// The Hangul T (final consonant) jamo base — U+11A7 is the *filler*
/// (sentinel for "no final"); real jongseong jamos start at U+11A8.
pub const T_BASE: u32 = 0x11A7;

/// Number of L (initial consonant) jamos: 19.
pub const L_COUNT: u32 = 19;
/// Number of V (medial vowel) jamos: 21.
pub const V_COUNT: u32 = 21;
/// Number of T (final consonant) slots: 28 (27 jongseong jamos plus one
/// sentinel for "no final").
pub const T_COUNT: u32 = 28;

/// Number of syllables spanned by a single L: 21 · 28 = 588.
pub const N_COUNT: u32 = V_COUNT * T_COUNT;

/// Number of precomposed syllables in the Hangul Syllable block: 19 ·
/// 21 · 28 = 11172. Equal to the size of U+AC00..=U+D7A3.
pub const S_COUNT: u32 = L_COUNT * N_COUNT;

/// The last precomposed Hangul syllable (`힣` U+D7A3).
pub const S_LAST: u32 = S_BASE + S_COUNT - 1;

/// Returns `true` if `c` is a precomposed Hangul syllable in
/// U+AC00..=U+D7A3.
///
/// This is the input predicate for [`decompose_syllable`] — for every
/// scalar `c` where `is_precomposed_syllable(c) == true`, that function
/// returns `Some(_)`.
#[inline]
#[must_use]
pub const fn is_precomposed_syllable(c: char) -> bool {
    let cp = c as u32;
    cp >= S_BASE && cp <= S_LAST
}

/// Decompose a precomposed Hangul syllable into its
/// `(L, V, Option<T>)` jamo triple.
///
/// Returns `None` if `c` is not a precomposed Hangul syllable in
/// U+AC00..=U+D7A3. Otherwise returns `Some((l, v, t))` where:
///
/// - `l` is the initial consonant (choseong) jamo in U+1100..=U+1112.
/// - `v` is the medial vowel (jungseong) jamo in U+1161..=U+1175.
/// - `t` is the final consonant (jongseong) jamo in U+11A8..=U+11C2, or
///   `None` when the syllable has no jongseong.
///
/// The three returned values are conjoining-jamo scalars from the
/// U+1100..=U+11FF block (not the legacy Hangul Compatibility Jamo
/// block, U+3130..=U+318F — see the [module docs](self)).
///
/// # Examples
///
/// ```
/// use stringcheese_ko::jamo::decompose_syllable;
///
/// // `가` (U+AC00) = ᄀ + ᅡ, no final.
/// assert_eq!(decompose_syllable('가'), Some(('\u{1100}', '\u{1161}', None)));
/// // `한` (U+D55C) = ᄒ + ᅡ + ᆫ.
/// assert_eq!(
///     decompose_syllable('한'),
///     Some(('\u{1112}', '\u{1161}', Some('\u{11AB}'))),
/// );
/// // A non-syllable returns None.
/// assert_eq!(decompose_syllable('A'), None);
/// ```
///
/// # Panics
///
/// Does not panic in practice — the jamo code points computed by the
/// decomposition formula all sit in U+1100..=U+11C2 (BMP, valid
/// Unicode), so the internal `char::from_u32` conversions never see a
/// surrogate or an out-of-range code point. The `.expect` sites are
/// documentation of that invariant, not a runtime check.
#[inline]
#[must_use]
pub fn decompose_syllable(c: char) -> Option<(char, char, Option<char>)> {
    if !is_precomposed_syllable(c) {
        return None;
    }
    let s_index = c as u32 - S_BASE;
    let l = L_BASE + s_index / N_COUNT;
    let v = V_BASE + (s_index % N_COUNT) / T_COUNT;
    let t_offset = s_index % T_COUNT;
    // `char::from_u32` on values known to be inside the BMP is
    // infallible; the `.expect` documents the invariant without
    // reaching for `unsafe`.
    let l_char = char::from_u32(l).expect("L jamo range is valid Unicode");
    let v_char = char::from_u32(v).expect("V jamo range is valid Unicode");
    let t_char = if t_offset == 0 {
        None
    } else {
        Some(char::from_u32(T_BASE + t_offset).expect("T jamo range is valid Unicode"))
    };
    Some((l_char, v_char, t_char))
}

/// Compose a jamo triple into a precomposed Hangul syllable.
///
/// The inverse of [`decompose_syllable`]. Returns `None` when any of
/// the jamos falls outside its expected range:
///
/// - `l` must be in U+1100..=U+1112 (an initial consonant jamo).
/// - `v` must be in U+1161..=U+1175 (a medial vowel jamo).
/// - `t` (when `Some`) must be in U+11A8..=U+11C2 (a final consonant
///   jamo); `None` represents the absent-jongseong case.
///
/// # Examples
///
/// ```
/// use stringcheese_ko::jamo::compose_jamo;
///
/// // ᄀ + ᅡ, no final → `가` (U+AC00).
/// assert_eq!(compose_jamo('\u{1100}', '\u{1161}', None), Some('가'));
/// // ᄒ + ᅡ + ᆫ → `한` (U+D55C).
/// assert_eq!(
///     compose_jamo('\u{1112}', '\u{1161}', Some('\u{11AB}')),
///     Some('한'),
/// );
/// ```
#[inline]
#[must_use]
pub fn compose_jamo(l: char, v: char, t: Option<char>) -> Option<char> {
    let l_cp = l as u32;
    let v_cp = v as u32;
    if !(L_BASE..L_BASE + L_COUNT).contains(&l_cp) {
        return None;
    }
    if !(V_BASE..V_BASE + V_COUNT).contains(&v_cp) {
        return None;
    }
    let t_index = match t {
        None => 0,
        Some(tc) => {
            let t_cp = tc as u32;
            // Real jongseong jamos start at T_BASE + 1 (U+11A8); the
            // sentinel T_BASE itself is not a legal input here.
            if !(T_BASE + 1..T_BASE + T_COUNT).contains(&t_cp) {
                return None;
            }
            t_cp - T_BASE
        }
    };
    let l_index = l_cp - L_BASE;
    let v_index = v_cp - V_BASE;
    let cp = S_BASE + (l_index * V_COUNT + v_index) * T_COUNT + t_index;
    char::from_u32(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_unicode_standard() {
        // 19 * 21 * 28 = 11172 is exactly the U+AC00..=U+D7A3 block size.
        assert_eq!(S_COUNT, 11172);
        assert_eq!(S_LAST, 0xD7A3);
        assert_eq!(N_COUNT, 588);
    }

    #[test]
    fn decompose_ga_has_no_final() {
        // `가` (U+AC00) — first syllable, no final.
        let (l, v, t) = decompose_syllable('가').expect("가 is a Hangul syllable");
        assert_eq!(l, '\u{1100}'); // ᄀ
        assert_eq!(v, '\u{1161}'); // ᅡ
        assert!(t.is_none());
    }

    #[test]
    fn decompose_han_has_final_nieun() {
        // `한` (U+D55C) — ᄒ + ᅡ + ᆫ.
        let (l, v, t) = decompose_syllable('한').expect("한 is a Hangul syllable");
        assert_eq!(l, '\u{1112}'); // ᄒ
        assert_eq!(v, '\u{1161}'); // ᅡ
        assert_eq!(t, Some('\u{11AB}')); // ᆫ
    }

    #[test]
    fn decompose_last_syllable_hih() {
        // `힣` (U+D7A3) — last syllable, ᄒ + ᅵ + ᇂ.
        let (l, v, t) = decompose_syllable('힣').expect("힣 is a Hangul syllable");
        assert_eq!(l, '\u{1112}'); // ᄒ
        assert_eq!(v, '\u{1175}'); // ᅵ
        assert_eq!(t, Some('\u{11C2}')); // ᇂ
    }

    #[test]
    fn decompose_rejects_non_syllable() {
        assert!(decompose_syllable('A').is_none());
        assert!(decompose_syllable(' ').is_none());
        // Compatibility jamo — not a syllable.
        assert!(decompose_syllable('\u{3131}').is_none()); // ㄱ compat
        // Just below the block.
        assert!(decompose_syllable('\u{ABFF}').is_none());
        // Just above the block.
        assert!(decompose_syllable('\u{D7A4}').is_none());
    }

    #[test]
    fn compose_ga_from_jamos() {
        assert_eq!(compose_jamo('\u{1100}', '\u{1161}', None), Some('가'));
    }

    #[test]
    fn compose_han_from_jamos() {
        assert_eq!(
            compose_jamo('\u{1112}', '\u{1161}', Some('\u{11AB}')),
            Some('한'),
        );
    }

    #[test]
    fn compose_rejects_out_of_range_jamos() {
        // L too low.
        assert!(compose_jamo('\u{10FF}', '\u{1161}', None).is_none());
        // L too high.
        assert!(compose_jamo('\u{1113}', '\u{1161}', None).is_none());
        // V too low.
        assert!(compose_jamo('\u{1100}', '\u{1160}', None).is_none());
        // V too high.
        assert!(compose_jamo('\u{1100}', '\u{1176}', None).is_none());
        // T sentinel (the "no final" marker) is not a legal T input.
        assert!(compose_jamo('\u{1100}', '\u{1161}', Some('\u{11A7}')).is_none());
        // T too high.
        assert!(compose_jamo('\u{1100}', '\u{1161}', Some('\u{11C3}')).is_none());
    }

    #[test]
    fn compose_of_decompose_is_identity_on_boundary_syllables() {
        for cp in [0xAC00_u32, 0xAC01, 0xAC1B, 0xD55C, 0xD7A3] {
            let c = char::from_u32(cp).unwrap();
            let (l, v, t) = decompose_syllable(c).unwrap();
            assert_eq!(compose_jamo(l, v, t), Some(c));
        }
    }
}
