//! An Icelandic-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Icelandic lacks a single canonical published phonetic encoder the
//! way English has Soundex or German has Kölner Phonetik. The
//! sensible choices:
//!
//! * **Language-independent Soundex** applied to ASCII-folded
//!   Icelandic — stable and portable, but linguistically inaccurate
//!   (the `þ`/`ð` dental fricatives collapse silently, the `hv`
//!   historical cluster loses its Modern-Icelandic /kʰv/ realization,
//!   and the vowel-accent + `æ`/`ö` letters all get swept under the
//!   English mapping).
//! * **PHONEX-family encoder** — the Icelandic sibling of the
//!   Swedish / Norwegian / Danish / Dutch / French PHONEX encoders
//!   shipped in the neighbouring language packs: apply
//!   Icelandic-specific preprocessing (the `þ → th` / `ð → dh`
//!   letter-to-digraph rewrites, the `æ → ae` / `ö → oe` vowel
//!   folds, the `hv → kv` historical cluster fold, and vowel-accent
//!   folding to base letters), then run a Soundex-shape encoder over
//!   the classification table.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Icelandic** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with Icelandic-tuned preprocessing and the standard PHONEX
//! classification table:
//!
//! 1. **Uppercase and un-accent.** `Á À Â Ä → A`, `É È Ê Ë → E`,
//!    `Í Ì Î Ï → I`, `Ó Ò Ô Ö → O`, `Ú Ù Û Ü → U`, `Ý → Y`. The
//!    accented-vowel folds collapse the six long-vowel scalars
//!    (`á é í ó ú ý`) into their base letters — the phonetic key
//!    treats vowel length as spelling-only rather than encoding it.
//! 2. **Icelandic letter-to-digraph rewrites.**
//!    * `Þ → TH`. The voiceless dental fricative /θ/ has no ASCII
//!      single-letter equivalent; the standard Anglicization
//!      digraph is `th`. This makes `Þór` and English `Thor` collide
//!      as intended.
//!    * `Ð → DH`. The voiced dental fricative /ð/ similarly rewrites
//!      to `dh`.
//!    * `Æ → AE`. The front-open diphthong /ai/ (Modern Icelandic)
//!      rewrites to the two-vowel digraph.
//!    * `Ö → OE`. The rounded-mid front vowel /œ/ rewrites to `oe`.
//! 3. **Icelandic cluster rewrites.** Longest-match first:
//!    * `HV → KV`. Modern Icelandic pronounces the historical `hv`
//!      cluster (from Old Norse) as /kʰv/ — the `hv-`/`kv-` merger
//!      is one of the most salient sound-changes in the modern
//!      language. Folding `hv → kv` at the phonetic-encoder level
//!      captures the merger: `hvíta` (white, fem acc sg) and
//!      hypothetical `kvíta` share a key.
//! 4. **Silent H.** After the `hv → kv` fold, any remaining `H` is
//!    stripped (word-initial or word-interior) to match the
//!    Norwegian/Dutch/Swedish/Danish PHONEX convention.
//! 5. **Soundex-shape encoding.** Retain the first letter; encode each
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
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Icelandic** — a parallel variable-length encoder
//!   with better discrimination; heavier to reference-test.
//! * **Preaspiration encoding.** Modern Icelandic contrasts geminate
//!   and preaspirated stops (`kk` /ʰk/ vs `k` /k/); not
//!   orthographically distinctive enough to encode at this layer.
//! * **`ll` → `dl`, `nn` → `dn` fortition.** Modern Icelandic
//!   fortifies the geminate liquids `ll`/`nn` to /tl/, /tn/ after
//!   long vowels or diphthongs. Capturing this cleanly requires
//!   knowledge of the preceding vowel's length — a lexicon-level
//!   distinction and out of scope for a spelling-based encoder.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Icelandic PHONEX encoder.
///
/// A zero-sized value; construct as [`IcelandicPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_is::IcelandicPhonex;
///
/// // 'Þór' → preprocess Þ→TH, Ó→O. → "THOR".
/// //   T(seed,last=3), H stripped, O reset, R(6) → "T600".
/// let key = IcelandicPhonex.encode("Þór").unwrap();
/// assert_eq!(key, "T600");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IcelandicPhonex;

impl IcelandicPhonex {
    /// Encodes `word` per the PHONEX-Icelandic algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or an input reduced to
    /// nothing after silent-`H` stripping). Otherwise returns a
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

/// Preprocess `word` into uppercase-ASCII letters after Icelandic
/// letter-to-digraph and cluster substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold to uppercase-ASCII, expanding Icelandic letters
    // into their digraph equivalents (`þ → TH`, `ð → DH`, `æ → AE`,
    // `ö → OE`) and dropping non-letter code points.
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        fold_letter(c, &mut ascii);
    }
    // Step 2: cluster substitutions on the ASCII form. Longest-match
    // first. Currently:
    //   - `HV → KV` (Modern Icelandic hv-/kv- merger).
    //   - `H → ` (silent, word-initial or word-interior).
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte digraph HV → KV.
        if b == b'H' && i + 1 < bytes.len() && bytes[i + 1] == b'V' {
            out.push('K');
            out.push('V');
            i += 2;
            continue;
        }
        // Single-letter substitutions.
        if b == b'H' {
            // Drop silent H (both word-initial and word-interior;
            // matches the PHONEX-Norwegian/Danish convention).
            i += 1;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out
}

/// Fold `c` to its uppercase-ASCII representation and push it into
/// `out`. Icelandic-specific letters expand to two-character digraphs.
/// Non-letter code points are silently dropped.
fn fold_letter(c: char, out: &mut String) {
    if c.is_ascii_alphabetic() {
        out.push(c.to_ascii_uppercase());
        return;
    }
    // Icelandic-specific digraph expansions.
    match c {
        'þ' | 'Þ' => {
            out.push('T');
            out.push('H');
            return;
        }
        'ð' | 'Ð' => {
            out.push('D');
            out.push('H');
            return;
        }
        'æ' | 'Æ' => {
            out.push('A');
            out.push('E');
            return;
        }
        'ö' | 'Ö' => {
            out.push('O');
            out.push('E');
            return;
        }
        _ => {}
    }
    // Single-letter accent folds. The Icelandic long-vowel scalars
    // `á é í ó ú ý` share arms with their Latin diacritic-carrying
    // neighbours so clippy::match_same_arms doesn't flag the collapse.
    let folded = match c {
        'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'Ó' | 'Ò' | 'Ô' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ý' | 'Ý' => 'Y',
        _ => return,
    };
    out.push(folded);
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

/// Adapter that exposes [`IcelandicPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Icelandic::phonetic_encoder`](crate::Icelandic) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IcelandicPhonexAdapter;

impl LanguagePhoneticEncoder for IcelandicPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        IcelandicPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-is"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        IcelandicPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(IcelandicPhonex.encode("").is_none());
        assert!(IcelandicPhonex.encode("   ").is_none());
        assert!(IcelandicPhonex.encode("---").is_none());
    }

    #[test]
    fn thorn_expands_to_th() {
        // 'Þór' → preprocess Þ→TH, Ó→O. → "THOR".
        //   T(seed,last=3), H stripped, O reset, R(6) → "T600"
        assert_eq!(p("Þór"), "T600");
        // 'Þór' and 'Thor' share the key by construction.
        assert_eq!(p("Þór"), p("Thor"));
    }

    #[test]
    fn eth_expands_to_dh() {
        // 'góður' (good, masc nom sg) → preprocess G,Ó→O,Ð→DH,U,R.
        //   → "GODHUR". G(seed,last=2), O reset, D(3), H stripped,
        //   U reset, R(6) → "G36" pad → "G360"
        assert_eq!(p("góður"), "G360");
    }

    #[test]
    fn ae_expands_to_ae_digraph() {
        // 'Æsir' → AE, S, I, R. → "AESIR".
        //   A(seed,last=0), E reset, S(7), I reset, R(6) → "A76" pad
        //   → "A760"
        assert_eq!(p("Æsir"), "A760");
    }

    #[test]
    fn o_umlaut_expands_to_oe_digraph() {
        // 'Björn' → B, J, Ö→OE, R, N. → "BJOERN".
        //   B(seed,last=1), J(2), O reset, E reset, R(6), N(5)
        //   → "B265"
        assert_eq!(p("Björn"), "B265");
    }

    #[test]
    fn hv_cluster_becomes_kv() {
        // 'hvíta' (white, fem acc sg) → HV→KV, then I(from Í), T, A.
        //   → "KVITA". K(seed,last=2), V(1), I reset, T(3), A reset
        //   → "K13" pad → "K130"
        assert_eq!(p("hvíta"), "K130");
        // hv-/kv- merger: 'hvíta' and 'kvíta' share the key.
        assert_eq!(p("hvíta"), p("kvíta"));
    }

    #[test]
    fn silent_h_is_stripped() {
        // 'Hafa' → H drops → AFA.
        //   A(seed,last=0), F(1), A reset → "A1" pad → "A100"
        assert_eq!(p("Hafa"), "A100");
        // Also matches ASCII 'Afa'.
        assert_eq!(p("Hafa"), p("Afa"));
    }

    #[test]
    fn common_icelandic_words() {
        // 'hestur' → HESTUR. H drops → ESTUR.
        //   E(seed,last=0), S(7), T(3), U reset, R(6) → "E736"
        assert_eq!(p("hestur"), "E736");
        // 'bók' → BOK. B(seed,last=1), O reset, K(2) → "B2" pad
        //   → "B200"
        assert_eq!(p("bók"), "B200");
        // 'kona' → KONA. K(seed,last=2), O reset, N(5), A reset
        //   → "K5" pad → "K500"
        assert_eq!(p("kona"), "K500");
        // 'vera' → VERA. V(seed,last=1), E reset, R(6), A reset
        //   → "V6" pad → "V600"
        assert_eq!(p("vera"), "V600");
    }

    #[test]
    fn common_icelandic_surnames() {
        // 'Jónsson' → J, Ó→O, N, S, S, O, N. → "JONSSON".
        //   J(seed,last=2), O reset, N(5), S(7), S dup, O reset,
        //   N(5) → "J575"
        assert_eq!(p("Jónsson"), "J575");
        // 'Sigurðsson' → S, I, G, U, R, Ð→DH, S, S, O, N.
        //   → "SIGURDHSSON".
        //   S(seed,last=7), I reset, G(2), U reset, R(6), D(3), H
        //   stripped, S(7), S dup, O reset, N(5) — capped → "S263"
        assert_eq!(p("Sigurðsson"), "S263");
    }

    #[test]
    fn accent_folds_are_case_insensitive() {
        // Long-vowel folds: Á/À/etc → A; Í/Ì/etc → I; etc.
        assert_eq!(p("ára"), p("ara"));
        assert_eq!(p("ís"), p("is"));
        assert_eq!(p("Ýr"), p("Yr"));
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("HESTUR"), p("hestur"));
        assert_eq!(p("ÞÓR"), p("þór"));
        assert_eq!(p("BJÖRN"), p("björn"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("ís"), "I700");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // 'Abba' — A(seed), B(1,push,last=1), B(dup drop), A reset
        //   → "A1" → "A100"
        assert_eq!(p("Abba"), "A100");
    }

    #[test]
    fn adapter_returns_name_phonex_is() {
        assert_eq!(IcelandicPhonexAdapter.name(), "phonex-is");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(IcelandicPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = IcelandicPhonexAdapter.encode("Þór").unwrap();
        assert_eq!(primary, "T600");
        assert!(alt.is_none());
    }
}
