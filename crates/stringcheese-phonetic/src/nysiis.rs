//! [`Nysiis`] — the 1970 Taft NYSIIS encoder.
//!
//! # Origin
//!
//! NYSIIS (the New York State Identification and Intelligence System name
//! encoder) was developed by Robert L. Taft for the New York State Division
//! of Criminal Justice Services and published in his 1970 monograph *Name
//! Search Techniques*. It was designed as a Soundex successor for law-
//! enforcement record linkage, and remains one of the most widely deployed
//! phonetic encoders in record-linkage tooling.
//!
//! This module implements Taft's classic 1970 algorithm — the descriptor
//! variant slug is [`VariantId("taft-1970")`](stringcheese_core::VariantId).
//! It follows the same rule set that Apache Commons Codec's `Nysiis`
//! implementation follows (with the truncation-to-six-characters step
//! enabled to match Taft's canonical output). Golden cases trace back to
//! Taft's paper where possible and to Apache Commons' widely mirrored
//! reference test suite otherwise; both sources are recorded alongside
//! each case in the crate's `golden` test module.
//!
//! # Rules
//!
//! The algorithm proceeds through seven ordered stages:
//!
//! 1. **Uppercase and filter to ASCII letters.** Non-ASCII and non-letter
//!    characters are dropped; if nothing remains, the result is the empty
//!    string.
//! 2. **Prefix rewrite** (once, at the start of the string):
//!    * `MAC` → `MCC`
//!    * `KN`  → `NN`
//!    * `K`   → `C` (single-letter prefix, only if `KN` did not match)
//!    * `PH`  → `FF`
//!    * `PF`  → `FF`
//!    * `SCH` → `SSS`
//! 3. **Suffix rewrite** (once, at the end of the string):
//!    * `EE` or `IE` → `Y`
//!    * `DT`, `RT`, `RD`, `NT`, `ND` → `D`
//! 4. **Retain the first character** of the (rewritten) source; then walk
//!    each subsequent character and translate it per the per-letter rules
//!    below.
//! 5. **Per-character translation:**
//!    * `EV` → `AF` (two characters consumed)
//!    * `SCH` → `SSS` (three characters consumed)
//!    * `PH`  → `FF` (two characters consumed)
//!    * `KN`  → `N`  (two characters consumed)
//!    * vowels (`A, E, I, O, U`) → `A`
//!    * `Q` → `G`, `Z` → `S`, `M` → `N`, `K` → `C`
//!    * `H` — if either the preceding *or* following source character is
//!      not a vowel, `H` is replaced by the preceding source character
//!      (equivalent to the classical "H is transparent unless surrounded
//!      by vowels" rule). If preceded and followed by vowels, `H` is
//!      preserved.
//!    * `W` — if the preceding source character is a vowel, `W` is
//!      replaced by `A`. Otherwise `W` is preserved.
//!    * Every other letter is preserved as-is.
//! 6. **Collapse consecutive duplicates.** The walk in step 5 appends each
//!    translated character to the output only when it differs from the last
//!    character already appended.
//! 7. **Suffix cleanup**, applied in order:
//!    * Remove a trailing `S`.
//!    * Rewrite a trailing `AY` to `Y`.
//!    * Remove a trailing `A`.
//! 8. **Truncate to six characters** — the classical Taft output length.
//!    Some later NYSIIS implementations omit this truncation; the
//!    canonical Taft 1970 spec includes it and this crate matches Taft.
//!
//! # Applicability
//!
//! NYSIIS is English-only, Latin-script, and was tuned for surnames drawn
//! from an American record-linkage corpus dominated by English, Germanic,
//! and Irish names. It has no particular provision for names from other
//! language families. See [`Nysiis::APPLICABILITY`] for the declared scope.
//!
//! # Non-ASCII input
//!
//! Non-ASCII characters and non-letters are stripped in step 1. Callers who
//! want other behavior (e.g. transliterate `"Müller"` → `"Mueller"` first)
//! should apply their transliteration before calling `encode`.
//!
//! # References
//!
//! * Taft, R. L. (1970). *Name Search Techniques*. New York State
//!   Identification and Intelligence System, Special Report No. 1. Albany,
//!   NY. — the original NYSIIS specification this module implements.
//! * Apache Software Foundation. *Apache Commons Codec* — `Nysiis.java`.
//!   URL: <https://commons.apache.org/proper/commons-codec/> — the widely
//!   mirrored reference implementation the rule ordering here follows.

use alloc::string::String;
use alloc::vec::Vec;
use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::encoder::{Applicability, LanguageTag, PhoneticEncoder, ScriptTag};

/// The Taft 1970 NYSIIS encoder.
///
/// A zero-sized unit struct; construct as `Nysiis` and reuse the value
/// freely across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules and origin.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Nysiis;

/// The maximum length of a NYSIIS key. Taft's 1970 spec truncates output at
/// six characters.
pub const MAX_KEY_LEN: usize = 6;

impl Nysiis {
    /// The algorithm descriptor for the Taft 1970 NYSIIS variant.
    ///
    /// The variant slug `"taft-1970"` names the algorithm's origin and
    /// distinguishes it from any future NYSIIS-family variant (a
    /// "refined NYSIIS", for example, would take a slug like
    /// `"apache-refined"`).
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Nysiis,
        variant: VariantId("taft-1970"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Name search techniques",
            authors: "R. L. Taft",
            year: 1970,
        },
    };

    /// The languages, scripts, and regions NYSIIS was designed for.
    pub const APPLICABILITY: Applicability = Applicability {
        languages: &[LanguageTag("en")],
        scripts: &[ScriptTag("Latn")],
        regions: &[],
        notes: "English (US law-enforcement record-linkage origin); \
                tuned for names in an English/Germanic/Irish surname corpus.",
    };

    /// Returns the algorithm descriptor. `const` accessor for use in `const`
    /// contexts.
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

    /// Encodes `input` as a NYSIIS key.
    ///
    /// Returns the empty string if `input` contains no ASCII alphabetic
    /// character. Otherwise returns a key of at most six ASCII uppercase
    /// characters.
    #[must_use]
    pub fn encode(input: &str) -> String {
        nysiis_encode(input)
    }
}

impl PhoneticEncoder for Nysiis {
    type Key = String;

    #[inline]
    fn encode(&self, input: &str) -> Self::Key {
        nysiis_encode(input)
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

#[inline]
fn is_vowel(b: u8) -> bool {
    matches!(b, b'A' | b'E' | b'I' | b'O' | b'U')
}

/// Returns true if `src[i..]` starts with `pat`.
#[inline]
fn matches_at(src: &[u8], i: usize, pat: &[u8]) -> bool {
    src.len() >= i + pat.len() && &src[i..i + pat.len()] == pat
}

/// Appends `b` to `out` only if it does not duplicate the previous byte.
/// This is the collapse-consecutive-duplicates step folded into the emit
/// path; it avoids a separate deduplication pass over the output.
#[inline]
fn emit(out: &mut Vec<u8>, b: u8) {
    if out.last() != Some(&b) {
        out.push(b);
    }
}

/// Applies the prefix rewrite rules (step 2) in-place.
///
/// Prefix rules are tried longest-first to avoid a longer prefix being
/// masked by a shorter one (e.g. `MAC` vs `M`).
fn apply_prefix(src: &mut Vec<u8>) {
    if matches_at(src, 0, b"MAC") {
        src.splice(0..3, *b"MCC");
    } else if matches_at(src, 0, b"SCH") {
        src.splice(0..3, *b"SSS");
    } else if matches_at(src, 0, b"KN") {
        src.splice(0..2, *b"NN");
    } else if matches_at(src, 0, b"PH") || matches_at(src, 0, b"PF") {
        // PH and PF both rewrite to FF — the two branches were collapsed into
        // a single arm at clippy's suggestion.
        src.splice(0..2, *b"FF");
    } else if src[0] == b'K' {
        src[0] = b'C';
    }
}

/// Applies the suffix rewrite rules (step 3) in-place.
fn apply_suffix(src: &mut Vec<u8>) {
    let n = src.len();
    if n < 2 {
        return;
    }
    let last2 = &src[n - 2..n];
    match last2 {
        b"EE" | b"IE" => {
            src.truncate(n - 2);
            src.push(b'Y');
        }
        b"DT" | b"RT" | b"RD" | b"NT" | b"ND" => {
            src.truncate(n - 2);
            src.push(b'D');
        }
        _ => {}
    }
}

/// The kernel: encode `input` to a NYSIIS key.
fn nysiis_encode(input: &str) -> String {
    let mut src: Vec<u8> = input
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .map(|b| b.to_ascii_uppercase())
        .collect();

    if src.is_empty() {
        return String::new();
    }

    apply_prefix(&mut src);
    apply_suffix(&mut src);

    let mut out: Vec<u8> = Vec::with_capacity(src.len());
    out.push(src[0]);

    let mut i = 1;
    while i < src.len() {
        // Multi-character forms are tried longest-first so a shorter form
        // is not applied to a prefix of a longer one.
        if matches_at(&src, i, b"SCH") {
            emit(&mut out, b'S');
            emit(&mut out, b'S');
            emit(&mut out, b'S');
            i += 3;
        } else if matches_at(&src, i, b"EV") {
            emit(&mut out, b'A');
            emit(&mut out, b'F');
            i += 2;
        } else if matches_at(&src, i, b"PH") {
            emit(&mut out, b'F');
            emit(&mut out, b'F');
            i += 2;
        } else if matches_at(&src, i, b"KN") {
            emit(&mut out, b'N');
            i += 2;
        } else {
            let c = src[i];
            let translated = match c {
                b'A' | b'E' | b'I' | b'O' | b'U' => b'A',
                b'Q' => b'G',
                b'Z' => b'S',
                b'M' => b'N',
                b'K' => b'C',
                b'H' => {
                    // Preceding character always exists here — we started
                    // at i = 1. Following character may be off the end,
                    // which is treated as non-vowel (so the H rule fires).
                    let prev = src[i - 1];
                    let next = src.get(i + 1).copied().unwrap_or(0);
                    if is_vowel(prev) && is_vowel(next) {
                        b'H'
                    } else {
                        prev
                    }
                }
                b'W' => {
                    let prev = src[i - 1];
                    if is_vowel(prev) { b'A' } else { b'W' }
                }
                other => other,
            };
            emit(&mut out, translated);
            i += 1;
        }
    }

    // Suffix cleanup (step 7). The guards on `out.len() > 1` prevent an
    // empty key when the first character was a lone `S` or `A`.
    if out.last() == Some(&b'S') && out.len() > 1 {
        out.pop();
    }
    if out.len() >= 2 && &out[out.len() - 2..] == b"AY" {
        // Turn AY into Y — drop the A.
        let idx = out.len() - 2;
        out.remove(idx);
    }
    if out.last() == Some(&b'A') && out.len() > 1 {
        out.pop();
    }

    if out.len() > MAX_KEY_LEN {
        out.truncate(MAX_KEY_LEN);
    }

    // Every byte in `out` is an ASCII uppercase letter — `unwrap` cannot
    // fail. Using `from_utf8_unchecked` would be an unsafe optimization for
    // a hot path that this encoder is not.
    String::from_utf8(out).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_core::{AlgorithmFamily, VariantId};

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = Nysiis::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Nysiis);
        assert_eq!(d.variant, VariantId("taft-1970"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1970, .. }
        ));
    }

    #[test]
    fn applicability_is_english_latin() {
        let a = Nysiis::applicability();
        assert_eq!(a.languages, &[LanguageTag("en")]);
        assert_eq!(a.scripts, &[ScriptTag("Latn")]);
    }

    #[test]
    fn robert_reduces_to_rabad() {
        // ROBERT → suffix RT → D → ROBED.
        // R kept; O→A; B kept (diff); E→A (diff); D kept (diff). → "RABAD".
        assert_eq!(Nysiis::encode("Robert"), "RABAD");
    }

    #[test]
    fn jackson_reduces_to_jacsan() {
        // JACKSON: no prefix, no suffix rule.
        // J kept; A→A (diff); C kept; K→C (dedup); S kept; O→A; N kept. → "JACSAN".
        assert_eq!(Nysiis::encode("Jackson"), "JACSAN");
    }

    #[test]
    fn mac_prefix_expands_and_truncates_to_six() {
        // MacGyver → MAC prefix expands to MCC → MCCGYVER.
        // M kept; C, C dedup, G, Y (Y is not a vowel in Taft), V, E→A, R
        //   → "MCGYVAR" (7 chars) → truncate to 6 → "MCGYVA".
        assert_eq!(Nysiis::encode("MacGyver"), "MCGYVA");
    }

    #[test]
    fn schmidt_encodes_to_snad() {
        // Schmidt → prefix SCH → SSS → SSSMIDT.
        //   Suffix DT → D → SSSMID.
        //   S kept; SS dedup; M→N; I→A; D → "SNAD".
        assert_eq!(Nysiis::encode("Schmidt"), "SNAD");
    }

    #[test]
    fn pfister_pf_prefix_becomes_ff() {
        // Pfister → PF prefix → FF → FFISTER.
        // F kept; F dedup; I→A; S; T; E→A; R → "FASTAR".
        assert_eq!(Nysiis::encode("Pfister"), "FASTAR");
    }

    #[test]
    fn brown_w_after_vowel_becomes_a() {
        // Brown: B kept; R; O→A; W preceded by O (vowel) → W becomes A
        //   (dedups against previous A); N → "BRAN".
        assert_eq!(Nysiis::encode("Brown"), "BRAN");
    }

    #[test]
    fn knight_kn_prefix_and_h_between_consonants() {
        // Knight → KN prefix → NN → NNIGHT.
        // N kept; N dedup; I→A; G; H between G (non-vowel) and T
        //   (non-vowel) → H becomes prev (G), dedups; T → "NAGT".
        assert_eq!(Nysiis::encode("Knight"), "NAGT");
    }

    #[test]
    fn phillips_reduces_to_falap() {
        // Phillips → prefix PH → FF → FFILLIPS.
        // F kept; F dedup; I→A; L; L dedup; I→A; P; S → "FALAPS".
        // Suffix: trailing S removed → "FALAP".
        assert_eq!(Nysiis::encode("Phillips"), "FALAP");
    }

    #[test]
    fn anderson_truncates_to_six() {
        // ANDERSON: A kept; N; D; E→A; R; S; O→A; N → "ANDARSAN".
        //   Not S/AY/A trailing. Truncate to 6 → "ANDARS".
        assert_eq!(Nysiis::encode("Anderson"), "ANDARS");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(Nysiis::encode(""), "");
        assert_eq!(Nysiis::encode("1234"), "");
        assert_eq!(Nysiis::encode("---"), "");
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(Nysiis::encode("robert"), Nysiis::encode("ROBERT"));
        assert_eq!(Nysiis::encode("Robert"), Nysiis::encode("ROBERT"));
    }

    #[test]
    fn strips_non_ascii_and_non_letters() {
        assert_eq!(Nysiis::encode("R-o-b-e-r-t"), Nysiis::encode("Robert"));
        assert_eq!(Nysiis::encode("Robert!"), Nysiis::encode("Robert"));
    }

    #[test]
    fn respects_maximum_length() {
        // A long name must be truncated to at most `MAX_KEY_LEN` characters.
        let out = Nysiis::encode("Constantinople");
        assert!(out.len() <= MAX_KEY_LEN, "got {out:?}");
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = Nysiis::DESCRIPTOR;
        assert_eq!(D.variant.0, "taft-1970");
    }

    #[test]
    fn trait_and_inherent_encode_agree() {
        let enc = Nysiis;
        for name in ["Robert", "Jackson", "Schmidt", "Phillips", "Brown", ""] {
            assert_eq!(
                <Nysiis as PhoneticEncoder>::encode(&enc, name),
                Nysiis::encode(name)
            );
        }
    }
}
