//! Bengali → ISO 15919 → PHONEX-Bengali phonetic encoder.
//!
//! # Origin
//!
//! Bengali is the second Brahmic-script pack in StringCheese (after
//! Hindi's Devanagari). Two candidate approaches present themselves
//! for a Bengali phonetic hookup:
//!
//! * **ISO 15919 transliteration.** The international-standard
//!   Indic-script romanization (successor to IAST, with diacritics
//!   for retroflex consonants `ṭ ṭh ḍ ḍh ṇ`, sibilants `ś ṣ`,
//!   long vowels `ā ī ū`, and the Bengali-specific khanda ta `ৎ`
//!   rendered `t̪`). Faithful to the script but not a
//!   *phonetic-equivalence* encoder — two words that sound alike
//!   don't necessarily transliterate alike (Bengali's active
//!   schwa-deletion makes `রাম` "rām" pronounced without a final
//!   schwa, but the transliteration retains it as `rāma`).
//! * **PHONEX-Bengali.** Transliterate to Latin first, then apply
//!   a Soundex-shape 4-character reduction that folds long / short
//!   vowels, drops retroflex under-dots, and collapses
//!   consonant-class duplicates. Produces sound-alike equivalence
//!   classes and matches the shape of the other Latin-alphabet
//!   packs' phonetic hookups (Czech, Dutch, French, Portuguese,
//!   Spanish, Swedish, Norwegian, Finnish, Polish, Hungarian,
//!   Vietnamese, Slovak — all PHONEX-family).
//!
//! # Implementation choice — two-stage: ISO 15919 then PHONEX
//!
//! This module ships both stages:
//!
//! 1. **[`BengaliIso15919`]** — the deterministic
//!    Bengali → Latin transliteration, honoring the inherent-schwa
//!    convention (see below). Public because it's useful in its own
//!    right (data-migration tools, IR display).
//! 2. **[`BengaliPhonex`]** — the 4-character Soundex-shape key
//!    computed *over* the ISO 15919 output. This is the
//!    encoder [`BengaliPhonexAdapter`] wraps for the
//!    [`LanguagePhoneticEncoder`] trait hookup; adapter name
//!    `"phonex-bn"`.
//!
//! # Inherent-vowel (schwa) handling — the crucial subtlety
//!
//! Bengali is an **abugida** — every base consonant carries an
//! implicit `a` (schwa, phonetically /ɔ/ in Bengali though we
//! romanize as `a` per ISO 15919) vowel unless a dependent vowel
//! sign (matra / kar) or a halant (`্` U+09CD) overrides it. So
//! `ক` alone represents `ka` (not just `k`), `ক্` (`ক` + halant)
//! represents bare `k`, and `কি` (`ক` + matra `ি`) represents
//! `ki`.
//!
//! The transliteration honors the inherent schwa **without applying
//! schwa-deletion rules**. Modern colloquial Bengali very actively
//! drops the schwa in word-final and many medial positions
//! (`রাম` is pronounced `ram`, not `rama`; `কমল` is `komol`, not
//! `kamala`), but the deletion is context-dependent and
//! lexicon-driven — the surface script writes the schwa either way.
//! The ISO 15919 convention this module follows is the classical
//! **explicit-schwa** rendering; the follow-up phonex step compresses
//! the vowel-carrying letters to zero-class digits anyway, so
//! schwa-retained vs. schwa-deleted spellings produce the same key.
//!
//! # ISO 15919 mapping table
//!
//! ## Independent vowels
//!
//! | Bengali | ISO 15919 | Notes                    |
//! |---------|-----------|--------------------------|
//! | `অ`     | `a`       | schwa                    |
//! | `আ`     | `ā`       | long a                   |
//! | `ই`     | `i`       | short i                  |
//! | `ঈ`     | `ī`       | long i                   |
//! | `উ`     | `u`       | short u                  |
//! | `ঊ`     | `ū`       | long u                   |
//! | `ঋ`     | `r̥`       | vocalic r                |
//! | `এ`     | `e`       |                          |
//! | `ঐ`     | `ai`      |                          |
//! | `ও`     | `o`       |                          |
//! | `ঔ`     | `au`      |                          |
//!
//! ## Dependent vowel signs (matras / kars) — appear after a consonant
//!
//! | Bengali | ISO 15919 | Notes                    |
//! |---------|-----------|--------------------------|
//! | (none)  | `a`       | inherent schwa           |
//! | `া`     | `ā`       |                          |
//! | `ি`     | `i`       |                          |
//! | `ী`     | `ī`       |                          |
//! | `ু`     | `u`       |                          |
//! | `ূ`     | `ū`       |                          |
//! | `ৃ`     | `r̥`       |                          |
//! | `ে`     | `e`       |                          |
//! | `ৈ`     | `ai`      |                          |
//! | `ো`     | `o`       |                          |
//! | `ৌ`     | `au`      |                          |
//!
//! ## Halant and other marks
//!
//! | Bengali | ISO 15919 | Notes                                   |
//! |---------|-----------|-----------------------------------------|
//! | `্`     | (∅)       | halant — suppresses inherent schwa      |
//! | `ং`     | `ṁ`       | anusvara                                |
//! | `ঁ`     | `m̐`       | chandrabindu                            |
//! | `ঃ`     | `ḥ`       | visarga                                 |
//! | `ৎ`     | `t`       | khanda ta (final t; ISO uses `t̪`)       |
//! | `়`     | (∅)       | nukta — passed through; the precomposed |
//! |         |           | nukta letters (`ড়`/`ঢ়`/`য়`) have         |
//! |         |           | their own entries                       |
//!
//! ## The 33 base consonants
//!
//! Same phonological groups as Devanagari, different scalars.
//!
//! | Bengali | ISO 15919 | Group          |
//! |---------|-----------|----------------|
//! | `ক`     | `k`       | velar          |
//! | `খ`     | `kh`      |                |
//! | `গ`     | `g`       |                |
//! | `ঘ`     | `gh`      |                |
//! | `ঙ`     | `ṅ`       | velar nasal    |
//! | `চ`     | `c`       | palatal        |
//! | `ছ`     | `ch`      |                |
//! | `জ`     | `j`       |                |
//! | `ঝ`     | `jh`      |                |
//! | `ঞ`     | `ñ`       | palatal nasal  |
//! | `ট`     | `ṭ`       | retroflex      |
//! | `ঠ`     | `ṭh`      |                |
//! | `ড`     | `ḍ`       |                |
//! | `ঢ`     | `ḍh`      |                |
//! | `ণ`     | `ṇ`       | retroflex nasal|
//! | `ত`     | `t`       | dental         |
//! | `থ`     | `th`      |                |
//! | `দ`     | `d`       |                |
//! | `ধ`     | `dh`      |                |
//! | `ন`     | `n`       | dental nasal   |
//! | `প`     | `p`       | labial         |
//! | `ফ`     | `ph`      |                |
//! | `ব`     | `b`       |                |
//! | `ভ`     | `bh`      |                |
//! | `ম`     | `m`       | labial nasal   |
//! | `য`     | `y`       | palatal glide  |
//! | `র`     | `r`       |                |
//! | `ল`     | `l`       |                |
//! | `শ`     | `ś`       | sibilant       |
//! | `ষ`     | `ṣ`       |                |
//! | `স`     | `s`       |                |
//! | `হ`     | `h`       |                |
//!
//! ## Nukta letters (Bengali extensions)
//!
//! Both the precomposed forms and the decomposed base + `়` forms
//! are handled.
//!
//! | Bengali | ISO 15919 | Notes                    |
//! |---------|-----------|--------------------------|
//! | `ড়` / `ড` + `়` | `ṛ` | rra (retroflex flap)   |
//! | `ঢ়` / `ঢ` + `়` | `ṛh`| rha (breathy)          |
//! | `য়` / `য` + `়` | `ẏ` | yya (palatal glide y)  |
//!
//! ## Digits
//!
//! Bengali digits `০..৯` (U+09E6..=U+09EF) map to ASCII digits
//! `0..9`.
//!
//! # PHONEX-Bengali reduction
//!
//! After transliteration, the encoder applies a Soundex-shape
//! 4-character reduction, matching the shape of the other
//! Latin-alphabet packs:
//!
//! 1. Fold Latin-with-diacritic scalars to their ASCII base
//!    (`ā → a`, `ī → i`, `ū → u`, `ṭ → t`, `ḍ → d`, `ṇ → n`,
//!    `ṅ → n`, `ñ → n`, `ś → s`, `ṣ → s`, `ṛ → r`, `ẏ → y`,
//!    `ṁ → m`, `ḥ → h`, `r̥ → r`), uppercase everything.
//! 2. Take the first ASCII letter as the seed.
//! 3. Walk subsequent letters, mapping each to its Soundex
//!    consonant class (`B/P/F/V/W = 1`, `C/K/G/Q/J/X = 2`,
//!    `D/T = 3`, `L = 4`, `M/N = 5`, `R = 6`, `S/Z = 7`;
//!    vowels reset the duplicate-collapse state).
//! 4. Collapse consecutive equal codes; pad to 4 characters with
//!    `'0'`.
//!
//! Adapter name: `"phonex-bn"`.
//!
//! # Byte-length caveat
//!
//! Every Bengali scalar is **3 bytes** in UTF-8 (U+0980..=U+09FF
//! falls in the 3-byte range). The transliterator walks characters
//! via [`str::chars`], never raw bytes, so it never risks slicing a
//! scalar apart. The output uses ASCII plus a small set of
//! Latin-with-diacritic scalars.
//!
//! # Non-Bengali pass-through
//!
//! ASCII characters, whitespace, punctuation, and any scalar
//! outside the Bengali block pass through the transliterator
//! unchanged. The phonex reduction then filters non-letters out.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Bengali → ISO 15919 transliterator.
///
/// A zero-sized value; construct as [`BengaliIso15919`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and
/// the discussion of inherent-vowel (schwa) handling.
///
/// # Example
///
/// ```
/// use stringcheese_bn::BengaliIso15919;
///
/// // "namaste" — ন + ম + স + ্ + ত + ে.
/// // The halant suppresses the schwa on স; the ে matra provides
/// // the final vowel on ত.
/// assert_eq!(BengaliIso15919.encode("নমস্তে"), "namaste");
/// // "Rama" — র + া + ম. The final ম has an inherent schwa.
/// assert_eq!(BengaliIso15919.encode("রাম"), "rāma");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BengaliIso15919;

impl BengaliIso15919 {
    /// Encode `text` per the Bengali → ISO 15919 transliteration.
    ///
    /// Every Bengali scalar in the mapping is replaced by its ISO
    /// 15919 counterpart; every scalar outside the mapping (ASCII,
    /// whitespace, other-script letters) passes through unchanged.
    ///
    /// The encoder honors the inherent-schwa convention: a base
    /// consonant with no following matra or halant emits the
    /// consonant letter followed by `a`; a following matra
    /// overrides the `a`; a following halant suppresses it.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len().saturating_add(8));

        let mut chars = text.chars().peekable();
        let mut pending_consonant: Option<&'static str> = None;
        let mut pending_nukta = false;

        while let Some(c) = chars.next() {
            let next = chars.peek().copied();

            if let Some(base) = pending_consonant.take() {
                if c == '\u{09BC}' && !pending_nukta {
                    if let Some(nukta_form) = nukta_variant(base) {
                        pending_consonant = Some(nukta_form);
                    } else {
                        pending_consonant = Some(base);
                    }
                    pending_nukta = true;
                    continue;
                }

                pending_nukta = false;

                // Halant suppresses the schwa.
                if c == '\u{09CD}' {
                    out.push_str(base);
                    continue;
                }

                if let Some(matra) = matra_iso(c) {
                    out.push_str(base);
                    out.push_str(matra);
                    continue;
                }

                if let Some(mark) = combining_mark_iso(c) {
                    out.push_str(base);
                    out.push('a');
                    out.push_str(mark);
                    continue;
                }

                out.push_str(base);
                out.push('a');
                // Fall through.
            }

            if let Some(v) = independent_vowel_iso(c) {
                out.push_str(v);
                continue;
            }

            if let Some(cons) = consonant_iso(c) {
                pending_consonant = Some(cons);
                pending_nukta = false;
                if next.is_none() {
                    out.push_str(cons);
                    out.push('a');
                    pending_consonant = None;
                }
                continue;
            }

            // Khanda ta (ৎ) — final t, never carries a schwa.
            if c == '\u{09CE}' {
                out.push('t');
                continue;
            }

            if let Some(d) = digit_iso(c) {
                out.push(d);
                continue;
            }

            if let Some(mark) = combining_mark_iso(c) {
                out.push_str(mark);
                continue;
            }

            if let Some(matra) = matra_iso(c) {
                out.push_str(matra);
                continue;
            }

            if c == '\u{09CD}' {
                continue;
            }

            if c == '\u{09BC}' {
                continue;
            }

            out.push(c);
        }

        if let Some(base) = pending_consonant.take() {
            out.push_str(base);
            out.push('a');
        }

        out
    }
}

/// Map an independent vowel scalar to its ISO 15919 string.
#[must_use]
pub const fn independent_vowel_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0985}' => "a",  // অ
        '\u{0986}' => "ā",  // আ
        '\u{0987}' => "i",  // ই
        '\u{0988}' => "ī",  // ঈ
        '\u{0989}' => "u",  // উ
        '\u{098A}' => "ū",  // ঊ
        '\u{098B}' => "r̥",  // ঋ
        '\u{098F}' => "e",  // এ
        '\u{0990}' => "ai", // ঐ
        '\u{0993}' => "o",  // ও
        '\u{0994}' => "au", // ঔ
        _ => return None,
    })
}

/// Map a dependent vowel sign (matra / kar) scalar to its ISO 15919
/// string.
#[must_use]
pub const fn matra_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{09BE}' => "ā",  // া
        '\u{09BF}' => "i",  // ি
        '\u{09C0}' => "ī",  // ী
        '\u{09C1}' => "u",  // ু
        '\u{09C2}' => "ū",  // ূ
        '\u{09C3}' => "r̥",  // ৃ
        '\u{09C7}' => "e",  // ে
        '\u{09C8}' => "ai", // ৈ
        '\u{09CB}' => "o",  // ো
        '\u{09CC}' => "au", // ৌ
        _ => return None,
    })
}

/// Map a combining vowel-modifier scalar to its ISO 15919 string.
#[must_use]
pub const fn combining_mark_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0982}' => "ṁ", // ং anusvara
        '\u{0981}' => "m̐", // ঁ chandrabindu
        '\u{0983}' => "ḥ", // ঃ visarga
        _ => return None,
    })
}

/// Map a base consonant scalar to its ISO 15919 string.
#[must_use]
pub const fn consonant_iso(c: char) -> Option<&'static str> {
    Some(match c {
        // Velars.
        '\u{0995}' => "k",  // ক
        '\u{0996}' => "kh", // খ
        '\u{0997}' => "g",  // গ
        '\u{0998}' => "gh", // ঘ
        '\u{0999}' => "ṅ",  // ঙ
        // Palatals.
        '\u{099A}' => "c",  // চ
        '\u{099B}' => "ch", // ছ
        '\u{099C}' => "j",  // জ
        '\u{099D}' => "jh", // ঝ
        '\u{099E}' => "ñ",  // ঞ
        // Retroflexes.
        '\u{099F}' => "ṭ",  // ট
        '\u{09A0}' => "ṭh", // ঠ
        '\u{09A1}' => "ḍ",  // ড
        '\u{09A2}' => "ḍh", // ঢ
        '\u{09A3}' => "ṇ",  // ণ
        // Dentals.
        '\u{09A4}' => "t",  // ত
        '\u{09A5}' => "th", // থ
        '\u{09A6}' => "d",  // দ
        '\u{09A7}' => "dh", // ধ
        '\u{09A8}' => "n",  // ন
        // Labials.
        '\u{09AA}' => "p",  // প
        '\u{09AB}' => "ph", // ফ
        '\u{09AC}' => "b",  // ব
        '\u{09AD}' => "bh", // ভ
        '\u{09AE}' => "m",  // ম
        // Semivowels and liquids.
        '\u{09AF}' => "y", // য
        '\u{09B0}' => "r", // র
        '\u{09B2}' => "l", // ল
        // Sibilants and h.
        '\u{09B6}' => "ś", // শ
        '\u{09B7}' => "ṣ", // ষ
        '\u{09B8}' => "s", // স
        '\u{09B9}' => "h", // হ
        // Precomposed nukta consonants.
        '\u{09DC}' => "ṛ",  // ড়
        '\u{09DD}' => "ṛh", // ঢ়
        '\u{09DF}' => "ẏ",  // য়
        _ => return None,
    })
}

/// If `base` is the ISO 15919 form of a base consonant that has a
/// nukta variant, return the nukta variant's ISO form.
#[must_use]
fn nukta_variant(base: &'static str) -> Option<&'static str> {
    match base {
        "ḍ" => Some("ṛ"),   // ড + ় → ড় → ṛ
        "ḍh" => Some("ṛh"), // ঢ + ় → ঢ় → ṛh
        "y" => Some("ẏ"),   // য + ় → য় → ẏ
        _ => None,
    }
}

/// Map a Bengali digit (`০..৯` U+09E6..=U+09EF) to its ASCII digit
/// counterpart.
#[must_use]
pub const fn digit_iso(c: char) -> Option<char> {
    match c {
        '\u{09E6}'..='\u{09EF}' => {
            let offset = c as u32 - 0x09E6;
            char::from_u32(0x0030 + offset)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// PHONEX-Bengali — 4-char Soundex-shape reduction over the ISO 15919
// output.
// ---------------------------------------------------------------------

/// The PHONEX-Bengali encoder.
///
/// A zero-sized value; construct as [`BengaliPhonex`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_bn::BengaliPhonex;
///
/// // "রাম" transliterates to "rāma"; phonex reduces to R500.
/// // R seed, A vowel (reset), M code=5, A vowel (reset)
/// //   → "R5" pad → "R500".
/// assert_eq!(BengaliPhonex.encode("রাম").unwrap(), "R500");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BengaliPhonex;

impl BengaliPhonex {
    /// Encodes `word` per the PHONEX-Bengali algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or reduced to nothing after
    /// filtering). Otherwise returns a 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let transliterated = BengaliIso15919.encode(word);
        let ascii = fold_to_ascii_upper(&transliterated);
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
                // Vowel — reset the duplicate-collapse state.
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

/// Fold `s` to uppercase-ASCII letters, dropping non-letter code
/// points. Handles the Latin-with-diacritic scalars the ISO 15919
/// output can carry.
fn fold_to_ascii_upper(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if let Some(letter) = fold_letter(c) {
            out.push(letter);
        }
    }
    out
}

/// Fold `c` to a single ASCII uppercase letter, or `None` if `c` is
/// not letter-like.
///
/// The `match_same_arms` lint is suppressed because arms grouped by
/// linguistic category (retroflex under-dots, sibilant / nasal
/// marks, nukta variants, combining-mark ISO forms) deliberately
/// map to the same ASCII target — merging them would obscure the
/// linguistic grouping, and the encoder's design intent is that
/// each ISO 15919 diacritic-carrying scalar folds *individually* to
/// its ASCII base for Soundex-shape classification.
#[allow(clippy::match_same_arms)]
fn fold_letter(c: char) -> Option<char> {
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase());
    }
    // ISO 15919-specific letter folds (Latin-with-diacritic scalars).
    let folded = match c {
        // Long vowels → short base.
        'ā' | 'Ā' => 'A',
        'ī' | 'Ī' => 'I',
        'ū' | 'Ū' => 'U',
        // Retroflex under-dots → base.
        'ṭ' | 'Ṭ' => 'T',
        'ḍ' | 'Ḍ' => 'D',
        'ṇ' | 'Ṇ' => 'N',
        // Sibilant / nasal marks → base.
        'ś' | 'Ś' => 'S',
        'ṣ' | 'Ṣ' => 'S',
        'ṅ' | 'Ṅ' => 'N',
        'ñ' | 'Ñ' => 'N',
        // Nukta variants → base.
        'ṛ' | 'Ṛ' => 'R',
        'ẏ' | 'Ẏ' => 'Y',
        // Combining-mark ISO forms.
        'ṁ' | 'Ṁ' => 'M',
        'ḥ' | 'Ḥ' => 'H',
        _ => return None,
    };
    Some(folded)
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

/// Adapter that exposes [`BengaliPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Bengali::phonetic_encoder`](crate::Bengali) hands back.
///
/// Returns `Some((key, None))` for input with at least one Bengali
/// scalar; returns `None` for input with no Bengali content
/// (transliteration passes through unchanged, which is not a useful
/// key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BengaliPhonexAdapter;

impl LanguagePhoneticEncoder for BengaliPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_bengali(word) {
            return None;
        }
        let key = BengaliPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-bn"
    }
}

/// Does `s` contain at least one scalar in the Bengali UTF-8 block
/// (U+0980..=U+09FF)?
fn contains_bengali(s: &str) -> bool {
    s.chars().any(|c| ('\u{0980}'..='\u{09FF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        BengaliIso15919.encode(s)
    }

    fn p(w: &str) -> String {
        BengaliPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Inherent-vowel (schwa) handling.
    // -------------------------------------------------------------

    #[test]
    fn bare_consonant_carries_inherent_schwa() {
        // ক alone → ka (schwa fires).
        assert_eq!(e("ক"), "ka");
    }

    #[test]
    fn halant_suppresses_schwa() {
        // ক + ্ → k (no schwa).
        assert_eq!(e("ক্"), "k");
    }

    #[test]
    fn matra_overrides_schwa() {
        // ক + ি → ki (matra overrides).
        assert_eq!(e("কি"), "ki");
        assert_eq!(e("কী"), "kī");
        assert_eq!(e("কু"), "ku");
        assert_eq!(e("কূ"), "kū");
        assert_eq!(e("কে"), "ke");
        assert_eq!(e("কো"), "ko");
        assert_eq!(e("কা"), "kā");
    }

    #[test]
    fn consonant_cluster_via_halant() {
        // সত্য = স + ত + ্ + য → sat + ya = satya.
        assert_eq!(e("সত্য"), "satya");
    }

    #[test]
    fn explicit_schwa_retention() {
        // রাম = র + া + ম → rāma (final ম carries schwa).
        // Modern colloquial Bengali pronounces "ram" without the
        // final schwa; ISO 15919 convention retains it.
        assert_eq!(e("রাম"), "rāma");
        // কমল = ক + ম + ল → kamala.
        assert_eq!(e("কমল"), "kamala");
    }

    // -------------------------------------------------------------
    // Independent vowels.
    // -------------------------------------------------------------

    #[test]
    fn independent_vowels() {
        assert_eq!(e("অ"), "a");
        assert_eq!(e("আ"), "ā");
        assert_eq!(e("ই"), "i");
        assert_eq!(e("ঈ"), "ī");
        assert_eq!(e("উ"), "u");
        assert_eq!(e("ঊ"), "ū");
        assert_eq!(e("এ"), "e");
        assert_eq!(e("ঐ"), "ai");
        assert_eq!(e("ও"), "o");
        assert_eq!(e("ঔ"), "au");
    }

    // -------------------------------------------------------------
    // Combining marks.
    // -------------------------------------------------------------

    #[test]
    fn anusvara_encodes_to_dot_m() {
        // বাংলা = ব + া + ং + ল + া → bāṁlā.
        assert_eq!(e("বাংলা"), "bāṁlā");
    }

    #[test]
    fn visarga_encodes_to_dot_h() {
        // দুঃখ = দ + ু + ঃ + খ. দ + ু → du; + ঃ → duḥ; + খ (no matra) → duḥkha.
        assert_eq!(e("দুঃখ"), "duḥkha");
    }

    // -------------------------------------------------------------
    // Khanda ta.
    // -------------------------------------------------------------

    #[test]
    fn khanda_ta_encodes_as_final_t() {
        // ৎ never carries a schwa.
        assert_eq!(e("ৎ"), "t");
    }

    // -------------------------------------------------------------
    // Nukta.
    // -------------------------------------------------------------

    #[test]
    fn decomposed_nukta_matches_precomposed() {
        // য + ় (decomposed) should encode the same as য় (precomposed).
        let decomposed: String = "\u{09AF}\u{09BC}".into();
        let precomposed: String = "\u{09DF}".into();
        assert_eq!(e(&decomposed), e(&precomposed));
        assert_eq!(e(&precomposed), "ẏa");
    }

    // -------------------------------------------------------------
    // Consonants — spot checks.
    // -------------------------------------------------------------

    #[test]
    fn retroflex_series_encodes_with_dots_below() {
        assert_eq!(e("ট"), "ṭa");
        assert_eq!(e("ঠ"), "ṭha");
        assert_eq!(e("ড"), "ḍa");
        assert_eq!(e("ঢ"), "ḍha");
        assert_eq!(e("ণ"), "ṇa");
    }

    #[test]
    fn sibilants_encode_correctly() {
        assert_eq!(e("শ"), "śa");
        assert_eq!(e("ষ"), "ṣa");
        assert_eq!(e("স"), "sa");
    }

    // -------------------------------------------------------------
    // Digits.
    // -------------------------------------------------------------

    #[test]
    fn bengali_digits_encode_to_ascii() {
        assert_eq!(e("২০২৬"), "2026");
        assert_eq!(e("০"), "0");
        assert_eq!(e("৯"), "9");
    }

    // -------------------------------------------------------------
    // Pass-through.
    // -------------------------------------------------------------

    #[test]
    fn ascii_passes_through() {
        assert_eq!(e(""), "");
        assert_eq!(e("hello"), "hello");
    }

    #[test]
    fn mixed_content_passes_through_non_bengali() {
        assert_eq!(e("hello রাম"), "hello rāma");
    }

    // -------------------------------------------------------------
    // PHONEX-Bengali.
    // -------------------------------------------------------------

    #[test]
    fn phonex_empty_input_returns_none() {
        assert!(BengaliPhonex.encode("").is_none());
        assert!(BengaliPhonex.encode("   ").is_none());
    }

    #[test]
    fn phonex_rama_encodes_to_r500() {
        // রাম → rāma → RAMA → R seed, A vow reset, M code=5, A vow reset → "R5" → "R500".
        assert_eq!(p("রাম"), "R500");
    }

    #[test]
    fn phonex_long_vowels_fold_to_short() {
        // রাম (rāma) and রম (rama, hypothetical) should share the key
        // because long-vowel folding collapses ā → A → vowel-reset.
        // Both begin with R, then all-vowel run, so both → "R500"
        // (bare ম contributes M=5 for both).
        assert_eq!(BengaliPhonex.encode("রাম"), BengaliPhonex.encode("রম"));
    }

    #[test]
    fn phonex_retroflex_folds_to_base() {
        // ট (ṭa) and ত (ta) — retroflex fold makes them share the
        // reduced letter T. Both → "T000".
        assert_eq!(BengaliPhonex.encode("ট"), BengaliPhonex.encode("ত"));
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_bn() {
        assert_eq!(BengaliPhonexAdapter.name(), "phonex-bn");
    }

    #[test]
    fn adapter_returns_some_for_bengali() {
        let out = BengaliPhonexAdapter.encode("রাম");
        assert_eq!(out, Some((String::from("R500"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_bengali() {
        assert!(BengaliPhonexAdapter.encode("").is_none());
        assert!(BengaliPhonexAdapter.encode("hello").is_none());
        assert!(BengaliPhonexAdapter.encode("123").is_none());
    }
}
