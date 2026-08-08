//! Korean phonetic encoder: Revised Romanization → PHONEX-Korean.
//!
//! # Two-step algorithm
//!
//! 1. **Revised Romanization (RR).** Decompose every precomposed
//!    Hangul syllable into its L (choseong) / V (jungseong) / T
//!    (jongseong) jamos via [`crate::jamo::decompose_syllable`], then
//!    romanize each jamo per the Revised Romanization of Korean (RR)
//!    tables published by the National Institute of the Korean
//!    Language in 2000. The romanization tables live in this module as
//!    private constants; see [`revised_romanization`] for the public
//!    entry point.
//! 2. **PHONEX reduction.** Fold the RR output into a Soundex-family
//!    4-character equivalence key (`<uppercase letter><three ASCII
//!    digits>`), using a Korean-tuned consonant classification. See
//!    [`KoreanPhonex`] for the reducer and [`KoreanPhonexAdapter`]
//!    for the [`Language`](stringcheese_lang::Language)-facing
//!    adapter.
//!
//! # Why RR as the intermediate step?
//!
//! Korean orthography is a **featural alphabet packed into syllable
//! blocks**. Each syllable block encodes onset / nucleus / coda in a
//! visually fused glyph, but the phonemes it represents are
//! individually addressable — the jamo decomposition formula in
//! [`crate::jamo`] recovers them algorithmically. Going through
//! romanization first gives us a Latin-script string with the same
//! phonetic shape the reader would pronounce, at which point a
//! Soundex-family reducer works exactly as it does for English or
//! Finnish.
//!
//! ## Simplifications vs. full RR
//!
//! Full Revised Romanization (RR) is *context-sensitive*: syllable
//! boundaries interact through **liaison** — when a syllable-final
//! consonant precedes a vowel-initial next syllable, the final
//! consonant migrates onto the next syllable's onset for pronunciation
//! (`한국어` transcribes as `hangugeo`, not `hankookeo`, because the
//! `ㄱ` at the end of `국` liaises across to become part of the
//! following syllable — but its underlying jamo is still the final
//! consonant of `국`). This encoder emits the **surface-form**
//! romanization by walking jamos left-to-right and applying the
//! reading-form values in `RR_L`, `RR_V`, and `RR_T` (private to
//! this module — see the source); it does
//! **not** implement the assimilation / palatalization / liaison
//! rules that full RR handles. For a phonetic-key encoder that is the
//! right trade-off: the surface form is deterministic and cheap; the
//! contextual rules require sentence-level parsing that adds cost with
//! only marginal benefit at the Soundex-key granularity.
//!
//! # Non-goals
//!
//! - **McCune-Reischauer.** The pre-2000 romanization used in North
//!   Korea and Western academic literature is *not* implemented — RR is
//!   the modern South Korean standard and the one the phonetic key
//!   targets. An `MccuneReischauer` alternate encoder could ship in a
//!   follow-up.
//! - **Yale.** The linguists' romanization is optimized for structural
//!   analysis, not pronunciation, so it makes a poor phonetic key.
//! - **Assimilation / liaison rules.** See "Simplifications vs. full
//!   RR" above.
//! - **Hanja (Chinese characters).** Korean occasionally mixes Chinese
//!   characters in the CJK Unified block (U+4E00..=U+9FFF) with
//!   Hangul; those pass through unchanged. Dictionary-driven Hanja
//!   pronunciation is out of scope.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

use crate::jamo::{L_BASE, T_BASE, V_BASE, decompose_syllable};

/// Revised Romanization values for the 19 L (initial consonant) jamos,
/// indexed by `L - 0x1100` (0..=18).
///
/// The `ᄋ` (U+110B) jamo is the "null onset" — a placeholder that
/// carries no consonantal value when it appears in the initial slot
/// (Korean orthography requires every syllable to have a written
/// initial, so a syllable that starts with a bare vowel writes the
/// null jamo `ᄋ` there). RR emits an empty string for the null onset.
const RR_L: &[&str] = &[
    "g",  // U+1100 ᄀ
    "kk", // U+1101 ᄁ
    "n",  // U+1102 ᄂ
    "d",  // U+1103 ᄃ
    "tt", // U+1104 ᄄ
    "r",  // U+1105 ᄅ (romanized as `r` in initial position, `l` at end)
    "m",  // U+1106 ᄆ
    "b",  // U+1107 ᄇ
    "pp", // U+1108 ᄈ
    "s",  // U+1109 ᄉ
    "ss", // U+110A ᄊ
    "",   // U+110B ᄋ  (null onset — silent)
    "j",  // U+110C ᄌ
    "jj", // U+110D ᄍ
    "ch", // U+110E ᄎ
    "k",  // U+110F ᄏ
    "t",  // U+1110 ᄐ
    "p",  // U+1111 ᄑ
    "h",  // U+1112 ᄒ
];

/// Revised Romanization values for the 21 V (medial vowel) jamos,
/// indexed by `V - 0x1161` (0..=20).
const RR_V: &[&str] = &[
    "a",   // U+1161 ᅡ
    "ae",  // U+1162 ᅢ
    "ya",  // U+1163 ᅣ
    "yae", // U+1164 ᅤ
    "eo",  // U+1165 ᅥ
    "e",   // U+1166 ᅦ
    "yeo", // U+1167 ᅧ
    "ye",  // U+1168 ᅨ
    "o",   // U+1169 ᅩ
    "wa",  // U+116A ᅪ
    "wae", // U+116B ᅫ
    "oe",  // U+116C ᅬ
    "yo",  // U+116D ᅭ
    "u",   // U+116E ᅮ
    "wo",  // U+116F ᅯ
    "we",  // U+1170 ᅰ
    "wi",  // U+1171 ᅱ
    "yu",  // U+1172 ᅲ
    "eu",  // U+1173 ᅳ
    "ui",  // U+1174 ᅴ
    "i",   // U+1175 ᅵ
];

/// Revised Romanization values for the 27 T (final consonant) jamos,
/// indexed by `T - 0x11A8` (0..=26).
///
/// Unlike the L table, several T jamos have distinct "reading form"
/// values in RR that differ from their base consonant (`ᆨ` reads as
/// `k`, not `g`, in the final position; `ᆮ` reads as `t`, not `d`;
/// `ᆸ` reads as `p`, not `b`; `ᆯ` reads as `l`, not `r`). The values
/// below are the RR reading forms.
const RR_T: &[&str] = &[
    "k",  // U+11A8 ᆨ  final of ᄀ
    "kk", // U+11A9 ᆩ
    "ks", // U+11AA ᆪ
    "n",  // U+11AB ᆫ
    "nj", // U+11AC ᆬ
    "nh", // U+11AD ᆭ
    "t",  // U+11AE ᆮ  final of ᄃ
    "l",  // U+11AF ᆯ  final of ᄅ (RR uses `l` in final position)
    "lk", // U+11B0 ᆰ
    "lm", // U+11B1 ᆱ
    "lb", // U+11B2 ᆲ
    "ls", // U+11B3 ᆳ
    "lt", // U+11B4 ᆴ
    "lp", // U+11B5 ᆵ
    "lh", // U+11B6 ᆶ
    "m",  // U+11B7 ᆷ
    "p",  // U+11B8 ᆸ  final of ᄇ
    "ps", // U+11B9 ᆹ
    "t",  // U+11BA ᆺ  final of ᄉ (RR: final ᄉ reads as `t`)
    "t",  // U+11BB ᆻ  final of ᄊ (same reading rule)
    "ng", // U+11BC ᆼ  final of ᄋ (final ᄋ is a real ng consonant)
    "t",  // U+11BD ᆽ  final of ᄌ
    "t",  // U+11BE ᆾ  final of ᄎ
    "k",  // U+11BF ᆿ  final of ᄏ
    "t",  // U+11C0 ᇀ  final of ᄐ
    "p",  // U+11C1 ᇁ  final of ᄑ
    "h",  // U+11C2 ᇂ  final of ᄒ
];

/// Romanize `text` per the Revised Romanization of Korean (RR).
///
/// Every precomposed Hangul syllable in `text` decomposes to L/V/T
/// jamos and the three-part romanization is emitted. Every non-Hangul
/// scalar passes through unchanged, so mixed-script input carries its
/// ASCII / Latin / other-script characters through as-is.
///
/// This encoder implements the **surface-form** RR (walking jamos
/// left-to-right, no context-sensitive assimilation or liaison
/// rules). See the [module docs](self#simplifications-vs-full-rr) for
/// the trade-off.
///
/// # Examples
///
/// ```
/// use stringcheese_ko::phonetic::revised_romanization;
///
/// assert_eq!(revised_romanization("한국"), "hanguk");
/// assert_eq!(revised_romanization("서울"), "seoul");
/// assert_eq!(revised_romanization("김치"), "gimchi");
/// ```
#[must_use]
pub fn revised_romanization(text: &str) -> String {
    let mut out = String::with_capacity(text.len().saturating_mul(2));
    for c in text.chars() {
        if let Some((l, v, t)) = decompose_syllable(c) {
            let l_index = l as u32 - L_BASE;
            let v_index = v as u32 - V_BASE;
            out.push_str(RR_L[l_index as usize]);
            out.push_str(RR_V[v_index as usize]);
            if let Some(tc) = t {
                // T sentinel is U+11A7; real jongseong jamos start at
                // U+11A8 — i.e. table index 0 corresponds to U+11A8.
                let t_index = tc as u32 - T_BASE - 1;
                out.push_str(RR_T[t_index as usize]);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The Korean PHONEX encoder — a Soundex-family reducer over the
/// Revised Romanization intermediate form.
///
/// A zero-sized value; construct as [`KoreanPhonex`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the two-step algorithm.
///
/// # Example
///
/// ```
/// use stringcheese_ko::phonetic::KoreanPhonex;
///
/// // "한국" → RR "hanguk" → PHONEX H-N-G-K
/// //   H(seed), A(vowel drop), N(5), G(2), U(vowel drop), K(2 → dedup).
/// //   → "H52" padded to "H520".
/// let key = KoreanPhonex.encode("한국").expect("non-empty");
/// assert!(key.starts_with('H'));
/// assert_eq!(key.len(), 4);
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KoreanPhonex;

impl KoreanPhonex {
    /// Encode `word` into its PHONEX-Korean key.
    ///
    /// Returns `None` when `word` has no letter content after
    /// romanization (empty input, pure whitespace, all punctuation).
    /// Otherwise returns a 4-character key.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Step 1: romanize to Latin via RR (Hangul → ASCII).
        let romanized = revised_romanization(word);
        // Step 2: preprocess to uppercase-ASCII letters only.
        let buf = preprocess(&romanized);
        if buf.is_empty() {
            return None;
        }
        let bytes = buf.as_bytes();

        // Step 3: Soundex-shape encoding.
        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
                // Vowels / silent letters reset the dedup state so a
                // consonant repeated across a vowel boundary emits
                // twice.
                last_code = b'0';
                continue;
            }
            if code == last_code {
                continue;
            }
            out.push(code as char);
            last_code = code;
            if out.len() == 4 {
                break;
            }
        }
        while out.len() < 4 {
            out.push('0');
        }
        Some(out)
    }
}

/// Preprocess an RR-romanized string into ASCII-uppercase letters
/// only (drop digits, punctuation, whitespace, and any non-ASCII
/// leftovers).
fn preprocess(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for c in word.chars() {
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_uppercase());
        }
    }
    out
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// The classification uses a Korean-tuned grouping:
///
/// | Code | Latin letters      | Korean role                           |
/// |------|--------------------|----------------------------------------|
/// | 1    | B P F V M W        | Bilabials + labiodentals               |
/// | 2    | C G K Q            | Velars                                 |
/// | 3    | D T                | Dental / alveolar stops                |
/// | 4    | L R                | Liquids (Korean `ㄹ` is both `r`/`l`)  |
/// | 5    | N                  | Alveolar nasal                         |
/// | 6    | NG (handled inline) unused as a single-letter code — see note |
/// | 7    | S Z X J            | Sibilants and affricates (ch/j)        |
/// | 0    | A E I O U Y H      | Vowels + silent-H                      |
///
/// Note: `ng` is emitted as two letters `N`+`G` from the RR step. The
/// N gets code 5 and the G gets code 2; the pair emerges as `52` in
/// the key. If a follow-up pass wants to fuse them into a single
/// nasal-velar code, the RR step should emit a sentinel character
/// first; this encoder keeps the letter-by-letter mapping for
/// simplicity.
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'M' | b'W' => b'1',
        b'C' | b'G' | b'K' | b'Q' => b'2',
        b'D' | b'T' => b'3',
        b'L' | b'R' => b'4',
        b'N' => b'5',
        b'S' | b'Z' | b'X' | b'J' => b'7',
        // A E I O U Y H — vowels + silent-H, dropped.
        _ => b'0',
    }
}

/// Adapter that exposes [`KoreanPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Korean::phonetic_encoder`](crate::Korean) hands back.
///
/// The adapter always returns `Some((key, None))` — PHONEX-Korean is a
/// single-key encoder — and returns `None` for input whose RR
/// romanization has no ASCII-letter content (empty input, pure
/// punctuation, or Hanja-only input with no Hangul to romanize).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct KoreanPhonexAdapter;

impl LanguagePhoneticEncoder for KoreanPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        KoreanPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-ko"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> String {
        revised_romanization(s)
    }

    // ---------------------------------------------------------------
    // Revised Romanization — reference values.
    // ---------------------------------------------------------------

    #[test]
    fn romanize_country_name_hanguk() {
        // 한국 = 한 (ᄒ+ᅡ+ᆫ = han) + 국 (ᄀ+ᅮ+ᆨ = guk) = "hanguk"
        assert_eq!(r("한국"), "hanguk");
    }

    #[test]
    fn romanize_capital_seoul() {
        // 서울 = 서 (ᄉ+ᅥ = seo) + 울 (ᄋ+ᅮ+ᆯ = ul) = "seoul"
        assert_eq!(r("서울"), "seoul");
    }

    #[test]
    fn romanize_kimchi() {
        // 김치 = 김 (ᄀ+ᅵ+ᆷ = gim) + 치 (ᄎ+ᅵ = chi) = "gimchi"
        assert_eq!(r("김치"), "gimchi");
    }

    #[test]
    fn romanize_hangul() {
        // 한글 = 한 (han) + 글 (ᄀ+ᅳ+ᆯ = geul) = "hangeul"
        assert_eq!(r("한글"), "hangeul");
    }

    #[test]
    fn romanize_double_consonant_onset() {
        // 까 = ᄁ + ᅡ (no final) = "kka"
        assert_eq!(r("까"), "kka");
    }

    #[test]
    fn romanize_double_consonant_final() {
        // 밖 = ᄇ + ᅡ + ᆩ = "bakk"
        assert_eq!(r("밖"), "bakk");
    }

    #[test]
    fn romanize_null_onset_syllable() {
        // 아 = ᄋ (null onset, romanized to empty) + ᅡ = "a"
        assert_eq!(r("아"), "a");
        // 이 = ᄋ + ᅵ = "i"
        assert_eq!(r("이"), "i");
    }

    #[test]
    fn romanize_final_ng() {
        // 강 = ᄀ + ᅡ + ᆼ = "gang" (final ᄋ = ng)
        assert_eq!(r("강"), "gang");
    }

    #[test]
    fn romanize_final_r_reads_as_l() {
        // 물 = ᄆ + ᅮ + ᆯ = "mul" (final ᄅ reads as `l`)
        assert_eq!(r("물"), "mul");
    }

    #[test]
    fn romanize_final_g_reads_as_k() {
        // 국 = ᄀ + ᅮ + ᆨ = "guk" (final ᄀ reads as `k`)
        assert_eq!(r("국"), "guk");
    }

    #[test]
    fn romanize_final_b_reads_as_p() {
        // 밥 = ᄇ + ᅡ + ᆸ = "bap" (final ᄇ reads as `p`)
        assert_eq!(r("밥"), "bap");
    }

    #[test]
    fn romanize_final_d_reads_as_t() {
        // 곧 = ᄀ + ᅩ + ᆮ = "got" (final ᄃ reads as `t`)
        assert_eq!(r("곧"), "got");
    }

    #[test]
    fn romanize_diphthong_wae() {
        // 왜 = ᄋ + ᅫ = "wae"
        assert_eq!(r("왜"), "wae");
    }

    #[test]
    fn romanize_passes_through_non_hangul() {
        assert_eq!(r("hello"), "hello");
        assert_eq!(r("hello 서울"), "hello seoul");
        assert_eq!(r(""), "");
    }

    // ---------------------------------------------------------------
    // PHONEX — end-to-end.
    // ---------------------------------------------------------------

    fn p(w: &str) -> String {
        KoreanPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn phonex_returns_none_for_empty_input() {
        assert!(KoreanPhonex.encode("").is_none());
        assert!(KoreanPhonex.encode("   ").is_none());
        assert!(KoreanPhonex.encode("---").is_none());
        assert!(KoreanPhonex.encode("123").is_none());
    }

    #[test]
    fn phonex_key_shape_is_letter_plus_three_digits() {
        for w in ["한국", "서울", "김치", "한글", "강남"] {
            let key = p(w);
            assert_eq!(key.len(), 4, "key {key:?} for {w:?} not 4 chars");
            let (first, rest) = key.split_at(1);
            assert!(
                first.chars().next().unwrap().is_ascii_uppercase(),
                "first char of {key:?} not uppercase",
            );
            assert!(
                rest.bytes().all(|b| b.is_ascii_digit()),
                "trailing {rest:?} not all digits",
            );
        }
    }

    #[test]
    fn phonex_seoul_reference() {
        // 서울 → "seoul" → S(seed), E(vowel drop), O(vowel drop),
        //   U(vowel drop), L(4) → "S4" → padded "S400".
        assert_eq!(p("서울"), "S400");
    }

    #[test]
    fn phonex_hanguk_reference() {
        // 한국 → "hanguk" → H(seed), A(drop), N(5), G(2),
        //   U(drop), K(2 → dedup after vowel reset → push).
        //   H(last=0) A(reset 0) N push→"H5"(last=5) G push→"H52"(last=2)
        //   U reset(0) K push→"H522"(last=2) — len==4 break.
        //   Result: "H522".
        assert_eq!(p("한국"), "H522");
    }

    #[test]
    fn phonex_ascii_input_still_works() {
        // ASCII passes through RR unchanged, then feeds the reducer.
        // "smith" → S(seed), M(1), I(drop), T(3), H(drop).
        //   S(last=7) M push→"S1"(last=1) I reset K nothing T push→"S13"(last=3)
        //   H drop. Pad to "S130".
        assert_eq!(p("smith"), "S130");
    }

    // ---------------------------------------------------------------
    // Adapter.
    // ---------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_ko() {
        assert_eq!(KoreanPhonexAdapter.name(), "phonex-ko");
    }

    #[test]
    fn adapter_returns_some_for_hangul() {
        let out = KoreanPhonexAdapter.encode("한국");
        assert!(out.is_some());
        let (primary, alt) = out.unwrap();
        assert_eq!(primary.len(), 4);
        assert!(alt.is_none());
    }

    #[test]
    fn adapter_returns_none_for_no_letters() {
        assert!(KoreanPhonexAdapter.encode("").is_none());
        assert!(KoreanPhonexAdapter.encode("123").is_none());
        assert!(KoreanPhonexAdapter.encode("...").is_none());
    }
}
