//! A Dutch-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Dutch lacks a single canonical published phonetic encoder the way
//! English has Soundex or German has Kölner Phonetik. The closest
//! candidates:
//!
//! * **Language-independent Soundex** applied to ASCII-folded Dutch —
//!   stable and portable, but linguistically inaccurate (the `ij`
//!   digraph, the `sch` cluster, the diaeresis, and the velar `ch` all
//!   get swept under the English mapping).
//! * **PHONEX-family encoders** — the Dutch equivalent of the French
//!   / Spanish / Portuguese PHONEX encoders shipped in the sibling
//!   language packs: apply Dutch-specific preprocessing (fold
//!   diacritics, collapse the `ij` digraph, split `sch` into
//!   sibilant-plus-velar, map `ch` to a velar placeholder), then run
//!   a Soundex-shape encoder over the Dutch-tuned classification table.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Dutch** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with a Dutch-tuned preprocessing pass and a Dutch-tuned
//! classification table:
//!
//! 1. **Uppercase and un-accent.** `Ä Á À Â → A`, `Ë É È Ê → E`,
//!    `Ï Í Ì Î → I`, `Ö Ó Ò Ô → O`, `Ü Ú Ù Û → U`. Dutch has no
//!    cedilla or nasal-vowel diacritics of its own, so no additional
//!    single-character folds are needed.
//! 2. **Dutch digraph and single-letter substitutions.**
//!    * `IJ → I` (the Dutch `ij` digraph fuses to a single sound
//!      pronounced roughly like English "eye" or Modern Dutch `ei`;
//!      it collapses to `I` for encoding purposes so `mijn` and
//!      `mien` produce similar keys).
//!    * `SCH → SX` (the initial `sch` cluster is /sx/ — a sibilant
//!      followed by the voiceless velar fricative. The `X`
//!      placeholder codes into the velar class below, so `schip`
//!      encodes with a sibilant + velar prefix.)
//!    * `CH → G` (the standalone `ch` in Dutch is the voiceless velar
//!      fricative /x/. Folding to `G` places it in the velar class,
//!      preserving the palatal-of-place phonetic content.)
//!    * `RR → R`.
//!    * `H → ` (silent — dropped entirely, matching the -pt / -es
//!      packs). Note: this fires *after* the `CH → G` digraph pass,
//!      so `ch` doesn't lose its consonant.
//! 3. **Soundex-shape encoding.** Retain the first letter; encode
//!    each subsequent letter by the classification table below; drop
//!    the zero class (vowels); collapse consecutive equal codes;
//!    truncate to three digits and left-pad with `'0'` to reach length
//!    four.
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
//! **Grouping rationale.** Dutch `B/P/F/V/W` are all labial obstruents
//! and glides (surface `V` is often devoiced to `/f/` between
//! consonants; `W` is `/ʋ/`, a labiodental approximant); `C/K/G/Q/J/X`
//! are the velar / palatal family with `X` used as an internal
//! placeholder for the velar fricative that `CH` and `SCH` code as;
//! `D/T` are dental stops; `S/Z` are the sibilant family; `L` and `R`
//! get their own classes; `M/N` are nasals. The Dutch `J` is `/j/`
//! (palatal glide) — placed in the velar/palatal class 2 alongside
//! `G` (which is also often /ʝ/ or /x/) so `Jansen` and `Ganseman`
//! don't fall through to unrelated classes.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Dutch** — a parallel variable-length encoder with
//!   better discrimination; heavier to reference-test.
//! * **Van Berkel Dutch phonetic algorithm** — a Dutch-specific
//!   published algorithm with rule-heavy morphology awareness; would
//!   need a substantial rule set out of scope for a starter pack.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Dutch PHONEX encoder.
///
/// A zero-sized value; construct as [`DutchPhonex`] and reuse the value
/// freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_nl::DutchPhonex;
///
/// let key = DutchPhonex.encode("Jansen").unwrap();
/// assert_eq!(key, "J575");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DutchPhonex;

impl DutchPhonex {
    /// Encodes `word` per the PHONEX-Dutch algorithm.
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

/// Preprocess `word` into uppercase-ASCII letters after Dutch digraph
/// and single-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold to uppercase-ASCII (drops non-letter code points).
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if let Some(letter) = fold_letter(c) {
            ascii.push(letter);
        }
    }
    // Step 2: digraph & single-letter substitutions.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Three-byte digraphs first (longest match).
        if i + 2 < bytes.len() && b == b'S' && bytes[i + 1] == b'C' && bytes[i + 2] == b'H' {
            out.push('S');
            out.push('X');
            i += 3;
            continue;
        }
        // Two-byte digraph substitutions.
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'I', b'J') => {
                    out.push('I');
                    i += 2;
                    continue;
                }
                (b'C', b'H') => {
                    out.push('G');
                    i += 2;
                    continue;
                }
                (b'R', b'R') => {
                    out.push('R');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter substitutions.
        match b {
            b'H' => { /* drop */ }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

/// Fold `c` to the single ASCII uppercase letter that stands for it,
/// or `None` if `c` is not letter-like.
fn fold_letter(c: char) -> Option<char> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Dutch-specific folds. The diaeresis marks a syllable break; the
    // acute marks primary stress. Both fold to the base vowel for
    // encoding.
    let folded = match c {
        'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
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

/// Adapter that exposes [`DutchPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Dutch::phonetic_encoder`](crate::Dutch) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DutchPhonexAdapter;

impl LanguagePhoneticEncoder for DutchPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        DutchPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-nl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        DutchPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(DutchPhonex.encode("").is_none());
        assert!(DutchPhonex.encode("   ").is_none());
        assert!(DutchPhonex.encode("---").is_none());
    }

    #[test]
    fn diaeresis_and_acute_fold_to_base_vowel() {
        assert_eq!(p("één"), p("een"));
        assert_eq!(p("ideeën"), p("ideeen"));
        assert_eq!(p("café"), p("cafe"));
    }

    #[test]
    fn ij_digraph_collapses_to_i() {
        // `mijn` → preprocess IJ → I → `MIN`. M(seed,last=5), I(reset),
        //   N(5,push,last=5 dup drop) — wait, M is 5 (seed), then I reset,
        //   then N is 5 → not dup (last was reset to 0). Push. → "MN" →
        //   pad → "M500"
        // Hmm actually let me recompute: M seed sets last_code = 5.
        //   Then I is class 0 → reset last_code = 0.
        //   Then N is 5. Not dup (5 != 0). Push. last_code = 5. → "MN" → "M500".
        assert_eq!(p("mijn"), "M500");
        // `mien` (matches `mijn` phonetically) → `M`, `I`, `E`, `N`.
        //   M seed last=5. I reset. E reset. N class 5. Push. → "MN" → "M500".
        assert_eq!(p("mien"), p("mijn"));
    }

    #[test]
    fn sch_cluster_encodes_as_sibilant_plus_velar() {
        // `schip` → preprocess: SCH → SX, then I, P. So "SXIP".
        //   S seed last=7. X class 2. Push → "SX" last=2. I reset last=0.
        //   P class 1. Push → "SXP" last=1. → "SXP" pad → "SXP0".
        //   Wait `SXP` = 3 chars. Pad to 4 → "SXP0".
        //   Hmm output has an alphabetic X in position 2. That's fine —
        //   the algorithm's seed is the raw first letter of the
        //   preprocessed string (S here), and subsequent chars are
        //   Soundex digits. But wait, I write out X which is a *digit*
        //   from the classification (class 2). So the output char is
        //   '2', not 'X'. Let me re-trace.
        //   S is 'S' (seed literal). Then for each subsequent byte,
        //   compute code_of(b) → digit char '2' for X. We push '2' as
        //   the digit character. So the key becomes "S" + "2" + ... =
        //   "S2...".
        //   Full trace: seed='S' last_code=b'7'. For X: code = b'2'.
        //   Not 0, not dup. Push '2' → "S2" last=b'2'. For I: code=0.
        //   Reset last=0. For P: code=1. Push '1' → "S21" last=1.
        //   Pad → "S210".
        assert_eq!(p("schip"), "S210");
    }

    #[test]
    fn ch_encodes_as_velar() {
        // `licht` → preprocess: L, I, CH→G, T. "LIGT".
        //   L seed last=4. I reset. G code=2. Push '2'. T code=3. Push '3'.
        //   → "L23" pad → "L230".
        assert_eq!(p("licht"), "L230");
        // `nacht` → N, A, CH→G, T. "NAGT". N seed last=5. A reset. G code=2.
        //   Push '2'. T code=3. Push '3'. → "N23" pad → "N230".
        assert_eq!(p("nacht"), "N230");
    }

    #[test]
    fn silent_h_is_stripped() {
        // `Hendrik` (Henry) — H drops → ENDRIK.
        //   E seed last=0. N code=5. Push '5'. D code=3. Push '3'.
        //   R code=6. Push '6'. I reset. K code=2. Push '2'. → "E5362".
        //   But cap at 4 → "E536".
        // `Endrik` → same result.
        assert_eq!(p("Hendrik"), p("Endrik"));
    }

    #[test]
    fn v_and_w_and_b_and_p_and_f_are_labials() {
        // All in class 1. `Bakker` and `Pakker` should share the
        // sibilant/velar/labial layout.
        //   Bakker: B seed last=1. A reset. K code=2. Push '2'. K dup drop.
        //     E reset. R code=6. Push '6'. → "B26" pad → "B260".
        //   Pakker: same shape → "P260".
        //   Vakker → "V260". Wakker → "W260".
        assert_eq!(p("Bakker"), "B260");
        assert_eq!(p("Pakker"), "P260");
        assert_eq!(p("Vakker"), "V260");
        assert_eq!(p("Wakker"), "W260");
    }

    #[test]
    fn common_dutch_surnames() {
        // Jansen: J seed last=2. A reset. N code=5. Push '5'. S code=7.
        //   Push '7'. E reset. N code=5. Push '5' — not dup (last was
        //   reset to 0 after E). → "J575" cap → "J575".
        assert_eq!(p("Jansen"), "J575");
        // Bakker: → "B260" (computed above).
        assert_eq!(p("Bakker"), "B260");
        // Visser: V seed last=1. I reset. S code=7. Push '7'. S dup drop.
        //   E reset. R code=6. Push '6'. → "V76" pad → "V760".
        assert_eq!(p("Visser"), "V760");
        // Vries: V seed last=1. R code=6. Push '6'. I reset. E reset.
        //   S code=7. Push '7'. → "V67" pad → "V670".
        assert_eq!(p("Vries"), "V670");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("JANSEN"), p("jansen"));
        assert_eq!(p("Één"), p("één"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("De"), "D000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Abba" — A(seed), B(1,push,last=1), B(dup drop), A(reset)
        //   → "A1" → "A100"
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_nl() {
        assert_eq!(DutchPhonexAdapter.name(), "phonex-nl");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(DutchPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = DutchPhonexAdapter.encode("Jansen").unwrap();
        assert_eq!(primary, "J575");
        assert!(alt.is_none());
    }
}
