//! Malayalam → ISO 15919 → PHONEX-Malayalam phonetic encoder.
//!
//! # Origin
//!
//! Malayalam is the fourth Brahmic-script pack in StringCheese (after
//! Hindi's Devanagari, Bengali, and Tamil) and the **second
//! Dravidian** pack (sibling of Tamil). Malayalam phonology is a
//! *hybrid* of the classical Sanskrit-inherited system and the
//! Dravidian tradition:
//!
//! * **Four-way stop series (like Devanagari and Bengali).**
//!   Malayalam retains the classical unaspirated-voiceless /
//!   aspirated-voiceless / unaspirated-voiced / aspirated-voiced
//!   distinction in five places of articulation (`k / kh / g / gh`,
//!   `c / ch / j / jh`, `ṭ / ṭh / ḍ / ḍh`, `t / th / d / dh`,
//!   `p / ph / b / bh`), unlike Tamil which collapses to one stop
//!   per place. This makes the ISO 15919 table structurally similar
//!   to the Bengali pack's.
//! * **Retroflex approximant and alveolar tap (like Tamil).**
//!   Malayalam has `ള` (ḷ, retroflex lateral), `ഴ` (ḻ, retroflex
//!   approximant), and `റ` (ṟ, alveolar tap) — the same
//!   Dravidian-family additions Tamil carries. Each folds to its
//!   ASCII base in the PHONEX-Malayalam reduction.
//! * **Chillu letters — Malayalam-only.** Six atomic "pure
//!   consonant" scalars in **U+0D7A..=U+0D7F** (`ൺ ൻ ർ ൽ ൾ ൿ`)
//!   that represent a word-final consonant without any following
//!   vowel. Each is treated as its base consonant with an implicit
//!   suppressed schwa — `ൻ` encodes to just `n`, `ർ` to just `r`,
//!   etc. — mirroring the base-consonant-plus-virama behavior.
//!
//! Two candidate approaches present themselves:
//!
//! * **ISO 15919 transliteration.** The international-standard
//!   Indic-script romanization. Faithful to the script but not a
//!   *phonetic-equivalence* encoder — two words that sound alike
//!   don't necessarily transliterate alike.
//! * **PHONEX-Malayalam.** Transliterate to Latin first, then apply
//!   a Soundex-shape 4-character reduction that folds long / short
//!   vowels, drops retroflex under-dots, collapses consonant-class
//!   duplicates. Produces sound-alike equivalence classes and
//!   matches the shape of the other Latin-alphabet packs' phonetic
//!   hookups.
//!
//! # Implementation choice — two-stage: ISO 15919 then PHONEX
//!
//! This module ships both stages:
//!
//! 1. **[`MalayalamIso15919`]** — the deterministic Malayalam →
//!    Latin transliteration, honoring the inherent-schwa convention,
//!    the virama-suppression rule, and the chillu-letter atomic
//!    form. Public because it's useful in its own right
//!    (data-migration tools, IR display).
//! 2. **[`MalayalamPhonex`]** — the 4-character Soundex-shape key
//!    computed *over* the ISO 15919 output. This is the encoder
//!    [`MalayalamPhonexAdapter`] wraps for the
//!    [`LanguagePhoneticEncoder`] trait hookup; adapter name
//!    `"phonex-ml"`.
//!
//! # Inherent-vowel, virama, and chillu handling
//!
//! Malayalam is an **abugida** — every base consonant carries an
//! implicit `a` (schwa) vowel unless a dependent vowel sign (matra)
//! or the **virama** (`്` U+0D4D) overrides it. So `ക` alone
//! represents `ka` (not just `k`), `ക്` (`ക` + virama) represents
//! bare `k`, `കി` (`ക` + matra `ി`) represents `ki`.
//!
//! **Unlike Tamil, Malayalam DOES form conjunct consonants via the
//! virama.** Some clusters are rendered as ligatures, some as
//! visible virama sequences. The transliterator treats them
//! uniformly — virama suppresses the schwa on the preceding
//! consonant regardless of the resulting glyph shape.
//!
//! **Chillu letters are treated as bare consonants.** The six
//! chillu scalars in U+0D7A..=U+0D7F each stand for a specific
//! consonant with a suppressed vowel — `ൻ` (U+0D7B) encodes to
//! `n`, `ർ` (U+0D7C) to `r`, `ൽ` (U+0D7D) to `l`, `ൾ` (U+0D7E) to
//! `ḷ`, `ൺ` (U+0D7A) to `ṇ`, `ൿ` (U+0D7F) to `k`. This mirrors the
//! `<base consonant> + virama` sequence semantically.
//!
//! # ISO 15919 mapping table
//!
//! ## Independent vowels
//!
//! | Malayalam | ISO 15919 | Notes                       |
//! |-----------|-----------|-----------------------------|
//! | `അ`       | `a`       | short a                     |
//! | `ആ`       | `ā`       | long a                      |
//! | `ഇ`       | `i`       | short i                     |
//! | `ഈ`       | `ī`       | long i                      |
//! | `ഉ`       | `u`       | short u                     |
//! | `ഊ`       | `ū`       | long u                      |
//! | `ഋ`       | `r̥`       | vocalic r                   |
//! | `എ`       | `e`       | short e (Dravidian)         |
//! | `ഏ`       | `ē`       | long e (Dravidian)          |
//! | `ഐ`       | `ai`      | diphthong                   |
//! | `ഒ`       | `o`       | short o (Dravidian)         |
//! | `ഓ`       | `ō`       | long o (Dravidian)          |
//! | `ഔ`       | `au`      | diphthong                   |
//!
//! ## Dependent vowel signs (matras) — appear after a consonant
//!
//! | Malayalam | ISO 15919 | Notes |
//! |-----------|-----------|-------|
//! | (none)    | `a`       | inherent schwa |
//! | `ാ`       | `ā`       |       |
//! | `ി`       | `i`       |       |
//! | `ീ`       | `ī`       |       |
//! | `ു`       | `u`       |       |
//! | `ൂ`       | `ū`       |       |
//! | `ൃ`       | `r̥`       |       |
//! | `െ`       | `e`       |       |
//! | `േ`       | `ē`       |       |
//! | `ൈ`       | `ai`      |       |
//! | `ൊ`       | `o`       |       |
//! | `ോ`       | `ō`       |       |
//! | `ൌ`       | `au`      |       |
//!
//! ## Special marks
//!
//! | Malayalam | ISO 15919 | Notes                                   |
//! |-----------|-----------|-----------------------------------------|
//! | `്`       | (∅)       | virama — suppresses inherent schwa      |
//! | `ം`       | `ṁ`       | anusvara                                |
//! | `ഃ`       | `ḥ`       | visarga                                 |
//!
//! ## Chillu letters (U+0D7A..=U+0D7F — Malayalam-only)
//!
//! Each chillu is a base consonant with a suppressed inherent
//! vowel — the atomic word-final form.
//!
//! | Malayalam | ISO 15919 | Base                    |
//! |-----------|-----------|-------------------------|
//! | `ൺ`       | `ṇ`       | from `ണ` (retroflex n)  |
//! | `ൻ`       | `n`       | from `ന` (dental n)     |
//! | `ർ`       | `r`       | from `ര`                |
//! | `ൽ`       | `l`       | from `ല`                |
//! | `ൾ`       | `ḷ`       | from `ള` (retroflex l)  |
//! | `ൿ`       | `k`       | from `ക`                |
//!
//! ## The 33 base consonants + 3 Dravidian additions
//!
//! Same phonological groups as Devanagari and Bengali, different
//! scalars, plus the three Dravidian additions (`ഴ ള റ`).
//!
//! | Malayalam | ISO 15919 | Group                          |
//! |-----------|-----------|--------------------------------|
//! | `ക`       | `k`       | velar                          |
//! | `ഖ`       | `kh`      |                                |
//! | `ഗ`       | `g`       |                                |
//! | `ഘ`       | `gh`      |                                |
//! | `ങ`       | `ṅ`       | velar nasal                    |
//! | `ച`       | `c`       | palatal                        |
//! | `ഛ`       | `ch`      |                                |
//! | `ജ`       | `j`       |                                |
//! | `ഝ`       | `jh`      |                                |
//! | `ഞ`       | `ñ`       | palatal nasal                  |
//! | `ട`       | `ṭ`       | retroflex                      |
//! | `ഠ`       | `ṭh`      |                                |
//! | `ഡ`       | `ḍ`       |                                |
//! | `ഢ`       | `ḍh`      |                                |
//! | `ണ`       | `ṇ`       | retroflex nasal                |
//! | `ത`       | `t`       | dental                         |
//! | `ഥ`       | `th`      |                                |
//! | `ദ`       | `d`       |                                |
//! | `ധ`       | `dh`      |                                |
//! | `ന`       | `n`       | dental nasal                   |
//! | `പ`       | `p`       | labial                         |
//! | `ഫ`       | `ph`      |                                |
//! | `ബ`       | `b`       |                                |
//! | `ഭ`       | `bh`      |                                |
//! | `മ`       | `m`       | labial nasal                   |
//! | `യ`       | `y`       | palatal glide                  |
//! | `ര`       | `r`       |                                |
//! | `റ`       | `ṟ`       | **alveolar tap** (Dravidian)   |
//! | `ല`       | `l`       |                                |
//! | `ള`       | `ḷ`       | **retroflex l** (Dravidian)    |
//! | `ഴ`       | `ḻ`       | **retroflex approx** (Dravid.) |
//! | `വ`       | `v`       |                                |
//! | `ശ`       | `ś`       | sibilant                       |
//! | `ഷ`       | `ṣ`       |                                |
//! | `സ`       | `s`       |                                |
//! | `ഹ`       | `h`       |                                |
//!
//! ## Digits
//!
//! Malayalam digits `൦..൯` (U+0D66..=U+0D6F) map to ASCII digits
//! `0..9`.
//!
//! # PHONEX-Malayalam reduction
//!
//! After transliteration, the encoder applies a Soundex-shape
//! 4-character reduction, matching the shape of the other
//! Latin-alphabet packs:
//!
//! 1. Fold Latin-with-diacritic scalars to their ASCII base
//!    (`ā → A`, `ī → I`, `ū → U`, `ē → E`, `ō → O`, `ṭ → T`,
//!    `ḍ → D`, `ṇ → N`, `ḷ → L`, `ḻ → L`, `ṟ → R`, `ṅ → N`,
//!    `ñ → N`, `ś → S`, `ṣ → S`, `ṁ → M`, `ḥ → H`), uppercase
//!    everything.
//! 2. Take the first ASCII letter as the seed.
//! 3. Walk subsequent letters, mapping each to its Soundex
//!    consonant class (`B/P/F/V/W = 1`, `C/K/G/Q/J/X = 2`,
//!    `D/T = 3`, `L = 4`, `M/N = 5`, `R = 6`, `S/Z = 7`;
//!    vowels reset the duplicate-collapse state).
//! 4. Collapse consecutive equal codes; pad to 4 characters with
//!    `'0'`.
//!
//! Adapter name: `"phonex-ml"`.
//!
//! # Byte-length caveat
//!
//! Every Malayalam scalar is **3 bytes** in UTF-8 (U+0D00..=U+0D7F
//! falls in the 3-byte range). The transliterator walks characters
//! via [`str::chars`], never raw bytes, so it never risks slicing a
//! scalar apart. The output uses ASCII plus a small set of
//! Latin-with-diacritic scalars.
//!
//! # Non-Malayalam pass-through
//!
//! ASCII characters, whitespace, punctuation, and any scalar
//! outside the Malayalam block pass through the transliterator
//! unchanged. The phonex reduction then filters non-letters out.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Malayalam → ISO 15919 transliterator.
///
/// A zero-sized value; construct as [`MalayalamIso15919`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and
/// the discussion of inherent-vowel (schwa) handling, virama
/// suppression, and chillu-letter atomic forms.
///
/// # Example
///
/// ```
/// use stringcheese_ml::MalayalamIso15919;
///
/// // "malayāḷam" — മ + ല + യ + ാ + ള + ം.
/// // The inherent schwa fires on മ, ല, യ; the ാ matra overrides on യ;
/// // ള carries schwa; ം anusvara is ṁ.
/// assert_eq!(MalayalamIso15919.encode("മലയാളം"), "malayāḷaṁ");
/// // "ñān" — ഞ + ാ + ൻ ("I"). The chillu ൻ suppresses the vowel.
/// assert_eq!(MalayalamIso15919.encode("ഞാൻ"), "ñān");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MalayalamIso15919;

impl MalayalamIso15919 {
    /// Encode `text` per the Malayalam → ISO 15919 transliteration.
    ///
    /// Every Malayalam scalar in the mapping is replaced by its ISO
    /// 15919 counterpart; every scalar outside the mapping (ASCII,
    /// whitespace, other-script letters) passes through unchanged.
    ///
    /// The encoder honors the inherent-schwa convention: a base
    /// consonant with no following matra or virama emits the
    /// consonant letter followed by `a`; a following matra
    /// overrides the `a`; a following virama suppresses it. Chillu
    /// letters emit their base consonant with no schwa.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len().saturating_add(8));

        let mut chars = text.chars().peekable();
        let mut pending_consonant: Option<&'static str> = None;

        while let Some(c) = chars.next() {
            let next = chars.peek().copied();

            if let Some(base) = pending_consonant.take() {
                // Virama suppresses the schwa.
                if c == '\u{0D4D}' {
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
                // Fall through — the current char still needs
                // dispatch.
            }

            if let Some(v) = independent_vowel_iso(c) {
                out.push_str(v);
                continue;
            }

            if let Some(cons) = consonant_iso(c) {
                pending_consonant = Some(cons);
                if next.is_none() {
                    out.push_str(cons);
                    out.push('a');
                    pending_consonant = None;
                }
                continue;
            }

            // Chillu letters — atomic word-final consonants with a
            // suppressed inherent vowel. Emit the base consonant
            // form and no `a`.
            if let Some(chillu) = chillu_iso(c) {
                out.push_str(chillu);
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
                // Stray matra with no preceding consonant (should
                // not happen in well-formed Malayalam, but pass
                // through gracefully).
                out.push_str(matra);
                continue;
            }

            if c == '\u{0D4D}' {
                // Stray virama — no schwa to suppress, drop it.
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
        '\u{0D05}' => "a",  // അ
        '\u{0D06}' => "ā",  // ആ
        '\u{0D07}' => "i",  // ഇ
        '\u{0D08}' => "ī",  // ഈ
        '\u{0D09}' => "u",  // ഉ
        '\u{0D0A}' => "ū",  // ഊ
        '\u{0D0B}' => "r̥",  // ഋ
        '\u{0D0E}' => "e",  // എ
        '\u{0D0F}' => "ē",  // ഏ
        '\u{0D10}' => "ai", // ഐ
        '\u{0D12}' => "o",  // ഒ
        '\u{0D13}' => "ō",  // ഓ
        '\u{0D14}' => "au", // ഔ
        _ => return None,
    })
}

/// Map a dependent vowel sign (matra) scalar to its ISO 15919 string.
#[must_use]
pub const fn matra_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0D3E}' => "ā",  // ാ
        '\u{0D3F}' => "i",  // ി
        '\u{0D40}' => "ī",  // ീ
        '\u{0D41}' => "u",  // ു
        '\u{0D42}' => "ū",  // ൂ
        '\u{0D43}' => "r̥",  // ൃ
        '\u{0D46}' => "e",  // െ
        '\u{0D47}' => "ē",  // േ
        '\u{0D48}' => "ai", // ൈ
        '\u{0D4A}' => "o",  // ൊ
        '\u{0D4B}' => "ō",  // ോ
        '\u{0D4C}' => "au", // ൌ
        _ => return None,
    })
}

/// Map a combining vowel-modifier scalar to its ISO 15919 string.
#[must_use]
pub const fn combining_mark_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0D02}' => "ṁ", // ം anusvara
        '\u{0D03}' => "ḥ", // ഃ visarga
        _ => return None,
    })
}

/// Map a base consonant scalar to its ISO 15919 string.
#[must_use]
pub const fn consonant_iso(c: char) -> Option<&'static str> {
    Some(match c {
        // Velars.
        '\u{0D15}' => "k",  // ക
        '\u{0D16}' => "kh", // ഖ
        '\u{0D17}' => "g",  // ഗ
        '\u{0D18}' => "gh", // ഘ
        '\u{0D19}' => "ṅ",  // ങ
        // Palatals.
        '\u{0D1A}' => "c",  // ച
        '\u{0D1B}' => "ch", // ഛ
        '\u{0D1C}' => "j",  // ജ
        '\u{0D1D}' => "jh", // ഝ
        '\u{0D1E}' => "ñ",  // ഞ
        // Retroflexes.
        '\u{0D1F}' => "ṭ",  // ട
        '\u{0D20}' => "ṭh", // ഠ
        '\u{0D21}' => "ḍ",  // ഡ
        '\u{0D22}' => "ḍh", // ഢ
        '\u{0D23}' => "ṇ",  // ണ
        // Dentals.
        '\u{0D24}' => "t",  // ത
        '\u{0D25}' => "th", // ഥ
        '\u{0D26}' => "d",  // ദ
        '\u{0D27}' => "dh", // ധ
        '\u{0D28}' => "n",  // ന
        // Labials.
        '\u{0D2A}' => "p",  // പ
        '\u{0D2B}' => "ph", // ഫ
        '\u{0D2C}' => "b",  // ബ
        '\u{0D2D}' => "bh", // ഭ
        '\u{0D2E}' => "m",  // മ
        // Semivowels, liquids, Dravidian additions.
        '\u{0D2F}' => "y", // യ
        '\u{0D30}' => "r", // ര
        '\u{0D31}' => "ṟ", // റ (alveolar tap)
        '\u{0D32}' => "l", // ല
        '\u{0D33}' => "ḷ", // ള (retroflex l)
        '\u{0D34}' => "ḻ", // ഴ (retroflex approx)
        '\u{0D35}' => "v", // വ
        // Sibilants and h.
        '\u{0D36}' => "ś", // ശ
        '\u{0D37}' => "ṣ", // ഷ
        '\u{0D38}' => "s", // സ
        '\u{0D39}' => "h", // ഹ
        _ => return None,
    })
}

/// Map a chillu letter (U+0D7A..=U+0D7F) to its ISO 15919 string.
///
/// Chillu letters are atomic word-final "pure consonant" forms — the
/// base consonant with a suppressed inherent vowel. They are unique
/// to Malayalam among the Brahmic scripts.
#[must_use]
pub const fn chillu_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0D7A}' => "ṇ", // ൺ chillu ṇ
        '\u{0D7B}' => "n", // ൻ chillu n
        '\u{0D7C}' => "r", // ർ chillu r
        '\u{0D7D}' => "l", // ൽ chillu l
        '\u{0D7E}' => "ḷ", // ൾ chillu ḷ
        '\u{0D7F}' => "k", // ൿ chillu k
        _ => return None,
    })
}

/// Map a Malayalam digit (`൦..൯` U+0D66..=U+0D6F) to its ASCII digit
/// counterpart.
#[must_use]
pub const fn digit_iso(c: char) -> Option<char> {
    match c {
        '\u{0D66}'..='\u{0D6F}' => {
            let offset = c as u32 - 0x0D66;
            char::from_u32(0x0030 + offset)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// PHONEX-Malayalam — 4-char Soundex-shape reduction over the ISO 15919
// output.
// ---------------------------------------------------------------------

/// The PHONEX-Malayalam encoder.
///
/// A zero-sized value; construct as [`MalayalamPhonex`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_ml::MalayalamPhonex;
///
/// // "ഞാൻ" transliterates to "ñān"; phonex folds ñ → N and n → N.
/// // N seed last=5; A vowel resets last=0; N code=5 ≠ last=0
/// //   push '5', out="N5" → pad "N500".
/// assert_eq!(MalayalamPhonex.encode("ഞാൻ").unwrap(), "N500");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MalayalamPhonex;

impl MalayalamPhonex {
    /// Encodes `word` per the PHONEX-Malayalam algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty
    /// input, pure whitespace, all punctuation, or reduced to
    /// nothing after filtering). Otherwise returns a 4-character key
    /// of the form `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let transliterated = MalayalamIso15919.encode(word);
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
/// linguistic category (long-vowel diacritics, retroflex under-dots,
/// alveolar / palatal / velar nasal marks, sibilant / laminal marks,
/// combining-mark ISO forms, Dravidian-specific retroflex / alveolar
/// distinctions) deliberately map to the same ASCII target — merging
/// them would obscure the linguistic grouping, and the encoder's
/// design intent is that each ISO 15919 diacritic-carrying scalar
/// folds *individually* to its ASCII base for Soundex-shape
/// classification.
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
        'ē' | 'Ē' => 'E',
        'ō' | 'Ō' => 'O',
        // Retroflex under-dots → base.
        'ṭ' | 'Ṭ' => 'T',
        'ḍ' | 'Ḍ' => 'D',
        'ṇ' | 'Ṇ' => 'N',
        'ḷ' | 'Ḷ' => 'L',
        // Dravidian-specific retroflex approximant and alveolar tap.
        'ḻ' | 'Ḻ' => 'L',
        'ṟ' | 'Ṟ' => 'R',
        // Velar / palatal nasal marks → base.
        'ṅ' | 'Ṅ' => 'N',
        'ñ' | 'Ñ' => 'N',
        // Sibilant marks → base.
        'ś' | 'Ś' => 'S',
        'ṣ' | 'Ṣ' => 'S',
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

/// Adapter that exposes [`MalayalamPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Malayalam::phonetic_encoder`](crate::Malayalam) hands back.
///
/// Returns `Some((key, None))` for input with at least one Malayalam
/// scalar; returns `None` for input with no Malayalam content
/// (transliteration passes through unchanged, which is not a useful
/// key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MalayalamPhonexAdapter;

impl LanguagePhoneticEncoder for MalayalamPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_malayalam(word) {
            return None;
        }
        let key = MalayalamPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-ml"
    }
}

/// Does `s` contain at least one scalar in the Malayalam UTF-8 block
/// (U+0D00..=U+0D7F)?
fn contains_malayalam(s: &str) -> bool {
    s.chars().any(|c| ('\u{0D00}'..='\u{0D7F}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        MalayalamIso15919.encode(s)
    }

    fn p(w: &str) -> String {
        MalayalamPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Inherent-vowel (schwa) and virama handling.
    // -------------------------------------------------------------

    #[test]
    fn bare_consonant_carries_inherent_schwa() {
        // ക alone → ka (schwa fires).
        assert_eq!(e("ക"), "ka");
    }

    #[test]
    fn virama_suppresses_schwa() {
        // ക + ് → k (no schwa).
        assert_eq!(e("ക്"), "k");
    }

    #[test]
    fn matra_overrides_schwa() {
        // ക + ി → ki (matra overrides).
        assert_eq!(e("കി"), "ki");
        assert_eq!(e("കീ"), "kī");
        assert_eq!(e("കു"), "ku");
        assert_eq!(e("കൂ"), "kū");
        assert_eq!(e("കേ"), "kē");
        assert_eq!(e("കോ"), "kō");
        assert_eq!(e("കാ"), "kā");
    }

    #[test]
    fn consonant_cluster_via_virama() {
        // സത്യം (truth) = സ + ത + ് + യ + ം → sa + t + ya + ṁ = satyaṁ.
        assert_eq!(e("സത്യം"), "satyaṁ");
    }

    // -------------------------------------------------------------
    // Chillu letters.
    // -------------------------------------------------------------

    #[test]
    fn chillu_n_encodes_as_bare_n() {
        // ൻ U+0D7B — chillu n, atomic pure consonant.
        assert_eq!(e("ൻ"), "n");
    }

    #[test]
    fn chillu_r_encodes_as_bare_r() {
        // ർ U+0D7C — chillu r.
        assert_eq!(e("ർ"), "r");
    }

    #[test]
    fn chillu_l_encodes_as_bare_l() {
        // ൽ U+0D7D — chillu l.
        assert_eq!(e("ൽ"), "l");
    }

    #[test]
    fn word_ending_in_chillu_n() {
        // ഞാൻ (I) = ഞ + ാ + ൻ → ñā + n = ñān.
        assert_eq!(e("ഞാൻ"), "ñān");
    }

    #[test]
    fn word_ending_in_chillu_r() {
        // അവർ (they) = അ + വ + ർ → a + va + r = avar.
        assert_eq!(e("അവർ"), "avar");
    }

    #[test]
    fn word_with_all_chillu_variants() {
        // Each chillu, tested in a minimal ka-plus-chillu form.
        assert_eq!(e("കൺ"), "kaṇ");
        assert_eq!(e("കൻ"), "kan");
        assert_eq!(e("കർ"), "kar");
        assert_eq!(e("കൽ"), "kal");
        assert_eq!(e("കൾ"), "kaḷ");
        assert_eq!(e("കൿ"), "kak");
    }

    // -------------------------------------------------------------
    // Independent vowels.
    // -------------------------------------------------------------

    #[test]
    fn independent_vowels() {
        assert_eq!(e("അ"), "a");
        assert_eq!(e("ആ"), "ā");
        assert_eq!(e("ഇ"), "i");
        assert_eq!(e("ഈ"), "ī");
        assert_eq!(e("ഉ"), "u");
        assert_eq!(e("ഊ"), "ū");
        assert_eq!(e("എ"), "e");
        assert_eq!(e("ഏ"), "ē");
        assert_eq!(e("ഐ"), "ai");
        assert_eq!(e("ഒ"), "o");
        assert_eq!(e("ഓ"), "ō");
        assert_eq!(e("ഔ"), "au");
    }

    // -------------------------------------------------------------
    // Dravidian-specific consonants.
    // -------------------------------------------------------------

    #[test]
    fn dravidian_l_variants_encode_correctly() {
        // ള (retroflex l) → ḷa.
        assert_eq!(e("ള"), "ḷa");
        // ഴ (retroflex approximant) → ḻa.
        assert_eq!(e("ഴ"), "ḻa");
        // ല (dental l) → la.
        assert_eq!(e("ല"), "la");
    }

    #[test]
    fn alveolar_r_encodes_correctly() {
        // റ (alveolar tap) → ṟa.
        assert_eq!(e("റ"), "ṟa");
        // ര (dental r) → ra.
        assert_eq!(e("ര"), "ra");
    }

    #[test]
    fn malayalam_own_name() {
        // മലയാളം (Malayalam) = മ + ല + യ + ാ + ള + ം → malayāḷaṁ.
        assert_eq!(e("മലയാളം"), "malayāḷaṁ");
    }

    // -------------------------------------------------------------
    // Consonants — spot checks.
    // -------------------------------------------------------------

    #[test]
    fn retroflex_series_encodes_with_dots_below() {
        assert_eq!(e("ട"), "ṭa");
        assert_eq!(e("ഠ"), "ṭha");
        assert_eq!(e("ഡ"), "ḍa");
        assert_eq!(e("ഢ"), "ḍha");
        assert_eq!(e("ണ"), "ṇa");
    }

    #[test]
    fn sibilants_encode_correctly() {
        assert_eq!(e("ശ"), "śa");
        assert_eq!(e("ഷ"), "ṣa");
        assert_eq!(e("സ"), "sa");
        assert_eq!(e("ഹ"), "ha");
    }

    // -------------------------------------------------------------
    // Combining marks.
    // -------------------------------------------------------------

    #[test]
    fn anusvara_encodes_to_dot_m() {
        // മലയാളം ends in ം → ṁ.
        assert_eq!(e("മലയാളം"), "malayāḷaṁ");
    }

    #[test]
    fn visarga_encodes_to_dot_h() {
        // ദുഃഖം (sorrow) = ദ + ു + ഃ + ഖ + ം → duḥkhaṁ.
        assert_eq!(e("ദുഃഖം"), "duḥkhaṁ");
    }

    // -------------------------------------------------------------
    // Digits.
    // -------------------------------------------------------------

    #[test]
    fn malayalam_digits_encode_to_ascii() {
        assert_eq!(e("൨൦൨൬"), "2026");
        assert_eq!(e("൦"), "0");
        assert_eq!(e("൯"), "9");
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
    fn mixed_content_passes_through_non_malayalam() {
        assert_eq!(e("hello മലയാളം"), "hello malayāḷaṁ");
    }

    // -------------------------------------------------------------
    // PHONEX-Malayalam.
    // -------------------------------------------------------------

    #[test]
    fn phonex_empty_input_returns_none() {
        assert!(MalayalamPhonex.encode("").is_none());
        assert!(MalayalamPhonex.encode("   ").is_none());
    }

    #[test]
    fn phonex_malayalam_own_name() {
        // മലയാളം → malayāḷaṁ → MALAYALAM (all diacritics folded).
        // M seed last=5; A vow last=0; L code=4 push '4', out="M4"
        //   last=4; A vow last=0; Y code=0 vow-like last=0;
        //   A vow last=0; L code=4 push '4', out="M44" -> wait, L
        // duplicate — but there was a vowel reset, so this pushes.
        // Then A vow; M code=5 push, out="M445" -> pad to "M445"?
        // Let me retrace carefully:
        //   M seed, last=5.
        //   A: vow, last=0.
        //   L: code=4 ≠ 0, push, out="M4", last=4.
        //   A: vow, last=0.
        //   Y: code=0 (Y not classed), last=0.
        //   A: vow, last=0.
        //   L: code=4 ≠ 0, push, out="M44", last=4.
        //   A: vow, last=0.
        //   M: code=5 ≠ 0, push, out="M445", last=5.
        //   → done, pad to "M445".
        // Wait the third character check — L, after previous L was
        // last=4 and vowel reset made last=0, so push. Good.
        assert_eq!(p("മലയാളം"), "M445");
    }

    #[test]
    fn phonex_nyan_encodes_correctly() {
        // ഞാൻ → ñān → NAN → N seed last=5; A vow last=0;
        //   N code=5 ≠ 0 push '5', out="N5", last=5. Pad "N500".
        assert_eq!(p("ഞാൻ"), "N500");
    }

    #[test]
    fn phonex_long_vowels_fold_to_short() {
        // ക → "ka" → K000. കാ → "kā" → K000. Same key.
        assert_eq!(MalayalamPhonex.encode("ക"), MalayalamPhonex.encode("കാ"));
    }

    #[test]
    fn phonex_retroflex_folds_to_base() {
        // ട (ṭa) and ത (ta) — retroflex fold makes them share the
        // reduced letter T. Both → "T000".
        assert_eq!(MalayalamPhonex.encode("ട"), MalayalamPhonex.encode("ത"));
    }

    #[test]
    fn phonex_retroflex_and_dental_nasals_share_a_key() {
        // ണ (ṇa) and ന (na) both fold to N.
        assert_eq!(MalayalamPhonex.encode("ണ"), MalayalamPhonex.encode("ന"));
    }

    #[test]
    fn phonex_all_l_variants_share_a_key() {
        // ല (la), ള (ḷa), ഴ (ḻa) all fold to L.
        let a = MalayalamPhonex.encode("ല").unwrap();
        let b = MalayalamPhonex.encode("ള").unwrap();
        let c = MalayalamPhonex.encode("ഴ").unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn phonex_chillu_and_base_consonant_share_a_key() {
        // Chillu ൻ and ന both fold to N.
        // Word "അൻ" and "അന" — both start with A seed, then N code=5.
        assert_eq!(MalayalamPhonex.encode("അൻ"), MalayalamPhonex.encode("അന"));
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_ml() {
        assert_eq!(MalayalamPhonexAdapter.name(), "phonex-ml");
    }

    #[test]
    fn adapter_returns_some_for_malayalam() {
        let out = MalayalamPhonexAdapter.encode("ഞാൻ");
        assert_eq!(out, Some((String::from("N500"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_malayalam() {
        assert!(MalayalamPhonexAdapter.encode("").is_none());
        assert!(MalayalamPhonexAdapter.encode("hello").is_none());
        assert!(MalayalamPhonexAdapter.encode("123").is_none());
    }
}
