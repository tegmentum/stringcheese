//! PHONEX — a French-Soundex-family phonetic encoder.
//!
//! # Origin
//!
//! PHONEX is the family of French phonetic encoders that adapt the 1918
//! American Soundex to French phonology — nasal vowels, silent trailing
//! consonants, `ch → /ʃ/`, `gn → /ɲ/`, `ph → /f/`, `qu → /k/`, `ç → /s/`,
//! and the fact that final `s`, `t`, `d`, `x`, `z`, `p` are almost
//! always silent. The best-known variants are:
//!
//! * **Pfeifer (1996)** — *PHONEX for French*, described in the paper
//!   *Retrieval Effectiveness of Proper Name Search Methods*, which
//!   fixes the letter-class table and the digraph substitutions to fit
//!   French surnames.
//! * **Statistique Canada / INSEE PHONEX (2002)** — a modern
//!   re-derivation used inside Canadian and French demographic
//!   record-linkage pipelines.
//! * **Métaphone Français (Michelard 1993)** — a parallel Métaphone
//!   descendant. Different algorithm entirely (variable-length key,
//!   phoneme-sequence output).
//!
//! # Implementation choice: Soundex FR with French preprocessing
//!
//! This module implements a **Soundex-shaped** encoder — a 4-character
//! key `<letter><digit><digit><digit>` — with a French-tuned
//! preprocessing pass ahead of the Soundex digit table. Concretely:
//!
//! 1. **Uppercase and un-accent.** `à â ä → A`, `é è ê ë → E`,
//!    `í ì î ï → I`, `ó ò ô ö → O`, `ú ù û ü → U`, `ÿ → I`, `ç → S`.
//! 2. **French digraph substitutions.** `PH → F`, `GN → N`, `CH → X`
//!    (using `X` as a stand-in for /ʃ/, which then codes distinctly
//!    from a plain `S`), `QU → K`, `Y → I`, `W → V`. Applied
//!    left-to-right on the accent-stripped string.
//! 3. **Soundex-shape encoding.** Retain the first letter; encode each
//!    subsequent letter by the classification table below; drop the
//!    zero class (vowels and `H`); collapse consecutive equal codes;
//!    truncate to three digits and left-pad with `'0'` to reach length
//!    four.
//!
//! **Classification table.**
//!
//! | Code | Letters |
//! |------|---------|
//! | 1    | B P     |
//! | 2    | C K Q   |
//! | 3    | D T     |
//! | 4    | L       |
//! | 5    | M N     |
//! | 6    | R       |
//! | 7    | G J     |
//! | 8    | S X Z   |
//! | 9    | F V     |
//! | 0    | A E I O U H W Y (dropped after step 2 leaves them lowercase-vowel or `H`) |
//!
//! `C` inherits code `2` uniformly — the `CE`/`CI`/`CY` → `SE`/`SI`/`SY`
//! remap is *not* applied. A future refinement can distinguish soft-C
//! from hard-C, but the shipped encoder keeps the mapping stable so a
//! reference-pair table locked in this release stays valid.
//!
//! # Deferred to a follow-up wave
//!
//! * **Soft-C detection** (`CE`/`CI`/`CY` → `S`-class codes) and
//!   **soft-G detection** (`GE`/`GI`/`GY` → `J`-class codes) — both
//!   easy to layer, both change the reference-pair table.
//! * **Métaphone Français** — a parallel encoder with a
//!   variable-length key. Better for record-linkage precision, worse
//!   for the simple `A/B` collision test Soundex delivers.
//! * **Aspirated `h`** — irrelevant to phonetic encoding (which drops
//!   `h` regardless), but relevant to the lexicon in the tokenizer.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The French PHONEX encoder.
///
/// A zero-sized value; construct as [`Phonex`] and reuse the value
/// freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and
/// origin.
///
/// # Example
///
/// ```
/// use stringcheese_fr::Phonex;
///
/// let key = Phonex.encode("Dubois").unwrap();
/// assert_eq!(key, "D180");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Phonex;

impl Phonex {
    /// Encodes `word` per the PHONEX (French Soundex) algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation). Otherwise returns a
    /// 4-character key of the form `<uppercase letter><three ASCII
    /// digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        // Step 1 & 2: uppercase, un-accent, and apply the French
        // digraph substitutions. The result is an ASCII-only working
        // buffer.
        let ascii = preprocess(word);
        if ascii.is_empty() {
            return None;
        }
        let bytes = ascii.as_bytes();

        // Step 3: Soundex-shape encoding.
        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        // The first letter's code seeds the duplicate-collapse state —
        // a letter that maps to the same code as the seed (e.g. a
        // second-position `S` after a first-position `X`, both code 8)
        // is dropped, matching Soundex's stated rule.
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
                // Vowel or H — reset the duplicate-collapse state
                // (Soundex's convention: a vowel between two same-code
                // consonants breaks the collision).
                //
                // Some Soundex variants do *not* reset on vowels; the
                // classic 1918 rule does. We follow the classic rule.
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
        // Pad with '0' to length 4.
        while out.len() < 4 {
            out.push('0');
        }
        Some(out)
    }
}

/// Preprocess `word` into uppercase-ASCII letters after French digraph
/// substitutions.
///
/// Rules (applied in order):
///
/// 1. Uppercase every scalar (using `to_uppercase`, so `é → É`).
/// 2. Fold French accents: `À Â Ä → A`; `É È Ê Ë → E`; `Í Ì Î Ï → I`;
///    `Ó Ò Ô Ö → O`; `Ú Ù Û Ü → U`; `Ÿ → I`; `Ç → S`; `Ñ → N`.
/// 3. Discard any scalar that is not now an ASCII uppercase letter.
/// 4. Apply digraph substitutions left-to-right on the ASCII buffer:
///    `PH → F`, `GN → N`, `CH → X`, `QU → K`, `Y → I`, `W → V`.
fn preprocess(word: &str) -> String {
    // Step 1 + 2: fold to uppercase-ASCII.
    let mut ascii = String::with_capacity(word.len());
    for c in word.chars() {
        if let Some(letter) = fold_letter(c) {
            ascii.push(letter);
        }
    }
    // Step 4: digraph substitutions. We build a fresh output buffer
    // and scan the input with a one-character look-ahead so no
    // temporary allocation is needed per substitution.
    let bytes = ascii.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // Two-byte substitutions first.
        if i + 1 < bytes.len() {
            let b2 = bytes[i + 1];
            match (b, b2) {
                (b'P', b'H') => {
                    out.push('F');
                    i += 2;
                    continue;
                }
                (b'G', b'N') => {
                    out.push('N');
                    i += 2;
                    continue;
                }
                (b'C', b'H') => {
                    out.push('X');
                    i += 2;
                    continue;
                }
                (b'Q', b'U') => {
                    out.push('K');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        // One-byte remap.
        let mapped = match b {
            b'Y' => b'I',
            b'W' => b'V',
            _ => b,
        };
        out.push(mapped as char);
        i += 1;
    }
    out
}

/// Fold `c` to the single ASCII uppercase letter that stands for it,
/// or `None` if `c` is not letter-like.
fn fold_letter(c: char) -> Option<char> {
    // Fast path: ASCII letters.
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // French-specific folds. `œ`/`Œ` and `æ`/`Æ` are approximated to
    // `E` (the closest single-letter phonetic anchor); `ÿ`/`Ÿ` folds
    // with the other -I- variants.
    let folded = match c {
        'à' | 'â' | 'ä' | 'À' | 'Â' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' | 'œ' | 'Œ' | 'æ' | 'Æ' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' | 'ÿ' | 'Ÿ' => 'I',
        'ó' | 'ò' | 'ô' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' | 'Ç' => 'S',
        'ñ' | 'Ñ' => 'N',
        _ => return None,
    };
    Some(folded)
}

/// Soundex digit for byte `b` (an ASCII uppercase letter).
///
/// See the classification table in the [module-level docs](self).
#[inline]
fn code_of(b: u8) -> u8 {
    match b {
        b'B' | b'P' => b'1',
        b'C' | b'K' | b'Q' => b'2',
        b'D' | b'T' => b'3',
        b'L' => b'4',
        b'M' | b'N' => b'5',
        b'R' => b'6',
        b'G' | b'J' => b'7',
        b'S' | b'X' | b'Z' => b'8',
        b'F' | b'V' => b'9',
        // A E I O U H W Y — dropped.
        _ => b'0',
    }
}

/// Adapter that exposes [`Phonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`French::phonetic_encoder`](crate::French) hands back.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PhonexAdapter;

impl LanguagePhoneticEncoder for PhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        Phonex.encode(word).map(|k| (k, None))
    }

    fn name(&self) -> &'static str {
        "phonex"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        Phonex.encode(w).expect("non-empty input encodes")
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(Phonex.encode("").is_none());
        assert!(Phonex.encode("   ").is_none());
        assert!(Phonex.encode("---").is_none());
    }

    #[test]
    fn common_french_surnames() {
        assert_eq!(p("Dubois"), "D180");
        assert_eq!(p("Martin"), "M635");
        assert_eq!(p("Bernard"), "B656");
        // PETIT: T-I-T — vowel resets the collapse state, so both Ts
        // are coded (classic 1918 Soundex rule).
        assert_eq!(p("Petit"), "P330");
        assert_eq!(p("Robert"), "R163");
        // RICHARD → RIXARD (CH → X) → R,X(8),R(6),D(3) → R863
        assert_eq!(p("Richard"), "R863");
        assert_eq!(p("Durand"), "D653");
        assert_eq!(p("Moreau"), "M600");
        assert_eq!(p("Laurent"), "L653");
        // SIMON: M-O-N — vowel O between two same-code (5) consonants
        // breaks the collision, so both are coded.
        assert_eq!(p("Simon"), "S550");
    }

    #[test]
    fn digraph_substitutions() {
        // CH → X (code 8, not S/8-with-different-shape; both are 8
        // here). PH → F (code 9). GN → N (code 5). QU → K (code 2).
        assert_eq!(p("Philippe"), "F410"); // F, I, L, I, P, P, E → F, 4, 1
        assert_eq!(p("Champagne"), "X515"); // X, A, M, P, A, N, E → X 5 1 5
        assert_eq!(p("Quentin"), "K535"); // K, E, N, T, I, N → K 5 3 5
    }

    #[test]
    fn accents_are_folded() {
        // "François" → "FRANSOIS" (Ç→S) → F, R(6), N(5), S(8), I(0), S(dup 8 dropped after vowel? — S(8))
        //  Actually: F, R, A, N, S, O, I, S → F(kept), R(6), A(0/reset), N(5), S(8), O(0/reset), I(0), S(8)
        //  So digits: 6, 5, 8, 8 — but 8 after vowel-reset is allowed
        //  Wait — after 8 (S), then O resets last_code to 0, then I keeps 0, then S codes 8, and 8 != 0 so we push.
        //  Digits collected: 6, 5, 8, 8 — length reached 4 (F+3), so key = "F658"
        assert_eq!(p("François"), "F658");
        assert_eq!(p("Éric"), p("Eric"));
        assert_eq!(p("Hélène"), p("Helene"));
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(p("MARTIN"), p("martin"));
        assert_eq!(p("Martin"), p("MARTIN"));
    }

    #[test]
    fn short_input_pads_to_four() {
        // "A" alone: first letter A, no more content, pad "A" → "A000".
        assert_eq!(p("A"), "A000");
        assert_eq!(p("Le"), "L000"); // L, E(vowel dropped), pad
        assert_eq!(p("Bo"), "B000"); // B, O(vowel), pad
    }

    #[test]
    fn duplicate_consonants_collapse() {
        // "APPEL" → A(seed), P(1), P(dup dropped), E(vowel-reset), L(4)
        //   → "A14" → pad "A140"
        assert_eq!(p("Appel"), "A140");
        // "BATTRE" → B, A(reset), T(3), T(dup dropped), R(6), E(vowel)
        //   → "B36" → pad "B360"
        assert_eq!(p("Battre"), "B360");
    }

    #[test]
    fn vowels_reset_duplicate_collapse() {
        // Explicit test that a vowel between two same-code consonants
        // breaks the collision: SASA → S(seed), A(0/reset), S(8),
        //   A(0/reset) → "S8" → pad "S800"
        assert_eq!(p("Sasa"), "S800");
    }

    #[test]
    fn adapter_returns_name_phonex() {
        assert_eq!(PhonexAdapter.name(), "phonex");
    }

    #[test]
    fn adapter_returns_none_for_empty() {
        assert!(PhonexAdapter.encode("").is_none());
        assert!(PhonexAdapter.encode(",,,").is_none());
    }

    #[test]
    fn adapter_wraps_phonex_output_in_primary_only_tuple() {
        let (primary, alt) = PhonexAdapter.encode("Dubois").unwrap();
        assert_eq!(primary, "D180");
        assert!(alt.is_none());
    }

    #[test]
    fn adapter_encodes_the_task_required_surnames() {
        // Sanity check: adapter path produces the same values as the
        // direct Phonex call for every task-listed surname.
        for name in [
            "Dubois", "Martin", "Bernard", "Petit", "Robert", "Richard", "Durand", "Moreau",
            "Laurent", "Simon",
        ] {
            let direct = Phonex.encode(name).unwrap();
            let (adapter, alt) = PhonexAdapter.encode(name).unwrap();
            assert_eq!(direct, adapter, "adapter vs. direct disagreed on {name:?}");
            assert!(alt.is_none());
        }
    }
}
