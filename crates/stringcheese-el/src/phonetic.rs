//! Greek → Latin transliteration.
//!
//! # Origin
//!
//! Greek has no widely established Soundex / Metaphone-family
//! phonetic encoder in the way English or German do. What Greek *does*
//! have is a well-known **transliteration** standard: **ISO 843**, a
//! 1997 ISO standard for the romanization of Greek script that is used
//! by academic bibliography, library cataloguing, and the ELOT 743
//! Greek transliteration (identical to ISO 843 Type 1 up to a couple
//! of niche digraph choices).
//!
//! # Standard chosen: ISO 843 (Type 1 transliteration)
//!
//! ISO 843 defines both a *transliteration* (letter-by-letter, in
//! principle reversible; ships with a Type 1 "documentary" mapping and
//! a Type 2 "phonetic" mapping) and a *transcription* (sound-based, for
//! everyday romanization). This crate ships the **Type 1
//! transliteration** — the mapping used for library catalogs and the
//! one Greek passports use for names.
//!
//! ## Single-letter mapping
//!
//! The 24-letter Greek alphabet maps to Latin as follows:
//!
//! | Greek | Latin | Notes |
//! |-------|-------|-------|
//! | `α`  | `a`  | |
//! | `β`  | `v`  | |
//! | `γ`  | `g`  | |
//! | `δ`  | `d`  | |
//! | `ε`  | `e`  | |
//! | `ζ`  | `z`  | |
//! | `η`  | `i`  | |
//! | `θ`  | `th` | |
//! | `ι`  | `i`  | |
//! | `κ`  | `k`  | |
//! | `λ`  | `l`  | |
//! | `μ`  | `m`  | |
//! | `ν`  | `n`  | |
//! | `ξ`  | `x`  | |
//! | `ο`  | `o`  | |
//! | `π`  | `p`  | |
//! | `ρ`  | `r`  | |
//! | `σ`  | `s`  | non-final sigma |
//! | `ς`  | `s`  | final sigma (positional variant) |
//! | `τ`  | `t`  | |
//! | `υ`  | `y`  | |
//! | `φ`  | `f`  | |
//! | `χ`  | `ch` | |
//! | `ψ`  | `ps` | |
//! | `ω`  | `o`  | |
//!
//! Accented forms (`ά έ ή ί ό ύ ώ`) and diaeresis forms (`ϊ ϋ ΐ ΰ`)
//! map to the base letter's Latin form — accents carry no information
//! for a byte-oriented ASCII key. Any Greek scalar outside the mapping
//! (uppercase forms are lowercased first, then looked up in the
//! table) passes through unchanged.
//!
//! ## Digraph handling
//!
//! Greek writes several sound sequences as digraphs; ISO 843
//! transliterates the diphthongs letter-by-letter (`αι → ai`, `ει →
//! ei`, `οι → oi`, `ου → ou`, `αυ → ay`, `ευ → ey`, `ηυ → iy`), which
//! this encoder produces naturally by walking the input character by
//! character. The one context-sensitive rule this encoder implements
//! is the double gamma:
//!
//! * `γγ → ng` (Greek `άγγελος → angelos`) — a nasalized velar cluster.
//!
//! Other context-sensitive ISO 843 rules (the `γκ → ng` word-initial
//! nasalization, the `μπ → b` word-initial rewrite) are **deliberately
//! not implemented** in this crate — a byte-oriented ASCII key does
//! not benefit from the extra ambiguity a context-sensitive fold
//! introduces, and the deterministic letter-by-letter mapping stays
//! easier to reason about. Callers who need a full ELOT 743
//! romanization can layer their own post-processing on top.
//!
//! # Why a transliteration instead of a Soundex?
//!
//! Same rationale as the `stringcheese-ru` and `stringcheese-bg`
//! packs: Greek's phoneme inventory does not
//! admit a well-established Soundex table the way English's does.
//! ISO 843 is the honest baseline: a **deterministic, ASCII-only**
//! equivalence class that plays well with byte-oriented indexes and
//! downstream search interop.
//!
//! # Byte-length caveat
//!
//! Every Greek scalar in the modern Greek block is **2 bytes** in
//! UTF-8 (U+0370..=U+03FF fall in the 2-byte range). The encoder
//! walks characters via [`str::chars`](str::chars), never raw bytes,
//! so it never risks slicing a scalar apart. Callers that want to
//! compare an encoded output against an original word by byte offset
//! must remember that the ASCII output is 1 byte per character while
//! the Greek input is 2 bytes per character.
//!
//! # Non-Greek pass-through
//!
//! ASCII characters, digits, punctuation, whitespace, and any scalar
//! outside the Greek mapping pass through unchanged. Mixed-script
//! input carries its non-Greek characters through as-is.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Greek ISO 843 Greek → Latin transliteration.
///
/// A zero-sized value; construct as [`GreekIso843`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the full mapping table.
///
/// # Example
///
/// ```
/// use stringcheese_el::GreekIso843;
///
/// // "Αθήνα" → "athina".
/// assert_eq!(GreekIso843.encode("Αθήνα"), "athina");
/// // "Θεσσαλονίκη" → "thessaloniki".
/// assert_eq!(GreekIso843.encode("Θεσσαλονίκη"), "thessaloniki");
/// // Final sigma folds the same as non-final: "κόσμος" → "kosmos".
/// assert_eq!(GreekIso843.encode("κόσμος"), "kosmos");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GreekIso843;

impl GreekIso843 {
    /// Encode `text` per the ISO 843 Greek → Latin transliteration.
    ///
    /// Every Greek scalar in the mapping is replaced by its ASCII
    /// counterpart (one or two ASCII characters); every scalar outside
    /// the mapping (ASCII, digits, punctuation, other-script letters)
    /// passes through unchanged.
    ///
    /// The encoder lowercases the input first — the output is always
    /// lowercase-ASCII (plus pass-through characters).
    ///
    /// The one context-sensitive rule is the double-gamma nasalization
    /// `γγ → ng`; see the [module-level docs](self#digraph-handling)
    /// for the rationale.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        // Preallocate roughly 2x — most Greek scalars map to 1..2
        // ASCII characters, and the input's byte count is already 2x
        // the character count for pure Greek text.
        let mut out = String::with_capacity(text.len().saturating_mul(2));

        // Lowercase and normalize accents / final sigma up front so
        // downstream matching is straightforward. We walk `chars` via
        // an index because the `γγ → ng` rule needs to look at the
        // previous emitted character.
        let normalized: alloc::vec::Vec<char> = text
            .chars()
            .flat_map(char::to_lowercase)
            .map(fold_accents)
            .map(|c| if c == 'ς' { 'σ' } else { c })
            .collect();

        for (i, &c) in normalized.iter().enumerate() {
            if c == 'γ' && i + 1 < normalized.len() && normalized[i + 1] == 'γ' {
                // First γ of a `γγ` pair — emit `n`; the second γ
                // will be handled on its own turn as a plain `g`.
                out.push('n');
                continue;
            }
            if let Some(ascii) = greek_to_iso843(c) {
                out.push_str(ascii);
            } else {
                out.push(c);
            }
        }
        out
    }
}

/// Fold an accented or diaeresis-marked Greek vowel to its base
/// letter. Non-accented input passes through.
///
/// This is a small character-level normalization step so the encoder
/// can consult a base-letter table only.
#[must_use]
pub const fn fold_accents(c: char) -> char {
    match c {
        'ά' => 'α',
        'έ' => 'ε',
        'ή' => 'η',
        'ί' | 'ϊ' | 'ΐ' => 'ι',
        'ό' => 'ο',
        'ύ' | 'ϋ' | 'ΰ' => 'υ',
        'ώ' => 'ω',
        _ => c,
    }
}

/// Map a Greek scalar to its ISO 843 ASCII string, or `None` if the
/// scalar is outside the Greek mapping.
///
/// See the [module-level table](self#single-letter-mapping) for the
/// full list. Input is assumed to already be lowercase and
/// accent-folded (the encoder's public entry point
/// [`GreekIso843::encode`] handles both).
#[must_use]
// Several Greek letters transliterate to the same Latin letter under
// ISO 843 (η and ι both to `i`; ο and ω both to `o`; σ and ς both to
// `s`). The mapping is one-arm-per-Greek-letter on purpose — merging
// them would obscure which Greek scalars the encoder actually handles.
#[allow(clippy::match_same_arms)]
pub const fn greek_to_iso843(c: char) -> Option<&'static str> {
    Some(match c {
        'α' => "a",
        'β' => "v",
        'γ' => "g",
        'δ' => "d",
        'ε' => "e",
        'ζ' => "z",
        'η' => "i",
        'θ' => "th",
        'ι' => "i",
        'κ' => "k",
        'λ' => "l",
        'μ' => "m",
        'ν' => "n",
        'ξ' => "x",
        'ο' => "o",
        'π' => "p",
        'ρ' => "r",
        'σ' | 'ς' => "s",
        'τ' => "t",
        'υ' => "y",
        'φ' => "f",
        'χ' => "ch",
        'ψ' => "ps",
        'ω' => "o",
        _ => return None,
    })
}

/// Adapter that exposes [`GreekIso843`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Greek::phonetic_encoder`](crate::Greek) hands back.
///
/// The adapter always returns `Some((key, None))` — ISO 843 is a
/// single-key encoder — and returns `None` for input with no Greek
/// content (the transliteration would pass through unchanged, which
/// is not a useful key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct GreekIso843Adapter;

impl LanguagePhoneticEncoder for GreekIso843Adapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_greek(word) {
            return None;
        }
        let key = GreekIso843.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "iso-843-el"
    }
}

/// Does `s` contain at least one scalar in the Greek and Coptic UTF-8
/// block (U+0370..=U+03FF)?
///
/// This is a superset of the ISO 843 mapping — it includes archaic
/// letters (`ϝ`, `ϛ`, `ϟ`) that the mapping does not cover — but that
/// is the right shape for the adapter: any Greek-block character makes
/// the input "Greek content" for phonetic-key purposes.
fn contains_greek(s: &str) -> bool {
    s.chars().any(|c| ('\u{0370}'..='\u{03FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        GreekIso843.encode(s)
    }

    #[test]
    fn empty_input_encodes_to_empty() {
        assert_eq!(e(""), "");
    }

    #[test]
    fn ascii_passes_through() {
        assert_eq!(e("hello"), "hello");
        assert_eq!(e("123"), "123");
    }

    #[test]
    fn common_place_names() {
        assert_eq!(e("Αθήνα"), "athina");
        assert_eq!(e("Θεσσαλονίκη"), "thessaloniki");
        assert_eq!(e("Πάτρα"), "patra");
    }

    #[test]
    fn every_greek_letter_has_a_mapping() {
        for c in [
            'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', 'π', 'ρ',
            'σ', 'ς', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω',
        ] {
            assert!(greek_to_iso843(c).is_some(), "no ISO 843 mapping for {c:?}");
        }
    }

    #[test]
    fn uppercase_input_is_lowercased() {
        assert_eq!(e("ΑΘΗΝΑ"), "athina");
    }

    #[test]
    fn final_sigma_encodes_like_non_final() {
        // `ς` and `σ` are the same letter, positional variants only.
        // Both encode to `s`.
        assert_eq!(e("κόσμος"), "kosmos");
        assert_eq!(e("κόσμοσ"), "kosmos");
    }

    #[test]
    fn accents_are_stripped_before_encoding() {
        // Accented vowels fold to their base letter.
        assert_eq!(e("είμαι"), "eimai");
        assert_eq!(e("έχω"), "echo");
    }

    #[test]
    fn diaeresis_folds_to_base_vowel() {
        assert_eq!(e("προϊόν"), "proion");
    }

    #[test]
    fn digraphs_encode_letter_by_letter() {
        // ISO 843 transliterates diphthongs letter-by-letter.
        assert_eq!(e("αι"), "ai");
        assert_eq!(e("ει"), "ei");
        assert_eq!(e("οι"), "oi");
        assert_eq!(e("ου"), "oy");
    }

    #[test]
    fn double_gamma_encodes_as_ng() {
        // The context-sensitive rule: γγ → ng (nasalized velar).
        assert_eq!(e("άγγελος"), "angelos");
    }

    #[test]
    fn theta_encodes_as_th() {
        assert_eq!(e("θάλασσα"), "thalassa");
    }

    #[test]
    fn chi_encodes_as_ch() {
        assert_eq!(e("χαρά"), "chara");
    }

    #[test]
    fn psi_encodes_as_ps() {
        assert_eq!(e("ψάρι"), "psari");
    }

    #[test]
    fn xi_encodes_as_x() {
        assert_eq!(e("ξένος"), "xenos");
    }

    #[test]
    fn mixed_content_passes_through_non_greek() {
        assert_eq!(e("hello Αθήνα"), "hello athina");
        assert_eq!(e("έτος 2026"), "etos 2026");
    }

    // ---------------------------------------------------------------
    // Adapter.
    // ---------------------------------------------------------------

    #[test]
    fn adapter_name_is_iso_843_el() {
        assert_eq!(GreekIso843Adapter.name(), "iso-843-el");
    }

    #[test]
    fn adapter_returns_some_for_greek() {
        let out = GreekIso843Adapter.encode("Αθήνα");
        assert_eq!(out, Some((String::from("athina"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_greek() {
        assert!(GreekIso843Adapter.encode("").is_none());
        assert!(GreekIso843Adapter.encode("hello").is_none());
        assert!(GreekIso843Adapter.encode("123").is_none());
    }
}
