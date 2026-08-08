//! Marathi → ISO 15919 → PHONEX-Marathi phonetic encoder.
//!
//! # Origin
//!
//! Marathi is the second Devanagari-script pack in StringCheese
//! (after Hindi's `stringcheese-hi`), so the transliteration table
//! is largely inherited from the Hindi IAST encoder — the two
//! languages share the same base 33-consonant / 10-vowel inventory
//! and the same abugida script model. The mapping in this module is
//! a **Marathi-tuned ISO 15919** variant with two additions that
//! reflect Marathi orthography beyond Hindi's:
//!
//! * **`ळ` (U+0933) — retroflex L** — a distinct Marathi phoneme
//!   /ɭ/ that Standard Modern Hindi lacks. Rendered `ḷ` in ISO 15919
//!   (with a dot below).
//! * **`ऱ` (U+0931) — R with lower diagonal** — a preserved sound
//!   used in Marathi and some other Indo-Aryan varieties. Rendered
//!   `ṟ` in ISO 15919 (with a bar below).
//!
//! Two candidate approaches present themselves for a Marathi phonetic
//! hookup:
//!
//! * **Pure ISO 15919 transliteration.** Faithful to the script but
//!   not a *phonetic-equivalence* encoder — two words that sound
//!   alike don't necessarily transliterate alike, and Marathi's
//!   active schwa-deletion (`कमळ` "lotus" is pronounced `kamaḷ`, not
//!   `kamaḷa`) makes the surface transliteration diverge from the
//!   spoken form.
//! * **PHONEX-Marathi.** Transliterate to Latin first, then apply a
//!   Soundex-shape 4-character reduction that folds long / short
//!   vowels, drops retroflex under-dots, and collapses consonant-class
//!   duplicates. Produces sound-alike equivalence classes and matches
//!   the shape of the Bengali pack (`phonex-bn`) and every other
//!   Latin-alphabet pack (`phonex-nl`, `phonex-pt`, `phonex-fr`, …).
//!
//! # Implementation choice — two-stage: ISO 15919 then PHONEX
//!
//! This module ships both stages:
//!
//! 1. **[`MarathiIso15919`]** — the deterministic
//!    Devanagari → Latin transliteration, honoring the inherent-schwa
//!    convention (see below), and including the two Marathi additions
//!    `ळ`/`ऱ`. Public because it's useful in its own right
//!    (data-migration tools, IR display).
//! 2. **[`MarathiPhonex`]** — the 4-character Soundex-shape key
//!    computed *over* the ISO 15919 output. This is the encoder
//!    [`MarathiPhonexAdapter`] wraps for the
//!    [`LanguagePhoneticEncoder`] trait hookup; adapter name
//!    `"phonex-mr"`.
//!
//! # Inherent-vowel (schwa) handling
//!
//! Devanagari is an abugida — every base consonant carries an
//! implicit `a` (schwa) vowel unless a dependent vowel sign (matra)
//! or a virama (`्` U+094D) overrides it. So `क` alone represents
//! `ka` (not just `k`), `क्` (`क` + virama) represents bare `k`, and
//! `कि` (`क` + matra `ि`) represents `ki`.
//!
//! The transliteration honors the inherent schwa **without applying
//! schwa-deletion rules**. Modern colloquial Marathi drops the schwa
//! in many word-final and some medial positions (`राम` is pronounced
//! `rām`, not `rāma`), but the deletion is context-dependent and
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
//! | Devanagari | ISO 15919 | Notes                    |
//! |------------|-----------|--------------------------|
//! | `अ`        | `a`       | schwa                    |
//! | `आ`        | `ā`       | long a                   |
//! | `इ`        | `i`       | short i                  |
//! | `ई`        | `ī`       | long i                   |
//! | `उ`        | `u`       | short u                  |
//! | `ऊ`        | `ū`       | long u                   |
//! | `ऋ`        | `r̥`       | vocalic r                |
//! | `ए`        | `e`       |                          |
//! | `ऐ`        | `ai`      |                          |
//! | `ओ`        | `o`       |                          |
//! | `औ`        | `au`      |                          |
//!
//! ## Dependent vowel signs (matras) — appear after a consonant
//!
//! | Devanagari | ISO 15919 | Notes                    |
//! |------------|-----------|--------------------------|
//! | (none)     | `a`       | inherent schwa           |
//! | `ा`        | `ā`       |                          |
//! | `ि`        | `i`       |                          |
//! | `ी`        | `ī`       |                          |
//! | `ु`        | `u`       |                          |
//! | `ू`        | `ū`       |                          |
//! | `ृ`        | `r̥`       |                          |
//! | `े`        | `e`       |                          |
//! | `ै`        | `ai`      |                          |
//! | `ो`        | `o`       |                          |
//! | `ौ`        | `au`      |                          |
//!
//! ## Virama and other marks
//!
//! | Devanagari | ISO 15919 | Notes                                |
//! |------------|-----------|--------------------------------------|
//! | `्`        | (∅)       | virama — suppresses inherent schwa   |
//! | `ं`        | `ṁ`       | anusvara                             |
//! | `ँ`        | `m̐`       | chandrabindu                         |
//! | `ः`        | `ḥ`       | visarga                              |
//! | `़`        | (∅)       | nukta — passed through; the           |
//! |            |           | precomposed nukta letters have their |
//! |            |           | own entries                          |
//!
//! ## Consonants — the 33 classical stops, sonorants, sibilants,
//! ## plus the two Marathi additions
//!
//! Base 33 consonants are inherited from the shared Devanagari
//! inventory (see the `stringcheese-hi` pack's IAST table for the
//! full listing). The Marathi-specific additions are:
//!
//! | Devanagari | ISO 15919 | Group                                    |
//! |------------|-----------|------------------------------------------|
//! | `ळ` U+0933 | `ḷ`       | retroflex L — a distinct Marathi phoneme |
//! | `ऱ` U+0931 | `ṟ`       | R with lower diagonal — preserved sound  |
//!
//! ## Nukta letters (Perso-Arabic loans, same as Hindi)
//!
//! Both the precomposed forms (single scalars) and the decomposed
//! forms (base + `़`) are handled — the encoder tracks the nukta
//! flag as it walks so a `ज` + `़` sequence encodes the same as
//! precomposed `ज़`.
//!
//! ## Digits
//!
//! Devanagari digits `०..९` (U+0966..=U+096F) map to ASCII digits
//! `0..9`.
//!
//! # PHONEX-Marathi reduction
//!
//! After transliteration, the encoder applies a Soundex-shape
//! 4-character reduction, matching the shape of the Bengali pack:
//!
//! 1. Fold Latin-with-diacritic scalars to their ASCII base
//!    (`ā → a`, `ī → i`, `ū → u`, `ṭ → t`, `ḍ → d`, `ṇ → n`,
//!    `ṅ → n`, `ñ → n`, `ś → s`, `ṣ → s`, `ṛ → r`, `ḷ → l`,
//!    `ṟ → r`, `ṁ → m`, `ḥ → h`, `ġ → g`), uppercase everything.
//! 2. Take the first ASCII letter as the seed.
//! 3. Walk subsequent letters, mapping each to its Soundex
//!    consonant class (`B/P/F/V/W = 1`, `C/K/G/Q/J/X = 2`,
//!    `D/T = 3`, `L = 4`, `M/N = 5`, `R = 6`, `S/Z = 7`;
//!    vowels reset the duplicate-collapse state).
//! 4. Collapse consecutive equal codes; pad to 4 characters with
//!    `'0'`.
//!
//! Adapter name: `"phonex-mr"`.
//!
//! # Byte-length caveat
//!
//! Every Devanagari scalar is **3 bytes** in UTF-8 (U+0900..=U+097F
//! falls in the 3-byte range). The transliterator walks characters
//! via [`str::chars`], never raw bytes, so it never risks slicing a
//! scalar apart. The output uses ASCII plus a small set of
//! Latin-with-diacritic scalars.
//!
//! # Attribution
//!
//! The Devanagari letter tables (independent vowels, matras,
//! consonants, combining marks, nukta variants, digits) below are
//! structurally adapted from the `stringcheese-hi` pack's IAST
//! encoder: the two languages share the identical base script, and
//! the mapping is essentially bijective letter-for-letter. This
//! module adds the two Marathi-specific consonants `ळ`/`ऱ`, uses
//! the ISO 15919 `ṁ`/`r̥` marks in place of Hindi's IAST `ṃ`/`ṛ`
//! variants for consistency with the Bengali pack, and layers the
//! Soundex-shape reduction on top.
//!
//! # Non-Devanagari pass-through
//!
//! ASCII characters, whitespace, punctuation, and any scalar outside
//! the Devanagari block pass through the transliterator unchanged.
//! The phonex reduction then filters non-letters out.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Marathi → ISO 15919 transliterator.
///
/// A zero-sized value; construct as [`MarathiIso15919`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and
/// the discussion of inherent-vowel (schwa) handling.
///
/// # Example
///
/// ```
/// use stringcheese_mr::MarathiIso15919;
///
/// // "namaste" — न + म + स + ् + त + े.
/// // The virama suppresses the schwa on स; the े matra provides the
/// // final vowel on त.
/// assert_eq!(MarathiIso15919.encode("नमस्ते"), "namaste");
/// // "kamaḷa" — क + म + ळ. The Marathi-specific ळ (U+0933) → ḷ.
/// assert_eq!(MarathiIso15919.encode("कमळ"), "kamaḷa");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MarathiIso15919;

impl MarathiIso15919 {
    /// Encode `text` per the Devanagari → ISO 15919 transliteration.
    ///
    /// Every Devanagari scalar in the mapping is replaced by its ISO
    /// 15919 counterpart (one or more Latin characters); every scalar
    /// outside the mapping (ASCII, whitespace, other-script letters)
    /// passes through unchanged.
    ///
    /// The encoder honors the inherent-schwa convention: a base
    /// consonant with no following matra or virama emits the
    /// consonant letter followed by `a`; a following matra overrides
    /// the `a`; a following virama suppresses it.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        // Preallocate roughly the same size — ISO output for pure
        // Devanagari input is typically ~1.5x the character count in
        // bytes because most consonants emit letter + `a` (2 bytes)
        // and the diacritic-carrying ASCII-extended scalars are
        // 2 bytes each.
        let mut out = String::with_capacity(text.len().saturating_add(8));

        // Walk the input with a one-scalar lookahead. State: `pending`
        // is `Some(letter)` when the *previous* scalar was a base
        // consonant whose inherent-schwa emission has not yet been
        // decided (i.e. we haven't yet seen whether the next scalar
        // is a matra or a virama). When we look at the current scalar
        // we resolve any pending schwa first, then classify the
        // current scalar.
        let mut chars = text.chars().peekable();
        // Tracks the pending base consonant's ISO letter (already
        // encoded, but the `a` hasn't been emitted yet).
        let mut pending_consonant: Option<&'static str> = None;
        // Tracks whether the pending consonant was followed by a
        // nukta scalar — if so, we may need to re-emit it as its
        // nukta-modified counterpart when we resolve the schwa.
        let mut pending_nukta = false;

        while let Some(c) = chars.next() {
            let next = chars.peek().copied();

            // A base consonant is queued. Decide whether the current
            // scalar overrides its schwa or not.
            if let Some(base) = pending_consonant.take() {
                // If the current scalar is a nukta and we haven't
                // already applied one, mark it and continue — the
                // nukta modifies the still-pending consonant.
                if c == '\u{093C}' && !pending_nukta {
                    if let Some(nukta_form) = nukta_variant(base) {
                        pending_consonant = Some(nukta_form);
                    } else {
                        pending_consonant = Some(base);
                    }
                    pending_nukta = true;
                    continue;
                }

                // Reset the nukta flag — we're moving off this
                // consonant.
                pending_nukta = false;

                // Virama suppresses the schwa.
                if c == '\u{094D}' {
                    out.push_str(base);
                    continue;
                }

                // A dependent vowel sign (matra) overrides the schwa.
                if let Some(matra) = matra_iso(c) {
                    out.push_str(base);
                    out.push_str(matra);
                    continue;
                }

                // Anusvara / visarga / chandrabindu attach *after*
                // the schwa. Emit `base + a + mark`.
                if let Some(mark) = combining_mark_iso(c) {
                    out.push_str(base);
                    out.push('a');
                    out.push_str(mark);
                    continue;
                }

                // Anything else: the pending consonant keeps its
                // schwa. Emit `base + a`, then re-process `c` in the
                // main dispatch below.
                out.push_str(base);
                out.push('a');
                // Fall through to the main dispatch to handle `c`.
            }

            // Independent vowel?
            if let Some(v) = independent_vowel_iso(c) {
                out.push_str(v);
                continue;
            }

            // Base consonant? Queue it — we need to see the next
            // scalar to decide whether the schwa fires.
            if let Some(cons) = consonant_iso(c) {
                pending_consonant = Some(cons);
                pending_nukta = false;
                // If this is the last scalar, the schwa fires by
                // default. Handled by the post-loop drain below.
                if next.is_none() {
                    // Drain immediately.
                    out.push_str(cons);
                    out.push('a');
                    pending_consonant = None;
                }
                continue;
            }

            // Digit?
            if let Some(d) = digit_iso(c) {
                out.push(d);
                continue;
            }

            // Combining mark seen without a pending consonant
            // (unusual — usually preceded by one). Emit the mark's
            // ISO alone.
            if let Some(mark) = combining_mark_iso(c) {
                out.push_str(mark);
                continue;
            }

            // Matra seen without a pending consonant (unusual — the
            // matra normally follows one). Emit its ISO alone.
            if let Some(matra) = matra_iso(c) {
                out.push_str(matra);
                continue;
            }

            // Virama seen without a pending consonant — pass through
            // silently (no-op).
            if c == '\u{094D}' {
                continue;
            }

            // Nukta without a preceding consonant — pass through
            // silently.
            if c == '\u{093C}' {
                continue;
            }

            // Anything else — pass through unchanged.
            out.push(c);
        }

        // Drain any still-pending consonant (last scalar was a bare
        // consonant with no override).
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
        '\u{0905}' => "a",  // अ
        '\u{0906}' => "ā",  // आ
        '\u{0907}' => "i",  // इ
        '\u{0908}' => "ī",  // ई
        '\u{0909}' => "u",  // उ
        '\u{090A}' => "ū",  // ऊ
        '\u{090B}' => "r̥",  // ऋ
        '\u{090F}' => "e",  // ए
        '\u{0910}' => "ai", // ऐ
        '\u{0913}' => "o",  // ओ
        '\u{0914}' => "au", // औ
        _ => return None,
    })
}

/// Map a dependent vowel sign (matra) scalar to its ISO 15919 string.
#[must_use]
pub const fn matra_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{093E}' => "ā",  // ा
        '\u{093F}' => "i",  // ि
        '\u{0940}' => "ī",  // ी
        '\u{0941}' => "u",  // ु
        '\u{0942}' => "ū",  // ू
        '\u{0943}' => "r̥",  // ृ
        '\u{0947}' => "e",  // े
        '\u{0948}' => "ai", // ै
        '\u{094B}' => "o",  // ो
        '\u{094C}' => "au", // ौ
        _ => return None,
    })
}

/// Map a combining vowel-modifier scalar to its ISO 15919 string.
#[must_use]
pub const fn combining_mark_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0902}' => "ṁ", // ं anusvara (ISO 15919 form; note Hindi's IAST uses ṃ)
        '\u{0901}' => "m̐", // ँ chandrabindu
        '\u{0903}' => "ḥ", // ः visarga
        _ => return None,
    })
}

/// Map a base consonant scalar to its ISO 15919 string.
///
/// The `match_same_arms` lint is suppressed because ख (U+0916) and
/// the precomposed nukta variant ख़ (U+0959) *deliberately* map to
/// the same ISO string `kh` — the uvular / non-uvular distinction
/// is not carried in this deterministic mapping.
#[must_use]
#[allow(clippy::match_same_arms)]
pub const fn consonant_iso(c: char) -> Option<&'static str> {
    Some(match c {
        // Velars.
        '\u{0915}' => "k",  // क
        '\u{0916}' => "kh", // ख
        '\u{0917}' => "g",  // ग
        '\u{0918}' => "gh", // घ
        '\u{0919}' => "ṅ",  // ङ
        // Palatals.
        '\u{091A}' => "c",  // च
        '\u{091B}' => "ch", // छ
        '\u{091C}' => "j",  // ज
        '\u{091D}' => "jh", // झ
        '\u{091E}' => "ñ",  // ञ
        // Retroflexes.
        '\u{091F}' => "ṭ",  // ट
        '\u{0920}' => "ṭh", // ठ
        '\u{0921}' => "ḍ",  // ड
        '\u{0922}' => "ḍh", // ढ
        '\u{0923}' => "ṇ",  // ण
        // Dentals.
        '\u{0924}' => "t",  // त
        '\u{0925}' => "th", // थ
        '\u{0926}' => "d",  // द
        '\u{0927}' => "dh", // ध
        '\u{0928}' => "n",  // न
        // Marathi-specific: ऩ (U+0929) — historically a Nagari-Tamil
        // variant of `n`; treated as bare `n` in the mapping.
        '\u{0929}' => "n", // ऩ
        // Labials.
        '\u{092A}' => "p",  // प
        '\u{092B}' => "ph", // फ
        '\u{092C}' => "b",  // ब
        '\u{092D}' => "bh", // भ
        '\u{092E}' => "m",  // म
        // Semivowels and liquids.
        '\u{092F}' => "y", // य
        '\u{0930}' => "r", // र
        // Marathi-specific: ऱ (U+0931) — R with lower diagonal;
        // rendered `ṟ` in ISO 15919.
        '\u{0931}' => "ṟ", // ऱ
        '\u{0932}' => "l", // ल
        '\u{0935}' => "v", // व
        // Sibilants and h.
        '\u{0936}' => "ś", // श
        '\u{0937}' => "ṣ", // ष
        '\u{0938}' => "s", // स
        '\u{0939}' => "h", // ह
        // Marathi-specific: ळ (U+0933) — retroflex L; rendered `ḷ`
        // in ISO 15919. Distinguishes Marathi from Standard Modern
        // Hindi.
        '\u{0933}' => "ḷ", // ळ
        // Precomposed nukta consonants — treated as bare consonants
        // (the nukta modification is baked into the ISO letter).
        '\u{0958}' => "q",  // क़
        '\u{0959}' => "kh", // ख़
        '\u{095A}' => "ġ",  // ग़
        '\u{095B}' => "z",  // ज़
        '\u{095C}' => "ṛ",  // ड़
        '\u{095D}' => "ṛh", // ढ़
        '\u{095E}' => "f",  // फ़
        _ => return None,
    })
}

/// If `base` is the ISO form of a base consonant that has a nukta
/// variant, return the nukta variant's ISO form.
///
/// This is used when the encoder sees a decomposed `base + ़`
/// sequence (rather than a precomposed nukta letter).
#[must_use]
fn nukta_variant(base: &'static str) -> Option<&'static str> {
    match base {
        "k" => Some("q"),   // क + ़ → क़ → q
        "kh" => Some("kh"), // ख + ़ → ख़ (uvular — same ISO)
        "g" => Some("ġ"),   // ग + ़ → ग़ → ġ
        "j" => Some("z"),   // ज + ़ → ज़ → z
        "ḍ" => Some("ṛ"),   // ड + ़ → ड़ → ṛ
        "ḍh" => Some("ṛh"), // ढ + ़ → ढ़ → ṛh
        "ph" => Some("f"),  // फ + ़ → फ़ → f
        _ => None,
    }
}

/// Map a Devanagari digit (`०..९` U+0966..=U+096F) to its ASCII digit
/// counterpart.
#[must_use]
pub const fn digit_iso(c: char) -> Option<char> {
    match c {
        '\u{0966}'..='\u{096F}' => {
            let offset = c as u32 - 0x0966;
            char::from_u32(0x0030 + offset)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// PHONEX-Marathi — 4-char Soundex-shape reduction over the ISO 15919
// output.
// ---------------------------------------------------------------------

/// The PHONEX-Marathi encoder.
///
/// A zero-sized value; construct as [`MarathiPhonex`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_mr::MarathiPhonex;
///
/// // "राम" transliterates to "rāma"; phonex reduces to R500.
/// // R seed, A vowel (reset), M code=5, A vowel (reset)
/// //   → "R5" pad → "R500".
/// assert_eq!(MarathiPhonex.encode("राम").unwrap(), "R500");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MarathiPhonex;

impl MarathiPhonex {
    /// Encodes `word` per the PHONEX-Marathi algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or reduced to nothing after
    /// filtering). Otherwise returns a 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let transliterated = MarathiIso15919.encode(word);
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
        // Marathi-specific letter folds — retroflex L and R-with-bar
        // fold to their ASCII bases for phonex-shape classification.
        'ḷ' | 'Ḷ' => 'L',
        'ṟ' | 'Ṟ' => 'R',
        // Combining-mark ISO forms.
        'ṁ' | 'Ṁ' => 'M',
        'ḥ' | 'Ḥ' => 'H',
        // Perso-Arabic loan letter ġ (from ग़).
        'ġ' | 'Ġ' => 'G',
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

/// Adapter that exposes [`MarathiPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Marathi::phonetic_encoder`](crate::Marathi) hands back.
///
/// Returns `Some((key, None))` for input with at least one Devanagari
/// scalar; returns `None` for input with no Devanagari content
/// (transliteration passes through unchanged, which is not a useful
/// key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MarathiPhonexAdapter;

impl LanguagePhoneticEncoder for MarathiPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_devanagari(word) {
            return None;
        }
        let key = MarathiPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-mr"
    }
}

/// Does `s` contain at least one scalar in the Devanagari UTF-8
/// block (U+0900..=U+097F)?
fn contains_devanagari(s: &str) -> bool {
    s.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        MarathiIso15919.encode(s)
    }

    fn p(w: &str) -> String {
        MarathiPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Inherent-vowel (schwa) handling.
    // -------------------------------------------------------------

    #[test]
    fn bare_consonant_carries_inherent_schwa() {
        // क alone → ka (schwa fires).
        assert_eq!(e("क"), "ka");
    }

    #[test]
    fn virama_suppresses_schwa() {
        // क + ् → k (no schwa).
        assert_eq!(e("क्"), "k");
    }

    #[test]
    fn matra_overrides_schwa() {
        assert_eq!(e("कि"), "ki");
        assert_eq!(e("की"), "kī");
        assert_eq!(e("कु"), "ku");
        assert_eq!(e("कू"), "kū");
        assert_eq!(e("के"), "ke");
        assert_eq!(e("को"), "ko");
        assert_eq!(e("का"), "kā");
    }

    #[test]
    fn consonant_cluster_via_virama() {
        // सत्य = स + त + ् + य → sat + ya = satya.
        assert_eq!(e("सत्य"), "satya");
    }

    #[test]
    fn explicit_schwa_retention() {
        // राम = र + ा + म → rāma (final म carries schwa).
        assert_eq!(e("राम"), "rāma");
    }

    // -------------------------------------------------------------
    // Marathi-specific letters.
    // -------------------------------------------------------------

    #[test]
    fn marathi_retroflex_l_encodes_to_dot_l() {
        // ळ (U+0933) → ḷ. Marathi-specific — Standard Hindi lacks it.
        assert_eq!(e("ळ"), "ḷa");
        // कमळ "lotus" — क + म + ळ → kamaḷa (all three consonants
        // carry inherent schwa).
        assert_eq!(e("कमळ"), "kamaḷa");
    }

    #[test]
    fn marathi_r_with_lower_diagonal_encodes_to_bar_r() {
        // ऱ (U+0931) → ṟ.
        assert_eq!(e("ऱ"), "ṟa");
    }

    // -------------------------------------------------------------
    // Independent vowels.
    // -------------------------------------------------------------

    #[test]
    fn independent_vowels() {
        assert_eq!(e("अ"), "a");
        assert_eq!(e("आ"), "ā");
        assert_eq!(e("इ"), "i");
        assert_eq!(e("ई"), "ī");
        assert_eq!(e("उ"), "u");
        assert_eq!(e("ऊ"), "ū");
        assert_eq!(e("ए"), "e");
        assert_eq!(e("ऐ"), "ai");
        assert_eq!(e("ओ"), "o");
        assert_eq!(e("औ"), "au");
    }

    // -------------------------------------------------------------
    // Combining marks.
    // -------------------------------------------------------------

    #[test]
    fn anusvara_encodes_to_dot_m() {
        // हैं = ह + ै + ं → haiṁ (ISO 15919 form).
        assert_eq!(e("हैं"), "haiṁ");
    }

    #[test]
    fn visarga_encodes_to_dot_h() {
        assert_eq!(e("अः"), "aḥ");
    }

    // -------------------------------------------------------------
    // Nukta.
    // -------------------------------------------------------------

    #[test]
    fn decomposed_nukta_matches_precomposed() {
        // ज + ़ (decomposed) should encode the same as ज़ (precomposed).
        let decomposed: String = "\u{091C}\u{093C}".into();
        let precomposed: String = "\u{095B}".into();
        assert_eq!(e(&decomposed), e(&precomposed));
        assert_eq!(e(&precomposed), "za");
    }

    // -------------------------------------------------------------
    // Digits.
    // -------------------------------------------------------------

    #[test]
    fn devanagari_digits_encode_to_ascii() {
        assert_eq!(e("२०२६"), "2026");
        assert_eq!(e("०"), "0");
        assert_eq!(e("९"), "9");
    }

    // -------------------------------------------------------------
    // Pass-through.
    // -------------------------------------------------------------

    #[test]
    fn ascii_passes_through() {
        assert_eq!(e(""), "");
        assert_eq!(e("hello"), "hello");
    }

    // -------------------------------------------------------------
    // PHONEX-Marathi.
    // -------------------------------------------------------------

    #[test]
    fn phonex_empty_input_returns_none() {
        assert!(MarathiPhonex.encode("").is_none());
        assert!(MarathiPhonex.encode("   ").is_none());
    }

    #[test]
    fn phonex_rama_encodes_to_r500() {
        // राम → rāma → RAMA → R seed, A vow, M code=5, A vow reset → "R5" → "R500".
        assert_eq!(p("राम"), "R500");
    }

    #[test]
    fn phonex_marathi_retroflex_l_folds_to_l() {
        // ळ → ḷa → LA → L seed, A vow reset → "L" → "L000".
        assert_eq!(p("ळ"), "L000");
        // ल (dental l) → la → LA → L000. Same key as ळ under phonex-
        // level folding — the retroflex/dental distinction is not
        // carried into the phonex key.
        assert_eq!(MarathiPhonex.encode("ळ"), MarathiPhonex.encode("ल"));
    }

    #[test]
    fn phonex_marathi_r_with_bar_folds_to_r() {
        // ऱ → ṟa → RA → R seed, A vow reset → "R" → "R000".
        assert_eq!(p("ऱ"), "R000");
        // र (dental r) → ra → RA → R000. Same key as ऱ under phonex-
        // level folding.
        assert_eq!(MarathiPhonex.encode("ऱ"), MarathiPhonex.encode("र"));
    }

    #[test]
    fn phonex_long_vowels_fold_to_short() {
        // Long vowel matra ā and short schwa a both fold to A → vowel
        // reset. So रम and राम differ only in the presence of ā,
        // which doesn't matter to the phonex code.
        assert_eq!(MarathiPhonex.encode("रम"), MarathiPhonex.encode("राम"));
    }

    #[test]
    fn phonex_retroflex_folds_to_base() {
        // ट (ṭa) and त (ta) — retroflex fold makes them share the
        // reduced letter T. Both → "T000".
        assert_eq!(MarathiPhonex.encode("ट"), MarathiPhonex.encode("त"));
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_mr() {
        assert_eq!(MarathiPhonexAdapter.name(), "phonex-mr");
    }

    #[test]
    fn adapter_returns_some_for_devanagari() {
        let out = MarathiPhonexAdapter.encode("राम");
        assert_eq!(out, Some((String::from("R500"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_devanagari() {
        assert!(MarathiPhonexAdapter.encode("").is_none());
        assert!(MarathiPhonexAdapter.encode("hello").is_none());
        assert!(MarathiPhonexAdapter.encode("123").is_none());
    }
}
