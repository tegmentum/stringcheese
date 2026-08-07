//! A Polish-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Polish lacks a single canonical published phonetic encoder the way
//! English has Soundex or German has Kölner Phonetik. The closest
//! options are:
//!
//! * **Language-independent Soundex** applied to ASCII-folded Polish
//!   — stable and portable, but linguistically inaccurate (the Polish
//!   digraphs `sz`, `cz`, `rz`, the diacritic-carrying letters, and
//!   the `ó / u` merger all get swept under the English mapping).
//! * **PHONEX-family encoders** — the Polish equivalent of the French
//!   / Spanish / Portuguese / Dutch PHONEX encoders shipped in the
//!   sibling language packs: apply Polish-specific preprocessing
//!   (`sz → S`, `cz → C`, `rz → R`, `ch → K`, `ó → U`, nasal
//!   `ą / ę → a / e`, `ż / ź` merge to `Z`, `ń → N`, `ć → C`,
//!   `ś → S`, `ł → L`), then run a Soundex-shape encoder over the
//!   Polish-tuned classification table.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Polish** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with a Polish-tuned preprocessing pass and a Polish-tuned
//! classification table:
//!
//! 1. **Uppercase and un-accent Polish letters.**
//!    * `Ą → A`, `Ę → E` — nasal vowels fold to their oral counterparts.
//!      **Rationale:** the phonetic key is a coarse equivalence class
//!      and Polish nasals are frequently transcribed without the
//!      nasal diacritic in casual text; folding them ensures
//!      `mąż` and `maż` and `mażz` all produce keys with the same
//!      vocalic skeleton.
//!    * `Ó → U` — the `ó` grapheme is phonetically **identical to
//!      `u`** /u/ in modern Polish; the historical distinction (`ó`
//!      was once /oː/) survives only in spelling. Merging them at the
//!      phonex level ensures `Kraków` and `krakuw` produce the same
//!      key. **Rationale:** any sound-alike encoder that ignores this
//!      merger will misclassify variants of the same word.
//!    * `Ć → C`, `Ń → N`, `Ś → S`, `Ź → Z`, `Ż → Z`, `Ł → L` — each
//!      accented consonant folds to its base letter for classification.
//!      `ż` and `ź` **both merge to `Z`** because Soundex-family
//!      encoders group all sibilants together and Polish
//!      speakers/writers frequently confuse the two.
//! 2. **Polish digraph substitutions** (applied left-to-right,
//!    longest-match first):
//!    * `SZ → S` — the /ʂ/ digraph collapses to the sibilant class.
//!    * `CZ → C` — the /tʂ/ digraph collapses to the palatal-affricate
//!      slot (class `2` alongside `c`, `k`, `q`, `x`, `g`, `j`).
//!    * `RZ → R` — the /ʐ/ digraph (etymologically /rʲ/) collapses to
//!      `R`; encoding it as class `6` matches `r`.
//!    * `CH → K` — the /x/ digraph folds to `K` (velar class `2`),
//!      matching the workspace-wide convention that treats velar
//!      fricatives as a velar-stop equivalent for Soundex-shape keys.
//!    * `RR → R` — collapse the doubled `r`.
//!    * `H → ` — silent (dropped entirely). Note: `H` is stripped
//!      **after** the `CH → K` digraph pass, so `ch` doesn't lose its
//!      consonant.
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
//! **Grouping rationale.** Polish `B/P/F/V/W` are all labial
//! obstruents and glides (`W` is the /v/ approximant in Polish, but
//! phonologically labial); `C/K/G/Q/J/X` are the velar / palatal
//! family (with `C` covering both `c` /ts/ and the palatalised `ć`
//! /tɕ/ and the digraph `cz` /tʂ/); `D/T` are dental stops; `S/Z`
//! are the sibilant family (which after digraph folding also
//! subsumes `sz`, `ż`, `ź`, and `ś`); `L` and `R` get their own
//! classes; `M/N` are nasals (which also subsumes `ń`).
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Polish** — a parallel variable-length encoder with
//!   better discrimination; heavier to reference-test.
//! * **Dedicated Polish surname encoder** — a rule-heavy encoder
//!   modeled on the Beider-Morse Phonetic Matching (BMPM) Polish
//!   ruleset; would need substantially more preprocessing rules than
//!   the Soundex-shape encoder shipped here.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Polish PHONEX encoder.
///
/// A zero-sized value; construct as [`PolishPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_pl::PolishPhonex;
///
/// let key = PolishPhonex.encode("Kowalski").unwrap();
/// assert_eq!(key, "K147");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PolishPhonex;

impl PolishPhonex {
    /// Encodes `word` per the PHONEX-Polish algorithm.
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

/// Preprocess `word` into uppercase-ASCII letters after Polish digraph
/// and single-letter substitutions.
fn preprocess(word: &str) -> String {
    // Step 1: fold to uppercase-ASCII (drops non-letter code points).
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if let Some(letter) = fold_letter(c) {
            ascii.push(letter);
        }
    }
    // Step 2: digraph & single-letter substitutions on the ASCII
    // upper-case buffer.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte digraph substitutions (longest match).
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'S', b'Z') => {
                    // /ʂ/ — collapse to sibilant class.
                    out.push('S');
                    i += 2;
                    continue;
                }
                (b'C', b'Z') => {
                    // /tʂ/ — collapse to the palatal-affricate slot.
                    out.push('C');
                    i += 2;
                    continue;
                }
                (b'R', b'Z') => {
                    // /ʐ/ (etym. /rʲ/) — collapse to `R`.
                    out.push('R');
                    i += 2;
                    continue;
                }
                (b'C', b'H') => {
                    // /x/ — velar fricative, fold to `K` (class 2).
                    out.push('K');
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
            b'H' => { /* drop as silent */ }
            _ => out.push(b as char),
        }
        i += 1;
    }
    out
}

/// Fold `c` to the single ASCII uppercase letter that stands for it,
/// or `None` if `c` is not letter-like.
///
/// Polish-specific folds:
/// * **Nasal vowels** `ą / ę` fold to their oral counterparts `a / e`.
/// * **Historical /oː/** `ó` merges with `u` (modern Polish /u/).
/// * **Palatalised consonants** `ć / ń / ś / ź / ż / ł` fold to their
///   base letters `C / N / S / Z / Z / L` (both `ź` and `ż` land in
///   the sibilant class `Z`).
///
/// General Latin diaeresis / acute / grave fallthrough is included
/// for loanwords — same handling as the Dutch pack for the shared
/// diaeresis / acute set. Polish text rarely carries these outside
/// loanwords, but folding them is the safe default.
fn fold_letter(c: char) -> Option<char> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    let folded = match c {
        // A-class: Polish nasal `ą` plus Latin diaeresis / acute /
        // grave / circumflex.
        'ą' | 'Ą' | 'á' | 'à' | 'â' | 'ä' | 'Á' | 'À' | 'Â' | 'Ä' => 'A',
        // E-class: Polish nasal `ę` plus Latin diaeresis / acute /
        // grave / circumflex.
        'ę' | 'Ę' | 'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        // I-class: Latin diaeresis / acute / grave / circumflex.
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        // O-class: Latin diaeresis / grave / circumflex (no Polish
        // extra — `ó` merges with `u`, see below).
        'ò' | 'ô' | 'ö' | 'Ò' | 'Ô' | 'Ö' => 'O',
        // U-class: Polish historical /oː/ `ó` merges here, plus Latin
        // diaeresis / acute / grave / circumflex.
        'ó' | 'Ó' | 'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        // Polish palatalised consonants fold to their base letter.
        'ć' | 'Ć' => 'C',
        'ń' | 'Ń' => 'N',
        'ś' | 'Ś' => 'S',
        // ż and ź both merge to Z (sibilant class).
        'ź' | 'Ź' | 'ż' | 'Ż' => 'Z',
        // ł folds to L.
        'ł' | 'Ł' => 'L',
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

/// Adapter that exposes [`PolishPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Polish::phonetic_encoder`](crate::Polish) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PolishPhonexAdapter;

impl LanguagePhoneticEncoder for PolishPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        PolishPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-pl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        PolishPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(PolishPhonex.encode("").is_none());
        assert!(PolishPhonex.encode("   ").is_none());
        assert!(PolishPhonex.encode("---").is_none());
    }

    #[test]
    fn nasal_vowels_fold_to_oral_counterparts() {
        // ą → a, ę → e. "mąż" and "maz" (with ż → Z) both encode the
        // same skeleton because ą folds to A.
        //   mąż → M A Z (ż → Z). M(seed,last=5). A reset. Z code=7.
        //     Push. → "M7" pad → "M700".
        //   maz → same shape.
        assert_eq!(p("mąż"), p("maż"));
        assert_eq!(p("gęś"), p("geś"));
    }

    #[test]
    fn o_kreska_merges_with_u() {
        // ó → u. "Kraków" and "Krakuw" should produce the same key.
        //   Kraków → K R A K Ó(U) W → K seed last=2, R code=6 push,
        //     A reset, K code=2 push (not dup — last was reset),
        //     U reset, W code=1 push → "K621" pad → "K621".
        //   Krakuw → same trace.
        assert_eq!(p("Kraków"), p("Krakuw"));
    }

    #[test]
    fn z_kropka_and_z_kreska_merge() {
        // Both ż and ź encode to Z (sibilant class).
        //   Żaba: Ż → Z. Z(seed,last=7), A reset, B code=1 push → "Z1"
        //     → "Z100".
        //   Źle:  Ź → Z. Z(seed,last=7), L code=4 push, E reset → "Z4"
        //     → "Z400". Not the same as żaba (different consonants
        //     after) — but the crucial equivalence is Ż vs Z (ASCII).
        assert_eq!(p("Żaba"), p("Zaba"));
        assert_eq!(p("Źle"), p("Zle"));
    }

    #[test]
    fn sz_cz_rz_digraphs_collapse() {
        // sz → S, cz → C, rz → R.
        //   szkoła → S K O L A. S(seed,last=7), K code=2 push, O reset,
        //     L code=4 push, A reset → "S24" pad → "S240".
        //   czas → C A S. C(seed,last=2), A reset, S code=7 push → "C7"
        //     → "C700".
        //   rzeka → R E K A. R(seed,last=6), E reset, K code=2 push,
        //     A reset → "R2" → "R200".
        assert_eq!(p("szkoła"), "S240");
        assert_eq!(p("czas"), "C700");
        assert_eq!(p("rzeka"), "R200");
    }

    #[test]
    fn ch_encodes_as_velar_k() {
        // ch → K (class 2).
        //   chleb → K L E B. K seed last=2, L code=4 push, E reset,
        //     B code=1 push → "K41" → "K410".
        //   chata → K A T A. K seed last=2, A reset, T code=3 push,
        //     A reset → "K3" → "K300".
        assert_eq!(p("chleb"), "K410");
        assert_eq!(p("chata"), "K300");
    }

    #[test]
    fn silent_h_is_stripped() {
        // h drops. "hala" → ALA. A seed last=0, L code=4 push, A reset
        //   → "A4" → "A400".
        // "ala" → same result.
        assert_eq!(p("hala"), p("ala"));
    }

    #[test]
    fn palatalized_consonants_fold_to_base_class() {
        // ć → C, ń → N, ś → S, ł → L. Confirm the fold happens.
        //   koń → K O N (ń → N). K seed last=2, O reset, N code=5 push
        //     → "K5" → "K500".
        //   kon → same trace.
        assert_eq!(p("koń"), p("kon"));
        //   łąka → L A K A (ł → L, ą → A). L seed last=4, A reset,
        //     K code=2 push, A reset → "L2" → "L200".
        //   laka → same trace.
        assert_eq!(p("łąka"), p("laka"));
        //   ćma → C M A. C seed last=2, M code=5 push, A reset → "C5"
        //     → "C500".
        //   cma → same.
        assert_eq!(p("ćma"), p("cma"));
    }

    #[test]
    fn common_polish_surnames() {
        // Kowalski: K O W A L S K I.
        //   K seed last=2. O reset. W code=1 push "K1" last=1.
        //   A reset. L code=4 push "K14" last=4. S code=7 push "K147".
        //   K code=2 push — but we're already at 4 chars, stop.
        // Wait, "K147" is 4 chars. So we stop there.
        // Hmm let me recount: K(1) 1(2) 4(3) 7(4) → "K147".
        // Actually wait — the assertion at the top of the module doc
        // is `K142`. Let me recount.
        // K seed. Push seed → out="K", last_code = code_of('K') = 2.
        // O: code=0. last_code reset to 0.
        // W: code=1. Push '1'. out="K1", last_code=1.
        // A: code=0. last_code reset to 0.
        // L: code=4. Push '4'. out="K14", last_code=4.
        // S: code=7. Push '7'. out="K147", last_code=7.
        // K: code=2. Push '2'. out="K1472"? — but out.len() == 4 check
        //   fires. Actually after pushing '2', out.len() == 5. Hmm.
        // Wait, the check is `if out.len() == 4 break`. So after pushing
        //   '7' out.len() == 4, and we break BEFORE processing K.
        // So we get "K147".
        // The doc example on PolishPhonex says K142. Let me fix the doc.
        //
        // Actually let me re-examine — no, my trace is right. "K147"
        // is what we get. Let me update the doc to match.
        assert_eq!(p("Kowalski"), "K147");
        // Nowak: N O W A K.
        //   N seed last=5. O reset. W code=1 push "N1". A reset. K code=2
        //     push "N12". → "N12" pad → "N120".
        assert_eq!(p("Nowak"), "N120");
        // Wiśniewski: W I Ś(S) N I E W S K I.
        //   W seed last=1. I reset. S code=7 push "W7" last=7. N code=5
        //     push "W75" last=5. I reset. E reset. W code=1 push "W751".
        //     S code=7 push — out.len() at push == 4 → break check after.
        //   Actually after pushing 1 we have "W751" == 4 chars → break.
        // So "W751".
        assert_eq!(p("Wiśniewski"), "W751");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("KOWALSKI"), p("kowalski"));
        assert_eq!(p("ŻABA"), p("żaba"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Za"), "Z000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Anna" — A(seed), N(5,push,last=5), N(dup drop), A(reset)
        //   → "A5" → "A500"
        assert_eq!(p("Anna"), "A500");
    }

    #[test]
    fn adapter_returns_name_phonex_pl() {
        assert_eq!(PolishPhonexAdapter.name(), "phonex-pl");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(PolishPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = PolishPhonexAdapter.encode("Kowalski").unwrap();
        assert_eq!(primary, "K147");
        assert!(alt.is_none());
    }
}
