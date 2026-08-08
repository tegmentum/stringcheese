//! A Danish-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Danish lacks a single canonical published phonetic encoder the way
//! English has Soundex or German has Kölner Phonetik. The closest
//! candidates:
//!
//! * **Language-independent Soundex** applied to ASCII-folded Danish —
//!   stable and portable, but linguistically inaccurate (the `sj` /
//!   `sk`-before-front-vowel clusters, the `dt` / `-d` softening, and
//!   the letters `å`/`æ`/`ø` all get swept under the English mapping).
//! * **PHONEX-family encoders** — the Danish equivalent of the Swedish
//!   / Norwegian / Dutch / French PHONEX encoders shipped in the
//!   sibling language packs: apply Danish-specific preprocessing
//!   (collapse the `sj` cluster to the sibilant class, collapse `sk`
//!   before front vowels to the same class, fold the three Danish-
//!   specific vowels for phonetic key stability), then run a Soundex-
//!   shape encoder over the classification table.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Danish** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with Danish-tuned preprocessing and the standard PHONEX
//! classification table:
//!
//! 1. **Uppercase and un-accent.** `Á À Â Ä → A`, `É È Ê Ë → E`, `Í Ì Î
//!    Ï → I`, `Ó Ò Ô Ö → O`, `Ú Ù Û Ü → U`. Danish-specific vowel
//!    folds: `Å → O` (open back rounded vowel, phonetically closer to
//!    /o/ or /ɔ/ than to /a/), `Æ → E` (open front vowel, phonetically
//!    closer to /ɛ/ than to /a/), `Ø → E` (rounded mid front vowel;
//!    for encoding purposes it collapses to `E`).
//! 2. **Danish digraph and single-letter substitutions.** Longest-match
//!    first:
//!    * `SJ → S`. Danish `sj` is the voiceless postalveolar fricative
//!      /ɕ/ or /ʃ/, uniformly the sibilant class.
//!    * `SK` before a front vowel `E I Y Æ Ø` → `S`. Before back vowels
//!      or consonants, `sk` stays as `s` + `k` (each classified
//!      separately). Danish has a milder version of this rule than
//!      Swedish/Norwegian — the palatalization is dialectal — but for
//!      phonetic key stability the fold captures the most common
//!      realization.
//!    * `CH → S`. Danish doesn't natively use `ch`; loanwords pronounce
//!      it as /ɕ/ or /ʃ/. Folding to `S` places it in the sibilant
//!      class.
//!    * `H → ` (dropped word-interior; also stripped word-initial to
//!      match the Norwegian/Dutch/Swedish PHONEX convention).
//! 3. **Soundex-shape encoding.** Retain the first letter; encode each
//!    subsequent letter by the classification table below; drop the
//!    zero class (vowels); collapse consecutive equal codes; truncate
//!    to three digits and left-pad with `'0'` to reach length four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P F V W |
//! | 2    | C K G Q J X |
//! | 3    | D T     |
//! | 4    | L       |
//! | 5    | M N     |
//! | 6    | R       |
//! | 7    | S Z     |
//! | 0    | A E I O U Y (dropped as vowels); H is stripped in preprocessing |
//!
//! **Grouping rationale.** Danish `B/P/F/V/W` are labial obstruents and
//! glides (surface `V` is /ʋ/, a labiodental approximant); `C/K/G/Q/J/
//! X` are the velar / palatal family; `D/T` are dental stops (Danish
//! `d` is often the fricative /ð/ but for encoding purposes shares the
//! dental class); `S/Z` are the sibilant family; `L` and `R` get their
//! own classes; `M/N` are nasals. The Danish `J` is `/j/` (palatal
//! glide) — placed in the velar/palatal class 2 alongside `G` and `K`
//! so `Jensen` and `Gensen` fall into related classes.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Danish** — a parallel variable-length encoder with
//!   better discrimination; heavier to reference-test.
//! * **Region-tuned variants** — Copenhagen / Jutland / Bornholm
//!   dialect PHONEX tables would need substantial rule sets out of
//!   scope for a starter pack.
//! * **Stød-aware encoding.** Danish uses a laryngealization feature
//!   (stød) that distinguishes some minimal pairs. Not orthographically
//!   represented and outside a spelling-based encoder's reach.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Danish PHONEX encoder.
///
/// A zero-sized value; construct as [`DanishPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_da::DanishPhonex;
///
/// // The silent-H preprocessor drops the initial H, so `Hansen`
/// // encodes from the residue `Ansen`: A-seed, N(5), S(7), N(5).
/// let key = DanishPhonex.encode("Hansen").unwrap();
/// assert_eq!(key, "A575");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DanishPhonex;

impl DanishPhonex {
    /// Encodes `word` per the PHONEX-Danish algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or an input reduced to nothing
    /// after silent-`H` stripping). Otherwise returns a 4-character
    /// key of the form `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
                // Vowel — reset the duplicate-collapse state.
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

/// Preprocess `word` into uppercase-ASCII letters after Danish digraph
/// and single-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold to uppercase-ASCII (drops non-letter code points).
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if let Some(letter) = fold_letter(c) {
            ascii.push(letter);
        }
    }
    // Step 2: digraph & single-letter substitutions. Longest-match
    // first.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte digraphs and front-vowel-context single-letter
        // substitutions.
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'S', b'K') if is_front_vowel(bytes.get(i + 2).copied()) => {
                    out.push('S');
                    i += 2;
                    continue;
                }
                // SJ and CH collapse to the sibilant class with the
                // same 2-byte skip.
                (b'S', b'J') | (b'C', b'H') => {
                    out.push('S');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'H' => { /* drop silent H */ }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

/// True if `b` is a Danish front vowel (`E I Y`). The Danish-specific
/// vowels `Æ` and `Ø` have already been folded to `E` in
/// [`fold_letter`], so after preprocessing they present as `E` here —
/// that's fine: the palatalization rule fires on the same segments
/// regardless of whether they were spelled `æ`/`ø` or `e`.
#[inline]
fn is_front_vowel(b: Option<u8>) -> bool {
    matches!(b, Some(b'E' | b'I' | b'Y'))
}

/// Fold `c` to the single ASCII uppercase letter that stands for it,
/// or `None` if `c` is not letter-like.
fn fold_letter(c: char) -> Option<char> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Danish-specific folds. `å → O` (open back rounded /ɔ/), `æ → E`
    // (open front /ɛ/), `ø → E` (rounded mid front). Diaeresis /
    // acute-carrying loans fold to base vowels. The Danish-specific
    // scalars share arms with their base-vowel target so
    // clippy::match_same_arms doesn't flag the collapse.
    let folded = match c {
        'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' | 'æ' | 'Æ' | 'ø' | 'Ø' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'å' | 'Å' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        _ => return None,
    };
    Some(folded)
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'C' | b'K' | b'G' | b'Q' | b'J' | b'X' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' => b'7',
        _ => b'0',
    }
}

/// Adapter that exposes [`DanishPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Danish::phonetic_encoder`](crate::Danish) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DanishPhonexAdapter;

impl LanguagePhoneticEncoder for DanishPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        DanishPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-da"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        DanishPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(DanishPhonex.encode("").is_none());
        assert!(DanishPhonex.encode("   ").is_none());
        assert!(DanishPhonex.encode("---").is_none());
    }

    #[test]
    fn danish_vowels_fold_to_base_letters() {
        // å → O, æ → E, ø → E.
        assert_eq!(p("så"), p("so"));
        assert_eq!(p("være"), p("vere"));
        assert_eq!(p("øje"), p("eje"));
    }

    #[test]
    fn diaeresis_and_acute_fold_to_base_vowel() {
        assert_eq!(p("café"), p("cafe"));
    }

    #[test]
    fn sj_cluster_encodes_as_sibilant() {
        // `sjæl` (soul) → preprocess SJ → S, then Æ → E, L. → "SEL".
        //   S(seed,last=7), E reset, L(4) → S, 4 → "S400"
        assert_eq!(p("sjæl"), "S400");
    }

    #[test]
    fn sk_before_front_vowel_encodes_as_sibilant() {
        // `ski` → preprocess SK before I → S. Then I. → "SI".
        //   S(seed,last=7), I reset → S → "S000"
        assert_eq!(p("ski"), "S000");
    }

    #[test]
    fn sk_before_back_vowel_stays_split() {
        // `skål` (cheers) → SK NOT before front vowel (Å → O is back).
        //   Stays as S + K. Å → O.
        //   Result: "SKOL". S(seed,last=7), K(2), O reset, L(4) → "S24"
        //   pad → "S240".
        assert_eq!(p("skål"), "S240");
    }

    #[test]
    fn ch_encodes_as_sibilant() {
        // `chef` (loan) → CH → S, then E, F. → "SEF".
        //   S(seed,last=7), E reset, F(1) → S, 1 → "S100"
        assert_eq!(p("chef"), "S100");
    }

    #[test]
    fn silent_h_is_stripped() {
        // `Hansen` → H drops → ANSEN.
        assert_eq!(p("Hansen"), p("Ansen"));
    }

    #[test]
    fn hansen_encodes() {
        // Hansen: H drops → ANSEN. A(seed), N(5), S(7), E reset, N(5).
        //   → "A575".
        assert_eq!(p("Hansen"), "A575");
    }

    #[test]
    fn common_danish_surnames() {
        // Jensen: J(seed,last=2), E reset, N(5), S(7), E reset, N(5)
        //   → "J575"
        assert_eq!(p("Jensen"), "J575");
        // Nielsen: N(seed,last=5), I reset, E reset, L(4), S(7), E
        //   reset, N(5) → "N475"
        assert_eq!(p("Nielsen"), "N475");
        // Andersen: A(seed,last=0), N(5), D(3), E reset, R(6), S(7), E
        //   reset, N(5) → "A536" (capped)
        assert_eq!(p("Andersen"), "A536");
        // Pedersen: P(seed,last=1), E reset, D(3), E reset, R(6), S(7),
        //   E reset, N(5) → "P367" (capped)
        assert_eq!(p("Pedersen"), "P367");
        // Christensen: CH → S, then RISTENSEN. S(seed,last=7),
        //   R(6), I reset, S(7), T(3), E reset, N(5), S(7) → "S6737"
        //   capped to "S673".
        assert_eq!(p("Christensen"), "S673");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("HANSEN"), p("hansen"));
        assert_eq!(p("SJÆL"), p("sjæl"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("På"), "P000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Abba" — A(seed), B(1,push,last=1), B(dup drop), A(reset)
        //   → "A1" → "A100"
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_da() {
        assert_eq!(DanishPhonexAdapter.name(), "phonex-da");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(DanishPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = DanishPhonexAdapter.encode("Hansen").unwrap();
        assert_eq!(primary, "A575");
        assert!(alt.is_none());
    }
}
