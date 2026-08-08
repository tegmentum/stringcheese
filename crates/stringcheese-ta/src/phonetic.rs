//! Tamil → ISO 15919 → PHONEX-Tamil phonetic encoder.
//!
//! # Origin
//!
//! Tamil is the third Brahmic-script pack in StringCheese (after
//! Hindi's Devanagari and Bengali) and the **first Dravidian** pack —
//! Tamil is Dravidian, not Indo-Aryan like Hindi or Bengali, and its
//! phonology differs accordingly. Two facts drive the phonetic
//! encoder's design:
//!
//! * **Single stop-consonant series.** Unlike Devanagari and Bengali,
//!   which distinguish voiced / voiceless / aspirated / unaspirated
//!   in five places of articulation (four-way split, ~20 stops),
//!   Tamil's native script has **one stop per place** (k, c, ṭ, t, p)
//!   plus the Grantha loans (`ஜ ஷ ஸ ஹ`). Voicing is contextually
//!   determined at pronunciation time, not orthographically marked.
//!   This simplifies the ISO 15919 map significantly — no `kh`, `gh`,
//!   `ch`, `jh`, `ṭh`, `ḍh`, `th`, `dh`, `ph`, `bh` letters exist.
//! * **Two liquids and two nasals more than Devanagari.** Tamil
//!   uniquely writes an alveolar tap (`ற` "ṟ") separate from the
//!   dental / retroflex `r` (`ர` "r"), an alveolar nasal (`ன` "ṉ")
//!   separate from the dental / palatal nasal (`ந` "n"), a
//!   retroflex approximant (`ழ` "ḻ" — the famous "tamiḻ" letter),
//!   and a retroflex lateral (`ள` "ḷ"). Each folds to its ASCII
//!   base in the PHONEX-Tamil reduction (`ṟ → R`, `ṉ → N`,
//!   `ḻ → L`, `ḷ → L`).
//!
//! Two candidate approaches present themselves:
//!
//! * **ISO 15919 transliteration.** The international-standard
//!   Indic-script romanization. Faithful to the script but not a
//!   *phonetic-equivalence* encoder — two words that sound alike
//!   don't necessarily transliterate alike.
//! * **PHONEX-Tamil.** Transliterate to Latin first, then apply a
//!   Soundex-shape 4-character reduction that folds long / short
//!   vowels, drops retroflex under-dots, collapses consonant-class
//!   duplicates. Produces sound-alike equivalence classes and
//!   matches the shape of the other Latin-alphabet packs' phonetic
//!   hookups.
//!
//! # Implementation choice — two-stage: ISO 15919 then PHONEX
//!
//! This module ships both stages:
//!
//! 1. **[`TamilIso15919`]** — the deterministic Tamil → Latin
//!    transliteration, honoring the inherent-schwa convention and
//!    the pulli-suppression rule. Public because it's useful in its
//!    own right (data-migration tools, IR display).
//! 2. **[`TamilPhonex`]** — the 4-character Soundex-shape key
//!    computed *over* the ISO 15919 output. This is the encoder
//!    [`TamilPhonexAdapter`] wraps for the
//!    [`LanguagePhoneticEncoder`] trait hookup; adapter name
//!    `"phonex-ta"`.
//!
//! # Inherent-vowel and pulli handling — the crucial subtlety
//!
//! Tamil is an **abugida** — every base consonant carries an
//! implicit `a` vowel unless a dependent vowel sign (matra) or a
//! **pulli** (`்` U+0BCD, Tamil's virama) overrides it. So `க` alone
//! represents `ka` (not just `k`), `க்` (`க` + pulli) represents
//! bare `k`, `கி` (`க` + matra `ி`) represents `ki`.
//!
//! **Unlike Devanagari, Tamil has no conjunct consonants.** Every
//! Tamil consonant cluster is written with an explicit, visible
//! pulli — there are no ligature-forming rules that "hide" the
//! virama. So `நன்றி` (thanks) is spelled `ந + ன + ் + ற + ி` (5
//! scalars) with the pulli firmly on the middle `ன` suppressing its
//! inherent vowel, and the transliteration is `naṉṟi` (n + inherent
//! schwa + ṉ + pulli suppresses + ṟ + i matra).
//!
//! # ISO 15919 mapping table
//!
//! ## Independent vowels (12)
//!
//! | Tamil | ISO 15919 | Notes                    |
//! |-------|-----------|--------------------------|
//! | `அ`   | `a`       | short a                  |
//! | `ஆ`   | `ā`       | long a                   |
//! | `இ`   | `i`       | short i                  |
//! | `ஈ`   | `ī`       | long i                   |
//! | `உ`   | `u`       | short u                  |
//! | `ஊ`   | `ū`       | long u                   |
//! | `எ`   | `e`       | short e (Dravidian)      |
//! | `ஏ`   | `ē`       | long e (Dravidian)       |
//! | `ஐ`   | `ai`      | diphthong                |
//! | `ஒ`   | `o`       | short o (Dravidian)      |
//! | `ஓ`   | `ō`       | long o (Dravidian)       |
//! | `ஔ`   | `au`      | diphthong                |
//!
//! ## Dependent vowel signs (matras) — appear after a consonant
//!
//! | Tamil | ISO 15919 | Notes                    |
//! |-------|-----------|--------------------------|
//! | (none)| `a`       | inherent schwa           |
//! | `ா`   | `ā`       |                          |
//! | `ி`   | `i`       |                          |
//! | `ீ`   | `ī`       |                          |
//! | `ு`   | `u`       |                          |
//! | `ூ`   | `ū`       |                          |
//! | `ெ`   | `e`       |                          |
//! | `ே`   | `ē`       |                          |
//! | `ை`   | `ai`      |                          |
//! | `ொ`   | `o`       |                          |
//! | `ோ`   | `ō`       |                          |
//! | `ௌ`   | `au`      |                          |
//!
//! ## Special marks
//!
//! | Tamil | ISO 15919 | Notes                                   |
//! |-------|-----------|-----------------------------------------|
//! | `்`   | (∅)       | pulli — suppresses inherent schwa       |
//! | `ஃ`   | `ḵ`       | āytam (aspirate)                        |
//!
//! ## The 18 native consonants + 5 Grantha loans
//!
//! | Tamil | ISO 15919 | Group                          |
//! |-------|-----------|--------------------------------|
//! | `க`   | `k`       | velar                          |
//! | `ங`   | `ṅ`       | velar nasal                    |
//! | `ச`   | `c`       | palatal                        |
//! | `ஞ`   | `ñ`       | palatal nasal                  |
//! | `ட`   | `ṭ`       | retroflex                      |
//! | `ண`   | `ṇ`       | retroflex nasal                |
//! | `த`   | `t`       | dental                         |
//! | `ந`   | `n`       | dental nasal                   |
//! | `ன`   | `ṉ`       | **alveolar nasal** (Tamil-only)|
//! | `ப`   | `p`       | labial                         |
//! | `ம`   | `m`       | labial nasal                   |
//! | `ய`   | `y`       | palatal glide                  |
//! | `ர`   | `r`       | dental / retroflex r           |
//! | `ற`   | `ṟ`       | **alveolar tap** (Tamil-only)  |
//! | `ல`   | `l`       | dental / alveolar lateral      |
//! | `ள`   | `ḷ`       | **retroflex lateral**          |
//! | `ழ`   | `ḻ`       | **retroflex approximant** (the |
//! |       |           | famous "ḻ" of tamiḻ)           |
//! | `வ`   | `v`       | labial glide                   |
//! | `ஜ`   | `j`       | palatal (Grantha)              |
//! | `ஷ`   | `ṣ`       | retroflex sibilant (Grantha)   |
//! | `ஸ`   | `s`       | dental sibilant (Grantha)      |
//! | `ஹ`   | `h`       | glottal (Grantha)              |
//!
//! ## Digits
//!
//! Tamil digits `௦..௯` (U+0BE6..=U+0BEF) map to ASCII digits
//! `0..9`.
//!
//! # PHONEX-Tamil reduction
//!
//! After transliteration, the encoder applies a Soundex-shape
//! 4-character reduction, matching the shape of the other
//! Latin-alphabet packs:
//!
//! 1. Fold Latin-with-diacritic scalars to their ASCII base
//!    (`ā → A`, `ī → I`, `ū → U`, `ē → E`, `ō → O`, `ṭ → T`,
//!    `ṇ → N`, `ṉ → N`, `ṟ → R`, `ḷ → L`, `ḻ → L`, `ṅ → N`,
//!    `ñ → N`, `ś → S`, `ṣ → S`, `ḵ → K`), uppercase everything.
//! 2. Take the first ASCII letter as the seed.
//! 3. Walk subsequent letters, mapping each to its Soundex
//!    consonant class (`B/P/F/V/W = 1`, `C/K/G/Q/J/X = 2`,
//!    `D/T = 3`, `L = 4`, `M/N = 5`, `R = 6`, `S/Z = 7`;
//!    vowels reset the duplicate-collapse state).
//! 4. Collapse consecutive equal codes; pad to 4 characters with
//!    `'0'`.
//!
//! Adapter name: `"phonex-ta"`.
//!
//! # Byte-length caveat
//!
//! Every Tamil scalar is **3 bytes** in UTF-8 (U+0B80..=U+0BFF
//! falls in the 3-byte range). The transliterator walks characters
//! via [`str::chars`], never raw bytes, so it never risks slicing a
//! scalar apart. The output uses ASCII plus a small set of
//! Latin-with-diacritic scalars.
//!
//! # Non-Tamil pass-through
//!
//! ASCII characters, whitespace, punctuation, and any scalar
//! outside the Tamil block pass through the transliterator
//! unchanged. The phonex reduction then filters non-letters out.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Tamil → ISO 15919 transliterator.
///
/// A zero-sized value; construct as [`TamilIso15919`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and
/// the discussion of inherent-vowel (schwa) and pulli handling.
///
/// # Example
///
/// ```
/// use stringcheese_ta::TamilIso15919;
///
/// // "tamiḻ" — த + ம + ி + ழ + ்.
/// // த carries inherent schwa (→ ta), ம + ி → mi, ழ + ் → ḻ.
/// assert_eq!(TamilIso15919.encode("தமிழ்"), "tamiḻ");
/// // "nāṉ" — ந + ா + ன + ் ("I").
/// assert_eq!(TamilIso15919.encode("நான்"), "nāṉ");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TamilIso15919;

impl TamilIso15919 {
    /// Encode `text` per the Tamil → ISO 15919 transliteration.
    ///
    /// Every Tamil scalar in the mapping is replaced by its ISO
    /// 15919 counterpart; every scalar outside the mapping (ASCII,
    /// whitespace, other-script letters) passes through unchanged.
    ///
    /// The encoder honors the inherent-schwa convention: a base
    /// consonant with no following matra or pulli emits the
    /// consonant letter followed by `a`; a following matra
    /// overrides the `a`; a following pulli suppresses it.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len().saturating_add(8));

        let mut chars = text.chars().peekable();
        let mut pending_consonant: Option<&'static str> = None;

        while let Some(c) = chars.next() {
            let next = chars.peek().copied();

            if let Some(base) = pending_consonant.take() {
                // Pulli suppresses the schwa.
                if c == '\u{0BCD}' {
                    out.push_str(base);
                    continue;
                }

                if let Some(matra) = matra_iso(c) {
                    out.push_str(base);
                    out.push_str(matra);
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

            // Āytam (ஃ U+0B83) — Tamil-specific aspirate.
            if c == '\u{0B83}' {
                out.push('ḵ');
                continue;
            }

            if let Some(d) = digit_iso(c) {
                out.push(d);
                continue;
            }

            if let Some(matra) = matra_iso(c) {
                // Stray matra with no preceding consonant (should not
                // happen in well-formed Tamil, but pass through
                // gracefully).
                out.push_str(matra);
                continue;
            }

            if c == '\u{0BCD}' {
                // Stray pulli — no schwa to suppress, drop it.
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
        '\u{0B85}' => "a",  // அ
        '\u{0B86}' => "ā",  // ஆ
        '\u{0B87}' => "i",  // இ
        '\u{0B88}' => "ī",  // ஈ
        '\u{0B89}' => "u",  // உ
        '\u{0B8A}' => "ū",  // ஊ
        '\u{0B8E}' => "e",  // எ
        '\u{0B8F}' => "ē",  // ஏ
        '\u{0B90}' => "ai", // ஐ
        '\u{0B92}' => "o",  // ஒ
        '\u{0B93}' => "ō",  // ஓ
        '\u{0B94}' => "au", // ஔ
        _ => return None,
    })
}

/// Map a dependent vowel sign (matra) scalar to its ISO 15919 string.
#[must_use]
pub const fn matra_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0BBE}' => "ā",  // ா
        '\u{0BBF}' => "i",  // ி
        '\u{0BC0}' => "ī",  // ீ
        '\u{0BC1}' => "u",  // ு
        '\u{0BC2}' => "ū",  // ூ
        '\u{0BC6}' => "e",  // ெ
        '\u{0BC7}' => "ē",  // ே
        '\u{0BC8}' => "ai", // ை
        '\u{0BCA}' => "o",  // ொ
        '\u{0BCB}' => "ō",  // ோ
        '\u{0BCC}' => "au", // ௌ
        _ => return None,
    })
}

/// Map a base consonant scalar to its ISO 15919 string.
#[must_use]
pub const fn consonant_iso(c: char) -> Option<&'static str> {
    Some(match c {
        // Native Tamil consonants (18).
        '\u{0B95}' => "k", // க
        '\u{0B99}' => "ṅ", // ங
        '\u{0B9A}' => "c", // ச
        '\u{0B9E}' => "ñ", // ஞ
        '\u{0B9F}' => "ṭ", // ட
        '\u{0BA3}' => "ṇ", // ண
        '\u{0BA4}' => "t", // த
        '\u{0BA8}' => "n", // ந
        '\u{0BA9}' => "ṉ", // ன (alveolar)
        '\u{0BAA}' => "p", // ப
        '\u{0BAE}' => "m", // ம
        '\u{0BAF}' => "y", // ய
        '\u{0BB0}' => "r", // ர
        '\u{0BB1}' => "ṟ", // ற (alveolar tap)
        '\u{0BB2}' => "l", // ல
        '\u{0BB3}' => "ḷ", // ள (retroflex l)
        '\u{0BB4}' => "ḻ", // ழ (retroflex approx)
        '\u{0BB5}' => "v", // வ
        // Grantha additions (5).
        '\u{0B9C}' => "j", // ஜ
        '\u{0BB7}' => "ṣ", // ஷ
        '\u{0BB8}' => "s", // ஸ
        '\u{0BB9}' => "h", // ஹ
        _ => return None,
    })
}

/// Map a Tamil digit (`௦..௯` U+0BE6..=U+0BEF) to its ASCII digit
/// counterpart.
#[must_use]
pub const fn digit_iso(c: char) -> Option<char> {
    match c {
        '\u{0BE6}'..='\u{0BEF}' => {
            let offset = c as u32 - 0x0BE6;
            char::from_u32(0x0030 + offset)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// PHONEX-Tamil — 4-char Soundex-shape reduction over the ISO 15919
// output.
// ---------------------------------------------------------------------

/// The PHONEX-Tamil encoder.
///
/// A zero-sized value; construct as [`TamilPhonex`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_ta::TamilPhonex;
///
/// // "தமிழ்" transliterates to "tamiḻ"; phonex folds ḻ → L.
/// // T seed, A vow reset, M code=5 push, I vow reset, L code=4 push
/// //   → "T54" pad → "T540".
/// assert_eq!(TamilPhonex.encode("தமிழ்").unwrap(), "T540");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TamilPhonex;

impl TamilPhonex {
    /// Encodes `word` per the PHONEX-Tamil algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or reduced to nothing after
    /// filtering). Otherwise returns a 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let transliterated = TamilIso15919.encode(word);
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
/// Tamil-specific alveolar / retroflex distinctions) deliberately
/// map to the same ASCII target — merging them would obscure the
/// linguistic grouping, and the encoder's design intent is that each
/// ISO 15919 diacritic-carrying scalar folds *individually* to its
/// ASCII base for Soundex-shape classification.
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
        'ṇ' | 'Ṇ' => 'N',
        'ḷ' | 'Ḷ' => 'L',
        // Tamil-specific retroflex approximant.
        'ḻ' | 'Ḻ' => 'L',
        // Alveolar under-bars → base.
        'ṉ' | 'Ṉ' => 'N',
        'ṟ' | 'Ṟ' => 'R',
        // Velar / palatal nasal marks → base.
        'ṅ' | 'Ṅ' => 'N',
        'ñ' | 'Ñ' => 'N',
        // Sibilant marks → base.
        'ś' | 'Ś' => 'S',
        'ṣ' | 'Ṣ' => 'S',
        // Āytam (aspirate) — folds to K in ISO 15919 ("ḵ").
        'ḵ' | 'Ḵ' => 'K',
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

/// Adapter that exposes [`TamilPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Tamil::phonetic_encoder`](crate::Tamil) hands back.
///
/// Returns `Some((key, None))` for input with at least one Tamil
/// scalar; returns `None` for input with no Tamil content
/// (transliteration passes through unchanged, which is not a useful
/// key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TamilPhonexAdapter;

impl LanguagePhoneticEncoder for TamilPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_tamil(word) {
            return None;
        }
        let key = TamilPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-ta"
    }
}

/// Does `s` contain at least one scalar in the Tamil UTF-8 block
/// (U+0B80..=U+0BFF)?
fn contains_tamil(s: &str) -> bool {
    s.chars().any(|c| ('\u{0B80}'..='\u{0BFF}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        TamilIso15919.encode(s)
    }

    fn p(w: &str) -> String {
        TamilPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Inherent-vowel (schwa) and pulli handling.
    // -------------------------------------------------------------

    #[test]
    fn bare_consonant_carries_inherent_schwa() {
        // க alone → ka (schwa fires).
        assert_eq!(e("க"), "ka");
    }

    #[test]
    fn pulli_suppresses_schwa() {
        // க + ் → k (no schwa).
        assert_eq!(e("க்"), "k");
    }

    #[test]
    fn matra_overrides_schwa() {
        // க + ி → ki (matra overrides).
        assert_eq!(e("கி"), "ki");
        assert_eq!(e("கீ"), "kī");
        assert_eq!(e("கு"), "ku");
        assert_eq!(e("கூ"), "kū");
        assert_eq!(e("கே"), "kē");
        assert_eq!(e("கோ"), "kō");
        assert_eq!(e("கா"), "kā");
    }

    #[test]
    fn consonant_cluster_via_pulli() {
        // நன்றி (thanks) = ந + ன + ் + ற + ி → na + ṉ + ṟi = naṉṟi.
        assert_eq!(e("நன்றி"), "naṉṟi");
    }

    #[test]
    fn explicit_schwa_retention() {
        // Tamil ISO 15919 retains the inherent schwa on any
        // consonant not followed by pulli or matra.
        // நகர = ந + க + ர → nakara.
        assert_eq!(e("நகர"), "nakara");
    }

    // -------------------------------------------------------------
    // Independent vowels.
    // -------------------------------------------------------------

    #[test]
    fn independent_vowels() {
        assert_eq!(e("அ"), "a");
        assert_eq!(e("ஆ"), "ā");
        assert_eq!(e("இ"), "i");
        assert_eq!(e("ஈ"), "ī");
        assert_eq!(e("உ"), "u");
        assert_eq!(e("ஊ"), "ū");
        assert_eq!(e("எ"), "e");
        assert_eq!(e("ஏ"), "ē");
        assert_eq!(e("ஐ"), "ai");
        assert_eq!(e("ஒ"), "o");
        assert_eq!(e("ஓ"), "ō");
        assert_eq!(e("ஔ"), "au");
    }

    // -------------------------------------------------------------
    // Tamil-specific alveolar and retroflex distinctions.
    // -------------------------------------------------------------

    #[test]
    fn alveolar_n_encodes_as_n_underdot() {
        // ன (alveolar n) is distinct from ந (dental n) — both fold to
        // ISO 15919 forms 'ṉ' and 'n' respectively.
        assert_eq!(e("ன்"), "ṉ");
        assert_eq!(e("ந்"), "n");
    }

    #[test]
    fn alveolar_r_encodes_as_r_underdot() {
        // ற (alveolar tap) is distinct from ர (dental r).
        assert_eq!(e("ற்"), "ṟ");
        assert_eq!(e("ர்"), "r");
    }

    #[test]
    fn retroflex_approx_l_encodes_correctly() {
        // ழ (the famous "ḻ" of tamiḻ).
        assert_eq!(e("ழ்"), "ḻ");
        // ள (retroflex lateral).
        assert_eq!(e("ள்"), "ḷ");
        // ல (dental lateral).
        assert_eq!(e("ல்"), "l");
    }

    #[test]
    fn tamil_famous_word() {
        // தமிழ் = "tamiḻ" — the language's own name in its script.
        assert_eq!(e("தமிழ்"), "tamiḻ");
    }

    // -------------------------------------------------------------
    // Consonants — spot checks.
    // -------------------------------------------------------------

    #[test]
    fn retroflex_series_encodes_with_dots_below() {
        assert_eq!(e("ட"), "ṭa");
        assert_eq!(e("ண"), "ṇa");
    }

    #[test]
    fn grantha_loans_encode_correctly() {
        assert_eq!(e("ஷ"), "ṣa");
        assert_eq!(e("ஸ"), "sa");
        assert_eq!(e("ஹ"), "ha");
        assert_eq!(e("ஜ"), "ja");
    }

    // -------------------------------------------------------------
    // Digits.
    // -------------------------------------------------------------

    #[test]
    fn tamil_digits_encode_to_ascii() {
        assert_eq!(e("௨௦௨௬"), "2026");
        assert_eq!(e("௦"), "0");
        assert_eq!(e("௯"), "9");
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
    fn mixed_content_passes_through_non_tamil() {
        assert_eq!(e("hello தமிழ்"), "hello tamiḻ");
    }

    // -------------------------------------------------------------
    // PHONEX-Tamil.
    // -------------------------------------------------------------

    #[test]
    fn phonex_empty_input_returns_none() {
        assert!(TamilPhonex.encode("").is_none());
        assert!(TamilPhonex.encode("   ").is_none());
    }

    #[test]
    fn phonex_tamiz_encodes_correctly() {
        // தமிழ் → tamiḻ → TAMIL → T seed, A vow, M code=5 push,
        // I vow, L code=4 push → "T54" pad → "T540".
        assert_eq!(p("தமிழ்"), "T540");
    }

    #[test]
    fn phonex_nan_encodes_correctly() {
        // நான் → nāṉ → NAN → N seed last=5; A vow last=0;
        // N code=5 ≠ 0, push '5', out="N5", last=5. Pad "N500".
        assert_eq!(p("நான்"), "N500");
    }

    #[test]
    fn phonex_long_vowels_fold_to_short() {
        // க → "ka" → K000. கா → "kā" → K000. Same key.
        assert_eq!(TamilPhonex.encode("க"), TamilPhonex.encode("கா"));
    }

    #[test]
    fn phonex_retroflex_folds_to_base() {
        // ட (ṭa) and த (ta) — retroflex fold makes them share the
        // reduced letter T. Both → "T000".
        assert_eq!(TamilPhonex.encode("ட"), TamilPhonex.encode("த"));
    }

    #[test]
    fn phonex_alveolar_and_dental_nasals_share_a_key() {
        // ன (ṉa) and ந (na) both fold to N.
        assert_eq!(TamilPhonex.encode("ன"), TamilPhonex.encode("ந"));
    }

    #[test]
    fn phonex_all_l_variants_share_a_key() {
        // ல (la), ள (ḷa), ழ (ḻa) all fold to L.
        let a = TamilPhonex.encode("ல").unwrap();
        let b = TamilPhonex.encode("ள").unwrap();
        let c = TamilPhonex.encode("ழ").unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_ta() {
        assert_eq!(TamilPhonexAdapter.name(), "phonex-ta");
    }

    #[test]
    fn adapter_returns_some_for_tamil() {
        let out = TamilPhonexAdapter.encode("தமிழ்");
        assert_eq!(out, Some((String::from("T540"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_tamil() {
        assert!(TamilPhonexAdapter.encode("").is_none());
        assert!(TamilPhonexAdapter.encode("hello").is_none());
        assert!(TamilPhonexAdapter.encode("123").is_none());
    }
}
