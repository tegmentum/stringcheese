//! [`Soundex`] — the 1918 NARA/American Soundex encoder.
//!
//! # Origin
//!
//! Soundex was developed by Robert C. Russell in 1918 (US patent 1,261,167)
//! and popularized by the US National Archives and Records Administration
//! (NARA) as the indexing scheme for the 1880 – 1940 US Census microfilm.
//! The variant this module implements is the NARA canonical form — the same
//! one taught in the "Soundex indexing system" reference published on the
//! Archives' website — under the slug
//! [`VariantId("english-1918-nara")`](stringcheese_core::VariantId).
//!
//! The classical citation and NARA's reference are the two sources every
//! golden case in the crate's `golden` test module traces back to. The
//! rules below are the ones NARA publishes; see the [Archives' Soundex
//! page][nara] for the canonical statement.
//!
//! [nara]: https://www.archives.gov/research/census/soundex
//!
//! # Rules
//!
//! 1. **Retain the first letter** of the input (uppercased). Non-alphabetic
//!    input characters are skipped entirely. If the input has no ASCII
//!    alphabetic character, the result is the empty string.
//! 2. **Encode each following letter** with a digit:
//!    * `B, F, P, V` → `1`
//!    * `C, G, J, K, Q, S, X, Z` → `2`
//!    * `D, T` → `3`
//!    * `L` → `4`
//!    * `M, N` → `5`
//!    * `R` → `6`
//!    * Vowels (`A, E, I, O, U`) and `Y` — treated as *separators*; they
//!      do not produce a digit but they *do* reset the collapse run so that
//!      an earlier code and a later same-code letter with a vowel between
//!      them both survive.
//!    * `H, W` — silent; they neither produce a digit nor reset the
//!      collapse run, so two same-code letters separated by only `H` or
//!      `W` still collapse.
//! 3. **Collapse consecutive same-code letters** (including collapses across
//!    a silent `H` or `W`) into a single digit. This rule also applies to
//!    the first letter: if the letter immediately after the retained first
//!    letter has the same Soundex code as that first letter, it is dropped.
//! 4. **Truncate or right-pad with `'0'`** to exactly four characters: the
//!    retained first letter plus three digits.
//!
//! # Applicability
//!
//! Soundex was designed for early-20th-century English surnames in the
//! Latin script. Feeding it Chinese, Japanese, or Arabic input does not
//! produce a meaningful result — the algorithm's ASCII-letter alphabet is
//! not what those languages present. See [`Soundex::APPLICABILITY`] for the
//! declared scope.
//!
//! # Non-ASCII input
//!
//! Non-ASCII characters are stripped in the first step. The result is the
//! Soundex of the input's ASCII-letter subsequence. Callers who want other
//! behavior (e.g. transliterate `"Müller"` → `"Mueller"` first) should apply
//! their transliteration before calling `encode`.
//!
//! # References
//!
//! * Russell, R. C. (1918). *Method and means for identifying names by
//!   phonetic sound*. US Patent 1,261,167 (filed 1917, issued April 2, 1918).
//!   The original Soundex patent.
//! * US National Archives and Records Administration. *The Soundex Indexing
//!   System*. URL: <https://www.archives.gov/research/census/soundex> — the
//!   canonical NARA specification this module implements.

use alloc::string::String;
use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::encoder::{Applicability, LanguageTag, PhoneticEncoder, ScriptTag};

/// The 1918 NARA/American Soundex encoder.
///
/// A zero-sized unit struct; construct as `Soundex` and reuse the value
/// freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and origin.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Soundex;

impl Soundex {
    /// The algorithm descriptor for the NARA canonical Soundex variant.
    ///
    /// The variant slug `"english-1918-nara"` names both the origin year
    /// (1918, Russell's patent) and the definitive reference (NARA's
    /// Soundex indexing publication).
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Soundex,
        variant: VariantId("english-1918-nara"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Standard {
            name: "NARA Soundex Indexing System, US National Archives",
        },
    };

    /// The languages, scripts, and regions Soundex was designed for.
    ///
    /// Soundex is an English-only, Latin-script encoder developed for US
    /// Census surname indexing.
    pub const APPLICABILITY: Applicability = Applicability {
        languages: &[LanguageTag("en")],
        scripts: &[ScriptTag("Latn")],
        regions: &[],
        notes: "English (1918 US Census indexing origin); \
                other languages will produce meaningless codes.",
    };

    /// Returns the algorithm descriptor. `const` accessor for use in `const`
    /// contexts (for example, as the `descriptor` field of a `GoldenCase`).
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Returns the applicability. `const` accessor.
    #[inline]
    #[must_use]
    pub const fn applicability() -> Applicability {
        Self::APPLICABILITY
    }

    /// Encodes `input` as a Soundex key.
    ///
    /// Returns the empty string if `input` contains no ASCII alphabetic
    /// character.
    ///
    /// The result is exactly four characters long for any non-empty
    /// alphabetic input: the uppercase first letter plus three digits
    /// (right-padded with `'0'` if the letter run does not produce three
    /// digits, or truncated if it produces more).
    #[must_use]
    pub fn encode(input: &str) -> String {
        soundex_encode(input)
    }
}

impl PhoneticEncoder for Soundex {
    type Key = String;

    #[inline]
    fn encode(&self, input: &str) -> Self::Key {
        soundex_encode(input)
    }

    #[inline]
    fn descriptor(&self) -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    #[inline]
    fn applicability(&self) -> Applicability {
        Self::APPLICABILITY
    }
}

/// Classification of an ASCII letter under Soundex rules.
///
/// The three variants correspond to the three collapse-run behaviors: a
/// consonant with a digit code, a vowel-class letter that resets the run
/// without emitting a digit, and a silent letter that neither resets nor
/// emits.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Class {
    /// A consonant with the given Soundex digit code (ASCII `'0'..='6'`).
    Code(u8),
    /// A vowel (`A, E, I, O, U`) or `Y`. Resets the collapse run without
    /// emitting a digit.
    Vowel,
    /// `H` or `W`. Neither emits a digit nor resets the collapse run.
    Silent,
}

/// Classifies an uppercased ASCII letter under Soundex rules.
///
/// The input is expected to already be uppercased and ASCII-alphabetic; the
/// caller ([`soundex_encode`]) filters and normalizes before calling this.
#[inline]
fn classify(c: u8) -> Class {
    match c {
        b'B' | b'F' | b'P' | b'V' => Class::Code(b'1'),
        b'C' | b'G' | b'J' | b'K' | b'Q' | b'S' | b'X' | b'Z' => Class::Code(b'2'),
        b'D' | b'T' => Class::Code(b'3'),
        b'L' => Class::Code(b'4'),
        b'M' | b'N' => Class::Code(b'5'),
        b'R' => Class::Code(b'6'),
        b'H' | b'W' => Class::Silent,
        // A, E, I, O, U, Y — vowels and Y are separators.
        _ => Class::Vowel,
    }
}

/// The kernel: encode `input` to a four-character Soundex key.
///
/// The input's non-ASCII-letter characters are filtered out first; the
/// remaining sequence drives the three-state collapse-run walk described in
/// the module-level rules.
fn soundex_encode(input: &str) -> String {
    // Filter to ASCII letters and uppercase them. Iterating byte-by-byte
    // once the input has been checked as ASCII-alphabetic would be marginally
    // faster, but the `.chars()` path handles arbitrary Unicode input
    // without a panic — which matters, because the encoder makes no promise
    // that its input is ASCII.
    let mut letters = input
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_uppercase() as u8);

    let Some(first) = letters.next() else {
        return String::new();
    };

    let mut result = String::with_capacity(4);
    result.push(first as char);

    // `last_code` tracks the digit that would collapse against the next
    // consonant. It is seeded with the first letter's own class (so the
    // second letter's collapse rule includes the first-letter case), and
    // is reset to `None` by a vowel and left unchanged by a silent H/W.
    let mut last_code: Option<u8> = match classify(first) {
        Class::Code(d) => Some(d),
        Class::Vowel | Class::Silent => None,
    };

    for letter in letters {
        match classify(letter) {
            Class::Code(d) => {
                if Some(d) != last_code {
                    result.push(d as char);
                    if result.len() >= 4 {
                        break;
                    }
                }
                last_code = Some(d);
            }
            Class::Vowel => {
                last_code = None;
            }
            Class::Silent => {
                // Neither reset nor emit; last_code carries over.
            }
        }
    }

    while result.len() < 4 {
        result.push('0');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_core::{AlgorithmFamily, VariantId};

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = Soundex::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Soundex);
        assert_eq!(d.variant, VariantId("english-1918-nara"));
    }

    #[test]
    fn applicability_is_english_latin() {
        let a = Soundex::applicability();
        assert_eq!(a.languages, &[LanguageTag("en")]);
        assert_eq!(a.scripts, &[ScriptTag("Latn")]);
    }

    #[test]
    fn robert_encodes_to_r163() {
        // Cited in NARA's Soundex reference and every published Soundex
        // implementation. R-o-b-e-r-t → R, (o=vowel), b=1, (e=vowel), r=6,
        // t=3 → "R163".
        assert_eq!(Soundex::encode("Robert"), "R163");
    }

    #[test]
    fn rupert_encodes_to_r163() {
        // Same Soundex code as Robert — that's Soundex's whole point.
        // R-u-p-e-r-t → R, (u=vowel), p=1, (e=vowel), r=6, t=3 → "R163".
        assert_eq!(Soundex::encode("Rupert"), "R163");
    }

    #[test]
    fn rubin_pads_to_r150() {
        // R-u-b-i-n → R, (u=vowel), b=1, (i=vowel), n=5 → "R15" padded to
        // "R150".
        assert_eq!(Soundex::encode("Rubin"), "R150");
    }

    #[test]
    fn ashcraft_collapses_across_h() {
        // A-s-h-c-r-a-f-t → A, s=2, h=silent (holds last_code=2), c=2
        // (collapses across the silent H — this is the "H/W between two
        // same-code letters" rule), r=6, a=vowel (resets), f=1, (truncated
        // before t). Result "A261".
        assert_eq!(Soundex::encode("Ashcraft"), "A261");
    }

    #[test]
    fn tymczak_two_of_the_three_2s_collapse() {
        // T-y-m-c-z-a-k → T, y=vowel, m=5, c=2, z=2 (collapses against the
        // adjacent C), a=vowel (resets), k=2 (a new run — the intervening
        // vowel reset it) → "T522".
        assert_eq!(Soundex::encode("Tymczak"), "T522");
    }

    #[test]
    fn pfister_first_letter_same_code_collapse() {
        // P-f-i-s-t-e-r → P (kept), then f=1. P's own Soundex code is 1,
        // so the f collapses against the retained first letter and does not
        // emit — this is the "first letter same-code drop" rule. Then
        // (i=vowel), s=2, t=3, (e=vowel), r=6 → "P236".
        assert_eq!(Soundex::encode("Pfister"), "P236");
    }

    #[test]
    fn honeyman_hw_transparency() {
        // H-o-n-e-y-m-a-n → H, o=vowel, n=5, e=vowel, y=vowel (Y treated as
        // a vowel), m=5 (a new run — the intervening vowels reset it), a
        // =vowel, n=5 (another new run) → "H555".
        // Cited as: NARA's teaching example for the vowel-reset behavior.
        assert_eq!(Soundex::encode("Honeyman"), "H555");
    }

    #[test]
    fn empty_input_returns_empty() {
        // Documented edge case: no ASCII alphabetic character → empty
        // string, distinguishable from a real code.
        assert_eq!(Soundex::encode(""), "");
        assert_eq!(Soundex::encode("1234"), "");
        assert_eq!(Soundex::encode("---"), "");
    }

    #[test]
    fn short_input_pads_with_zeros() {
        // A single letter has no consonants after it — pad to length 4.
        assert_eq!(Soundex::encode("A"), "A000");
        assert_eq!(Soundex::encode("B"), "B000");
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(Soundex::encode("robert"), "R163");
        assert_eq!(Soundex::encode("ROBERT"), "R163");
        assert_eq!(Soundex::encode("Robert"), "R163");
    }

    #[test]
    fn strips_non_ascii_and_non_letters() {
        // Non-ASCII letters and non-letters are stripped before encoding.
        assert_eq!(Soundex::encode("R-obert!"), "R163");
        assert_eq!(Soundex::encode("R o b e r t"), "R163");
    }

    #[test]
    fn returns_exactly_four_chars_for_nonempty_input() {
        for name in [
            "A", "AB", "ABC", "ABCD", "ABCDE", "ABCDEFGH", "ROBERT", "MONTAGUE", "OMBRELLA",
        ] {
            let code = Soundex::encode(name);
            assert_eq!(
                code.len(),
                4,
                "Soundex({name:?}) produced {code:?}, expected 4 chars"
            );
        }
    }

    #[test]
    fn descriptor_is_const() {
        // Available at const time, so downstream can pin it into
        // `const GOLDEN_CASE: GoldenCase<...>` records.
        const D: AlgorithmDescriptor = Soundex::DESCRIPTOR;
        assert_eq!(D.variant.0, "english-1918-nara");
    }

    #[test]
    fn trait_and_inherent_encode_agree() {
        let enc = Soundex;
        for name in ["Robert", "Ashcraft", "Pfister", "Honeyman", "Tymczak", ""] {
            assert_eq!(
                <Soundex as PhoneticEncoder>::encode(&enc, name),
                Soundex::encode(name)
            );
        }
    }
}
