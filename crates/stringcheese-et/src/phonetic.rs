//! An Estonian-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Estonian has no widely established phonetic encoder in the Soundex
//! / Métaphone / PHONEX family. This is deliberate: Estonian
//! orthography is **highly phonetic** — the modern Latin alphabet
//! standardized in the 19th century (Aavik / Veski reforms) maps very
//! nearly 1:1 onto the language's phoneme inventory. There are no
//! silent letters, no digraph rewrites like English `ph → f`, and no
//! `c`, `q`, `w`, `x`, or `y` outside a handful of loanwords / proper
//! names.
//!
//! # Implementation choice
//!
//! This module ships a **light PHONEX-Estonian** encoder: a 4-character
//! Soundex-shape key with Estonian-tuned preprocessing (long-consonant
//! and long-vowel collapse, diacritic folds) and a conservative
//! consonant classification. The design prioritizes:
//!
//! 1. **Long-consonant collapse.** Estonian contrasts short / long /
//!    overlong consonants (a three-way phonological length distinction
//!    unique to Finnic languages — spelled with single or double
//!    letters: `kabi` vs. `kappi` vs. `kappi` in Q3 pronunciation),
//!    but the phonetic key aims for equivalence-class matching, so
//!    `ll`, `kk`, `pp`, `tt`, `mm`, `nn`, `ss`, `rr` all fold to
//!    their single-letter counterparts before Soundex encoding. This
//!    is a deliberate loss for the record-linkage use case.
//! 2. **Long-vowel collapse.** Similarly, `aa`, `ee`, `ii`, `oo`,
//!    `uu`, `õõ`, `ää`, `öö`, `üü` fold to their short counterparts.
//!    Vowels are dropped by the Soundex encoding step anyway, so
//!    this just avoids the vowel-doubled input triggering spurious
//!    `last_code` resets.
//! 3. **Diacritic folds.** `ä → a`, `ö → o`, `ü → u`, `õ → o`,
//!    `š → s`, `ž → z`. Both native vowels and loanword sibilants
//!    fold to their base ASCII form. Note `õ` and `ö` both fold to
//!    `o` — the two Estonian back-vowel diacritics collapse to the
//!    same ASCII letter for encoding purposes.
//!
//! **Classification table.** Adapted from the classic Soundex family:
//!
//! | Code | Estonian letters              |
//! |------|-------------------------------|
//! | 1    | B P F V W                     |
//! | 2    | C G K Q J                     |
//! | 3    | D T                           |
//! | 4    | L                             |
//! | 5    | M N                           |
//! | 6    | R                             |
//! | 7    | S Z X                         |
//! | 0    | A E I O U H (vowels + silent-H) |
//!
//! Estonian has no palatal-glide `y` (unlike English / German) —
//! `y` appears only in foreign words and here classifies as a vowel
//! (mirroring the Finnish pack's treatment of the front-rounded
//! Finnish `y`).
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone-shaped variable-length encoder.** A future refinement
//!   could produce a stem-like variable-length key; better precision
//!   for record linkage but heavier to reference-test.
//! * **Quantity-preserving encoding.** Estonian's three-way length
//!   contrast (short / long / overlong) is not marked orthographically
//!   for consonants beyond the single / double distinction, which is
//!   already collapsed by this encoder. A quantity-preserving encoder
//!   would need a stress lexicon to disambiguate Q2 vs. Q3.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Estonian PHONEX encoder.
///
/// A zero-sized value; construct as [`EstonianPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_et::EstonianPhonex;
///
/// // "Tallinn" — T seed, all subsequent consonants coded.
/// let key = EstonianPhonex.encode("Tallinn").unwrap();
/// assert_eq!(key, "T450");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct EstonianPhonex;

impl EstonianPhonex {
    /// Encodes `word` per the PHONEX-Estonian algorithm.
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
///    (Estonian has no locale-specific case-fold quirks — the default
///    fold does the right thing for every letter including `ä`, `ö`,
///    `ü`, `õ`, `š`, `ž`).
/// 2. Fold Estonian special letters to their ASCII base: `ä → a`,
///    `ö → o`, `ü → u`, `õ → o`, `š → s`, `ž → z`.
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
    // Estonian-specific single-scalar folds. `ö` and `õ` both fold to
    // `O` — the two back-vowel diacritics collapse in the phonetic
    // key. Clippy may warn about identical match-arm bodies, but
    // Estonian orthography really does map both to the same ASCII
    // letter for phonetic-key purposes.
    let folded = match c {
        'ä' => 'A',
        'ö' | 'õ' => 'O',
        'ü' => 'U',
        'š' => 'S',
        'ž' => 'Z',
        _ => return None,
    };
    Some(folded)
}

/// Collapse adjacent duplicate letters (long consonants and long
/// vowels) to a single copy — Estonian's geminate contrast is not
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
/// Estonian `y` (only in loanwords / proper names) classifies as a
/// vowel — the sentinel `0` code.
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

/// Adapter that exposes [`EstonianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Estonian::phonetic_encoder`](crate::Estonian) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct EstonianPhonexAdapter;

impl LanguagePhoneticEncoder for EstonianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        EstonianPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-et"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        EstonianPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(EstonianPhonex.encode("").is_none());
        assert!(EstonianPhonex.encode("   ").is_none());
        assert!(EstonianPhonex.encode("---").is_none());
    }

    #[test]
    fn estonian_special_vowels_fold_to_ascii() {
        // "küla" (village) → lowercase → fold ü→u → "KULA"
        //   Doubles: none. Encode: K seed last=2. U reset. L(4) push.
        //     A reset → "K4" → "K400".
        assert_eq!(p("küla"), "K400");
        // "õnn" (happiness) → fold õ→o → "ONN" → doubles: NN → N →
        //   "ON". Encode: O seed last=0. N(5) push → "O5" → "O500".
        assert_eq!(p("õnn"), "O500");
    }

    #[test]
    fn long_consonants_collapse() {
        // Signature Estonian equivalence: geminate consonants collapse.
        assert_eq!(p("kabi"), p("kappi"));
        // "linn" → "LINN" → collapse NN → "LIN" → L seed, I reset,
        //   N(5) push → "L500".
        assert_eq!(p("linn"), "L500");
    }

    #[test]
    fn long_vowels_collapse() {
        // "maa" (land) → collapses to "ma" → M seed, A drop → "M000".
        assert_eq!(p("maa"), "M000");
        assert_eq!(p("maa"), p("ma"));
        // "öö" (night) → fold ö→o → "OO" → collapse → "O" → "O000".
        assert_eq!(p("öö"), "O000");
    }

    #[test]
    fn o_variants_fold_together() {
        // Both `õ` and `ö` fold to `o` — for key purposes they merge.
        assert_eq!(p("õnn"), p("önn"));
    }

    #[test]
    fn u_diacritic_folds_to_u() {
        // ü → u fold.
        assert_eq!(p("üks"), p("uks"));
    }

    #[test]
    fn loanword_sibilants_fold_to_ascii() {
        // š → s, ž → z folds.
        assert_eq!(p("šokolaad"), p("sokolaad"));
        assert_eq!(p("žanr"), p("zanr"));
    }

    #[test]
    fn tallinn_encodes_as_expected() {
        // "Tallinn" → lowercase "tallinn" → doubles: LL → L, NN → N →
        //   "TALIN". Encode: T seed last=3. A reset. L(4) push. I
        //   reset. N(5) push → "T45" → "T450".
        assert_eq!(p("Tallinn"), "T450");
    }

    #[test]
    fn ascii_and_lowercase_are_equivalent() {
        assert_eq!(p("Tallinn"), p("tallinn"));
        assert_eq!(p("KÜLA"), p("küla"));
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
    fn adapter_returns_name_phonex_et() {
        assert_eq!(EstonianPhonexAdapter.name(), "phonex-et");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(EstonianPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = EstonianPhonexAdapter.encode("Tallinn").unwrap();
        assert_eq!(primary, "T450");
        assert!(alt.is_none());
    }
}
