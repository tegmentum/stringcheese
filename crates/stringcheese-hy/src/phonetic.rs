//! Armenian → Latin fold → PHONEX-Armenian phonetic encoder.
//!
//! # Origin
//!
//! Armenian has no widely established Soundex / Métaphone-family
//! phonetic encoder in the way English or German do. What Armenian
//! *does* have is a well-known **transliteration** standard —
//! **Hübschmann-Meillet** (an academic Indo-Europeanist transliteration
//! for Classical / Old Armenian), together with the modern
//! **Reforma Armenian** romanization used for library cataloguing. Both
//! provide a one-letter-per-Armenian-scalar Latin mapping that this
//! module folds through before running a Soundex-shape 4-character
//! reduction.
//!
//! # Implementation choice — two-stage: transliteration then PHONEX
//!
//! Same pattern as the sibling `stringcheese-{th,ko,bn,hi,vi}` packs.
//! Internally the encoder:
//!
//! 1. **Lowercases and normalizes.** Rust's default `to_lowercase`
//!    fold handles Armenian's 39-letter case pairs correctly. The
//!    two-letter `եւ` spelling normalizes to the single-scalar `և`
//!    ligature (both encode identically).
//! 2. **Folds each Armenian scalar to a Hübschmann-Meillet-family
//!    Latin letter.** The full table is documented below. Vowels
//!    (`ա ե է ը ի ո օ ու`) fold to their base Latin vowel (dropped
//!    later by the Soundex step). Consonants collapse **across
//!    aspiration** to their base class:
//!
//!    * Labial stops `պ պ / փ / բ` all fold to `P` (`P` is class 1
//!      alongside `B`/`F`/`V`/`W` — the class captures the labial
//!      family).
//!    * Dental stops `տ / թ / դ` all fold to `T` (class 3 alongside
//!      `D`).
//!    * Velar stops `կ / ք / գ` all fold to `K` (class 2 alongside
//!      `C`/`G`/`Q`/`J`/`X`).
//!    * Dental affricates `ծ / ց / ձ` all fold to `C` (class 2 — the
//!      same family as the velars because Soundex does not have a
//!      dedicated affricate class).
//!    * Palato-alveolar affricates `ճ / չ / ջ` all fold to `J`
//!      (class 2 — matches the affricate group).
//!    * Velar fricatives `խ / ղ` both fold to `X` (class 7 alongside
//!      `S`/`Z`).
//!    * Sibilants `ս / շ / զ / ժ` fold to `S`/`Z` (both class 7).
//!    * Nasals `մ / ն` fold to `M`/`N` (both class 5).
//!    * Liquids `լ / ր / ռ` fold to `L`/`R` (classes 4 and 6).
//!
//!    Digraph `ու` (o + w) folds to `U` (a single vowel).
//!    Ligature `և` folds to `EV` (both its two components).
//! 3. **Runs a Soundex-shape 4-character reduction over the folded
//!    Latin.** The first Latin letter becomes the seed; subsequent
//!    letters classify by Soundex family, consecutive equal codes
//!    collapse, the result pads to 4 characters with `'0'`. This is
//!    the same PHONEX shape as the sibling packs
//!    (`stringcheese-{cs,da,es,et,fi,fr,hu,ko,nl,pl,pt,sk,sv,vi,bn,th}`).
//!
//! Adapter name: `"phonex-hy"`.
//!
//! # Byte-length caveat
//!
//! Every Armenian scalar (U+0530..=U+058F) is encoded as **two UTF-8
//! bytes** (this range falls entirely inside U+0080..=U+07FF, UTF-8's
//! 2-byte window). The encoder walks scalars via [`str::chars`],
//! never raw bytes, so it never risks slicing an Armenian scalar
//! apart.
//!
//! # The Armenian → Latin family map
//!
//! | Armenian | Family | Notes                                     |
//! |----------|--------|-------------------------------------------|
//! | `ա`      | A      | /ɑ/                                       |
//! | `բ`      | P      | /b/ Eastern; labial stop family           |
//! | `գ`      | K      | /g/; velar stop family                    |
//! | `դ`      | T      | /d/; dental stop family                   |
//! | `ե`      | E      | /jɛ/ initially, /ɛ/ elsewhere             |
//! | `զ`      | Z      | /z/                                       |
//! | `է`      | E      | /ɛ/                                       |
//! | `ը`      | E      | /ə/ schwa                                 |
//! | `թ`      | T      | /tʰ/; dental stop family                  |
//! | `ժ`      | Z      | /ʒ/; sibilant                             |
//! | `ի`      | I      | /i/                                       |
//! | `լ`      | L      | /l/                                       |
//! | `խ`      | X      | /x/; velar fricative                      |
//! | `ծ`      | C      | /ts/; dental affricate                    |
//! | `կ`      | K      | /k/; velar stop family                    |
//! | `հ`      | H      | /h/; silent under Soundex vowel step      |
//! | `ձ`      | C      | /dz/; dental affricate                    |
//! | `ղ`      | X      | /ɣ/; velar fricative                      |
//! | `ճ`      | J      | /tʃ/; palato-alveolar affricate           |
//! | `մ`      | M      | /m/                                       |
//! | `յ`      | Y      | /j/; palatal glide (vowel-like)           |
//! | `ն`      | N      | /n/                                       |
//! | `շ`      | S      | /ʃ/; sibilant                             |
//! | `ո`      | O      | /vɔ/ initially, /ɔ/ elsewhere             |
//! | `չ`      | J      | /tʃʰ/; palato-alveolar affricate          |
//! | `պ`      | P      | /p/; labial stop family                   |
//! | `ջ`      | J      | /dʒ/; palato-alveolar affricate           |
//! | `ռ`      | R      | /r/ trilled                               |
//! | `ս`      | S      | /s/                                       |
//! | `վ`      | V      | /v/                                       |
//! | `տ`      | T      | /t/; dental stop family                   |
//! | `ր`      | R      | /ɾ/ tapped                                |
//! | `ց`      | C      | /tsʰ/; dental affricate                   |
//! | `ւ`      | V      | classical /w/; folds to V for phonex      |
//! | `փ`      | P      | /pʰ/; labial stop family                  |
//! | `ք`      | K      | /kʰ/; velar stop family                   |
//! | `օ`      | O      | /ɔ/                                       |
//! | `ֆ`      | F      | /f/                                       |
//! | `և`      | EV     | ligature ech-yiwn — two-letter fold       |
//!
//! # Soundex-shape classification table
//!
//! Runs after the fold. Same table as the sibling PHONEX-family
//! encoders:
//!
//! | Class | Letters       |
//! |-------|---------------|
//! | 1     | B P F V W     |
//! | 2     | C K G Q J X   |
//! | 3     | D T           |
//! | 4     | L             |
//! | 5     | M N           |
//! | 6     | R             |
//! | 7     | S Z           |
//! | 0     | A E I O U Y H (dropped as vowels / silent) |
//!
//! # Non-goals
//!
//! - **Faithful transliteration** (ISO 9985, BGN/PCGN). Both
//!   standards produce readable romanizations with diacritics
//!   (`ë ə́ ǰ č̣`); PHONEX is a *phonetic key generator*, not a
//!   scholarly transliteration. Future
//!   `stringcheese-hy-{iso9985,bgnpcgn}` siblings could expose the
//!   full outputs as distinct public APIs.
//! - **Western Armenian phonology.** Western Armenian reads `բ` as
//!   /pʰ/ (matching Ancient / Classical), where Eastern Armenian
//!   reads it as /b/. This encoder targets Eastern; the aspiration
//!   fold means the two dialects still produce the same PHONEX key
//!   for the same word.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// Map an Armenian scalar to its PHONEX-family Latin fold, or `None`
/// for scalars outside the Armenian block.
///
/// Returns a `&'static str` because the ligature `և` folds to the
/// two-letter `EV` (its two component sounds).
///
/// See the [module-level table](self#the-armenian--latin-family-map)
/// for the full mapping. Input is assumed to already be lowercase
/// (the encoder's public entry point [`ArmenianPhonex::encode`]
/// handles the case fold).
///
/// The `match_same_arms` lint is suppressed because arms grouped by
/// consonant class (all labial stops → P, all dental stops → T, all
/// velars → K, all dental affricates → C, all palato-alveolar
/// affricates → J, all velar fricatives → X, etc.) deliberately map
/// to the same ASCII target — merging them would obscure the
/// phonological grouping, and the encoder's design intent is that
/// each Armenian consonant folds *individually* to its family letter.
#[must_use]
#[allow(clippy::match_same_arms)]
pub const fn armenian_to_latin(c: char) -> Option<&'static str> {
    Some(match c {
        // Vowels.
        'ա' => "A",
        'ե' => "E",
        'է' => "E",
        'ը' => "E",
        'ի' => "I",
        'ո' => "O",
        'օ' => "O",
        // Labial stops — Eastern /b/ /p/ /pʰ/ all fold to labial
        // family P (class 1 alongside B/F/V/W).
        'բ' => "P",
        'պ' => "P",
        'փ' => "P",
        // Dental stops — Eastern /d/ /t/ /tʰ/ all fold to dental
        // family T (class 3 alongside D).
        'դ' => "T",
        'տ' => "T",
        'թ' => "T",
        // Velar stops — Eastern /g/ /k/ /kʰ/ all fold to velar
        // family K (class 2 alongside C/G/Q/J/X).
        'գ' => "K",
        'կ' => "K",
        'ք' => "K",
        // Dental affricates — /dz/ /ts/ /tsʰ/ all fold to C.
        'ձ' => "C",
        'ծ' => "C",
        'ց' => "C",
        // Palato-alveolar affricates — /dʒ/ /tʃ/ /tʃʰ/ all fold to J.
        'ջ' => "J",
        'ճ' => "J",
        'չ' => "J",
        // Velar fricatives — /x/ /ɣ/ fold to X.
        'խ' => "X",
        'ղ' => "X",
        // Sibilants.
        'զ' => "Z",
        'ժ' => "Z",
        'ս' => "S",
        'շ' => "S",
        // Fricatives.
        'վ' => "V",
        'ֆ' => "F",
        'հ' => "H",
        // Nasals.
        'մ' => "M",
        'ն' => "N",
        // Liquids.
        'լ' => "L",
        'ր' => "R",
        'ռ' => "R",
        // Glide (classified as vowel in Soundex, but keep as Y so it
        // surfaces as a seed if word-initial).
        'յ' => "Y",
        // Classical /w/ — folds to V (Eastern reads this as /v/ in
        // most positions; it only survives as the second half of the
        // `ու` digraph, which is handled at the encoder level).
        'ւ' => "V",
        // Ligature ech-yiwn — /jɛv/. Fold to `EV` (its two
        // component sounds).
        'և' => "EV",
        _ => return None,
    })
}

/// The Armenian PHONEX encoder.
///
/// A zero-sized value; construct as [`ArmenianPhonex`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_hy::ArmenianPhonex;
///
/// // Երևան (Yerevan) — vowels drop under Soundex except the seed.
/// let key = ArmenianPhonex.encode("Երևան").unwrap();
/// // E seed, R(6), V(1), N(5) → "E615".
/// assert_eq!(key, "E615");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArmenianPhonex;

impl ArmenianPhonex {
    /// Encodes `word` per the PHONEX-Armenian algorithm.
    ///
    /// Returns `None` when `word` has no Armenian-letter content
    /// (empty input, pure whitespace, all punctuation, or pure
    /// non-Armenian text). Otherwise returns a 4-character key of
    /// the form `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let folded = fold_armenian_to_ascii(word);
        if folded.is_empty() {
            return None;
        }
        let bytes = folded.as_bytes();

        let mut out = String::with_capacity(4);
        out.push(bytes[0] as char);
        let mut last_code = code_of(bytes[0]);
        for &b in &bytes[1..] {
            let code = code_of(b);
            if code == b'0' {
                // Vowel / silent — reset the duplicate-collapse
                // state.
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

/// Fold `s` to uppercase-ASCII letters:
///
/// - Lowercase under Rust's default Unicode fold first.
/// - Normalize the `եւ → և` two-letter spelling.
/// - Rewrite the `ու → U` digraph (Armenian's /u/ vowel is written
///   as `o + w`).
/// - Armenian consonants and vowels map through
///   [`armenian_to_latin`].
/// - ASCII letters pass through (uppercased).
/// - Everything else (spaces, punctuation, digits, non-Armenian
///   letters) drops.
fn fold_armenian_to_ascii(s: &str) -> String {
    // Case-fold first, then normalize the two-letter `եւ` spelling
    // to the single-scalar ligature `և` so both spellings encode
    // identically.
    let lowered: String = s.chars().flat_map(char::to_lowercase).collect();
    let normalized = lowered.replace("եւ", "և");

    // Walk the char stream so we can look ahead one scalar for the
    // `ու` digraph (Armenian writes /u/ as `o + w`).
    let chars: alloc::vec::Vec<char> = normalized.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // `ու` digraph: fold to `U`.
        if c == 'ո' && i + 1 < chars.len() && chars[i + 1] == 'ւ' {
            out.push('U');
            i += 2;
            continue;
        }
        if let Some(ascii) = armenian_to_latin(c) {
            out.push_str(ascii);
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_uppercase());
        }
        i += 1;
    }
    out
}

/// Soundex-family digit for byte `b` (an ASCII uppercase letter).
///
/// See the [module-level docs](self) for the full classification
/// table.
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

/// Adapter that exposes [`ArmenianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Armenian::phonetic_encoder`](crate::Armenian) hands back.
///
/// Returns `Some((key, None))` for input with at least one Armenian
/// letter; returns `None` for input with no Armenian content (empty,
/// all whitespace, all punctuation, or pure non-Armenian text).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArmenianPhonexAdapter;

impl LanguagePhoneticEncoder for ArmenianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_armenian(word) {
            return None;
        }
        let key = ArmenianPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-hy"
    }
}

/// Does `s` contain at least one Armenian-script scalar
/// (U+0530..=U+058F)?
fn contains_armenian(s: &str) -> bool {
    s.chars().any(|c| ('\u{0530}'..='\u{058F}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        ArmenianPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Consonant family lookups.
    // -------------------------------------------------------------

    #[test]
    fn labial_stops_all_fold_to_p() {
        for c in ['բ', 'պ', 'փ'] {
            assert_eq!(armenian_to_latin(c), Some("P"), "{c:?} should fold to P");
        }
    }

    #[test]
    fn dental_stops_all_fold_to_t() {
        for c in ['դ', 'տ', 'թ'] {
            assert_eq!(armenian_to_latin(c), Some("T"), "{c:?} should fold to T");
        }
    }

    #[test]
    fn velar_stops_all_fold_to_k() {
        for c in ['գ', 'կ', 'ք'] {
            assert_eq!(armenian_to_latin(c), Some("K"), "{c:?} should fold to K");
        }
    }

    #[test]
    fn dental_affricates_all_fold_to_c() {
        for c in ['ձ', 'ծ', 'ց'] {
            assert_eq!(armenian_to_latin(c), Some("C"), "{c:?} should fold to C");
        }
    }

    #[test]
    fn palato_alveolar_affricates_all_fold_to_j() {
        for c in ['ջ', 'ճ', 'չ'] {
            assert_eq!(armenian_to_latin(c), Some("J"), "{c:?} should fold to J");
        }
    }

    #[test]
    fn velar_fricatives_fold_to_x() {
        for c in ['խ', 'ղ'] {
            assert_eq!(armenian_to_latin(c), Some("X"), "{c:?} should fold to X");
        }
    }

    #[test]
    fn ligature_folds_to_ev() {
        assert_eq!(armenian_to_latin('և'), Some("EV"));
    }

    #[test]
    fn non_armenian_returns_none() {
        assert!(armenian_to_latin('a').is_none());
        assert!(armenian_to_latin(' ').is_none());
        assert!(armenian_to_latin('!').is_none());
    }

    // -------------------------------------------------------------
    // Encoder behaviour.
    // -------------------------------------------------------------

    #[test]
    fn empty_input_returns_none() {
        assert!(ArmenianPhonex.encode("").is_none());
        assert!(ArmenianPhonex.encode("   ").is_none());
    }

    #[test]
    fn bare_consonant_encodes_to_seed_plus_zeros() {
        // բ → P → "P000".
        assert_eq!(p("բ"), "P000");
        // ս → S → "S000".
        assert_eq!(p("ս"), "S000");
    }

    #[test]
    fn aspiration_fold_produces_same_key() {
        // բ / պ / փ all fold to P — same key.
        assert_eq!(p("բ"), p("պ"));
        assert_eq!(p("պ"), p("փ"));
        // դ / տ / թ all fold to T.
        assert_eq!(p("դ"), p("տ"));
        assert_eq!(p("տ"), p("թ"));
        // գ / կ / ք all fold to K.
        assert_eq!(p("գ"), p("կ"));
        assert_eq!(p("կ"), p("ք"));
    }

    #[test]
    fn ou_digraph_folds_to_u() {
        // `ու` → U (single vowel). Bare digraph encodes to "U000".
        assert_eq!(p("ու"), "U000");
    }

    #[test]
    fn yerevan_encodes_correctly() {
        // Երևան = ե + ր + և + ա + ն.
        // Fold: E R EV A N = E R E V A N.
        // E seed. R(6). E reset. V(1). A reset. N(5). → "E615".
        assert_eq!(p("Երևան"), "E615");
    }

    #[test]
    fn hayastan_encodes_correctly() {
        // Հայաստան = հ + ա + յ + ա + ս + տ + ա + ն.
        // Fold: H A Y A S T A N.
        // H seed. A reset. Y is class 0 too (vowel-like). S(7).
        // T(3). A reset. N(5). → "H735".
        assert_eq!(p("Հայաստան"), "H735");
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_hy() {
        assert_eq!(ArmenianPhonexAdapter.name(), "phonex-hy");
    }

    #[test]
    fn adapter_returns_some_for_armenian() {
        let out = ArmenianPhonexAdapter.encode("Երևան");
        assert!(out.is_some(), "expected Some for Armenian input");
        let (primary, alt) = out.unwrap();
        assert!(!primary.is_empty());
        assert!(alt.is_none());
    }

    #[test]
    fn adapter_returns_none_for_no_armenian() {
        assert!(ArmenianPhonexAdapter.encode("").is_none());
        assert!(ArmenianPhonexAdapter.encode("hello").is_none());
        assert!(ArmenianPhonexAdapter.encode("123").is_none());
    }
}
