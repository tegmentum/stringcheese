//! [`Buckwalter`] — the Buckwalter Arabic transliteration.
//!
//! # Origin
//!
//! Tim Buckwalter's ASCII transliteration is the *de facto* standard
//! machine-readable representation for Arabic. Every scalar in the
//! Arabic UTF-8 block that carries lexical information (letters,
//! hamza variants, teh marbuta, diacritics) maps to exactly one ASCII
//! character. The mapping is a **bijection** on the Arabic-letter
//! subset that Buckwalter defined — every ASCII character in the
//! output maps back to exactly one Arabic character, so
//! [`Buckwalter::inverse`] round-trips the encoding.
//!
//! # Why Buckwalter as a phonetic key?
//!
//! The Arabic script has ~35 letters, several of which do not
//! correspond to any single Latin phoneme (`ع`, `ح`, `خ`, `ط`, `ض`,
//! `ص`, `ظ`, `ق`, `ء`). English-first phonetic encoders (Soundex,
//! Metaphone) collapse those into approximations that lose the very
//! distinctions Arabic IR needs. Buckwalter's mapping is *deterministic
//! and reversible* — the encoded string is a compact, ASCII-only,
//! lossless equivalence-class key that plays well with the phonetic
//! subsystem's `String` return type and with any downstream
//! byte-oriented index.
//!
//! It is not, strictly speaking, a *phonetic* encoding — two words
//! that sound alike do not necessarily encode alike. But it *is* a
//! stable, deterministic, ASCII-only equivalence class the language
//! trait's `phonetic_encoder` accessor accepts; and for Arabic that is
//! the honest baseline. A future `stringcheese-ar` release could add a
//! true phonetic encoder (`AraSoundex`, ISRI, or the like) alongside
//! Buckwalter.
//!
//! # Mapping
//!
//! The full table (Arabic scalar → ASCII character):
//!
//! | Codepoint | Arabic | Buckwalter |
//! |-----------|--------|------------|
//! | U+0621    | `ء`   | `'`        |
//! | U+0622    | `آ`   | `|`        |
//! | U+0623    | `أ`   | `>`        |
//! | U+0624    | `ؤ`   | `&`        |
//! | U+0625    | `إ`   | `<`        |
//! | U+0626    | `ئ`   | `}`        |
//! | U+0627    | `ا`   | `A`        |
//! | U+0628    | `ب`   | `b`        |
//! | U+0629    | `ة`   | `p`        |
//! | U+062A    | `ت`   | `t`        |
//! | U+062B    | `ث`   | `v`        |
//! | U+062C    | `ج`   | `j`        |
//! | U+062D    | `ح`   | `H`        |
//! | U+062E    | `خ`   | `x`        |
//! | U+062F    | `د`   | `d`        |
//! | U+0630    | `ذ`   | `*`        |
//! | U+0631    | `ر`   | `r`        |
//! | U+0632    | `ز`   | `z`        |
//! | U+0633    | `س`   | `s`        |
//! | U+0634    | `ش`   | `$`        |
//! | U+0635    | `ص`   | `S`        |
//! | U+0636    | `ض`   | `D`        |
//! | U+0637    | `ط`   | `T`        |
//! | U+0638    | `ظ`   | `Z`        |
//! | U+0639    | `ع`   | `E`        |
//! | U+063A    | `غ`   | `g`        |
//! | U+0640    | `ـ`   | `_`        |
//! | U+0641    | `ف`   | `f`        |
//! | U+0642    | `ق`   | `q`        |
//! | U+0643    | `ك`   | `k`        |
//! | U+0644    | `ل`   | `l`        |
//! | U+0645    | `م`   | `m`        |
//! | U+0646    | `ن`   | `n`        |
//! | U+0647    | `ه`   | `h`        |
//! | U+0648    | `و`   | `w`        |
//! | U+0649    | `ى`   | `Y`        |
//! | U+064A    | `ي`   | `y`        |
//! | U+064B    | `ً`   | `F`        |
//! | U+064C    | `ٌ`   | `N`        |
//! | U+064D    | `ٍ`   | `K`        |
//! | U+064E    | `َ`   | `a`        |
//! | U+064F    | `ُ`   | `u`        |
//! | U+0650    | `ِ`   | `i`        |
//! | U+0651    | `ّ`   | `~`        |
//! | U+0652    | `ْ`   | `o`        |
//! | U+0670    | `ٰ`   | `\``       |
//!
//! # Non-Arabic passes through
//!
//! ASCII characters, digits, punctuation, whitespace, and any scalar
//! outside the mapping pass through the encoder unchanged. The output
//! is guaranteed ASCII when the input is entirely composed of Arabic
//! scalars in the mapping table — mixed-script input carries its
//! non-Arabic characters over as-is.
//!
//! # Inverse (`unbuckwalter`)
//!
//! The mapping is a bijection on the ASCII subset the forward mapping
//! produces. [`Buckwalter::inverse`] does the reverse walk — every
//! ASCII character in the Buckwalter alphabet maps back to exactly one
//! Arabic scalar; anything not in the alphabet passes through. This
//! makes the round-trip `inverse(encode(x))` equal to `x` for input
//! that is entirely Arabic-scalar-in-the-mapping (and equal to `x`
//! for pure-ASCII input that avoids the Buckwalter alphabet).
//!
//! # RTL note
//!
//! The encoder walks the input in **logical order** (UTF-8 byte
//! order). Each Arabic scalar produces one ASCII character; the
//! resulting ASCII string is naturally rendered left-to-right by any
//! renderer, but it represents the Arabic word in its
//! first-consonant-first logical order — which is what a matching /
//! indexing pipeline wants.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Buckwalter Arabic transliteration.
///
/// A zero-sized value; construct as [`Buckwalter`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the full mapping table.
///
/// # Example
///
/// ```
/// use stringcheese_ar::Buckwalter;
///
/// // Muhammad — the standard reference example.
/// assert_eq!(Buckwalter.encode("محمد"), "mHmd");
/// // Ali (with the terminal yeh spelling).
/// assert_eq!(Buckwalter.encode("علي"), "Ely");
/// // With hamza above alef — Ahmad.
/// assert_eq!(Buckwalter.encode("أحمد"), ">Hmd");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Buckwalter;

impl Buckwalter {
    /// Encode `text` per the Buckwalter transliteration.
    ///
    /// Every Arabic scalar in the mapping is replaced by one ASCII
    /// character; every scalar outside the mapping (ASCII, digits,
    /// punctuation, other-script letters) passes through unchanged.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            match arabic_to_buckwalter(c) {
                Some(ascii) => out.push(ascii),
                None => out.push(c),
            }
        }
        out
    }

    /// Reverse the Buckwalter transliteration.
    ///
    /// Every ASCII character in the Buckwalter alphabet maps back to
    /// exactly one Arabic scalar; every character outside the alphabet
    /// passes through unchanged. The round-trip
    /// `Buckwalter.inverse(&Buckwalter.encode(x))` equals `x` for
    /// input `x` composed entirely of Arabic scalars in the mapping
    /// table (and for pure-ASCII input that avoids the Buckwalter
    /// alphabet).
    #[must_use]
    pub fn inverse(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() * 2);
        for c in text.chars() {
            match buckwalter_to_arabic(c) {
                Some(arabic) => out.push(arabic),
                None => out.push(c),
            }
        }
        out
    }
}

/// Map an Arabic scalar to its Buckwalter ASCII counterpart, or `None`
/// if the scalar is outside the mapping.
///
/// See the [module-level table](self#mapping) for the full list.
#[must_use]
pub const fn arabic_to_buckwalter(c: char) -> Option<char> {
    Some(match c {
        // Hamza variants.
        '\u{0621}' => '\'',
        '\u{0622}' => '|',
        '\u{0623}' => '>',
        '\u{0624}' => '&',
        '\u{0625}' => '<',
        '\u{0626}' => '}',
        // Consonants.
        '\u{0627}' => 'A',
        '\u{0628}' => 'b',
        '\u{0629}' => 'p',
        '\u{062A}' => 't',
        '\u{062B}' => 'v',
        '\u{062C}' => 'j',
        '\u{062D}' => 'H',
        '\u{062E}' => 'x',
        '\u{062F}' => 'd',
        '\u{0630}' => '*',
        '\u{0631}' => 'r',
        '\u{0632}' => 'z',
        '\u{0633}' => 's',
        '\u{0634}' => '$',
        '\u{0635}' => 'S',
        '\u{0636}' => 'D',
        '\u{0637}' => 'T',
        '\u{0638}' => 'Z',
        '\u{0639}' => 'E',
        '\u{063A}' => 'g',
        // Tatweel.
        '\u{0640}' => '_',
        // Consonants continued.
        '\u{0641}' => 'f',
        '\u{0642}' => 'q',
        '\u{0643}' => 'k',
        '\u{0644}' => 'l',
        '\u{0645}' => 'm',
        '\u{0646}' => 'n',
        '\u{0647}' => 'h',
        '\u{0648}' => 'w',
        '\u{0649}' => 'Y',
        '\u{064A}' => 'y',
        // Diacritics (tanween + short vowels + shadda + sukun).
        '\u{064B}' => 'F',
        '\u{064C}' => 'N',
        '\u{064D}' => 'K',
        '\u{064E}' => 'a',
        '\u{064F}' => 'u',
        '\u{0650}' => 'i',
        '\u{0651}' => '~',
        '\u{0652}' => 'o',
        // Dagger alef.
        '\u{0670}' => '`',
        _ => return None,
    })
}

/// Map a Buckwalter ASCII scalar to its Arabic counterpart, or `None`
/// if the scalar is outside the mapping.
///
/// This is the inverse of [`arabic_to_buckwalter`].
#[must_use]
pub const fn buckwalter_to_arabic(c: char) -> Option<char> {
    Some(match c {
        '\'' => '\u{0621}',
        '|' => '\u{0622}',
        '>' => '\u{0623}',
        '&' => '\u{0624}',
        '<' => '\u{0625}',
        '}' => '\u{0626}',
        'A' => '\u{0627}',
        'b' => '\u{0628}',
        'p' => '\u{0629}',
        't' => '\u{062A}',
        'v' => '\u{062B}',
        'j' => '\u{062C}',
        'H' => '\u{062D}',
        'x' => '\u{062E}',
        'd' => '\u{062F}',
        '*' => '\u{0630}',
        'r' => '\u{0631}',
        'z' => '\u{0632}',
        's' => '\u{0633}',
        '$' => '\u{0634}',
        'S' => '\u{0635}',
        'D' => '\u{0636}',
        'T' => '\u{0637}',
        'Z' => '\u{0638}',
        'E' => '\u{0639}',
        'g' => '\u{063A}',
        '_' => '\u{0640}',
        'f' => '\u{0641}',
        'q' => '\u{0642}',
        'k' => '\u{0643}',
        'l' => '\u{0644}',
        'm' => '\u{0645}',
        'n' => '\u{0646}',
        'h' => '\u{0647}',
        'w' => '\u{0648}',
        'Y' => '\u{0649}',
        'y' => '\u{064A}',
        'F' => '\u{064B}',
        'N' => '\u{064C}',
        'K' => '\u{064D}',
        'a' => '\u{064E}',
        'u' => '\u{064F}',
        'i' => '\u{0650}',
        '~' => '\u{0651}',
        'o' => '\u{0652}',
        '`' => '\u{0670}',
        _ => return None,
    })
}

/// Adapter that exposes [`Buckwalter`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Arabic::phonetic_encoder`](crate::Arabic) hands back.
///
/// The adapter always returns `Some((key, None))` — Buckwalter is a
/// single-key encoder — and returns `None` for input with no Arabic
/// content (the transliteration would pass through unchanged, which is
/// not a phonetic key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BuckwalterAdapter;

impl LanguagePhoneticEncoder for BuckwalterAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_arabic(word) {
            return None;
        }
        let key = Buckwalter.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "buckwalter"
    }
}

/// Does `s` contain at least one scalar in the Arabic UTF-8 block
/// (U+0600..=U+06FF)?
///
/// This is a *superset* of the Buckwalter mapping — it includes
/// scalars the mapping does not cover (Arabic digits U+0660..=U+0669,
/// punctuation like the Arabic comma U+060C, etc.) — but that is the
/// right shape for the adapter: any Arabic-block character makes the
/// input "Arabic content" for phonetic-key purposes.
fn contains_arabic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        Buckwalter.encode(s)
    }

    fn i(s: &str) -> String {
        Buckwalter.inverse(s)
    }

    // -------------------------------------------------------------
    // Canonical reference names.
    // -------------------------------------------------------------

    #[test]
    fn encodes_muhammad() {
        assert_eq!(e("محمد"), "mHmd");
    }

    #[test]
    fn encodes_ali() {
        assert_eq!(e("علي"), "Ely");
    }

    #[test]
    fn encodes_ahmad_with_hamza() {
        // أ (hamza above alef) encodes to '>'.
        assert_eq!(e("أحمد"), ">Hmd");
    }

    #[test]
    fn encodes_ibrahim_with_hamza_below() {
        assert_eq!(e("إبراهيم"), "<brAhym");
    }

    #[test]
    fn encodes_omar() {
        assert_eq!(e("عمر"), "Emr");
    }

    #[test]
    fn encodes_yusuf() {
        assert_eq!(e("يوسف"), "ywsf");
    }

    #[test]
    fn encodes_fatima_with_teh_marbuta() {
        // ة encodes to 'p'.
        assert_eq!(e("فاطمة"), "fATmp");
    }

    // -------------------------------------------------------------
    // Mapping totality — every table entry round-trips.
    // -------------------------------------------------------------

    #[test]
    fn forward_reverse_round_trip_for_every_mapped_scalar() {
        for cp in 0x0621u32..=0x0655 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if let Some(a) = arabic_to_buckwalter(c) {
                let back = buckwalter_to_arabic(a);
                assert_eq!(
                    back,
                    Some(c),
                    "arabic {c:?} → buckwalter {a:?} → {back:?} did not round-trip"
                );
            }
        }
        // Dagger alef.
        let d = '\u{0670}';
        let a = arabic_to_buckwalter(d).unwrap();
        assert_eq!(buckwalter_to_arabic(a), Some(d));
        // Tatweel.
        let t = '\u{0640}';
        let a = arabic_to_buckwalter(t).unwrap();
        assert_eq!(buckwalter_to_arabic(a), Some(t));
    }

    // -------------------------------------------------------------
    // Diacritics.
    // -------------------------------------------------------------

    #[test]
    fn encodes_fully_vocalized_muhammad() {
        // مُحَمَّد, encoded with the source-file's combining-mark order.
        // The encoder walks scalars in the order the input stores
        // them; Unicode canonical combining class (fatha U+064E ccc=30,
        // shadda U+0651 ccc=33) causes NFC normalization to place
        // fatha before shadda, so this reference reflects the NFC
        // spelling emitted by any modern text editor.
        assert_eq!(e("مُحَمَّد"), "muHama~d");
    }

    // -------------------------------------------------------------
    // Pass-through.
    // -------------------------------------------------------------

    #[test]
    fn ascii_passes_through() {
        assert_eq!(e("hello"), "hello");
        assert_eq!(e(""), "");
    }

    #[test]
    fn mixed_arabic_and_ascii() {
        assert_eq!(e("hello محمد"), "hello mHmd");
    }

    // -------------------------------------------------------------
    // Inverse.
    // -------------------------------------------------------------

    #[test]
    fn inverse_of_reference_names() {
        assert_eq!(i("mHmd"), "محمد");
        assert_eq!(i(">Hmd"), "أحمد");
        assert_eq!(i("fATmp"), "فاطمة");
    }

    #[test]
    fn round_trip_reference_names() {
        for w in ["محمد", "أحمد", "علي", "فاطمة", "إبراهيم", "يوسف"] {
            let enc = e(w);
            let back = i(&enc);
            assert_eq!(back, w, "round-trip failed on {w:?}");
        }
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_buckwalter() {
        assert_eq!(BuckwalterAdapter.name(), "buckwalter");
    }

    #[test]
    fn adapter_returns_some_for_arabic() {
        let out = BuckwalterAdapter.encode("محمد");
        assert_eq!(out, Some((String::from("mHmd"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_arabic() {
        assert!(BuckwalterAdapter.encode("").is_none());
        assert!(BuckwalterAdapter.encode("hello").is_none());
        assert!(BuckwalterAdapter.encode("123").is_none());
    }
}
