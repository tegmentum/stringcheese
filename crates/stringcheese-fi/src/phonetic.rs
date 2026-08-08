//! A Finnish-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Finnish has no widely established phonetic encoder in the Soundex /
//! Métaphone / PHONEX family. This is deliberate: Finnish orthography
//! is **famously phonetic** — the modern Latin alphabet used since the
//! 16th century (and standardized by Agricola) maps very nearly 1:1
//! onto the language's phoneme inventory. There are no silent letters,
//! no digraph rewrites like English `ph → f`, and no `q`, `w`, `x`, or
//! `z` outside a handful of loanwords.
//!
//! # Implementation choice
//!
//! This module ships a **light PHONEX-Finnish** encoder: a 4-character
//! Soundex-shape key with Finnish-tuned preprocessing (long-consonant
//! and long-vowel collapse, `å` fold to `o`) and a conservative
//! consonant classification. The design prioritizes:
//!
//! 1. **Long-consonant collapse.** Finnish contrasts single and
//!    geminate consonants (`tuli` "fire" vs. `tulli` "customs"), but
//!    the phonetic key aims for equivalence-class matching, so `ll`,
//!    `kk`, `pp`, `tt`, `mm`, `nn`, `ss`, `rr` all fold to their
//!    single-letter counterparts before Soundex encoding. This is a
//!    deliberate loss for the record-linkage use case.
//! 2. **Long-vowel collapse.** Similarly, `aa`, `ee`, `ii`, `oo`, `uu`,
//!    `yy`, `ää`, `öö` fold to their short counterparts. Vowels are
//!    dropped by the Soundex encoding step anyway, so this just avoids
//!    the vowel-doubled input triggering spurious `last_code` resets.
//! 3. **`å → o` fold.** Finnish `å` is a Swedish letter (retained for
//!    loanwords / Swedish-origin names) representing /oː/; folding to
//!    `o` merges Swedish and Finnish spellings of the same name into
//!    one key.
//! 4. **`ä → a`, `ö → o` fold.** The two native Finnish additions
//!    fold to their base vowels for encoding purposes. Both are
//!    dropped by the vowel step anyway (all vowels get the sentinel
//!    `0` code), so the fold matters only for the *first* character
//!    of the key (which is retained as a letter).
//!
//! **Classification table.** Adapted from the classic Soundex family:
//!
//! | Code | Finnish letters         |
//! |------|-------------------------|
//! | 1    | B P F V W               |
//! | 2    | C G K Q J               |
//! | 3    | D T                     |
//! | 4    | L                       |
//! | 5    | M N                     |
//! | 6    | R                       |
//! | 7    | S Z X                   |
//! | 0    | A E I O U Y H (vowels + silent-H) |
//!
//! Finnish's `y` is a **front rounded vowel** /y/ (like German ü),
//! *not* the English-style consonant /j/ — it is classified as a
//! vowel here, which matches its phonetic role in Finnish.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone-shaped variable-length encoder.** A future refinement
//!   could produce a stem-like variable-length key; better precision
//!   for record linkage but heavier to reference-test.
//! * **Voiced/voiceless clustering.** Finnish has minimal voiced/
//!   voiceless contrast (no /b/, /d/, /g/ in native words — `d` shows
//!   up almost entirely in loanwords and gradation contexts). A
//!   Finnish-aware clustering could merge `b/p`, `d/t`, `g/k` more
//!   aggressively.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Finnish PHONEX encoder.
///
/// A zero-sized value; construct as [`FinnishPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_fi::FinnishPhonex;
///
/// // "Helsinki" — H seed, all subsequent consonants coded.
/// let key = FinnishPhonex.encode("Helsinki").unwrap();
/// assert_eq!(key, "H475");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FinnishPhonex;

impl FinnishPhonex {
    /// Encodes `word` per the PHONEX-Finnish algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form `<uppercase letter><three ASCII
    /// digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Step 1: preprocess into ASCII uppercase.
        let mut buf = preprocess(word);
        if buf.is_empty() {
            return None;
        }
        // Step 2: collapse long consonants (kk → k, tt → t, …) and
        // long vowels (aa → a, uu → u, …). Doubles fold to singles.
        collapse_doubles(&mut buf);
        let bytes = buf.as_bytes();

        // Step 3: Soundex-shape encoding.
        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
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

/// Preprocess `word` into uppercase-ASCII letters.
///
/// 1. Lowercase each scalar via Rust's default `to_lowercase`
///    (Finnish has no locale-specific case-fold quirks — the default
///    fold does the right thing for every letter including `ä`, `ö`,
///    `å`).
/// 2. Fold Finnish special letters to their ASCII base: `ä → a`,
///    `ö → o`, `å → o`.
/// 3. Uppercase.
/// 4. Drop non-letter scalars.
fn preprocess(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for c in word.chars() {
        let lowered = c.to_lowercase().next().unwrap_or(c);
        if let Some(a) = fold_to_ascii_upper(lowered) {
            out.push(a);
        }
    }
    out
}

/// Fold `c` (assumed already lowercased) to a single ASCII uppercase
/// letter, or return `None` if `c` isn't letter-like.
fn fold_to_ascii_upper(c: char) -> Option<char> {
    // ASCII fast path.
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Finnish-specific single-scalar folds. `ö` and `å` both fold
    // to `O` (identical bodies are the intent — clippy warns about
    // this pattern, but Finnish orthography really does map both
    // to the same ASCII letter for phonetic-key purposes).
    let folded = match c {
        'ä' => 'A',
        'ö' | 'å' => 'O',
        _ => return None,
    };
    Some(folded)
}

/// Collapse adjacent duplicate letters (long consonants and long
/// vowels) to a single copy — Finnish's geminate contrast is not
/// relevant for equivalence-class matching, and folding here avoids
/// the vowel-doubled input triggering spurious `last_code` resets in
/// the Soundex step.
fn collapse_doubles(buf: &mut String) {
    // Walk the ASCII byte buffer; write to a new String in place.
    let bytes = buf.as_bytes();
    let mut collapsed = String::with_capacity(bytes.len());
    let mut prev: Option<u8> = None;
    for &b in bytes {
        if Some(b) == prev {
            continue;
        }
        collapsed.push(b as char);
        prev = Some(b);
    }
    *buf = collapsed;
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// See the classification table in the [module-level docs](self).
/// Finnish `y` is a **vowel** (front rounded /y/), so it maps to the
/// vowel sentinel `0` — the shape English/German Soundex would give
/// to a consonant classification differs from Finnish here.
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'C' | b'G' | b'K' | b'Q' | b'J' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' | b'X' => b'7',
        // A E I O U Y H — vowels + silent-H, dropped.
        _ => b'0',
    }
}

/// Adapter that exposes [`FinnishPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Finnish::phonetic_encoder`](crate::Finnish) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FinnishPhonexAdapter;

impl LanguagePhoneticEncoder for FinnishPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        FinnishPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-fi"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        FinnishPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(FinnishPhonex.encode("").is_none());
        assert!(FinnishPhonex.encode("   ").is_none());
        assert!(FinnishPhonex.encode("---").is_none());
    }

    #[test]
    fn finnish_special_vowels_fold_to_ascii() {
        // "sydän" → lowercase "sydän" → fold ä→a → "SYDAN"
        //   Doubles: no doubles. bytes = "SYDAN".
        //   Encode: S seed last=7. Y: code=0 reset. D(3) push. A: reset.
        //     N(5) push. → "S35" → "S350".
        assert_eq!(p("sydän"), "S350");
    }

    #[test]
    fn long_consonants_collapse() {
        // "tuli" (fire) vs "tulli" (customs) — should share a PHONEX
        // key after long-consonant collapse.
        assert_eq!(p("tuli"), p("tulli"));
        // "kukka" (flower) → "kuka" after LL/KK collapse.
        assert_eq!(p("kukka"), p("kuka"));
    }

    #[test]
    fn long_vowels_collapse() {
        // "maa" (land) → collapses to "ma" → M seed, A drop → "M000".
        assert_eq!(p("maa"), "M000");
        assert_eq!(p("maa"), p("ma"));
    }

    #[test]
    fn a_ring_folds_to_o() {
        // "Åland" → "OLAND" after ä/å folds.
        //   Doubles: none. Encode: O seed. L(4) push. A reset. N(5) push
        //     → "O45" → "O450". D(3) push → "O453".
        //   Wait — trace: O seed last=0. bytes[1..]=[L,A,N,D].
        //     L: code=4, != 0, push → "O4", last=4.
        //     A: reset last=0.
        //     N: code=5, != 0, push → "O45", last=5.
        //     D: code=3, != 5, push → "O453".
        assert_eq!(p("Åland"), "O453");
    }

    #[test]
    fn helsinki_encodes_as_expected() {
        // "Helsinki" → HELSINKI (lowercase already ASCII). No doubles.
        //   H seed last=0. E: reset. L(4) push → "H4". S(7) push → "H47".
        //   I: reset. N(5) push → "H475". Wait: after S push, last=7; I
        //   resets last=0; N(5) push → "H475" — len==4 break. Wait, we
        //   want H452 per the doc example. Let me re-check.
        //
        //   Actually: HELSINKI. H(seed). E(0 reset). L(4 push). S(7 push).
        //     I(0 reset). N(5 push). K(2 — but out is now length 4, break).
        //   Result: "H475"? No wait — "H" + "4" + "7" + "5" = "H475". But
        //   the docstring example says "H452". Let me re-check…
        //
        //   Hmm: the H452 example in the doc-comment: H(seed), E reset, L(4),
        //     S(7), I reset, N(5), K(2), I(0). If we're going to length 4:
        //     H + 4 + 7 + 5 = "H475"; K would be the 5th push. So docstring
        //     example was wrong. Correct value: "H475".
        assert_eq!(p("Helsinki"), "H475");
    }

    #[test]
    fn ascii_and_lowercase_are_equivalent() {
        assert_eq!(p("Helsinki"), p("helsinki"));
        assert_eq!(p("JYVÄSKYLÄ"), p("jyväskylä"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Ei"), "E000");
    }

    #[test]
    fn duplicate_consonants_collapse_in_key() {
        // "Appa" → doubles collapse → "APA" → A(seed), P(1), A(reset)
        //   → "A1" → "A100".
        assert_eq!(p("Appa"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_fi() {
        assert_eq!(FinnishPhonexAdapter.name(), "phonex-fi");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(FinnishPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = FinnishPhonexAdapter.encode("Helsinki").unwrap();
        assert_eq!(primary, "H475");
        assert!(alt.is_none());
    }
}
