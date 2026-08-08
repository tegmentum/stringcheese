//! A Hungarian-tuned Soundex-family phonetic encoder — PHONEX-Hungarian.
//!
//! # Origin
//!
//! Hungarian, like Czech and Polish, has no widely established
//! phonetic encoder in the Soundex / Métaphone / PHONEX family.
//! Hungarian orthography is highly regular — a near-1:1 mapping from
//! grapheme to phoneme once the digraph and long-vowel conventions
//! are accounted for. This makes a Soundex-shape encoder practical:
//! the preprocessing collapses the orthographic surface variants and
//! the digraphs into single ASCII scalars, and the classification
//! table then does standard consonant-family grouping.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Hungarian** encoder for consistency
//! with the other Latin-alphabet language packs (Czech, Dutch,
//! Polish, Portuguese, Spanish, French, Slovak, Turkish, Vietnamese
//! all ship PHONEX-family encoders). Concretely, the algorithm is a
//! 4-character `<letter><digit><digit><digit>` Soundex-shape key
//! with Hungarian-tuned preprocessing and classification:
//!
//! 1. **Digraph and trigraph rewrites** run first, on the still-cased
//!    Unicode text. Hungarian's collation-single-letter multi-scalar
//!    graphemes rewrite to single ASCII placeholder letters (some
//!    "primed" with an ASCII apostrophe to distinguish related
//!    sounds). Rewrites (trigraph first so `dz` / `zs` don't consume
//!    it piecewise): `dzs → J` (voiced postalveolar affricate),
//!    `cs → C` (voiceless postalveolar affricate), `dz → Z` (voiced
//!    alveolar affricate), `gy → G'` (voiced palatal stop), `ly → J`
//!    (palatal approximant / historic /ʎ/, now merged with `j` in
//!    the standard — encoded as plain `J` to reflect the merger),
//!    `ny → N'` (palatal nasal), `sz → S` (voiceless alveolar
//!    fricative), `ty → T'` (voiceless palatal stop), `zs → Z'`
//!    (voiced postalveolar fricative). The primed placeholders
//!    (`G'`, `N'`, `T'`, `Z'`) are two-scalar tokens in the internal
//!    buffer; the Soundex-shape encoder treats the apostrophe as a
//!    **transparent joiner** — it stays with its preceding letter
//!    for the classification pass but never emits a digit of its
//!    own, and never resets the duplicate-collapse state. Concretely:
//!    a primed pair encodes as **the base letter's code once**, which
//!    is exactly what a Soundex-shape key needs.
//! 2. **Long-vowel fold to short.** The seven long vowels fold to
//!    their short counterparts: `á → A`, `é → E`, `í → I`, `ó → O`,
//!    `ő → Ö` (rounded front), `ú → U`, `ű → Ü` (rounded front).
//!    Vowels are then classified as the zero class (dropped) in the
//!    encoding pass, so the fold's practical effect is on the **seed**
//!    letter of the key: a word beginning with `Ó` and one beginning
//!    with `O` produce the same 4-character key.
//! 3. **Umlaut-vowel fold** for the seed: `ö → O`, `ü → U`. In the
//!    interior the fold has no visible effect (interior vowels are
//!    dropped) but keeps the seed comparable to the ASCII base.
//! 4. **Uppercase and drop non-letter scalars.**
//! 5. **Soundex-shape encoding.** Retain the first letter; classify
//!    each subsequent letter; drop the zero class (vowels); collapse
//!    consecutive equal codes; truncate to three digits and left-pad
//!    with `'0'` to reach length four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P F V W |
//! | 2    | C G K Q J X (includes `cs → C`, `ly → J`, `dzs → J`, `gy → G'`, `ty → T'` — the primed pair encodes to the base's class) |
//! | 3    | D T |
//! | 4    | L |
//! | 5    | M N (includes `ny → N'`) |
//! | 6    | R |
//! | 7    | S Z (includes `sz → S`, `zs → Z'`, `dz → Z`) |
//! | 0    | A E I O U Y H (vowels + silent-H) |
//!
//! # Adapter name
//!
//! `"phonex-hu"` — chosen for consistency with the other Latin-
//! alphabet packs' PHONEX adapters (`phonex-nl`, `phonex-pt`,
//! `phonex-es`, `phonex-fr`, `phonex-cs`, `phonex-pl`, `phonex-sk`).
//!
//! # Deferred to a follow-up wave
//!
//! * **ISO 9-style Hungarian transliteration adapter.** A parallel
//!   diacritic-strip-only ASCII rendering for library-catalog
//!   interop. Shipped as an alternative encoder, not a replacement.
//! * **Métaphone Hungarian.** A variable-length encoder with better
//!   discrimination; heavier to reference-test.
//! * **Vowel-harmony-aware key extension.** A back-vs-front-flavour
//!   suffix character could carry harmony class information into
//!   record-linkage keys; skipped to keep the key ASCII-only.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Hungarian PHONEX encoder.
///
/// A zero-sized value; construct as [`HungarianPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_hu::HungarianPhonex;
///
/// // "Budapest" — B seed, U vowel drop, D=3 push, A vowel drop,
/// //   P=1 push, E vowel drop, S=7 push (out.len()==4, break) →
/// //   "B317".
/// let key = HungarianPhonex.encode("Budapest").unwrap();
/// assert_eq!(key, "B317");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HungarianPhonex;

impl HungarianPhonex {
    /// Encodes `word` per the PHONEX-Hungarian algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        let mut out = String::with_capacity(4);
        // Seed: the first byte is always a base letter (the prime
        // marker only appears after a base, never at position 0 after
        // preprocess).
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        let mut i = 1;
        while i < bytes.len() {
            let b = bytes[i];
            // A prime marker (`'`) is a transparent joiner: it belongs
            // to the letter *before* it, which was already handled in
            // the previous loop iteration. Skip and continue.
            if b == b'\'' {
                i += 1;
                continue;
            }
            let code = code_of(b);
            if code == b'0' {
                // Vowel — reset the duplicate-collapse state.
                last_code = b'0';
                i += 1;
                continue;
            }
            if code == last_code {
                // Duplicate consonant class — collapse. But if the
                // *next* byte is a prime marker, the collapse must not
                // fire (the primed pair is a distinct grapheme — e.g.,
                // `gy` after a plain `g` is still one letter, not a
                // duplicate). We conservatively drop only when the
                // next byte is not a prime marker.
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    // Primed pair — treat as a fresh push of the
                    // base class (still one code, but the sequence
                    // "G, G'" reads as g+gy, two distinct letters
                    // — encode as two same codes collapsed to one is
                    // the wanted behaviour anyway).
                    // Fall through to the normal push path? No —
                    // duplicate collapse should still fire; the
                    // Soundex-shape convention is class-based, not
                    // grapheme-based. Consume the base + prime.
                    i += 2;
                    continue;
                }
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
/// prime-marker joiners after Hungarian digraph substitutions.
///
/// The output alphabet is `{A..Z, '}`. The prime marker `'` follows a
/// base letter and marks a palatalized / primed variant (from `gy`,
/// `ny`, `ty`, `zs`); the Soundex-shape encoder in
/// [`HungarianPhonex::encode`] handles the marker as a transparent
/// joiner.
fn preprocess(word: &str) -> String {
    // Step 1: fold to a Vec<char> of Unicode-lowercased scalars. Doing
    // the lowercase pass first is what lets the digraph search below
    // be a single (lower-only) match without worrying about mixed-case
    // inputs like "Cs" vs "cs".
    let lowered: alloc::vec::Vec<char> = word.chars().flat_map(char::to_lowercase).collect();

    // Step 2: fold Hungarian-specific letters to their ASCII bases and
    // build up a scalar buffer we can then walk to catch digraphs.
    // (We fold to lowercase base letters here — uppercasing happens at
    // the very end, once the digraph rewrites have run.)
    let mut base: alloc::vec::Vec<char> = alloc::vec::Vec::with_capacity(lowered.len());
    for c in lowered {
        if let Some(b) = fold_to_lower_ascii(c) {
            base.push(b);
        }
    }

    // Step 3: apply Hungarian digraph and trigraph rewrites, longest
    // first (`dzs` before `dz` and `zs`). The output uses `'`
    // (apostrophe) as a prime marker for palatalized graphemes.
    let mut out = String::with_capacity(base.len());
    let mut i = 0;
    while i < base.len() {
        let c = base[i];
        // Try trigraph first.
        if i + 2 < base.len() && c == 'd' && base[i + 1] == 'z' && base[i + 2] == 's' {
            out.push('J');
            i += 3;
            continue;
        }
        // Two-scalar digraphs.
        if i + 1 < base.len() {
            let n = base[i + 1];
            match (c, n) {
                ('c', 's') => {
                    out.push('C');
                    i += 2;
                    continue;
                }
                ('d', 'z') => {
                    out.push('Z');
                    i += 2;
                    continue;
                }
                ('g', 'y') => {
                    out.push('G');
                    out.push('\'');
                    i += 2;
                    continue;
                }
                ('l', 'y') => {
                    out.push('J');
                    i += 2;
                    continue;
                }
                ('n', 'y') => {
                    out.push('N');
                    out.push('\'');
                    i += 2;
                    continue;
                }
                ('s', 'z') => {
                    out.push('S');
                    i += 2;
                    continue;
                }
                ('t', 'y') => {
                    out.push('T');
                    out.push('\'');
                    i += 2;
                    continue;
                }
                ('z', 's') => {
                    out.push('Z');
                    out.push('\'');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Fallback: single letter, uppercase for the Soundex-shape
        // classification pass.
        out.push(c.to_ascii_uppercase());
        i += 1;
    }
    out
}

/// Fold a Unicode-lowercased scalar to a lowercase ASCII base letter,
/// or return `None` if `c` is not letter-like. Handles Hungarian long
/// and umlaut vowels.
fn fold_to_lower_ascii(c: char) -> Option<char> {
    // Silent `h`: dropped in preprocess (matches the sibling Latin
    // packs' convention). Its classification would be the zero class
    // anyway, but dropping it entirely means a leading `h` never
    // occupies the key's seed slot.
    if c == 'h' {
        return None;
    }
    if c.is_ascii_alphabetic() {
        return Some(c);
    }
    let folded = match c {
        // Long / umlaut vowel folds.
        'á' => 'a',
        'é' => 'e',
        'í' => 'i',
        'ó' | 'ö' | 'ő' => 'o',
        'ú' | 'ü' | 'ű' => 'u',
        _ => return None,
    };
    Some(folded)
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// See the classification table in the [module-level docs](self).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' | b'F' | b'V' | b'W' => b'1',
        b'C' | b'G' | b'K' | b'Q' | b'J' | b'X' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'S' | b'Z' => b'7',
        // A E I O U Y H — dropped.
        _ => b'0',
    }
}

/// Adapter that exposes [`HungarianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Hungarian::phonetic_encoder`](crate::Hungarian) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HungarianPhonexAdapter;

impl LanguagePhoneticEncoder for HungarianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        HungarianPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-hu"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        HungarianPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(HungarianPhonex.encode("").is_none());
        assert!(HungarianPhonex.encode("   ").is_none());
        assert!(HungarianPhonex.encode("---").is_none());
    }

    #[test]
    fn long_vowels_fold_to_short_at_seed() {
        // Seed vowels get lowercased and folded, then re-uppercased
        // for the key.
        assert_eq!(p("Álom"), p("Alom"));
        assert_eq!(p("Éva"), p("Eva"));
        assert_eq!(p("Íj"), p("Ij"));
        assert_eq!(p("Óra"), p("Ora"));
        assert_eq!(p("Új"), p("Uj"));
        // Rounded-front folds: `ö → O`, `ü → U`.
        assert_eq!(p("Öröm"), p("Orom"));
        assert_eq!(p("Üveg"), p("Uveg"));
        // Long rounded-front folds: `ő → O`, `ű → U`.
        assert_eq!(p("Őz"), p("Oz"));
        assert_eq!(p("Űr"), p("Ur"));
    }

    #[test]
    fn cs_digraph_encodes_as_c() {
        // "csirke" → C, I(drop), R=6, K=2, E(drop) → "C" + "6" + "2"
        //   → "C62" pad → "C620".
        assert_eq!(p("csirke"), "C620");
    }

    #[test]
    fn sz_digraph_encodes_as_s() {
        // "szó" → S, O(drop) → "S" pad → "S000".
        assert_eq!(p("szó"), "S000");
        // "szép" → S, E(drop), P=1 → "S1" pad → "S100".
        assert_eq!(p("szép"), "S100");
    }

    #[test]
    fn zs_digraph_encodes_as_primed_z() {
        // "zsák" → Z' (seed), A(drop), K=2 → "Z" (seed) then skip
        //   apostrophe joiner, then K=2 push → "Z2" pad → "Z200".
        assert_eq!(p("zsák"), "Z200");
    }

    #[test]
    fn gy_digraph_encodes_as_primed_g() {
        // "gyerek" → G' (seed), E(drop), R=6, E(drop), K=2 → "G62"
        //   pad → "G620".
        assert_eq!(p("gyerek"), "G620");
    }

    #[test]
    fn ny_digraph_encodes_as_primed_n() {
        // "nyár" → N' (seed), A(drop), R=6 → "N6" pad → "N600".
        assert_eq!(p("nyár"), "N600");
    }

    #[test]
    fn ty_digraph_encodes_as_primed_t() {
        // "tyúk" → T' (seed), U(drop), K=2 → "T2" pad → "T200".
        assert_eq!(p("tyúk"), "T200");
    }

    #[test]
    fn ly_digraph_encodes_as_j() {
        // "lyuk" → J (seed), U(drop), K=2 → "J2" pad → "J200".
        assert_eq!(p("lyuk"), "J200");
    }

    #[test]
    fn dz_digraph_encodes_as_z() {
        // "edz" → E (seed), then D+Z (→ Z) push → last was 0 (seed
        //   vowel), Z code=7 push → "E7" pad → "E700".
        assert_eq!(p("edz"), "E700");
    }

    #[test]
    fn dzs_trigraph_encodes_as_j() {
        // "dzsem" → J (seed), E(drop), M=5 push → "J5" pad → "J500".
        assert_eq!(p("dzsem"), "J500");
    }

    #[test]
    fn silent_h_is_dropped() {
        // "hat" — H drops → "AT". A (seed), T=3 push → "A3" pad
        //   → "A300".
        // vs. "at" — A (seed), T=3 push → "A3" pad → "A300". Match.
        assert_eq!(p("hat"), p("at"));
    }

    #[test]
    fn common_hungarian_words() {
        // "Budapest" — B seed last=1. U vow reset. D=3 push → "B3"
        //   last=3. A vow reset. P=1 push → "B31" last=1. E vow reset.
        //   S=7 push → "B317" last=7. T=3 push? cap 4 already: "B313"
        //   wait — after S push out=[B,3,1,7], len 4 → break. Actually
        //   let me retrace step by step.
        //   Actually after S push we have out.len()==4, then break.
        //   So key is "B317".
        //   Wait — B seed = 1 char. Then digits are added one at a
        //   time and we break when out.len() == 4. So we get
        //   B + 3 + 1 + 7 → "B317".
        assert_eq!(p("Budapest"), "B317");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("BUDAPEST"), p("budapest"));
        assert_eq!(p("SZÓ"), p("szó"));
        assert_eq!(p("Nyár"), p("nyár"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Ne"), "N000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "abba" — A seed last=0. B code=1 push → "A1" last=1. B dup
        //   drop. A vow reset. Pad → "A100".
        assert_eq!(p("abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_hu() {
        assert_eq!(HungarianPhonexAdapter.name(), "phonex-hu");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(HungarianPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = HungarianPhonexAdapter.encode("Budapest").unwrap();
        assert_eq!(primary, "B317");
        assert!(alt.is_none());
    }
}
