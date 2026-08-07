//! [`PersianBuckwalter`] — a Persian-adapted Buckwalter transliteration.
//!
//! # Origin
//!
//! Tim Buckwalter's ASCII transliteration is the *de facto* standard
//! machine-readable representation for Arabic. Persian uses the same
//! script with four additional consonants (`پ چ ژ گ`) and two
//! distinct-shape variants of Arabic letters (`ک ی` where Arabic writes
//! `ك ي`), so a "Persian-Buckwalter" table is the natural adaptation —
//! keep the 28 classical Arabic mappings, add ASCII codes for the four
//! Persian additions, and fold the two Persian variants onto the same
//! codes as their Arabic counterparts. That last choice is deliberate:
//! Persian text on the web mixes the Persian and Arabic yeh / kaf
//! constantly, and a phonetic key that collapses them is more useful
//! than one that distinguishes them.
//!
//! # Why Persian-Buckwalter as a phonetic key?
//!
//! Same rationale as the Arabic Buckwalter — the Persian phonology
//! contains several sounds (`ع`, `ح`, `خ`, `ط`, `ض`, `ص`, `ظ`, `ق`,
//! `ء`) that English-first phonetic encoders (Soundex, Metaphone)
//! collapse into approximations that lose the very distinctions
//! Persian IR needs. Buckwalter's ASCII mapping is *deterministic and
//! reversible* — the encoded string is a compact, ASCII-only,
//! lossless-per-scalar equivalence-class key that plays well with the
//! phonetic subsystem's `String` return type.
//!
//! It is not, strictly speaking, a *phonetic* encoding — two words
//! that sound alike do not necessarily encode alike. But it *is* a
//! stable, deterministic, ASCII-only equivalence class the language
//! trait's `phonetic_encoder` accessor accepts.
//!
//! # Mapping — the four Persian additions
//!
//! | Codepoint | Persian | Persian-Buckwalter | Notes                                   |
//! |-----------|---------|--------------------|-----------------------------------------|
//! | U+067E    | `پ`    | `p`                | conflicts with Arabic-Buckwalter's teh marbuta (rare in Persian, not in this map) |
//! | U+0686    | `چ`    | `c`                | novel — unused in Arabic-Buckwalter     |
//! | U+0698    | `ژ`    | `J`                | novel — unused in Arabic-Buckwalter     |
//! | U+06AF    | `گ`    | `g`                | conflicts with Arabic-Buckwalter's ghain (this pack uses `G` for ghain — see below) |
//!
//! Two of these codes collide with existing Arabic-Buckwalter
//! assignments — teh marbuta (`ة`) and ghain (`غ`). Persian text
//! *does* contain ghain (in Arabic loanwords: `مغز` "brain"), so this
//! pack breaks the collision by rewriting ghain as capital `G`. Teh
//! marbuta is not native to Persian and is omitted from the mapping
//! (it passes through unchanged); a caller who processes Persian text
//! quoting Arabic can pre-strip teh marbuta or hand-fold it before
//! encoding.
//!
//! # Mapping — the two Persian variants
//!
//! | Codepoint | Letter  | Persian-Buckwalter | Notes                              |
//! |-----------|---------|--------------------|------------------------------------|
//! | U+06A9    | `ک`    | `k`                | Persian kaf                        |
//! | U+0643    | `ك`    | `k`                | Arabic kaf — same code (folded)    |
//! | U+06CC    | `ی`    | `y`                | Persian yeh                        |
//! | U+064A    | `ي`    | `y`                | Arabic yeh — same code (folded)    |
//! | U+0649    | `ى`    | `Y`                | alef maqsura — separate code       |
//!
//! Encoder folds are Persian yeh + Arabic yeh → `y` and Persian kaf +
//! Arabic kaf → `k`. The *inverse* mapping picks the Persian form
//! for `y` / `k` (the canonical Persian spelling), so a round-trip
//! `inverse(encode(x))` normalizes any Arabic-yeh / Arabic-kaf in `x`
//! to the Persian form.
//!
//! # Mapping — the rest
//!
//! The 28 classical Arabic consonants, hamza forms, diacritics, and
//! tatweel keep their original Buckwalter codes:
//!
//! | Codepoint | Letter | Buckwalter | Codepoint | Letter | Buckwalter |
//! |-----------|--------|------------|-----------|--------|------------|
//! | U+0621    | `ء`   | `'`        | U+0635    | `ص`   | `S`        |
//! | U+0622    | `آ`   | `\|`        | U+0636    | `ض`   | `D`        |
//! | U+0623    | `أ`   | `>`        | U+0637    | `ط`   | `T`        |
//! | U+0624    | `ؤ`   | `&`        | U+0638    | `ظ`   | `Z`        |
//! | U+0625    | `إ`   | `<`        | U+0639    | `ع`   | `E`        |
//! | U+0626    | `ئ`   | `}`        | U+063A    | `غ`   | `G` (Persian-adapted) |
//! | U+0627    | `ا`   | `A`        | U+0640    | `ـ`   | `_`        |
//! | U+0628    | `ب`   | `b`        | U+0641    | `ف`   | `f`        |
//! | U+062A    | `ت`   | `t`        | U+0642    | `ق`   | `q`        |
//! | U+062B    | `ث`   | `v`        | U+0644    | `ل`   | `l`        |
//! | U+062C    | `ج`   | `j`        | U+0645    | `م`   | `m`        |
//! | U+062D    | `ح`   | `H`        | U+0646    | `ن`   | `n`        |
//! | U+062E    | `خ`   | `x`        | U+0647    | `ه`   | `h`        |
//! | U+062F    | `د`   | `d`        | U+0648    | `و`   | `w`        |
//! | U+0630    | `ذ`   | `*`        | U+0670    | `ٰ`   | `` ` ``   |
//! | U+0631    | `ر`   | `r`        | U+064B    | `ً`   | `F`        |
//! | U+0632    | `ز`   | `z`        | U+064C    | `ٌ`   | `N`        |
//! | U+0633    | `س`   | `s`        | U+064D    | `ٍ`   | `K`        |
//! | U+0634    | `ش`   | `$`        | U+064E    | `َ`   | `a`        |
//! |           |        |            | U+064F    | `ُ`   | `u`        |
//! |           |        |            | U+0650    | `ِ`   | `i`        |
//! |           |        |            | U+0651    | `ّ`   | `~`        |
//! |           |        |            | U+0652    | `ْ`   | `o`        |
//!
//! # Non-Persian passes through
//!
//! ASCII characters, digits, punctuation, whitespace, ZWNJ, and any
//! scalar outside the mapping pass through the encoder unchanged. In
//! particular Persian digits `۰..۹` (U+06F0..=U+06F9) are *not*
//! transliterated — a caller who wants Western digits should run
//! [`crate::normalize::PersianNormalizer::with_western_digits`] first.
//!
//! # Inverse (`inverse`)
//!
//! The reverse walk maps every ASCII character in the Persian-Buckwalter
//! alphabet back to exactly one Persian scalar; anything not in the
//! alphabet passes through. The `y` / `k` inverses pick the **Persian**
//! forms (U+06CC yeh, U+06A9 kaf), so a round-trip
//! `inverse(encode(x))` normalizes Arabic-yeh / Arabic-kaf in `x` to
//! the Persian form. Round-trip on the Persian scalars is exact.
//!
//! # RTL note
//!
//! The encoder walks the input in **logical order** (UTF-8 byte
//! order). Each mapped scalar produces one ASCII character; the
//! resulting ASCII string is naturally rendered left-to-right by any
//! renderer, but it represents the Persian word in its
//! first-consonant-first logical order.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Persian-Buckwalter transliteration.
///
/// A zero-sized value; construct as [`PersianBuckwalter`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table.
///
/// # Example
///
/// ```
/// use stringcheese_fa::PersianBuckwalter;
///
/// // "Persian" — پارسی. Persian additions: پ → p.
/// assert_eq!(PersianBuckwalter.encode("پارسی"), "pArsy");
/// // Ali (with Persian yeh) — yeh → y.
/// assert_eq!(PersianBuckwalter.encode("علی"), "Ely");
/// // "book" — Persian kaf → k.
/// assert_eq!(PersianBuckwalter.encode("کتاب"), "ktAb");
/// // "brain" — ghain gets Persian-adapted code G.
/// assert_eq!(PersianBuckwalter.encode("مغز"), "mGz");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PersianBuckwalter;

impl PersianBuckwalter {
    /// Encode `text` per the Persian-Buckwalter transliteration.
    ///
    /// Every Persian scalar in the mapping is replaced by one ASCII
    /// character; every scalar outside the mapping (Persian digits,
    /// ZWNJ, ASCII, other-script letters) passes through unchanged.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            match persian_to_buckwalter(c) {
                Some(ascii) => out.push(ascii),
                None => out.push(c),
            }
        }
        out
    }

    /// Reverse the Persian-Buckwalter transliteration.
    ///
    /// Every ASCII character in the Persian-Buckwalter alphabet maps
    /// back to exactly one Persian scalar; every character outside the
    /// alphabet passes through unchanged. The round-trip
    /// `PersianBuckwalter.inverse(&PersianBuckwalter.encode(x))` equals
    /// `x` for input `x` composed of Persian scalars in the mapping —
    /// with the caveat that Arabic-yeh and Arabic-kaf, if present in
    /// `x`, are normalized to their Persian counterparts.
    #[must_use]
    pub fn inverse(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() * 2);
        for c in text.chars() {
            match buckwalter_to_persian(c) {
                Some(persian) => out.push(persian),
                None => out.push(c),
            }
        }
        out
    }
}

/// Map a Persian scalar to its Persian-Buckwalter ASCII counterpart,
/// or `None` if the scalar is outside the mapping.
///
/// See the [module-level tables](self) for the full list.
#[must_use]
pub const fn persian_to_buckwalter(c: char) -> Option<char> {
    Some(match c {
        // Hamza variants — inherited from Arabic-Buckwalter.
        '\u{0621}' => '\'',
        '\u{0622}' => '|',
        '\u{0623}' => '>',
        '\u{0624}' => '&',
        '\u{0625}' => '<',
        '\u{0626}' => '}',
        // Consonants — 28 classical Arabic.
        '\u{0627}' => 'A',
        '\u{0628}' => 'b',
        // U+0629 teh marbuta — omitted (would collide with پ; not native Persian).
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
        // Ghain — Persian-adapted: capital G (Arabic-Buckwalter used
        // lowercase g, which this pack reassigns to gaf).
        '\u{063A}' => 'G',
        // Tatweel.
        '\u{0640}' => '_',
        // Consonants continued.
        '\u{0641}' => 'f',
        '\u{0642}' => 'q',
        // Arabic kaf (U+0643) and Persian kaf (U+06A9) both encode to
        // the same ASCII code — a deliberate fold; see the module docs.
        '\u{0643}' | '\u{06A9}' => 'k',
        '\u{0644}' => 'l',
        '\u{0645}' => 'm',
        '\u{0646}' => 'n',
        '\u{0647}' => 'h',
        '\u{0648}' => 'w',
        // Alef maqsura — separate code.
        '\u{0649}' => 'Y',
        // Arabic yeh (U+064A) and Persian yeh (U+06CC) both encode to
        // the same ASCII code — a deliberate fold; see the module docs.
        '\u{064A}' | '\u{06CC}' => 'y',
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
        // Persian additions.
        '\u{067E}' => 'p', // peh
        '\u{0686}' => 'c', // cheh
        '\u{0698}' => 'J', // jeh (zhe)
        '\u{06AF}' => 'g', // gaf
        _ => return None,
    })
}

/// Map a Persian-Buckwalter ASCII scalar to its Persian counterpart,
/// or `None` if the scalar is outside the mapping.
///
/// This is *almost* the inverse of [`persian_to_buckwalter`] — the
/// exception is `y` and `k`, which both have two Arabic-script
/// sources (Persian and Arabic yeh / kaf) but a single canonical
/// inverse (the Persian form). Round-tripping an Arabic-yeh or
/// Arabic-kaf input therefore normalizes it to the Persian form.
#[must_use]
pub const fn buckwalter_to_persian(c: char) -> Option<char> {
    Some(match c {
        '\'' => '\u{0621}',
        '|' => '\u{0622}',
        '>' => '\u{0623}',
        '&' => '\u{0624}',
        '<' => '\u{0625}',
        '}' => '\u{0626}',
        'A' => '\u{0627}',
        'b' => '\u{0628}',
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
        // Persian-adapted: capital G decodes to ghain.
        'G' => '\u{063A}',
        '_' => '\u{0640}',
        'f' => '\u{0641}',
        'q' => '\u{0642}',
        // `k` decodes to Persian kaf (canonical).
        'k' => '\u{06A9}',
        'l' => '\u{0644}',
        'm' => '\u{0645}',
        'n' => '\u{0646}',
        'h' => '\u{0647}',
        'w' => '\u{0648}',
        'Y' => '\u{0649}',
        // `y` decodes to Persian yeh (canonical).
        'y' => '\u{06CC}',
        'F' => '\u{064B}',
        'N' => '\u{064C}',
        'K' => '\u{064D}',
        'a' => '\u{064E}',
        'u' => '\u{064F}',
        'i' => '\u{0650}',
        '~' => '\u{0651}',
        'o' => '\u{0652}',
        '`' => '\u{0670}',
        // Persian additions.
        'p' => '\u{067E}', // peh
        'c' => '\u{0686}', // cheh
        'J' => '\u{0698}', // jeh
        'g' => '\u{06AF}', // gaf
        _ => return None,
    })
}

/// Adapter that exposes [`PersianBuckwalter`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Persian::phonetic_encoder`](crate::Persian) hands back.
///
/// The adapter always returns `Some((key, None))` — Persian-Buckwalter
/// is a single-key encoder — and returns `None` for input with no
/// Arabic-script content (the transliteration would pass through
/// unchanged, which is not a phonetic key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PersianBuckwalterAdapter;

impl LanguagePhoneticEncoder for PersianBuckwalterAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_arabic_script(word) {
            return None;
        }
        let key = PersianBuckwalter.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "persian-buckwalter"
    }
}

/// Does `s` contain at least one scalar in the Arabic UTF-8 block
/// (U+0600..=U+06FF)?
///
/// The block covers both Arabic and Persian letters (they share the
/// Unicode block); any letter in the range makes the input
/// "Arabic-script content" for phonetic-key purposes.
fn contains_arabic_script(s: &str) -> bool {
    s.chars().any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        PersianBuckwalter.encode(s)
    }

    fn i(s: &str) -> String {
        PersianBuckwalter.inverse(s)
    }

    // -------------------------------------------------------------
    // Persian additions.
    // -------------------------------------------------------------

    #[test]
    fn encodes_peh() {
        // "Persian" — پارسی.
        assert_eq!(e("پارسی"), "pArsy");
    }

    #[test]
    fn encodes_cheh() {
        // "why" — چرا.
        assert_eq!(e("چرا"), "crA");
    }

    #[test]
    fn encodes_zheh() {
        // "deep" — ژرف.
        assert_eq!(e("ژرف"), "Jrf");
    }

    #[test]
    fn encodes_gaf() {
        // "flower" — گل.
        assert_eq!(e("گل"), "gl");
    }

    // -------------------------------------------------------------
    // Persian yeh / kaf variants — folded onto same codes.
    // -------------------------------------------------------------

    #[test]
    fn persian_and_arabic_yeh_encode_the_same() {
        // علی (Persian yeh) vs. علي (Arabic yeh) — both encode to Ely.
        assert_eq!(e("علی"), "Ely");
        assert_eq!(e("علي"), "Ely");
    }

    #[test]
    fn persian_and_arabic_kaf_encode_the_same() {
        assert_eq!(e("کتاب"), "ktAb"); // Persian kaf
        assert_eq!(e("كتاب"), "ktAb"); // Arabic kaf
    }

    // -------------------------------------------------------------
    // Ghain — Persian-adapted code.
    // -------------------------------------------------------------

    #[test]
    fn encodes_ghain_as_capital_g() {
        // "brain" — مغز.
        assert_eq!(e("مغز"), "mGz");
    }

    // -------------------------------------------------------------
    // Round-trip for Persian scalars.
    // -------------------------------------------------------------

    #[test]
    fn round_trip_persian_specific_scalars() {
        // Every Persian-only scalar round-trips.
        for c in [
            '\u{067E}', '\u{0686}', '\u{0698}', '\u{06A9}', '\u{06AF}', '\u{06CC}',
        ] {
            let ascii = persian_to_buckwalter(c).unwrap();
            let back = buckwalter_to_persian(ascii).unwrap();
            assert_eq!(
                back, c,
                "Persian {c:?} → {ascii:?} → {back:?} did not round-trip"
            );
        }
    }

    #[test]
    fn round_trip_ghain_via_capital_g() {
        assert_eq!(persian_to_buckwalter('\u{063A}'), Some('G'));
        assert_eq!(buckwalter_to_persian('G'), Some('\u{063A}'));
    }

    #[test]
    fn round_trip_persian_words() {
        for w in ["پارسی", "چرا", "ژرف", "گل", "کتاب", "علی", "مغز"] {
            let enc = e(w);
            let back = i(&enc);
            assert_eq!(back, w, "round-trip failed on {w:?}");
        }
    }

    #[test]
    fn round_trip_arabic_kaf_normalizes_to_persian() {
        // Inverse of an Arabic-kaf input yields Persian kaf (the
        // canonical inverse).
        let enc = e("كتاب"); // Arabic kaf
        assert_eq!(enc, "ktAb");
        let back = i(&enc);
        assert_eq!(back, "کتاب"); // Persian kaf
    }

    #[test]
    fn round_trip_arabic_yeh_normalizes_to_persian() {
        let enc = e("علي"); // Arabic yeh
        assert_eq!(enc, "Ely");
        let back = i(&enc);
        assert_eq!(back, "علی"); // Persian yeh
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
    fn persian_digits_pass_through_unchanged() {
        // Digits are outside the letter mapping.
        assert_eq!(e("۲۰۲۴"), "۲۰۲۴");
    }

    #[test]
    fn zwnj_passes_through_unchanged() {
        // ZWNJ is outside the mapping.
        assert_eq!(e("می\u{200C}روم"), "my\u{200C}rwm");
    }

    #[test]
    fn mixed_persian_and_ascii() {
        assert_eq!(e("hello پارسی"), "hello pArsy");
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_persian_buckwalter() {
        assert_eq!(PersianBuckwalterAdapter.name(), "persian-buckwalter");
    }

    #[test]
    fn adapter_returns_some_for_persian() {
        let out = PersianBuckwalterAdapter.encode("پارسی");
        assert_eq!(out, Some((String::from("pArsy"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_arabic_script() {
        assert!(PersianBuckwalterAdapter.encode("").is_none());
        assert!(PersianBuckwalterAdapter.encode("hello").is_none());
        assert!(PersianBuckwalterAdapter.encode("123").is_none());
    }
}
