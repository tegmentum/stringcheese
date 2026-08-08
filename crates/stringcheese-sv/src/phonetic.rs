//! A Swedish-tuned Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! Swedish lacks a single canonical published phonetic encoder the way
//! English has Soundex or German has Kölner Phonetik. The closest
//! candidates:
//!
//! * **Language-independent Soundex** applied to ASCII-folded Swedish —
//!   stable and portable, but linguistically inaccurate (the `sj`
//!   cluster, the `tj` cluster, the palatalized `k` before front
//!   vowels, and the three Swedish-specific vowels `å`, `ä`, `ö` all
//!   get swept under the English mapping).
//! * **PHONEX-family encoders** — the Swedish equivalent of the French
//!   / Spanish / Portuguese / Dutch / Polish / Czech PHONEX encoders
//!   shipped in the sibling language packs: apply Swedish-specific
//!   preprocessing (fold `å ä ö` to base vowels; rewrite the sj-family
//!   sibilant clusters `sj`, `sk` before front vowels, `stj`, `skj`,
//!   `sch` to a single sibilant `S`; rewrite the tj-family palatal
//!   clusters `tj`, `kj`, and `k` before front vowels to a single
//!   palatal `C`), then run a Soundex-shape encoder over a Swedish-
//!   tuned classification table.
//!
//! # Implementation choice
//!
//! This module ships a **PHONEX-Swedish** encoder. Concretely, the
//! algorithm is a 4-character `<letter><digit><digit><digit>` Soundex
//! key with Swedish-tuned preprocessing and classification:
//!
//! 1. **Uppercase and un-accent.** `Å → O`, `Ä → E`, `Ö → E`, plus the
//!    stock diacritic folds `Á À Â → A`, `É È Ê → E`, etc. Swedish's
//!    three extra letters `å`, `ä`, `ö` collapse to the base vowels
//!    they phonetically resemble (rounded back `å` ≈ `o`; front `ä`
//!    and `ö` ≈ `e`). This means the encoder does *not* discriminate
//!    between `hår` and `hor`, or between `är` and `er` — deliberate,
//!    matching the record-linkage philosophy of a phonetic key.
//! 2. **Swedish digraph and cluster substitutions**, applied left-to-
//!    right with the longest match preferred:
//!    * `STJ` / `SKJ` → `S` (both are /ɧ/, the "sj-sound").
//!    * `SCH` → `S` (the /ɧ/ cluster in loanwords).
//!    * `SJ` → `S` (the /ɧ/ cluster).
//!    * `SK` before a front vowel (`E`, `I`, `Y`, `Ä`, `Ö` — post-fold
//!      `E`, `I`, `Y`) → `S` (the /ɧ/ cluster in native words like
//!      `sked`, `skära`). Before a back vowel (`A`, `O`, `U`, `Å`,
//!      post-fold `A`, `O`, `U`), `SK` stays as `S` + `K`.
//!    * `TJ` → `C` (the "tj-sound" /ɕ/).
//!    * `KJ` → `C` (the "tj-sound" /ɕ/).
//!    * `K` before a front vowel (`E`, `I`, `Y`) → `C` (palatalized to
//!      /ɕ/ in native words like `köpa`, `kyrka` — the front vowel is
//!      the trigger).
//!    * `CH` → `S` (the /ɧ/ cluster in loanwords like `choklad`).
//!    * `H` → dropped after `S`, `T`, `C`, `D`, `P` in loanword
//!      contexts. Kept elsewhere as an ordinary consonant per Swedish
//!      practice (Swedish `h` is voiced at word-onset).
//! 3. **Soundex-shape encoding.** Retain the first letter of the
//!    preprocessed form; encode each subsequent letter by the
//!    classification table below; drop the zero class (vowels);
//!    collapse consecutive equal codes; truncate to three digits and
//!    left-pad with `'0'` to reach length four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P F V W |
//! | 2    | C K G Q J X |
//! | 3    | D T |
//! | 4    | L |
//! | 5    | M N |
//! | 6    | R |
//! | 7    | S Z |
//! | 0    | A E I O U Y (dropped as vowels); H remains a class-0 (dropped) consonant to match the Soundex/Kölner practice of ignoring stand-alone `h` for encoding purposes |
//!
//! **Grouping rationale.** Swedish `B/P/F/V/W` are all labial obstruents
//! and glides (`W` occurs only in loanwords, encoded like `V`);
//! `C/K/G/Q/J/X` are the velar/palatal family with `C` used as the
//! internal placeholder for the tj-sound; `D/T` are dental stops; `S/Z`
//! are the sibilant family with `S` also used as the internal placeholder
//! for the sj-sound; `L` and `R` get their own classes; `M/N` are nasals.
//! Swedish `Z` occurs only in loanwords and merges with `S`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Swedish** — a parallel encoder with a variable-length
//!   key; better for record-linkage precision, but heavier to
//!   reference-test.
//! * **Nordic Soundex** (a shared code for Swedish / Norwegian / Danish)
//!   — a candidate for a `stringcheese-phonetic` cross-pack encoder.
//! * **Finland-Swedish tuning.** The sj-sound is realized differently
//!   in Finland-Swedish; the current encoder collapses both realisations
//!   to `S`.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Swedish PHONEX encoder.
///
/// A zero-sized value; construct as [`SwedishPhonex`] and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_sv::SwedishPhonex;
///
/// let key = SwedishPhonex.encode("Andersson").unwrap();
/// assert_eq!(key, "A536");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SwedishPhonex;

impl SwedishPhonex {
    /// Encodes `word` per the PHONEX-Swedish algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or an input reduced to
    /// nothing after preprocessing). Otherwise returns a 4-character
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
                // Vowel / class-0 consonant — reset the duplicate-
                // collapse state.
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

/// Preprocess `word` into uppercase-ASCII letters after Swedish digraph
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
        // Three-byte digraphs / clusters first (longest match).
        if i + 2 < bytes.len() && b == b'S' {
            let b2 = bytes[i + 1];
            let b3 = bytes[i + 2];
            // STJ / SKJ → S (the sj-sound in native and loanword forms).
            // SCH → S (loanword /ɧ/).
            let is_stj_skj = matches!(b2, b'T' | b'K') && b3 == b'J';
            let is_sch = b2 == b'C' && b3 == b'H';
            if is_stj_skj || is_sch {
                out.push('S');
                i += 3;
                continue;
            }
        }
        // Two-byte digraph substitutions.
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                // SJ → S (the sj-sound). CH → S (loanword /ɧ/, e.g.
                // choklad). SK before a front vowel → S; before a back
                // vowel or consonant, SK falls through and each letter
                // encodes independently.
                (b'S', b'J') | (b'C', b'H') => {
                    out.push('S');
                    i += 2;
                    continue;
                }
                (b'S', b'K') if i + 2 < bytes.len() && is_front_vowel_ascii(bytes[i + 2]) => {
                    out.push('S');
                    i += 2;
                    continue;
                }
                // TJ / KJ → C (the tj-sound).
                (b'T' | b'K', b'J') => {
                    out.push('C');
                    i += 2;
                    continue;
                }
                // K before a front vowel → C (palatalized /ɕ/).
                (b'K', v) if is_front_vowel_ascii(v) => {
                    out.push('C');
                    i += 1;
                    continue;
                }
                _ => {}
            }
        }
        // Single-letter passthrough. `H` stays as a class-0 letter (see
        // the module doc) — no explicit strip needed.
        out.push(b as char);
        i += 1;
    }
    out
}

/// Fold `c` to the single ASCII uppercase letter that stands for it,
/// or `None` if `c` is not letter-like.
///
/// The three Swedish extras `å`, `ä`, `ö` fold to base vowels: `å → O`
/// (rounded back), `ä → E`, `ö → E`. The stock Continental diacritics
/// fold to their base vowels.
fn fold_letter(c: char) -> Option<char> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // Groups collapsed by output class — `å` folds with the back-round
    // vowels to `O`; `ä ö` fold with the front vowels to `E`.
    let folded = match c {
        'á' | 'à' | 'â' | 'Á' | 'À' | 'Â' => 'A',
        'ä' | 'Ä' | 'ö' | 'Ö' | 'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'å' | 'Å' | 'ó' | 'ò' | 'ô' | 'Ó' | 'Ò' | 'Ô' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        _ => return None,
    };
    Some(folded)
}

/// Front-vowel test on an ASCII uppercase byte. Used by the palatal /
/// sj-cluster triggers.
#[inline]
fn is_front_vowel_ascii(b: u8) -> bool {
    matches!(b, b'E' | b'I' | b'Y')
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

/// Adapter that exposes [`SwedishPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Swedish::phonetic_encoder`](crate::Swedish) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SwedishPhonexAdapter;

impl LanguagePhoneticEncoder for SwedishPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        SwedishPhonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex-sv"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        SwedishPhonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(SwedishPhonex.encode("").is_none());
        assert!(SwedishPhonex.encode("   ").is_none());
        assert!(SwedishPhonex.encode("---").is_none());
    }

    #[test]
    fn swedish_extra_vowels_fold_to_base_vowels() {
        // å → O, ä → E, ö → E. Only the class-0 collapse matters — the
        // key emerges class-blind for these.
        assert_eq!(p("hår"), p("hor"));
        assert_eq!(p("är"), p("er"));
        assert_eq!(p("över"), p("ever"));
    }

    #[test]
    fn sj_cluster_encodes_as_s() {
        // sjö → SJ → S, then Ö → E (class 0 drop). Result: "S000".
        //   Actually trace: preprocess "sjö" → SJ + Ö → 'S' + 'E'.
        //   Encoding: seed 'S' last_code=7. Then 'E' code=0 (vowel).
        //   → "S" + pad → "S000".
        assert_eq!(p("sjö"), "S000");
        // "sjuk" (sick) → SJ + U + K → S + U + K.
        //   Seed 'S' last=7. 'U' class 0 reset. 'K' class 2 push → "SK"
        //   → "S2" pad → "S200".
        assert_eq!(p("sjuk"), "S200");
    }

    #[test]
    fn sk_before_front_vowel_encodes_as_s() {
        // "sked" (spoon) — SK before E → S. Then E class 0. D class 3.
        //   Seed 'S' last=7. 'E' class 0 reset. 'D' class 3 push → "SD"
        //   → "S3" pad → "S300".
        assert_eq!(p("sked"), "S300");
        // "skära" (to cut) — SK before Ä (which folds to E — front). But
        //   wait: preprocess step 1 folds Ä→E first, so at digraph step
        //   we see "SKERA". Then SK before E → S. Then E, R, A → S, E
        //   (reset), R (6, push), A (reset).
        //   Seed 'S' last=7. E reset. R code=6 push → "SR" → "S6" last=6.
        //   A reset. → "S6" pad → "S600".
        assert_eq!(p("skära"), "S600");
    }

    #[test]
    fn sk_before_back_vowel_stays_separate() {
        // "skola" (school) — SK before O (back). Falls through: S + K
        //   + O + L + A.
        //   Seed 'S' last=7. K code=2 push → "SK" → "S2" last=2. O
        //   reset. L code=4 push → "S24" last=4. A reset. → "S24" pad
        //   → "S240".
        assert_eq!(p("skola"), "S240");
    }

    #[test]
    fn tj_and_kj_encode_as_c() {
        // "tjugo" (twenty) — TJ → C. U reset. G code=2. O reset.
        //   Seed 'C' last=2. U reset. G code=2. Not dup (last was reset
        //   to 0). Push → "CG" → "C2" last=2. O reset. → "C2" pad →
        //   "C200".
        assert_eq!(p("tjugo"), "C200");
        // "kjol" (skirt) — KJ → C. O reset. L code=4.
        //   Seed 'C' last=2. O reset. L code=4 push → "CL" → "C4" pad
        //   → "C400".
        assert_eq!(p("kjol"), "C400");
    }

    #[test]
    fn k_before_front_vowel_encodes_as_c() {
        // "köpa" (to buy) — K before Ö (folds to E — front) → C. Then
        //   Ö (already consumed by the K+front-vowel rule? No — the
        //   rule pushes 'C' and advances by 1, so the vowel is
        //   preserved in the next iteration).
        //   Preprocess "köpa" → "KEPA" (Ö→E first). Then K before E →
        //   C, i += 1 (Ö consumed effectively). Wait — the rule is
        //   `K before front vowel → C` and advances by 1 (consuming
        //   only K). So the vowel is preserved for further encoding.
        //   After the rule: out = "C", i moved from 0 to 1. Then E at
        //   i=1: no digraph match. Push 'E'. Then 'P' push. Then 'A'
        //   push.
        //   So preprocessed string becomes "CEPA".
        //   Seed 'C' last=2. E reset. P code=1 push → "CP" → "C1" last=1.
        //   A reset. → "C1" pad → "C100".
        assert_eq!(p("köpa"), "C100");
    }

    #[test]
    fn ch_encodes_as_s() {
        // "choklad" — CH → S. O reset. K code=2. L code=4. A reset.
        //   D code=3.
        //   Seed 'S' last=7. O reset. K code=2 push → "SK" → "S2" last=2.
        //   L code=4 push → "S24" last=4. A reset. D code=3 push → "S243"
        //   last=3. → "S243".
        assert_eq!(p("choklad"), "S243");
    }

    #[test]
    fn common_swedish_surnames() {
        // Andersson: A(seed,last=0), N(5), D(3), E(reset), R(6),
        //   S(7), S(dup drop), O(reset), N(5)
        //   → A, 5, 3, 6 (cap 4). → "A536".
        assert_eq!(p("Andersson"), "A536");
        // Johansson: J(seed,last=2), O(reset), H(class 0 drop —
        //   preserves H as class-0 letter for reset), A(reset),
        //   N(5), S(7), S(dup drop), O(reset), N(5).
        //   Trace: seed 'J' last=2. O reset last=0. H class 0 reset
        //   last=0. A reset last=0. N code=5 push → "JN" → "J5" last=5.
        //   S code=7 push → "J57" last=7. S dup drop. O reset. N code=5
        //   push → "J575" last=5. → "J575".
        assert_eq!(p("Johansson"), "J575");
        // Karlsson: K(seed,last=2), A(reset), R(6), L(4), S(7), S(dup
        //   drop), O(reset), N(5).
        //   Seed 'K' last=2. A reset. R code=6 push → "KR" → "K6" last=6.
        //   L code=4 push → "K64" last=4. S code=7 push → "K647" cap 4
        //   → "K647".
        assert_eq!(p("Karlsson"), "K647");
        // Nilsson: N(seed,last=5), I(reset), L(4), S(7), S(dup drop),
        //   O(reset), N(5).
        //   Seed 'N' last=5. I reset. L code=4 push → "N4" last=4.
        //   S code=7 push → "N47" last=7. S dup drop. O reset. N code=5
        //   push → "N475" cap 4 → "N475".
        assert_eq!(p("Nilsson"), "N475");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("ANDERSSON"), p("andersson"));
        assert_eq!(p("HÅKAN"), p("håkan"));
    }

    #[test]
    fn short_input_pads_to_four() {
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Ö"), "E000");
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "Ebba" — E(seed), B(1,push,last=1), B(dup drop), A(reset)
        //   → "EB" → "E1" → "E100".
        assert_eq!(p("Ebba"), "E100");
    }

    #[test]
    fn adapter_returns_name_phonex_sv() {
        assert_eq!(SwedishPhonexAdapter.name(), "phonex-sv");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(SwedishPhonexAdapter.encode("").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = SwedishPhonexAdapter.encode("Andersson").unwrap();
        assert_eq!(primary, "A536");
        assert!(alt.is_none());
    }
}
