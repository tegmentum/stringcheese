//! [`DoubleMetaphone`] — a primary-key implementation of Lawrence Philips'
//! 1999 Double Metaphone encoder.
//!
//! # Scope: primary key only
//!
//! Full Double Metaphone produces **up to two** phonetic keys per input, a
//! primary and an optional alternate, to capture regional-pronunciation
//! variance (e.g. `"Schmidt"` primary `"XMT"` and alternate `"SMT"`). This
//! crate ships the **primary key only** for the 0.1 release. The
//! [`DoubleMetaphoneKey`] type carries an `alternate` field for forward
//! compatibility, but this implementation always populates it with `None`.
//!
//! **Why primary-only:** Double Metaphone is by a wide margin the most
//! complex algorithm in this crate — Lawrence Philips' original C code
//! spans several hundred lines of context-sensitive rules with substantial
//! overlap between the primary and alternate branches. Landing a defensible
//! primary implementation and iterating to the two-key form in a later
//! release is a better trade than shipping a half-tested two-key variant
//! that silently disagrees with published references. See [the design
//! doc][phon] for the multi-key comparator contract that the alternate
//! extension will slot into without breaking existing call sites.
//!
//! The variant slug is [`VariantId("philips-1999-primary-only")`][vid] so
//! that a future full-two-key variant can share the family with a distinct
//! slug (`"philips-1999-full"`) and existing golden cases cannot silently
//! be validated against the wrong variant.
//!
//! [phon]: https://github.com/zacharywhitley/comparand/blob/main/docs/design/phonetic-subsystem.md
//! [vid]: comparand_core::VariantId
//!
//! # Rules implemented
//!
//! The implementation covers the classical primary-key branch of Philips'
//! algorithm, following the widely mirrored reference structure in Apache
//! Commons Codec's `DoubleMetaphone.java`. The rules are:
//!
//! * **Silent starts.** Words beginning with `GN`, `KN`, `PN`, `WR`, or
//!   `PS` skip their first letter.
//! * **Initial X.** A word starting with `X` emits `S` (as in *Xavier*).
//! * **Vowels** (`A, E, I, O, U, Y`) emit `A` if they are the first character
//!   contributing to the key, and are otherwise silent.
//! * **`B`** → `P`; doubled `BB` collapses.
//! * **`C`** — the classical soft/hard C branch with digraph handling for
//!   `CH` (→ `X`), `CK` (→ `K`), and `CIA` (→ `X`).
//! * **`D`** → `T`; `DG` before `E/I/Y` → `J`; doubled `DD`/`DT` collapses.
//! * **`F`** → `F`; doubled `FF` collapses.
//! * **`G`** — soft G before `E/I/Y` → `J`; `GH` silent after a vowel,
//!   `K` otherwise; hard G → `K`.
//! * **`H`** — silent except when at the start of the key or between two
//!   vowels.
//! * **`J`** → `J`.
//! * **`K, L, M, N, P, R, T`** — each maps to itself, with doubled forms
//!   collapsing.
//! * **`PH`** → `F`.
//! * **`Q`** → `K`.
//! * **`SH`** → `X`; **`SCH`** → `X`; otherwise `S` → `S` (doubled collapses).
//! * **`TH`** → `0` (theta placeholder, spelled as ASCII `0` in the key)
//!   except in the *Thomas* / *Thames* family (`TH` followed by `OM`/`AM`),
//!   where it emits `T`.
//! * **`V`** → `F`.
//! * **`W`** is silent (a rich Double Metaphone would model regional
//!   Germanic pronunciation via the alternate key; primary-only implements
//!   the Anglicized behavior).
//! * **`X`** → `KS` in the middle of a word; silent when at the end
//!   preceded by `AU` or `OU` (French endings). Initial `X` is handled by
//!   the silent-start rule above.
//! * **`Z`** → `S`.
//!
//! Result length is capped at four characters, matching Philips' original
//! specification.
//!
//! # Applicability
//!
//! Double Metaphone was designed for English but includes rules for
//! Germanic, Slavic, and Romance surnames often encountered in English-
//! language records. Its output on non-Latin-script input (Chinese in Han
//! characters, Arabic in Arabic script) is meaningless — the algorithm's
//! ASCII-letter alphabet is not what those languages present.
//!
//! # Non-ASCII input
//!
//! Non-ASCII characters and non-letters are stripped in the first step;
//! callers who want other behavior should transliterate before calling
//! `encode`.

use alloc::string::String;
use alloc::vec::Vec;
use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::encoder::{Applicability, LanguageTag, PhoneticEncoder, ScriptTag};

/// The maximum length of a Double Metaphone key, per Philips' original
/// specification.
pub const MAX_KEY_LEN: usize = 4;

/// A Double Metaphone key: a primary code and an optional alternate.
///
/// This crate's [`DoubleMetaphone`] encoder is currently primary-only, so
/// [`alternate`](Self::alternate) is always `None`. The field exists so a
/// future two-key implementation (variant slug
/// `"philips-1999-full"`) can populate it without a breaking change to the
/// [`crate::PhoneticEncoder::Key`] associated type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DoubleMetaphoneKey {
    /// The primary phonetic key. At most four characters.
    pub primary: String,
    /// The optional alternate phonetic key, for regional pronunciation
    /// variance. Always `None` in the primary-only variant.
    pub alternate: Option<String>,
}

impl DoubleMetaphoneKey {
    /// Constructs a key with only a primary code. Convenience for the
    /// primary-only variant.
    #[inline]
    #[must_use]
    pub const fn primary_only(primary: String) -> Self {
        Self {
            primary,
            alternate: None,
        }
    }
}

/// The Double Metaphone (primary-only) encoder.
///
/// A zero-sized unit struct; construct as `DoubleMetaphone` and reuse the
/// value freely across threads and calls.
///
/// See the [module-level docs](self) for scope, rules, and future
/// extension plans.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DoubleMetaphone;

impl DoubleMetaphone {
    /// The algorithm descriptor for the primary-only Double Metaphone
    /// variant.
    ///
    /// The `"primary-only"` suffix in the slug distinguishes this variant
    /// from the future `"philips-1999-full"` two-key variant, so golden
    /// cases cannot silently be validated against the wrong one.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::DoubleMetaphone,
        variant: VariantId("philips-1999-primary-only"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::ReferenceImplementation {
            name: "Apache Commons Codec DoubleMetaphone (primary key path)",
        },
    };

    /// The languages, scripts, and regions Double Metaphone was designed for.
    ///
    /// The algorithm's rule set is English-centric with adaptations for
    /// Germanic, Slavic, and Romance surnames found in English-language
    /// records. It is Latin-script only.
    pub const APPLICABILITY: Applicability = Applicability {
        languages: &[LanguageTag("en")],
        scripts: &[ScriptTag("Latn")],
        regions: &[],
        notes: "English-centric with Germanic / Slavic / Romance surname \
                adaptations; Latin script only.",
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

    /// Encodes `input` to a [`DoubleMetaphoneKey`]. In the primary-only
    /// variant the [`DoubleMetaphoneKey::alternate`] field is always `None`.
    #[must_use]
    pub fn encode(input: &str) -> DoubleMetaphoneKey {
        double_metaphone_encode(input)
    }
}

impl PhoneticEncoder for DoubleMetaphone {
    type Key = DoubleMetaphoneKey;

    #[inline]
    fn encode(&self, input: &str) -> Self::Key {
        double_metaphone_encode(input)
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
    matches!(b, b'A' | b'E' | b'I' | b'O' | b'U' | b'Y')
}

/// Returns the byte at `src[i]`, or `0` if out of bounds. `0` is convenient
/// because it never matches any ASCII uppercase letter.
#[inline]
fn at(src: &[u8], i: usize) -> u8 {
    src.get(i).copied().unwrap_or(0)
}

/// Returns true if `src[i..]` starts with `pat`.
#[inline]
fn matches_at(src: &[u8], i: usize, pat: &[u8]) -> bool {
    src.len() >= i + pat.len() && &src[i..i + pat.len()] == pat
}

/// Appends `c` to `out` if the output has room; returns `true` if the output
/// is now at capacity.
#[inline]
fn push_if_room(out: &mut String, c: char) -> bool {
    if out.len() < MAX_KEY_LEN {
        out.push(c);
    }
    out.len() >= MAX_KEY_LEN
}

/// The kernel: encode `input` to a primary-only Double Metaphone key.
#[allow(
    clippy::too_many_lines,
    reason = "The algorithm's rules are best expressed as one flat match; \
              breaking it up would obscure the letter-by-letter structure."
)]
fn double_metaphone_encode(input: &str) -> DoubleMetaphoneKey {
    let src: Vec<u8> = input
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .map(|b| b.to_ascii_uppercase())
        .collect();

    if src.is_empty() {
        return DoubleMetaphoneKey::primary_only(String::new());
    }

    let mut out = String::with_capacity(MAX_KEY_LEN);

    // Silent-start prefixes: skip the first letter.
    let mut i = 0usize;
    if src.len() >= 2 && matches!(&src[0..2], b"GN" | b"KN" | b"PN" | b"WR" | b"PS") {
        i = 1;
    }

    // Initial X → S (as in "Xavier"). Handled before the main loop because
    // subsequent X's have different rules.
    if src[0] == b'X' {
        out.push('S');
        i = 1;
    }

    while i < src.len() && out.len() < MAX_KEY_LEN {
        let c = src[i];
        match c {
            b'A' | b'E' | b'I' | b'O' | b'U' | b'Y' => {
                // Vowels contribute only when they are the first character
                // committed to the key; internal vowels are silent.
                if out.is_empty() {
                    out.push('A');
                }
                i += 1;
            }
            b'B' => {
                if push_if_room(&mut out, 'P') {
                    break;
                }
                i += if at(&src, i + 1) == b'B' { 2 } else { 1 };
            }
            b'C' => {
                if matches_at(&src, i, b"CH") {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 2;
                } else if matches_at(&src, i, b"CK") {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += 2;
                } else if matches_at(&src, i, b"CIA") {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 3;
                } else if matches!(at(&src, i + 1), b'E' | b'I' | b'Y') {
                    // Soft C
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                    i += 1;
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'C' { 2 } else { 1 };
                }
            }
            b'D' => {
                if at(&src, i + 1) == b'G' && matches!(at(&src, i + 2), b'E' | b'I' | b'Y') {
                    if push_if_room(&mut out, 'J') {
                        break;
                    }
                    i += 3;
                } else {
                    if push_if_room(&mut out, 'T') {
                        break;
                    }
                    i += if matches!(at(&src, i + 1), b'D' | b'T') {
                        2
                    } else {
                        1
                    };
                }
            }
            b'F' => {
                if push_if_room(&mut out, 'F') {
                    break;
                }
                i += if at(&src, i + 1) == b'F' { 2 } else { 1 };
            }
            b'G' => {
                if at(&src, i + 1) == b'H' {
                    // GH silent after a vowel (as in "bright", "though")
                    // otherwise emits K (as in "ghost" — start of word)
                    if i > 0 && is_vowel(src[i - 1]) {
                        i += 2;
                    } else {
                        if push_if_room(&mut out, 'K') {
                            break;
                        }
                        i += 2;
                    }
                } else if matches!(at(&src, i + 1), b'E' | b'I' | b'Y') {
                    // Soft G
                    if push_if_room(&mut out, 'J') {
                        break;
                    }
                    i += 1;
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'G' { 2 } else { 1 };
                }
            }
            b'H' => {
                // H emits only at start of key or between two vowels.
                let prev_vowel = i > 0 && is_vowel(src[i - 1]);
                let next_vowel = is_vowel(at(&src, i + 1));
                let should_emit = out.is_empty() || (prev_vowel && next_vowel);
                if should_emit && push_if_room(&mut out, 'H') {
                    break;
                }
                i += 1;
            }
            b'J' => {
                if push_if_room(&mut out, 'J') {
                    break;
                }
                i += 1;
            }
            b'K' => {
                if push_if_room(&mut out, 'K') {
                    break;
                }
                i += if at(&src, i + 1) == b'K' { 2 } else { 1 };
            }
            b'L' => {
                if push_if_room(&mut out, 'L') {
                    break;
                }
                i += if at(&src, i + 1) == b'L' { 2 } else { 1 };
            }
            b'M' => {
                if push_if_room(&mut out, 'M') {
                    break;
                }
                i += if at(&src, i + 1) == b'M' { 2 } else { 1 };
            }
            b'N' => {
                if push_if_room(&mut out, 'N') {
                    break;
                }
                i += if at(&src, i + 1) == b'N' { 2 } else { 1 };
            }
            b'P' => {
                if at(&src, i + 1) == b'H' {
                    if push_if_room(&mut out, 'F') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'P') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'P' { 2 } else { 1 };
                }
            }
            b'Q' => {
                if push_if_room(&mut out, 'K') {
                    break;
                }
                i += 1;
            }
            b'R' => {
                if push_if_room(&mut out, 'R') {
                    break;
                }
                i += if at(&src, i + 1) == b'R' { 2 } else { 1 };
            }
            b'S' => {
                if matches_at(&src, i, b"SCH") {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 3;
                } else if at(&src, i + 1) == b'H' {
                    if push_if_room(&mut out, 'X') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                    i += if at(&src, i + 1) == b'S' { 2 } else { 1 };
                }
            }
            b'T' => {
                if at(&src, i + 1) == b'H' {
                    // Thomas / Thames family: TH + OM/AM → T (not theta).
                    if matches!(&src[i + 2..src.len().min(i + 4)], b"OM" | b"AM") {
                        if push_if_room(&mut out, 'T') {
                            break;
                        }
                    } else if push_if_room(&mut out, '0') {
                        break;
                    }
                    i += 2;
                } else {
                    if push_if_room(&mut out, 'T') {
                        break;
                    }
                    i += if matches!(at(&src, i + 1), b'T' | b'D') {
                        2
                    } else {
                        1
                    };
                }
            }
            b'V' => {
                if push_if_room(&mut out, 'F') {
                    break;
                }
                i += if at(&src, i + 1) == b'V' { 2 } else { 1 };
            }
            b'W' => {
                // Primary-only variant treats W as silent; the alternate key
                // (future work) would emit F for Germanic origins.
                i += 1;
            }
            b'X' => {
                // Silent at end of word after AU/OU (French endings like
                // "PEUX", "FAUX"); KS elsewhere.
                let at_end = i == src.len() - 1;
                let after_ou_au = i >= 2 && matches!(&src[i - 2..i], b"OU" | b"AU");
                if at_end && after_ou_au {
                    // silent
                } else {
                    if push_if_room(&mut out, 'K') {
                        break;
                    }
                    if push_if_room(&mut out, 'S') {
                        break;
                    }
                }
                i += 1;
            }
            b'Z' => {
                if push_if_room(&mut out, 'S') {
                    break;
                }
                i += if at(&src, i + 1) == b'Z' { 2 } else { 1 };
            }
            _ => {
                i += 1;
            }
        }
    }

    DoubleMetaphoneKey::primary_only(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use comparand_core::{AlgorithmFamily, VariantId};

    /// Convenience wrapper for the primary key.
    fn primary(name: &str) -> String {
        DoubleMetaphone::encode(name).primary
    }

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = DoubleMetaphone::descriptor();
        assert_eq!(d.family, AlgorithmFamily::DoubleMetaphone);
        assert_eq!(d.variant, VariantId("philips-1999-primary-only"));
    }

    #[test]
    fn applicability_is_english_latin() {
        let a = DoubleMetaphone::applicability();
        assert_eq!(a.languages, &[LanguageTag("en")]);
        assert_eq!(a.scripts, &[ScriptTag("Latn")]);
    }

    #[test]
    fn alternate_is_always_none() {
        // Primary-only variant contract.
        for name in ["Schmidt", "Xavier", "Wagner", "Thompson", "Smith"] {
            let key = DoubleMetaphone::encode(name);
            assert_eq!(
                key.alternate, None,
                "{name:?} produced a non-None alternate"
            );
        }
    }

    #[test]
    fn xavier_starts_with_s() {
        // Initial X → S; A skipped as internal vowel; V → F; I skipped;
        // E skipped; R → R. → "SFR".
        assert_eq!(primary("Xavier"), "SFR");
    }

    #[test]
    fn wagner_starts_with_a() {
        // W silent; A becomes first-committed-letter → A; G → K; N → N;
        // E skipped; R → R. → "AKNR".
        assert_eq!(primary("Wagner"), "AKNR");
    }

    #[test]
    fn schmidt_uses_sch_x() {
        // SCH → X; M → M; I skipped; D → T. → "XMT".
        assert_eq!(primary("Schmidt"), "XMT");
    }

    #[test]
    fn thomas_uses_th_t_exception() {
        // TH+OM → T (Thomas exception); O skipped; M → M; A skipped; S → S.
        //   → "TMS".
        assert_eq!(primary("Thomas"), "TMS");
    }

    #[test]
    fn thompson_uses_th_t_exception() {
        // TH+OM → T; O skipped; M → M; P → P; S → S (truncated at 4).
        //   → "TMPS".
        assert_eq!(primary("Thompson"), "TMPS");
    }

    #[test]
    fn smith_uses_th_theta() {
        // S → S; M → M; I skipped; TH → 0 (theta, not TH+OM/AM). → "SM0".
        assert_eq!(primary("Smith"), "SM0");
    }

    #[test]
    fn knight_silent_kn_start() {
        // KN silent start → K skipped; N → N; I skipped; GH silent after
        //   vowel (I); T → T. → "NT".
        assert_eq!(primary("Knight"), "NT");
    }

    #[test]
    fn gnome_silent_gn_start() {
        // GN silent start → G skipped; N → N; O skipped; M → M; E skipped.
        //   → "NM".
        assert_eq!(primary("Gnome"), "NM");
    }

    #[test]
    fn phillips_ph_becomes_f() {
        // PH → F; I skipped; L → L; L (double collapsed); I skipped;
        //   P → P; S → S (but truncated at 4). → "FLPS".
        assert_eq!(primary("Phillips"), "FLPS");
    }

    #[test]
    fn empty_and_junk_input_return_empty_primary() {
        assert_eq!(primary(""), "");
        assert_eq!(primary("1234"), "");
        assert_eq!(primary("---"), "");
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(primary("smith"), primary("Smith"));
        assert_eq!(primary("SMITH"), primary("Smith"));
    }

    #[test]
    fn primary_length_bounded_by_four() {
        for name in [
            "A",
            "AB",
            "Constantinople",
            "Antidisestablishmentarianism",
            "Bratislavsky",
        ] {
            assert!(
                primary(name).len() <= MAX_KEY_LEN,
                "primary({name:?}) exceeded {MAX_KEY_LEN}"
            );
        }
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = DoubleMetaphone::DESCRIPTOR;
        assert_eq!(D.variant.0, "philips-1999-primary-only");
    }

    #[test]
    fn trait_and_inherent_encode_agree() {
        let enc = DoubleMetaphone;
        for name in ["Schmidt", "Xavier", "Wagner", ""] {
            assert_eq!(
                <DoubleMetaphone as PhoneticEncoder>::encode(&enc, name),
                DoubleMetaphone::encode(name)
            );
        }
    }
}
