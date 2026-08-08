//! A Belarusian-tuned Soundex-family phonetic encoder — PHONEX-Belarusian.
//!
//! # Origin
//!
//! Belarusian has no widely established Soundex / Metaphone-family
//! phonetic encoder. Belarusian orthography is highly regular — a
//! near-1:1 mapping from grapheme to phoneme once the two Belarusian-
//! specific graphemes (`ў`, the short-u glide) and the two consonant
//! digraphs (`дж`, `дз`) are accounted for. This makes a Soundex-shape
//! encoder practical: the preprocessing collapses the digraphs into
//! single ASCII placeholders and the classification table then does
//! standard Slavic-consonant-family grouping.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Belarusian** encoder for consistency
//! with the other language packs that ship PHONEX-family encoders
//! (Czech, Dutch, Hungarian, Polish, Portuguese, Spanish, French,
//! Slovak, Turkish, Vietnamese, Korean, Bengali). Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>`
//! Soundex-shape key with Belarusian-tuned preprocessing and
//! classification:
//!
//! 1. **Digraph rewrites** run first, on the Unicode-lowercased text.
//!    Belarusian's two orthographic digraphs are single graphemes for
//!    collation but two Cyrillic scalars in UTF-8; the rewrites
//!    collapse them to a single ASCII placeholder letter in the
//!    fricative/affricate class:
//!    * `дж → J` (voiced postalveolar affricate /d͡ʒ/, same class as
//!      Belarusian `ж`).
//!    * `дз → Z` (voiced alveolar affricate /d͡z/, same class as
//!      Belarusian `з`).
//! 2. **Cyrillic-to-ASCII fold** collapses each remaining Belarusian
//!    letter to a lowercase ASCII placeholder (uppercased at the end
//!    of preprocess). The Belarusian-specific short u `ў` folds to
//!    `w` — a labial glide placeholder in class 1 alongside `в`, `б`,
//!    `п`, `ф`. Note that **`ў` is a consonant** in the classification
//!    (the short-u glide), not a vowel — so it never triggers a
//!    vowel-drop.
//! 3. **Drop non-letter scalars** (soft sign `ь`, apostrophes, digits,
//!    punctuation, whitespace).
//! 4. **Soundex-shape encoding.** Retain the first letter as the seed;
//!    classify each subsequent letter; drop the zero class (vowels);
//!    collapse consecutive equal codes; truncate to three digits and
//!    left-pad with `'0'` to reach length four.
//!
//! **Classification table.**
//!
//! | Code | Cyrillic → ASCII placeholder | Sound family |
//! |------|-----------------------------|--------------|
//! | 1    | Б→B П→P Ф→F В→V Ў→W          | Labials + short-u glide |
//! | 2    | Г→G К→K Х→H Й→Y              | Velars + palatal glide |
//! | 3    | Д→D Т→T                      | Coronals |
//! | 4    | Л→L                          | Lateral |
//! | 5    | М→M Н→N                      | Nasals |
//! | 6    | Р→R                          | Rhotic |
//! | 7    | С→S З→Z Ц→C Ч→Q Ш→X Ж→J Дз→Z Дж→J | Sibilants + affricates |
//! | 0    | А Е Ё І О У Ы Э Ю Я → A E O I O U I E U A | Vowels (dropped) |
//! | —    | Ь (soft sign)                | Dropped |
//!
//! # Adapter name
//!
//! `"phonex-be"` — chosen for consistency with the other language
//! packs' PHONEX adapters (`phonex-nl`, `phonex-pt`, `phonex-es`,
//! `phonex-fr`, `phonex-cs`, `phonex-pl`, `phonex-sk`, `phonex-hu`,
//! `phonex-tr`, `phonex-vi`, `phonex-ko`, `phonex-bn`).
//!
//! # Byte-vs-char safety
//!
//! Every Cyrillic scalar in the Belarusian block is **2 bytes** in
//! UTF-8 (U+0400..=U+04FF and the U+045E `ў` fall in the 2-byte range
//! U+0080..=U+07FF). The encoder walks characters via
//! [`str::chars`](str::chars), never raw bytes, so it never risks
//! slicing a scalar apart. The internal buffer is uppercase ASCII —
//! 1 byte per character — so the Soundex-shape pass is byte-safe.
//!
//! # Non-Cyrillic input
//!
//! The adapter gates on the presence of at least one Cyrillic-block
//! scalar (U+0400..=U+04FF). Pure-ASCII or pure-Latin input returns
//! `None` from the adapter — the encoder has no meaningful
//! classification for Latin letters.
//!
//! # Deferred to a follow-up wave
//!
//! * **Slavic-Metaphone alternate.** The cross-Slavic
//!   [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//!   encoder from `stringcheese-phonetic` is a plausible alternate;
//!   shipping it under a Cargo feature (as Russian, Ukrainian, and
//!   Serbian do) is deferred.
//! * **Taraškievič / Narkamaŭka orthography toggle.** The two
//!   Belarusian orthographies would need a preprocessing switch to
//!   normalize soft-sign placement (`сьвет` → `свет`) before
//!   encoding. Deferred.
//! * **GOST 7.79-B transliteration adapter.** A Belarusian-tuned
//!   deterministic Cyrillic → Latin mapping alongside PHONEX, on the
//!   Russian / Ukrainian / Bulgarian shape. Deferred.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Belarusian PHONEX encoder.
///
/// A zero-sized value; construct as [`BelarusianPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_be::BelarusianPhonex;
///
/// // "Мінск" — M seed, І vowel drop, N=5 push, S=7 push, K=2 push →
/// //   "M572".
/// let key = BelarusianPhonex.encode("Мінск").unwrap();
/// assert_eq!(key, "M572");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BelarusianPhonex;

impl BelarusianPhonex {
    /// Encodes `word` per the PHONEX-Belarusian algorithm.
    ///
    /// Returns `None` when `word` has no letter content after
    /// preprocessing (empty input, pure whitespace, all punctuation,
    /// no mappable Cyrillic scalar). Otherwise returns a 4-character
    /// key of the form `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        let mut out = String::with_capacity(4);
        // Seed: retain the first letter verbatim.
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        let mut i = 1;
        while i < bytes.len() {
            let b = bytes[i];
            let code = code_of(b);
            if code == b'0' {
                // Vowel — reset the duplicate-collapse state so a
                // sequence like `N-vowel-N` emits two 5s rather than
                // one.
                last_code = b'0';
                i += 1;
                continue;
            }
            if code == last_code {
                // Duplicate consonant class — collapse.
                i += 1;
                continue;
            }
            out.push(code as char);
            last_code = code;
            if out.len() == 4 {
                break;
            }
            i += 1;
        }
        while out.len() < 4 {
            out.push('0');
        }
        Some(out)
    }
}

/// Preprocess `word` into an uppercase-ASCII letter sequence with
/// Belarusian digraph substitutions applied.
///
/// The output alphabet is the uppercase ASCII letters — the digraph
/// placeholders (`J` for `дж`, `Z` for `дз`) share their base
/// letter's class, so no prime-marker joiner is needed. The soft
/// sign `ь`, the ASCII apostrophe, digits, and every other non-letter
/// scalar drop out.
fn preprocess(word: &str) -> String {
    // Step 1: Unicode-lowercase into a Vec<char> so we can look ahead
    // for digraphs in Cyrillic space (dropping raw byte offsets, since
    // every Cyrillic scalar is 2 UTF-8 bytes and byte-level scanning
    // would corrupt boundaries).
    let lowered: alloc::vec::Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

    let mut out = String::with_capacity(lowered.len());
    let mut i = 0;
    while i < lowered.len() {
        let c = lowered[i];
        // Digraph rewrites — try before the single-scalar fold so a
        // `д` followed by `ж` becomes `J`, not `D` + `J` (which would
        // encode as two separate class-7 codes and mis-count the
        // affricate as two graphemes).
        if i + 1 < lowered.len() {
            let n = lowered[i + 1];
            match (c, n) {
                ('д', 'ж') => {
                    out.push('J');
                    i += 2;
                    continue;
                }
                ('д', 'з') => {
                    out.push('Z');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-scalar fold — every Belarusian Cyrillic letter and
        // every ASCII letter maps to an uppercase ASCII placeholder;
        // every other scalar (soft sign, apostrophe, punctuation,
        // digits, whitespace) drops.
        if let Some(b) = fold_to_upper_ascii(c) {
            out.push(b);
        }
        i += 1;
    }
    out
}

/// Fold a Unicode-lowercased scalar to an uppercase ASCII base letter,
/// or return `None` if `c` is not letter-like. Handles the full
/// Belarusian Cyrillic alphabet plus the Russian-only letters that
/// might slip in (`ё`, `ъ`, `и`, `э`, `щ`, `ы`) as best-effort
/// pass-through — Belarusian does not officially carry `и`, `щ`, or
/// `ъ`, but folding them anyway keeps the encoder tolerant of
/// mixed-Cyrillic input.
fn fold_to_upper_ascii(c: char) -> Option<char> {
    // Soft sign — dropped in preprocess. Its classification would be
    // the zero class anyway, but dropping it entirely keeps it out of
    // the seed slot.
    if c == 'ь' {
        return None;
    }
    let base = match c {
        // ASCII letters — treat as their own base (rare but harmless
        // if the input mixes scripts).
        c if c.is_ascii_alphabetic() => c.to_ascii_lowercase(),
        // Vowels.
        'а' | 'я' => 'a',
        'е' | 'э' => 'e',
        'ё' | 'о' => 'o',
        'у' | 'ю' => 'u',
        'ы' | 'і' | 'и' => 'i',
        // Labials + short-u glide (class 1).
        'б' => 'b',
        'п' => 'p',
        'ф' => 'f',
        'в' => 'v',
        'ў' => 'w',
        // Velars + palatal glide (class 2).
        'г' => 'g',
        'к' => 'k',
        'х' => 'h',
        'й' => 'y',
        // Coronals (class 3).
        'д' => 'd',
        'т' => 't',
        // Lateral (class 4).
        'л' => 'l',
        // Nasals (class 5).
        'м' => 'm',
        'н' => 'n',
        // Rhotic (class 6).
        'р' => 'r',
        // Sibilants + affricates (class 7).
        'с' => 's',
        'з' => 'z',
        'ц' => 'c',
        'ч' => 'q',
        // `ш` and Russian-only `щ` both fold to the same class-7
        // placeholder `x` — Belarusian orthography spells the Russian
        // /ʃtʃ/ as the digraph `шч`, so `щ` never appears in native
        // Belarusian text; the fold is here so mixed-Cyrillic input
        // (a Russian loanword slipped into a Belarusian corpus) still
        // encodes without dropping the letter.
        'ш' | 'щ' => 'x',
        'ж' => 'j',
        _ => return None,
    };
    Some(base.to_ascii_uppercase())
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// See the classification table in the [module-level docs](self).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'G' | b'K' | b'H' | b'Y' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' | b'C' | b'Q' | b'X' | b'J' => b'7',
        // A E I O U — vowels are dropped (class 0).
        _ => b'0',
    }
}

/// Adapter that exposes [`BelarusianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Belarusian::phonetic_encoder`](crate::Belarusian) hands back.
///
/// Returns `Some((key, None))` when the input contains at least one
/// Cyrillic scalar; returns `None` otherwise. The gate is on the
/// Cyrillic block (U+0400..=U+04FF), not the specific Belarusian
/// alphabet — mixed-Cyrillic input still encodes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BelarusianPhonexAdapter;

impl LanguagePhoneticEncoder for BelarusianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_cyrillic(word) {
            return None;
        }
        BelarusianPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-be"
    }
}

/// Does `s` contain at least one scalar in the Cyrillic UTF-8 block
/// (U+0400..=U+04FF)?
///
/// A superset of the Belarusian mapping — it includes non-Belarusian
/// Cyrillic (Russian `ё`, Ukrainian `ї`, Serbian `љ`, …) that the
/// mapping either folds tolerantly or drops — but that is the right
/// shape for the adapter: any Cyrillic-block character makes the
/// input "Cyrillic content" for phonetic-key purposes.
fn contains_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        BelarusianPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(BelarusianPhonex.encode("").is_none());
        assert!(BelarusianPhonex.encode("   ").is_none());
        assert!(BelarusianPhonex.encode("---").is_none());
    }

    #[test]
    fn minsk_common_word() {
        // "Мінск" — M seed, І vowel drop, N=5 push, S=7 push, K=2
        //   push → out=[M,5,7,2] len=4 break → "M572".
        assert_eq!(p("Мінск"), "M572");
    }

    #[test]
    fn short_u_encodes_as_w_class_one() {
        // "аўтар" — A seed last=0, Ў→W code=1 push, T=3 push, A vow
        //   reset, R=6 push → out=[A,1,3,6] → "A136".
        assert_eq!(p("аўтар"), "A136");
    }

    #[test]
    fn dz_digraph_encodes_as_class_seven() {
        // "падзея" — P seed last=1, A vow reset, ДЗ→Z code=7 push,
        //   E vow reset, A vow reset → out=[P,7] pad → "P700".
        assert_eq!(p("падзея"), "P700");
    }

    #[test]
    fn dj_digraph_encodes_as_class_seven() {
        // "джэм" — ДЖ→J seed last=7, E vow reset, M=5 push →
        //   out=[J,5] pad → "J500".
        assert_eq!(p("джэм"), "J500");
    }

    #[test]
    fn soft_sign_is_dropped() {
        // "путь" (way) — P seed, U vow, T=3, Ь drop → out=[P,3] pad
        //   → "P300".
        // vs. "пут" — P seed, U vow, T=3 → "P300". Match.
        assert_eq!(p("путь"), p("пут"));
    }

    #[test]
    fn apostrophe_is_dropped() {
        // "аб'ект" — A seed, B=1 push, apostrophe drops, E vow reset,
        //   K=2 push, T=3 push → out=[A,1,2,3] → "A123".
        assert_eq!(p("аб'ект"), "A123");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("МІНСК"), p("мінск"));
        assert_eq!(p("АЎТАР"), p("аўтар"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("а"), "A000");
        assert_eq!(p("не"), "N000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // Two consecutive coronals collapse. "татта" (invented) —
        // T seed last=3, A vow reset, T=3 push, T=3 dup drop, A vow
        //   reset → out=[T,3] pad → "T300".
        // vs "тата" — T seed last=3, A vow reset, T=3 push, A vow reset
        //   → out=[T,3] → "T300". Same.
        assert_eq!(p("татта"), p("тата"));
    }

    #[test]
    fn vowels_are_dropped_in_interior() {
        // "вока" (eye) — V seed, O vow, K=2 push, A vow → "V2" pad
        //   → "V200".
        assert_eq!(p("вока"), "V200");
    }

    #[test]
    fn adapter_returns_name_phonex_be() {
        assert_eq!(BelarusianPhonexAdapter.name(), "phonex-be");
    }

    #[test]
    fn adapter_returns_none_for_no_cyrillic() {
        assert!(BelarusianPhonexAdapter.encode("").is_none());
        assert!(BelarusianPhonexAdapter.encode("hello").is_none());
        assert!(BelarusianPhonexAdapter.encode("123").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = BelarusianPhonexAdapter.encode("Мінск").unwrap();
        assert_eq!(primary, "M572");
        assert!(alt.is_none());
    }
}
