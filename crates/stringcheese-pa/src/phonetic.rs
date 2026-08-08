//! Punjabi (Gurmukhi) → ISO 15919 → PHONEX-Punjabi phonetic encoder.
//!
//! # Origin
//!
//! Punjabi is the first Gurmukhi-script pack in StringCheese and (with
//! Malayalam) the fifth Brahmic-script pack after Devanagari (Hindi),
//! Bengali, Tamil, and Malayalam. The Gurmukhi script
//! (U+0A00..=U+0A7F) is an abugida in the Brahmic family — same
//! processing model as `stringcheese-hi`, `stringcheese-bn`,
//! `stringcheese-ta`, and `stringcheese-ml` — but Punjabi's phonology
//! carries **one unusual property** the encoder has to account for:
//!
//! * **Punjabi is a tonal language.** Historical Sanskrit-inherited
//!   voiced aspirates (`ਘ`/`ਝ`/`ਢ`/`ਧ`/`ਭ` — the letters that in
//!   Devanagari and Bengali represent `gh`/`jh`/`ḍh`/`dh`/`bh`) have
//!   **lost their aspiration and voicing** in modern Punjabi. What
//!   remains is a *tone contour* on the adjacent vowel — low tone
//!   when the historical aspirate begins the syllable, high tone
//!   when it ends the syllable. The letter shapes are still on
//!   Sikh signs and religious texts, but the phone is now the same
//!   as the corresponding voiceless-unaspirated stop.
//!
//! # Implementation choice — two-stage: ISO 15919 then tone-collapsed PHONEX
//!
//! This module ships both stages:
//!
//! 1. **[`PunjabiIso15919`]** — the deterministic Gurmukhi → Latin
//!    transliteration, honoring the inherent-schwa convention, the
//!    virama-suppression rule, addak-driven gemination, and
//!    tippi/bindi nasalization. **Preserves** `gh`/`jh`/`ḍh`/`dh`/`bh`
//!    faithfully so callers using the transliterator for scholarly
//!    romanization or data-migration get the ISO-standard output.
//!    Public because it's useful in its own right.
//! 2. **[`PunjabiPhonex`]** — the 4-character Soundex-shape key
//!    computed *over* a tone-collapsed variant of the ISO 15919
//!    output. Before Soundex reduction the historical voiced
//!    aspirates are folded to their voiceless-unaspirated
//!    counterparts (`gh → k`, `jh → c`, `ḍh → ṭ`, `dh → t`,
//!    `bh → p`) so that tone-marked and unmarked forms of the same
//!    word share a key. This is the encoder [`PunjabiPhonexAdapter`]
//!    wraps for the [`LanguagePhoneticEncoder`] trait hookup;
//!    adapter name `"phonex-pa"`.
//!
//! # Inherent-vowel and virama handling — the crucial subtlety
//!
//! Gurmukhi is an **abugida** — every base consonant carries an
//! implicit `a` (schwa) vowel unless a dependent vowel sign (matra)
//! or a **virama** (`੍` U+0A4D, Punjabi's halant) overrides it. So
//! `ਕ` alone represents `ka` (not just `k`), `ਕ੍` (`ਕ` + virama)
//! represents bare `k`, and `ਕਿ` (`ਕ` + matra `ਿ`) represents `ki`.
//!
//! **Addak (`ੱ` U+0A71) geminates the following consonant.** So
//! `ਪੱਕਾ` (pakkā, "ripe") — `ਪ + ੱ + ਕ + ਾ` — decodes as
//! `p` + inherent schwa + gemination-marker + `k` + `ā`, transcribed
//! as `pakkā`. The encoder emits the addak-preceding consonant's
//! schwa, then doubles the following consonant when it arrives.
//!
//! **Tippi (`ੰ` U+0A70) and bindi (`ਂ` U+0A02) mark
//! nasalization.** Tippi typically writes anusvara-style
//! nasalization on short-vowel and consonant environments; bindi
//! writes chandrabindu-style vowel nasalization. Both attach as a
//! nasal to the preceding vowel — this module transliterates tippi
//! as `ṁ` (matching Bengali's anusvara) and bindi as `m̐` (matching
//! Bengali's chandrabindu). Both fold to `M` in the phonex reduction.
//!
//! # ISO 15919 mapping table
//!
//! ## Independent vowels
//!
//! | Gurmukhi | ISO 15919 | Notes                    |
//! |----------|-----------|--------------------------|
//! | `ਅ`      | `a`       | schwa                    |
//! | `ਆ`      | `ā`       | long a                   |
//! | `ਇ`      | `i`       | short i                  |
//! | `ਈ`      | `ī`       | long i                   |
//! | `ਉ`      | `u`       | short u                  |
//! | `ਊ`      | `ū`       | long u                   |
//! | `ਏ`      | `e`       |                          |
//! | `ਐ`      | `ai`      |                          |
//! | `ਓ`      | `o`       |                          |
//! | `ਔ`      | `au`      |                          |
//!
//! ## Dependent vowel signs (matras) — appear after a consonant
//!
//! | Gurmukhi | ISO 15919 | Notes                    |
//! |----------|-----------|--------------------------|
//! | (none)   | `a`       | inherent schwa           |
//! | `ਾ`      | `ā`       |                          |
//! | `ਿ`      | `i`       |                          |
//! | `ੀ`      | `ī`       |                          |
//! | `ੁ`      | `u`       |                          |
//! | `ੂ`      | `ū`       |                          |
//! | `ੇ`      | `e`       |                          |
//! | `ੈ`      | `ai`      |                          |
//! | `ੋ`      | `o`       |                          |
//! | `ੌ`      | `au`      |                          |
//!
//! ## Virama, gemination, nasalization, and other marks
//!
//! | Gurmukhi | ISO 15919 | Notes                                     |
//! |----------|-----------|-------------------------------------------|
//! | `੍`      | (∅)       | virama — suppresses inherent schwa        |
//! | `ੱ`      | (gem)     | addak — doubles the following consonant   |
//! | `ੰ`      | `ṁ`       | tippi — nasalization (anusvara-like)      |
//! | `ਂ`      | `m̐`       | bindi — nasalization (chandrabindu-like)  |
//! | `ਃ`      | `ḥ`       | visarga (rare in Punjabi)                 |
//! | `਼`      | (∅)       | nukta — combines with base; precomposed   |
//! |          |           | forms (`ਖ਼`/`ਗ਼`/`ਜ਼`/`ੜ`/`ਫ਼`) have their  |
//! |          |           | own entries                               |
//!
//! ## The 33 base consonants + 5 nukta letters + retroflex flap
//!
//! Same abugida shape as Devanagari but with Punjabi's tone-bearing
//! reinterpretation of the historical voiced aspirates. The
//! transliterator preserves the ISO 15919 spelling — tone collapse
//! happens only in the PHONEX stage.
//!
//! | Gurmukhi | ISO 15919 | Group          |
//! |----------|-----------|----------------|
//! | `ਕ`      | `k`       | velar          |
//! | `ਖ`      | `kh`      |                |
//! | `ਗ`      | `g`       |                |
//! | `ਘ`      | `gh`      | **tone-bearing** |
//! | `ਙ`      | `ṅ`       | velar nasal    |
//! | `ਚ`      | `c`       | palatal        |
//! | `ਛ`      | `ch`      |                |
//! | `ਜ`      | `j`       |                |
//! | `ਝ`      | `jh`      | **tone-bearing** |
//! | `ਞ`      | `ñ`       | palatal nasal  |
//! | `ਟ`      | `ṭ`       | retroflex      |
//! | `ਠ`      | `ṭh`      |                |
//! | `ਡ`      | `ḍ`       |                |
//! | `ਢ`      | `ḍh`      | **tone-bearing** |
//! | `ਣ`      | `ṇ`       | retroflex nasal|
//! | `ਤ`      | `t`       | dental         |
//! | `ਥ`      | `th`      |                |
//! | `ਦ`      | `d`       |                |
//! | `ਧ`      | `dh`      | **tone-bearing** |
//! | `ਨ`      | `n`       | dental nasal   |
//! | `ਪ`      | `p`       | labial         |
//! | `ਫ`      | `ph`      |                |
//! | `ਬ`      | `b`       |                |
//! | `ਭ`      | `bh`      | **tone-bearing** |
//! | `ਮ`      | `m`       | labial nasal   |
//! | `ਯ`      | `y`       | palatal glide  |
//! | `ਰ`      | `r`       |                |
//! | `ਲ`      | `l`       |                |
//! | `ਵ`      | `v`       |                |
//! | `ਸ`      | `s`       | sibilant       |
//! | `ਹ`      | `h`       |                |
//! | `ਸ਼` / `ਸ` + `਼` | `ś` | palatal sibilant (nukta on s) |
//! | `ਲ਼` / `ਲ` + `਼` | `ḷ` | retroflex lateral (nukta on l) |
//!
//! ## Perso-Arabic loans (nukta letters)
//!
//! Both the precomposed forms and the decomposed base + `਼` forms
//! are handled.
//!
//! | Gurmukhi | ISO 15919 | Notes                    |
//! |----------|-----------|--------------------------|
//! | `ਖ਼` / `ਖ` + `਼` | `x` | voiceless velar fricative (Perso-Arabic) |
//! | `ਗ਼` / `ਗ` + `਼` | `ġ` | voiced velar fricative (Perso-Arabic)    |
//! | `ਜ਼` / `ਜ` + `਼` | `z` | voiced sibilant (Perso-Arabic)           |
//! | `ੜ` / `ਡ` + `਼` | `ṛ` | retroflex flap (native Punjabi)          |
//! | `ਫ਼` / `ਫ` + `਼` | `f` | voiceless labiodental fricative (Perso-Arabic) |
//!
//! ## Digits
//!
//! Gurmukhi digits `੦..੯` (U+0A66..=U+0A6F) map to ASCII digits
//! `0..9`.
//!
//! # PHONEX-Punjabi reduction
//!
//! After transliteration, the encoder applies a Soundex-shape
//! 4-character reduction, matching the shape of the other
//! Latin-alphabet packs:
//!
//! 1. **Tone-collapse:** rewrite the ISO 15919 output to fold the
//!    historical voiced aspirates to their voiceless-unaspirated
//!    counterparts (`gh → k`, `jh → c`, `ḍh → ṭ`, `dh → t`,
//!    `bh → p`). Punjabi's tone letters no longer represent the
//!    voiced-aspirate phones and folding them here makes tone-marked
//!    and unmarked spellings of the same word share a key.
//! 2. Fold Latin-with-diacritic scalars to their ASCII base
//!    (`ā → A`, `ī → I`, `ū → U`, `ṭ → T`, `ḍ → D`, `ṇ → N`,
//!    `ṅ → N`, `ñ → N`, `ś → S`, `ṛ → R`, `ḷ → L`, `ṁ → M`,
//!    `ġ → G`, `ḥ → H`), uppercase everything.
//! 3. Take the first ASCII letter as the seed.
//! 4. Walk subsequent letters, mapping each to its Soundex
//!    consonant class (`B/P/F/V/W = 1`, `C/K/G/Q/J/X = 2`,
//!    `D/T = 3`, `L = 4`, `M/N = 5`, `R = 6`, `S/Z = 7`;
//!    vowels reset the duplicate-collapse state).
//! 5. Collapse consecutive equal codes; pad to 4 characters with
//!    `'0'`.
//!
//! Adapter name: `"phonex-pa"`.
//!
//! # Byte-length caveat
//!
//! Every Gurmukhi scalar is **3 bytes** in UTF-8 (U+0A00..=U+0A7F
//! falls in the 3-byte range). The transliterator walks characters
//! via [`str::chars`], never raw bytes, so it never risks slicing a
//! scalar apart. The output uses ASCII plus a small set of
//! Latin-with-diacritic scalars.
//!
//! # Non-Gurmukhi pass-through
//!
//! ASCII characters, whitespace, punctuation, and any scalar
//! outside the Gurmukhi block pass through the transliterator
//! unchanged. The phonex reduction then filters non-letters out.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Gurmukhi → ISO 15919 transliterator.
///
/// A zero-sized value; construct as [`PunjabiIso15919`] and reuse
/// across threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and
/// the discussion of inherent-vowel, virama, addak, and nasalization
/// handling.
///
/// # Example
///
/// ```
/// use stringcheese_pa::PunjabiIso15919;
///
/// // "pakkā" — ਪ + ੱ + ਕ + ਾ. Addak geminates the following ਕ.
/// assert_eq!(PunjabiIso15919.encode("ਪੱਕਾ"), "pakkā");
/// // "paṁjāba" — ਪ + ੰ + ਜ + ਾ + ਬ. Tippi attaches as nasal.
/// assert_eq!(PunjabiIso15919.encode("ਪੰਜਾਬ"), "paṁjāba");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PunjabiIso15919;

impl PunjabiIso15919 {
    /// Encode `text` per the Gurmukhi → ISO 15919 transliteration.
    ///
    /// Every Gurmukhi scalar in the mapping is replaced by its ISO
    /// 15919 counterpart; every scalar outside the mapping (ASCII,
    /// whitespace, other-script letters) passes through unchanged.
    ///
    /// The encoder honors the inherent-schwa convention: a base
    /// consonant with no following matra or virama emits the
    /// consonant letter followed by `a`; a following matra
    /// overrides the `a`; a virama suppresses it. Addak (`ੱ`)
    /// causes the *next* consonant to be emitted twice (gemination).
    /// Tippi (`ੰ`) and bindi (`ਂ`) attach as nasals to the
    /// preceding vowel.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len().saturating_add(8));

        let mut chars = text.chars().peekable();
        let mut pending_consonant: Option<&'static str> = None;
        let mut pending_nukta = false;
        let mut geminate_next = false;

        while let Some(c) = chars.next() {
            let next = chars.peek().copied();

            if let Some(base) = pending_consonant.take() {
                if c == '\u{0A3C}' && !pending_nukta {
                    if let Some(nukta_form) = nukta_variant(base) {
                        pending_consonant = Some(nukta_form);
                    } else {
                        pending_consonant = Some(base);
                    }
                    pending_nukta = true;
                    continue;
                }

                pending_nukta = false;

                // Virama suppresses the schwa.
                if c == '\u{0A4D}' {
                    out.push_str(base);
                    continue;
                }

                // Addak on the pending consonant: emit its schwa,
                // set the gemination flag for the following consonant.
                if c == '\u{0A71}' {
                    out.push_str(base);
                    out.push('a');
                    geminate_next = true;
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
                // Fall through — the current char still needs dispatch.
            }

            if let Some(v) = independent_vowel_iso(c) {
                out.push_str(v);
                continue;
            }

            if let Some(cons) = consonant_iso(c) {
                if geminate_next {
                    // Gemination: emit the bare consonant first (no
                    // schwa) so it doubles the following one that
                    // becomes the new pending. `ਪੱਕਾ` → `p`+`a`+
                    // `k`+`kā` = `pakkā`.
                    out.push_str(cons);
                    geminate_next = false;
                }
                pending_consonant = Some(cons);
                pending_nukta = false;
                if next.is_none() {
                    out.push_str(cons);
                    out.push('a');
                    pending_consonant = None;
                }
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
                // Stray matra with no preceding consonant (should not
                // happen in well-formed Gurmukhi, but pass through
                // gracefully).
                out.push_str(matra);
                continue;
            }

            if c == '\u{0A4D}' || c == '\u{0A3C}' || c == '\u{0A71}' {
                // Stray virama / nukta / addak — drop.
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
        '\u{0A05}' => "a",  // ਅ
        '\u{0A06}' => "ā",  // ਆ
        '\u{0A07}' => "i",  // ਇ
        '\u{0A08}' => "ī",  // ਈ
        '\u{0A09}' => "u",  // ਉ
        '\u{0A0A}' => "ū",  // ਊ
        '\u{0A0F}' => "e",  // ਏ
        '\u{0A10}' => "ai", // ਐ
        '\u{0A13}' => "o",  // ਓ
        '\u{0A14}' => "au", // ਔ
        _ => return None,
    })
}

/// Map a dependent vowel sign (matra) scalar to its ISO 15919 string.
#[must_use]
pub const fn matra_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0A3E}' => "ā",  // ਾ
        '\u{0A3F}' => "i",  // ਿ
        '\u{0A40}' => "ī",  // ੀ
        '\u{0A41}' => "u",  // ੁ
        '\u{0A42}' => "ū",  // ੂ
        '\u{0A47}' => "e",  // ੇ
        '\u{0A48}' => "ai", // ੈ
        '\u{0A4B}' => "o",  // ੋ
        '\u{0A4C}' => "au", // ੌ
        _ => return None,
    })
}

/// Map a combining nasalization / visarga scalar to its ISO 15919
/// string. Tippi and bindi both mark nasalization on the preceding
/// vowel; visarga is very rare in Punjabi but included for
/// completeness.
#[must_use]
pub const fn combining_mark_iso(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0A70}' => "ṁ", // ੰ tippi — anusvara-like
        '\u{0A02}' => "m̐", // ਂ bindi — chandrabindu-like
        '\u{0A03}' => "ḥ", // ਃ visarga
        _ => return None,
    })
}

/// Map a base consonant scalar to its ISO 15919 string.
#[must_use]
pub const fn consonant_iso(c: char) -> Option<&'static str> {
    Some(match c {
        // Velars.
        '\u{0A15}' => "k",  // ਕ
        '\u{0A16}' => "kh", // ਖ
        '\u{0A17}' => "g",  // ਗ
        '\u{0A18}' => "gh", // ਘ (tone-bearing)
        '\u{0A19}' => "ṅ",  // ਙ
        // Palatals.
        '\u{0A1A}' => "c",  // ਚ
        '\u{0A1B}' => "ch", // ਛ
        '\u{0A1C}' => "j",  // ਜ
        '\u{0A1D}' => "jh", // ਝ (tone-bearing)
        '\u{0A1E}' => "ñ",  // ਞ
        // Retroflexes.
        '\u{0A1F}' => "ṭ",  // ਟ
        '\u{0A20}' => "ṭh", // ਠ
        '\u{0A21}' => "ḍ",  // ਡ
        '\u{0A22}' => "ḍh", // ਢ (tone-bearing)
        '\u{0A23}' => "ṇ",  // ਣ
        // Dentals.
        '\u{0A24}' => "t",  // ਤ
        '\u{0A25}' => "th", // ਥ
        '\u{0A26}' => "d",  // ਦ
        '\u{0A27}' => "dh", // ਧ (tone-bearing)
        '\u{0A28}' => "n",  // ਨ
        // Labials.
        '\u{0A2A}' => "p",  // ਪ
        '\u{0A2B}' => "ph", // ਫ
        '\u{0A2C}' => "b",  // ਬ
        '\u{0A2D}' => "bh", // ਭ (tone-bearing)
        '\u{0A2E}' => "m",  // ਮ
        // Semivowels and liquids.
        '\u{0A2F}' => "y", // ਯ
        '\u{0A30}' => "r", // ਰ
        '\u{0A32}' => "l", // ਲ
        '\u{0A33}' => "ḷ", // ਲ਼ (precomposed retroflex lateral)
        '\u{0A35}' => "v", // ਵ
        // Sibilants and h.
        '\u{0A36}' => "ś", // ਸ਼ (precomposed sha with nukta)
        '\u{0A38}' => "s", // ਸ
        '\u{0A39}' => "h", // ਹ
        // Precomposed Perso-Arabic nukta consonants and native
        // retroflex flap.
        '\u{0A59}' => "x", // ਖ਼ (voiceless velar fricative)
        '\u{0A5A}' => "ġ", // ਗ਼ (voiced velar fricative)
        '\u{0A5B}' => "z", // ਜ਼ (voiced sibilant)
        '\u{0A5C}' => "ṛ", // ੜ (retroflex flap)
        '\u{0A5E}' => "f", // ਫ਼ (voiceless labiodental fricative)
        _ => return None,
    })
}

/// If `base` is the ISO 15919 form of a base consonant that has a
/// nukta variant, return the nukta variant's ISO form. This mirrors
/// Unicode's canonical decompositions of the precomposed nukta
/// consonants (`ਖ਼ = ਖ + ਼`, etc.).
#[must_use]
fn nukta_variant(base: &'static str) -> Option<&'static str> {
    match base {
        "kh" => Some("x"), // ਖ + ਼ → ਖ਼ → x
        "g" => Some("ġ"),  // ਗ + ਼ → ਗ਼ → ġ
        "j" => Some("z"),  // ਜ + ਼ → ਜ਼ → z
        "ḍ" => Some("ṛ"),  // ਡ + ਼ → ੜ → ṛ
        "ph" => Some("f"), // ਫ + ਼ → ਫ਼ → f
        "s" => Some("ś"),  // ਸ + ਼ → ਸ਼ → ś
        "l" => Some("ḷ"),  // ਲ + ਼ → ਲ਼ → ḷ
        _ => None,
    }
}

/// Map a Gurmukhi digit (`੦..੯` U+0A66..=U+0A6F) to its ASCII digit
/// counterpart.
#[must_use]
pub const fn digit_iso(c: char) -> Option<char> {
    match c {
        '\u{0A66}'..='\u{0A6F}' => {
            let offset = c as u32 - 0x0A66;
            char::from_u32(0x0030 + offset)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// Tone collapse — the Punjabi-specific pre-pass before Soundex fold.
// ---------------------------------------------------------------------

/// Fold the historical voiced-aspirate digrams (`gh`, `jh`, `ḍh`,
/// `dh`, `bh`) in `iso` to their voiceless-unaspirated counterparts.
/// Called between ISO 15919 transliteration and Soundex fold in the
/// [`PunjabiPhonex`] pipeline.
///
/// The digrams from the tone-bearing letters `ਘ`/`ਝ`/`ਢ`/`ਧ`/`ਭ` fold
/// to `k`/`c`/`ṭ`/`t`/`p` respectively. The voiceless-aspirated
/// digrams `kh`, `ch`, `ṭh`, `th`, `ph` (from `ਖ`/`ਛ`/`ਠ`/`ਥ`/`ਫ`)
/// pass through unchanged — those letters are *not* tone-bearing.
fn collapse_tone(iso: &str) -> String {
    let mut out = String::with_capacity(iso.len());
    let mut chars = iso.chars().peekable();
    while let Some(c) = chars.next() {
        let peek = chars.peek().copied();
        let replacement = match (c, peek) {
            ('g', Some('h')) => Some('k'),
            ('j', Some('h')) => Some('c'),
            ('ḍ', Some('h')) => Some('ṭ'),
            ('d', Some('h')) => Some('t'),
            ('b', Some('h')) => Some('p'),
            _ => None,
        };
        if let Some(r) = replacement {
            out.push(r);
            // Consume the paired 'h'.
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------
// PHONEX-Punjabi — 4-char Soundex-shape reduction over the
// tone-collapsed ISO 15919 output.
// ---------------------------------------------------------------------

/// The PHONEX-Punjabi encoder.
///
/// A zero-sized value; construct as [`PunjabiPhonex`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the algorithm's rules.
///
/// # Example
///
/// ```
/// use stringcheese_pa::PunjabiPhonex;
///
/// // "ਘਰ" transliterates to "ghara"; tone-collapse folds gh→k, giving
/// // "kara". Fold: K seed, A vow reset, R pushes code '6', A vow reset
/// // → "K6" pad → "K600". Same key as "ਕਰ" (kara).
/// assert_eq!(PunjabiPhonex.encode("ਘਰ").unwrap(), "K600");
/// assert_eq!(PunjabiPhonex.encode("ਕਰ").unwrap(), "K600");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PunjabiPhonex;

impl PunjabiPhonex {
    /// Encodes `word` per the PHONEX-Punjabi algorithm.
    ///
    /// Returns `None` when `word` has no letter content (empty input,
    /// pure whitespace, all punctuation, or reduced to nothing after
    /// filtering). Otherwise returns a 4-character key of the form
    /// `<uppercase letter><three ASCII digits>`.
    #[must_use]
    pub fn encode(&self, word: &str) -> Option<String> {
        let transliterated = PunjabiIso15919.encode(word);
        let collapsed = collapse_tone(&transliterated);
        let ascii = fold_to_ascii_upper(&collapsed);
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
                // Vowel / H — reset the duplicate-collapse state.
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
/// linguistic category (long vowels, retroflex under-dots, sibilant /
/// nasal marks, Perso-Arabic velar-fricative letters, retroflex
/// flap, combining-mark ISO forms) deliberately map to the same
/// ASCII target — merging them would obscure the linguistic
/// grouping, and the encoder's design intent is that each ISO 15919
/// diacritic-carrying scalar folds *individually* to its ASCII base
/// for Soundex-shape classification.
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
        'ḷ' | 'Ḷ' => 'L',
        // Sibilant / nasal marks → base.
        'ś' | 'Ś' => 'S',
        'ṅ' | 'Ṅ' => 'N',
        'ñ' | 'Ñ' => 'N',
        // Perso-Arabic voiced velar fricative — folds to G.
        'ġ' | 'Ġ' => 'G',
        // Retroflex flap (native Punjabi ੜ).
        'ṛ' | 'Ṛ' => 'R',
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

/// Adapter that exposes [`PunjabiPhonex`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Punjabi::phonetic_encoder`](crate::Punjabi) hands back.
///
/// Returns `Some((key, None))` for input with at least one Gurmukhi
/// scalar; returns `None` for input with no Gurmukhi content
/// (transliteration passes through unchanged, which is not a useful
/// key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PunjabiPhonexAdapter;

impl LanguagePhoneticEncoder for PunjabiPhonexAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_gurmukhi(word) {
            return None;
        }
        let key = PunjabiPhonex.encode(word)?;
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "phonex-pa"
    }
}

/// Does `s` contain at least one scalar in the Gurmukhi UTF-8 block
/// (U+0A00..=U+0A7F)?
fn contains_gurmukhi(s: &str) -> bool {
    s.chars().any(|c| ('\u{0A00}'..='\u{0A7F}').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(s: &str) -> String {
        PunjabiIso15919.encode(s)
    }

    fn p(w: &str) -> String {
        PunjabiPhonex.encode(w).expect("non-empty encodes")
    }

    // -------------------------------------------------------------
    // Inherent-vowel (schwa) handling.
    // -------------------------------------------------------------

    #[test]
    fn bare_consonant_carries_inherent_schwa() {
        // ਕ alone → ka (schwa fires).
        assert_eq!(e("ਕ"), "ka");
    }

    #[test]
    fn virama_suppresses_schwa() {
        // ਕ + ੍ → k (no schwa).
        assert_eq!(e("ਕ੍"), "k");
    }

    #[test]
    fn matra_overrides_schwa() {
        // ਕ + ਿ → ki (matra overrides).
        assert_eq!(e("ਕਿ"), "ki");
        assert_eq!(e("ਕੀ"), "kī");
        assert_eq!(e("ਕੁ"), "ku");
        assert_eq!(e("ਕੂ"), "kū");
        assert_eq!(e("ਕੇ"), "ke");
        assert_eq!(e("ਕੋ"), "ko");
        assert_eq!(e("ਕਾ"), "kā");
    }

    #[test]
    fn consonant_cluster_via_virama() {
        // ਸਤ੍ਯ = ਸ + ਤ + ੍ + ਯ → sat + ya = satya.
        assert_eq!(e("ਸਤ੍ਯ"), "satya");
    }

    #[test]
    fn explicit_schwa_retention() {
        // ਪੰਜਾਬ = ਪ + ੰ + ਜ + ਾ + ਬ → paṁ + jā + ba = paṁjāba.
        assert_eq!(e("ਪੰਜਾਬ"), "paṁjāba");
        // ਘਰ = ਘ + ਰ → gha + ra = ghara.
        assert_eq!(e("ਘਰ"), "ghara");
    }

    // -------------------------------------------------------------
    // Addak — gemination.
    // -------------------------------------------------------------

    #[test]
    fn addak_geminates_following_consonant() {
        // ਪੱਕਾ = ਪ + ੱ + ਕ + ਾ → pa + k + kā = pakkā.
        assert_eq!(e("ਪੱਕਾ"), "pakkā");
        // ਬੱਚਾ = ਬ + ੱ + ਚ + ਾ → ba + c + cā = baccā.
        assert_eq!(e("ਬੱਚਾ"), "baccā");
    }

    // -------------------------------------------------------------
    // Tippi / bindi — nasalization.
    // -------------------------------------------------------------

    #[test]
    fn tippi_encodes_to_dot_m() {
        // ਪੰਜ = ਪ + ੰ + ਜ → paṁja.
        assert_eq!(e("ਪੰਜ"), "paṁja");
    }

    #[test]
    fn bindi_encodes_to_m_with_below_ring() {
        // ਮੈਂ = ਮ + ੈ + ਂ → mai + m̐ = maim̐.
        assert_eq!(e("ਮੈਂ"), "maim̐");
    }

    // -------------------------------------------------------------
    // Independent vowels.
    // -------------------------------------------------------------

    #[test]
    fn independent_vowels() {
        assert_eq!(e("ਅ"), "a");
        assert_eq!(e("ਆ"), "ā");
        assert_eq!(e("ਇ"), "i");
        assert_eq!(e("ਈ"), "ī");
        assert_eq!(e("ਉ"), "u");
        assert_eq!(e("ਊ"), "ū");
        assert_eq!(e("ਏ"), "e");
        assert_eq!(e("ਐ"), "ai");
        assert_eq!(e("ਓ"), "o");
        assert_eq!(e("ਔ"), "au");
    }

    // -------------------------------------------------------------
    // Nukta — decomposed matches precomposed.
    // -------------------------------------------------------------

    #[test]
    fn decomposed_nukta_matches_precomposed() {
        // ਖ + ਼ (decomposed) should encode the same as ਖ਼ (precomposed).
        let decomposed: String = "\u{0A16}\u{0A3C}".into();
        let precomposed: String = "\u{0A59}".into();
        assert_eq!(e(&decomposed), e(&precomposed));
        assert_eq!(e(&precomposed), "xa");
    }

    #[test]
    fn decomposed_z_matches_precomposed() {
        // ਜ + ਼ → ਜ਼ (za).
        let decomposed: String = "\u{0A1C}\u{0A3C}".into();
        let precomposed: String = "\u{0A5B}".into();
        assert_eq!(e(&decomposed), e(&precomposed));
        assert_eq!(e(&precomposed), "za");
    }

    // -------------------------------------------------------------
    // Retroflex flap — ੜ.
    // -------------------------------------------------------------

    #[test]
    fn retroflex_flap_encodes_to_r_underdot() {
        // ੜ → ṛa.
        assert_eq!(e("ੜ"), "ṛa");
    }

    // -------------------------------------------------------------
    // Consonants — spot checks.
    // -------------------------------------------------------------

    #[test]
    fn tone_letters_preserved_in_iso() {
        // The transliterator preserves the ISO 15919 spelling of the
        // tone-bearing letters — folding happens only in the phonex
        // stage.
        assert_eq!(e("ਘ"), "gha");
        assert_eq!(e("ਝ"), "jha");
        assert_eq!(e("ਢ"), "ḍha");
        assert_eq!(e("ਧ"), "dha");
        assert_eq!(e("ਭ"), "bha");
    }

    #[test]
    fn perso_arabic_loans_encode_correctly() {
        assert_eq!(e("ਖ਼"), "xa");
        assert_eq!(e("ਗ਼"), "ġa");
        assert_eq!(e("ਜ਼"), "za");
        assert_eq!(e("ਫ਼"), "fa");
    }

    // -------------------------------------------------------------
    // Digits.
    // -------------------------------------------------------------

    #[test]
    fn gurmukhi_digits_encode_to_ascii() {
        assert_eq!(e("੨੦੨੬"), "2026");
        assert_eq!(e("੦"), "0");
        assert_eq!(e("੯"), "9");
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
    fn mixed_content_passes_through_non_gurmukhi() {
        assert_eq!(e("hello ਪੰਜਾਬ"), "hello paṁjāba");
    }

    // -------------------------------------------------------------
    // PHONEX-Punjabi.
    // -------------------------------------------------------------

    #[test]
    fn phonex_empty_input_returns_none() {
        assert!(PunjabiPhonex.encode("").is_none());
        assert!(PunjabiPhonex.encode("   ").is_none());
    }

    #[test]
    fn phonex_ghar_shares_key_with_kar() {
        // ਘ → gh → k (tone collapse). So ਘਰ and ਕਰ share a phonex key.
        assert_eq!(PunjabiPhonex.encode("ਘਰ"), PunjabiPhonex.encode("ਕਰ"));
        // K seed, A vow, R pushes '6', A vow → "K6" pad → "K600".
        assert_eq!(p("ਘਰ"), "K600");
        assert_eq!(p("ਕਰ"), "K600");
    }

    #[test]
    fn phonex_all_tone_letters_collapse() {
        // Each tone letter shares its voiceless-unaspirated counterpart's
        // key when followed by the same tail.
        assert_eq!(PunjabiPhonex.encode("ਘ"), PunjabiPhonex.encode("ਕ"));
        assert_eq!(PunjabiPhonex.encode("ਝ"), PunjabiPhonex.encode("ਚ"));
        assert_eq!(PunjabiPhonex.encode("ਢ"), PunjabiPhonex.encode("ਟ"));
        assert_eq!(PunjabiPhonex.encode("ਧ"), PunjabiPhonex.encode("ਤ"));
        assert_eq!(PunjabiPhonex.encode("ਭ"), PunjabiPhonex.encode("ਪ"));
    }

    #[test]
    fn phonex_paṁjāb_encodes_correctly() {
        // ਪੰਜਾਬ → "paṁjāba" → PAMJABA → P seed, A vow, M pushes '5',
        //   J pushes '2', A vow, B pushes '1' → "P521" (4 chars, break).
        assert_eq!(p("ਪੰਜਾਬ"), "P521");
    }

    #[test]
    fn phonex_long_vowels_fold_to_short() {
        // ਕ (ka) → "K000". ਕਾ (kā) → "K000". Same key.
        assert_eq!(PunjabiPhonex.encode("ਕ"), PunjabiPhonex.encode("ਕਾ"));
    }

    #[test]
    fn phonex_retroflex_folds_to_base() {
        // ਟ (ṭa) and ਤ (ta) — retroflex fold makes them share the
        // reduced letter T.
        assert_eq!(PunjabiPhonex.encode("ਟ"), PunjabiPhonex.encode("ਤ"));
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_phonex_pa() {
        assert_eq!(PunjabiPhonexAdapter.name(), "phonex-pa");
    }

    #[test]
    fn adapter_returns_some_for_gurmukhi() {
        let out = PunjabiPhonexAdapter.encode("ਪੰਜਾਬ");
        assert_eq!(out, Some((String::from("P521"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_gurmukhi() {
        assert!(PunjabiPhonexAdapter.encode("").is_none());
        assert!(PunjabiPhonexAdapter.encode("hello").is_none());
        assert!(PunjabiPhonexAdapter.encode("123").is_none());
    }
}
