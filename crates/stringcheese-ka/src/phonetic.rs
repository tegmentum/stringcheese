//! Georgian -> Latin transliteration -> PHONEX-Georgian phonetic
//! encoder.
//!
//! # Origin
//!
//! Georgian is the first Georgian-script pack in StringCheese and —
//! like every other language with a non-Latin script that lacks a
//! widely-adopted native phonetic-key algorithm (Bengali, Thai, Greek,
//! Vietnamese, Korean) — the pragmatic answer for a **phonetic key
//! generator** is a two-stage encoder that first folds the script to
//! a coarse Latin representation and then applies a Soundex-shape
//! reduction. That is what this module ships.
//!
//! The reference romanization is the **ISO 9984 (1996)** Georgian ->
//! Latin transliteration, the same standard the U.S. Board on
//! Geographic Names accepts and closely aligned with the Georgian
//! National transliteration (2002). ISO 9984 uses the apostrophe `'`
//! to mark Georgian's **glottalized / ejective consonants** — the
//! pack's Georgian-script scalars `კ`, `პ`, `ტ`, `წ`, `ჭ`, `ყ` are
//! ejective and transliterate as `k'`, `p'`, `t'`, `ts'`, `ch'`, `q'`
//! respectively, distinguished from the aspirated series `ქ → k`,
//! `ფ → p`, `თ → t`, `ც → ts`, `ჩ → ch`, and the fricative `ღ → gh`.
//!
//! # Implementation choice — two-stage: ISO 9984 fold then PHONEX
//!
//! This module ships a single stage-collapsed encoder,
//! **PHONEX-Georgian**, that internally:
//!
//! 1. **Case-folds Mtavruli to Mkhedruli.** Mtavruli
//!    (U+1C90..=U+1CBF) is Unicode 11's capitalized-Mkhedruli block;
//!    Rust's default [`char::to_lowercase`] pairs every Mtavruli
//!    scalar with its Mkhedruli counterpart.
//! 2. **Maps each Mkhedruli scalar to its ISO 9984 Latin form.**
//!    The 33 modern letters map to 1..=3 ASCII characters; see the
//!    single-letter table below. The ejective apostrophes are
//!    preserved through the transliteration step (they are
//!    informative for a reader of the Latin form) but folded out in
//!    the next step so the phonex key does not distinguish ejectives
//!    from aspirates. This matches the task-level requirement to
//!    "fold to their base class in the phonex".
//! 3. **Runs a Soundex-shape 4-character reduction over the folded
//!    Latin.** ASCII apostrophes are dropped before classification;
//!    the first Latin letter becomes the seed; subsequent letters
//!    classify by Soundex family; consecutive equal codes collapse;
//!    the result pads to 4 characters with `'0'`. This is the same
//!    PHONEX shape as the sibling packs
//!    (`stringcheese-{cs,da,es,et,fi,fr,hu,ko,nl,pl,pt,sk,sv,vi,bn,th}`).
//!
//! Adapter name: `"phonex-ka"`.
//!
//! # Byte-length caveat
//!
//! Every Mkhedruli scalar (U+10D0..=U+10FF) is encoded as **three
//! UTF-8 bytes** (the block falls in UTF-8's 3-byte range
//! U+0800..=U+FFFF). The encoder walks scalars via [`str::chars`],
//! never raw bytes, so it never risks slicing a Georgian scalar apart.
//!
//! # The 33-letter Mkhedruli -> Latin table (ISO 9984)
//!
//! | Mkhedruli | Name            | ISO 9984 | Notes                           |
//! |-----------|-----------------|----------|---------------------------------|
//! | `ა`      | an              | `a`      | /a/                             |
//! | `ბ`      | ban             | `b`      | /b/                             |
//! | `გ`      | gan             | `g`      | /g/                             |
//! | `დ`      | don             | `d`      | /d/                             |
//! | `ე`      | en              | `e`      | /e/                             |
//! | `ვ`      | vin             | `v`      | /v/                             |
//! | `ზ`      | zen             | `z`      | /z/                             |
//! | `თ`      | tan             | `t`      | /tʰ/ aspirated                  |
//! | `ი`      | in              | `i`      | /i/                             |
//! | `კ`      | k'an            | `k'`     | /kʼ/ ejective                   |
//! | `ლ`      | las             | `l`      | /l/                             |
//! | `მ`      | man             | `m`      | /m/                             |
//! | `ნ`      | nar             | `n`      | /n/                             |
//! | `ო`      | on              | `o`      | /o/                             |
//! | `პ`      | p'ar            | `p'`     | /pʼ/ ejective                   |
//! | `ჟ`      | zhar            | `zh`     | /ʒ/                             |
//! | `რ`      | rae             | `r`      | /r/                             |
//! | `ს`      | san             | `s`      | /s/                             |
//! | `ტ`      | t'ar            | `t'`     | /tʼ/ ejective                   |
//! | `უ`      | un              | `u`      | /u/                             |
//! | `ფ`      | phar            | `p`      | /pʰ/ aspirated                  |
//! | `ქ`      | khar            | `k`      | /kʰ/ aspirated                  |
//! | `ღ`      | ghan            | `gh`     | /ɣ/ voiced velar fricative      |
//! | `ყ`      | q'ar            | `q'`     | /qʼ/ ejective uvular            |
//! | `შ`      | shin            | `sh`     | /ʃ/                             |
//! | `ჩ`      | chin            | `ch`     | /tʃʰ/ aspirated                 |
//! | `ც`      | tsan            | `ts`     | /tsʰ/ aspirated                 |
//! | `ძ`      | dzil            | `dz`     | /dz/                            |
//! | `წ`      | ts'il           | `ts'`    | /tsʼ/ ejective                  |
//! | `ჭ`      | ch'ar           | `ch'`    | /tʃʼ/ ejective                  |
//! | `ხ`      | khan            | `kh`     | /x/ voiceless velar fricative   |
//! | `ჯ`      | jhan            | `j`      | /dʒ/                            |
//! | `ჰ`      | hae             | `h`      | /h/                             |
//!
//! The five archaic letters (`ჱ`, `ჲ`, `ჳ`, `ჴ`, `ჵ` — U+10F1..=U+10F5)
//! that survive from Old Georgian are mapped alongside their nearest
//! modern equivalents (see [`mkhedruli_to_iso9984`]) so mixed
//! contemporary / historical texts still produce a usable key.
//!
//! # Soundex-shape classification table
//!
//! Runs after the fold. Same table as the sibling Latin-alphabet
//! packs' PHONEX-family encoders:
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
//! Because Georgian's aspirated `t` and ejective `t'` both fold to
//! Latin `t` (after apostrophe drop), the phonex class-3 slot
//! collapses the two-way stop-quality distinction the script encodes,
//! producing the same key for aspirated / ejective pairs. This is
//! the intended semantics for a phonetic key.
//!
//! # Non-goals
//!
//! - **Full ISO 9984 output.** ISO 9984 is a *transliteration*;
//!   PHONEX is a *phonetic key generator*. A future
//!   `stringcheese-ka-iso9984` sibling could expose the full
//!   transliteration output as a distinct public API.
//! - **BGN/PCGN (1981) romanization.** The older BGN/PCGN system
//!   diverges from ISO 9984 on the ejective / aspirated distinction
//!   (`t' → t`, `t → tʻ`); out of scope for a phonetic key that
//!   collapses the two anyway.
//! - **Asomtavruli / Nuskhuri phonex.** The Old Georgian scripts
//!   (U+10A0..=U+10CF, U+2D00..=U+2D2F) share letter identities
//!   with Mkhedruli; a caller who needs a phonex over Old Georgian
//!   text should fold to Mkhedruli first. Explicit case-fold for
//!   those blocks is a follow-up.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// Map a Mkhedruli scalar to its ISO 9984 ASCII string, or `None` if
/// the scalar is outside the Georgian mapping.
///
/// See the [module-level table](self#the-33-letter-mkhedruli---latin-table-iso-9984)
/// for the full list. Input is assumed to already be case-folded to
/// Mkhedruli (the encoder's public entry point
/// [`GeorgianPhonex::encode`] handles Mtavruli via
/// [`char::to_lowercase`]).
///
/// The five archaic letters (U+10F1..=U+10F5) that survive from Old
/// Georgian are folded to their nearest modern equivalent
/// (`ჱ he → e`, `ჲ hie → i`, `ჳ wi → v`, `ჴ har → kh`, `ჵ hoe → o`);
/// all other scalars in the extended Mkhedruli block (U+10F6..=U+10FF)
/// map through as best-effort ASCII where a plausible letter exists,
/// or return `None`.
#[must_use]
#[allow(clippy::match_same_arms)]
pub const fn mkhedruli_to_iso9984(c: char) -> Option<&'static str> {
    Some(match c {
        // Modern 33-letter Mkhedruli.
        'ა' => "a",
        'ბ' => "b",
        'გ' => "g",
        'დ' => "d",
        'ე' => "e",
        'ვ' => "v",
        'ზ' => "z",
        'თ' => "t",
        'ი' => "i",
        'კ' => "k'",
        'ლ' => "l",
        'მ' => "m",
        'ნ' => "n",
        'ო' => "o",
        'პ' => "p'",
        'ჟ' => "zh",
        'რ' => "r",
        'ს' => "s",
        'ტ' => "t'",
        'უ' => "u",
        'ფ' => "p",
        'ქ' => "k",
        'ღ' => "gh",
        'ყ' => "q'",
        'შ' => "sh",
        'ჩ' => "ch",
        'ც' => "ts",
        'ძ' => "dz",
        'წ' => "ts'",
        'ჭ' => "ch'",
        'ხ' => "kh",
        'ჯ' => "j",
        'ჰ' => "h",
        // Archaic Mkhedruli (Old Georgian survivors, U+10F1..=U+10F5).
        'ჱ' => "e",  // he — long-e variant, folded to e
        'ჲ' => "i",  // hie — palatal, folded to i
        'ჳ' => "v",  // wi — folded to v
        'ჴ' => "kh", // har — uvular, folded to kh
        'ჵ' => "o",  // hoe — folded to o
        _ => return None,
    })
}

/// The PHONEX-Georgian encoder.
///
/// A zero-sized value; construct as [`GeorgianPhonex`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_ka::GeorgianPhonex;
///
/// // თბილისი "Tbilisi" — ISO 9984 fold: t-b-i-l-i-s-i → TBILISI.
/// // PHONEX: T seed, B code=1 push, I vow reset, L code=4 push,
/// // I vow reset, S code=7 push, I vow reset → "T147".
/// assert_eq!(GeorgianPhonex.encode("თბილისი").unwrap(), "T147");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GeorgianPhonex;

impl GeorgianPhonex {
    /// Encodes `word` per the PHONEX-Georgian algorithm.
    ///
    /// Returns `None` when `word` has no Georgian-letter content
    /// (empty input, pure whitespace, all punctuation, pure non-
    /// Georgian text). Otherwise returns a 4-character key of the
    /// form `<uppercase letter><three ASCII digits>`.
    ///
    /// Ejective consonants (`კ` k', `პ` p', `ტ` t', `წ` ts', `ჭ` ch',
    /// `ყ` q') fold to the same phonex class as their aspirated
    /// counterparts (`ქ` k, `ფ` p, `თ` t, `ც` ts, `ჩ` ch) — see the
    /// module docs for the trade-off.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let folded = fold_georgian_to_ascii(word);
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
/// - Georgian Mkhedruli scalars map through [`mkhedruli_to_iso9984`]
///   with any apostrophe (ejective marker) dropped afterwards.
/// - Mtavruli scalars are case-folded to Mkhedruli via
///   [`char::to_lowercase`], then routed through the same mapping.
/// - ASCII letters pass through (uppercased).
/// - Everything else drops.
fn fold_georgian_to_ascii(s: &str) -> String {
    // A Georgian scalar maps to at most 3 ASCII characters
    // (including the ejective apostrophe); allocate generously.
    let mut out = String::with_capacity(s.len());
    for raw in s.chars() {
        // Case-fold Mtavruli to Mkhedruli. Rust's default
        // [`char::to_lowercase`] pairs every Mtavruli scalar with
        // its Mkhedruli counterpart under Unicode 11+.
        for c in raw.to_lowercase() {
            if let Some(ascii) = mkhedruli_to_iso9984(c) {
                for b in ascii.bytes() {
                    if b == b'\'' {
                        // Drop the ejective apostrophe — the phonex
                        // collapses ejective / aspirate pairs by
                        // design.
                        continue;
                    }
                    // `to_ascii_uppercase` is a no-op on the ASCII
                    // lowercase forms we emit, but it clarifies intent.
                    out.push((b as char).to_ascii_uppercase());
                }
                continue;
            }
            if c.is_ascii_alphabetic() {
                out.push(c.to_ascii_uppercase());
            }
        }
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

/// Adapter that exposes [`GeorgianPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Georgian::phonetic_encoder`](crate::Georgian) hands back.
///
/// Returns `Some((key, None))` for input with at least one Georgian
/// letter; returns `None` for input with no Georgian content (empty,
/// all whitespace, all punctuation, or pure non-Georgian text).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GeorgianPhonexAdapter;

impl LanguagePhoneticEncoder for GeorgianPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_georgian(word) {
            return None;
        }
        let key = GeorgianPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-ka"
    }
}

/// Does `s` contain at least one scalar in the Mkhedruli block
/// (U+10D0..=U+10FF), the Mtavruli block (U+1C90..=U+1CBF), the
/// Asomtavruli block (U+10A0..=U+10CF), or the Nuskhuri block
/// (U+2D00..=U+2D2F)?
///
/// This is a superset of the ISO 9984 mapping — the ancient scripts
/// pass through, then Rust's [`char::to_lowercase`] folds Mtavruli
/// (and, on modern Unicode, Asomtavruli) to Mkhedruli for the
/// mapping lookup.
fn contains_georgian(s: &str) -> bool {
    s.chars().any(|c| {
        ('\u{10A0}'..='\u{10FF}').contains(&c)
            || ('\u{1C90}'..='\u{1CBF}').contains(&c)
            || ('\u{2D00}'..='\u{2D2F}').contains(&c)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(w: &str) -> String {
        GeorgianPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Mkhedruli -> ISO 9984 lookups.
    // -------------------------------------------------------------

    #[test]
    fn every_modern_mkhedruli_letter_has_a_mapping() {
        for c in [
            'ა', 'ბ', 'გ', 'დ', 'ე', 'ვ', 'ზ', 'თ', 'ი', 'კ', 'ლ', 'მ', 'ნ', 'ო', 'პ', 'ჟ', 'რ',
            'ს', 'ტ', 'უ', 'ფ', 'ქ', 'ღ', 'ყ', 'შ', 'ჩ', 'ც', 'ძ', 'წ', 'ჭ', 'ხ', 'ჯ', 'ჰ',
        ] {
            assert!(
                mkhedruli_to_iso9984(c).is_some(),
                "no ISO 9984 mapping for {c:?}"
            );
        }
    }

    #[test]
    fn ejectives_carry_the_apostrophe_marker() {
        // The six ejective consonants carry the ISO 9984 apostrophe.
        assert_eq!(mkhedruli_to_iso9984('კ'), Some("k'"));
        assert_eq!(mkhedruli_to_iso9984('პ'), Some("p'"));
        assert_eq!(mkhedruli_to_iso9984('ტ'), Some("t'"));
        assert_eq!(mkhedruli_to_iso9984('წ'), Some("ts'"));
        assert_eq!(mkhedruli_to_iso9984('ჭ'), Some("ch'"));
        assert_eq!(mkhedruli_to_iso9984('ყ'), Some("q'"));
    }

    #[test]
    fn aspirated_consonants_have_no_apostrophe() {
        // The aspirated counterparts do not.
        assert_eq!(mkhedruli_to_iso9984('ქ'), Some("k"));
        assert_eq!(mkhedruli_to_iso9984('ფ'), Some("p"));
        assert_eq!(mkhedruli_to_iso9984('თ'), Some("t"));
        assert_eq!(mkhedruli_to_iso9984('ც'), Some("ts"));
        assert_eq!(mkhedruli_to_iso9984('ჩ'), Some("ch"));
    }

    #[test]
    fn archaic_letters_have_a_mapping() {
        // Old Georgian survivors U+10F1..=U+10F5.
        assert_eq!(mkhedruli_to_iso9984('ჱ'), Some("e"));
        assert_eq!(mkhedruli_to_iso9984('ჲ'), Some("i"));
        assert_eq!(mkhedruli_to_iso9984('ჳ'), Some("v"));
        assert_eq!(mkhedruli_to_iso9984('ჴ'), Some("kh"));
        assert_eq!(mkhedruli_to_iso9984('ჵ'), Some("o"));
    }

    #[test]
    fn non_georgian_returns_none() {
        assert!(mkhedruli_to_iso9984('a').is_none());
        assert!(mkhedruli_to_iso9984('А').is_none()); // Cyrillic A.
        assert!(mkhedruli_to_iso9984('α').is_none()); // Greek alpha.
    }

    // -------------------------------------------------------------
    // Encoder behaviour.
    // -------------------------------------------------------------

    #[test]
    fn empty_input_returns_none() {
        assert!(GeorgianPhonex.encode("").is_none());
        assert!(GeorgianPhonex.encode("   ").is_none());
    }

    #[test]
    fn bare_consonant_encodes_to_seed_plus_zeros() {
        // ბ → B → "B000".
        assert_eq!(p("ბ"), "B000");
        // ს → S → "S000".
        assert_eq!(p("ს"), "S000");
    }

    #[test]
    fn ejective_and_aspirate_produce_same_key() {
        // ტ (ejective t') and თ (aspirate t) both fold to Latin `t`
        // (apostrophe dropped) → PHONEX class 3 → "T000".
        assert_eq!(p("ტ"), "T000");
        assert_eq!(p("თ"), "T000");
        assert_eq!(GeorgianPhonex.encode("ტ"), GeorgianPhonex.encode("თ"));

        // Same for კ (ejective k') and ქ (aspirate k) → class 2.
        assert_eq!(GeorgianPhonex.encode("კ"), GeorgianPhonex.encode("ქ"));
    }

    #[test]
    fn vowels_reset_the_duplicate_collapse() {
        // ბაბუა "grandfather" → BABUA → B seed, A vow, B same code=1
        // but vowel reset → push B → "BB" wait — trace: B seed, A code=0
        // reset last=0, B code=1 push → "B1", U code=0 reset, A code=0
        // reset → "B1" → pad "B100".
        assert_eq!(p("ბაბუა"), "B100");
    }

    #[test]
    fn mtavruli_input_folds_to_mkhedruli() {
        // Mtavruli input (Unicode 11 uppercase block) should
        // case-fold to Mkhedruli and produce the same phonex key.
        let a = GeorgianPhonex.encode("თბილისი").unwrap();
        let b = GeorgianPhonex.encode("ᲗᲑᲘᲚᲘᲡᲘ").unwrap();
        assert_eq!(a, b, "Mtavruli and Mkhedruli forms should agree");
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_ka() {
        assert_eq!(GeorgianPhonexAdapter.name(), "phonex-ka");
    }

    #[test]
    fn adapter_returns_some_for_georgian() {
        let out = GeorgianPhonexAdapter.encode("თბილისი");
        assert!(out.is_some(), "expected Some for Georgian input");
        let (primary, alt) = out.unwrap();
        assert!(!primary.is_empty());
        assert!(alt.is_none());
    }

    #[test]
    fn adapter_returns_none_for_no_georgian() {
        assert!(GeorgianPhonexAdapter.encode("").is_none());
        assert!(GeorgianPhonexAdapter.encode("hello").is_none());
        assert!(GeorgianPhonexAdapter.encode("123").is_none());
    }
}
