//! Devanagari → IAST (International Alphabet of Sanskrit
//! Transliteration) transliteration.
//!
//! # Origin
//!
//! IAST is the standard scholarly romanization for Sanskrit, and by
//! convention also for Hindi and other Indic languages written in
//! Devanagari. Each Devanagari letter maps to a fixed Latin letter
//! (some with diacritics) — long vowels get macrons (`ā ī ū`),
//! retroflex consonants get under-dots (`ṭ ṭh ḍ ḍh ṇ`), sibilants get
//! diacritics (`ś ṣ`), and the two velar / palatal nasals get their
//! own marks (`ṅ ñ`). The mapping is bijective at the letter level
//! (modulo the schwa handling discussed below).
//!
//! # Inherent-vowel (schwa) handling — the crucial subtlety
//!
//! Devanagari is an **abugida** — every base consonant carries an
//! implicit `a` (schwa) vowel unless a dependent vowel sign (matra)
//! or a virama (`्` U+094D) overrides it. So `क` alone represents
//! `ka` (not just `k`), while `क् ` (`क` + virama) represents bare
//! `k`, and `कि` (`क` + matra `ि`) represents `ki`.
//!
//! This transliteration honors the inherent schwa **without applying
//! schwa-deletion rules**. Modern colloquial Hindi drops the schwa
//! in many word-final and some medial positions (`राम` is pronounced
//! `rām`, not `rāma`; `कमल` is `kamal`, not `kamala`), but the
//! deletion is context-dependent and lexicon-driven — the surface
//! script writes the schwa either way. Sanskrit and formal Hindi
//! recitation retain every schwa. The IAST convention this module
//! follows is the Sanskrit-style **explicit-schwa** rendering:
//!
//! * Base consonant with no following matra or virama → letter + `a`
//!   (`क` → `ka`; `कमल` → `kamala`).
//! * Consonant + virama → letter only (`क्` → `k`; `सत्य` → `satya`
//!   because the `त्` is `t`, not `ta`, and the following `य` carries
//!   its own inherent `a`).
//! * Consonant + matra → letter + matra's vowel (`कि` → `ki`; `की`
//!   → `kī`).
//! * Consonant + consonant with no explicit virama or matra →
//!   letter + `a` + letter + `a` (inherent schwa preserved on both).
//!
//! Callers who need Hindi-style schwa-deletion should post-process
//! the IAST output with a Hindi lexicon or the aksharamukha /
//! indic-transliteration rules — outside the scope of this
//! deterministic, lexicon-free encoder.
//!
//! # Mapping table
//!
//! ## Independent vowels
//!
//! | Devanagari | IAST | Notes                    |
//! |------------|------|--------------------------|
//! | `अ`        | `a`  | schwa                    |
//! | `आ`        | `ā`  | long a                   |
//! | `इ`        | `i`  | short i                  |
//! | `ई`        | `ī`  | long i                   |
//! | `उ`        | `u`  | short u                  |
//! | `ऊ`        | `ū`  | long u                   |
//! | `ऋ`        | `ṛ`  | vocalic r                |
//! | `ए`        | `e`  |                          |
//! | `ऐ`        | `ai` |                          |
//! | `ओ`        | `o`  |                          |
//! | `औ`        | `au` |                          |
//!
//! ## Dependent vowel signs (matras) — appear after a consonant
//!
//! | Devanagari | IAST | Notes                    |
//! |------------|------|--------------------------|
//! | (none)     | `a`  | inherent schwa           |
//! | `ा`        | `ā`  |                          |
//! | `ि`        | `i`  |                          |
//! | `ी`        | `ī`  |                          |
//! | `ु`        | `u`  |                          |
//! | `ू`        | `ū`  |                          |
//! | `ृ`        | `ṛ`  |                          |
//! | `े`        | `e`  |                          |
//! | `ै`        | `ai` |                          |
//! | `ो`        | `o`  |                          |
//! | `ौ`        | `au` |                          |
//!
//! ## Virama and other marks
//!
//! | Devanagari | IAST | Notes                                |
//! |------------|------|--------------------------------------|
//! | `्`        | (∅)  | virama — suppresses inherent schwa   |
//! | `ं`        | `ṃ`  | anusvara                             |
//! | `ँ`        | `m̐`  | chandrabindu (rendered as `m̐`)       |
//! | `ः`        | `ḥ`  | visarga                              |
//! | `़`        | (∅)  | nukta — passed through; the           |
//! |           |       | precomposed nukta letters have their |
//! |           |       | own entries                           |
//!
//! ## Consonants — the 33 classical stops, sonorants, sibilants
//!
//! | Devanagari | IAST | Group          |
//! |------------|------|----------------|
//! | `क`        | `k`  | velar          |
//! | `ख`        | `kh` |                |
//! | `ग`        | `g`  |                |
//! | `घ`        | `gh` |                |
//! | `ङ`        | `ṅ`  | velar nasal    |
//! | `च`        | `c`  | palatal        |
//! | `छ`        | `ch` |                |
//! | `ज`        | `j`  |                |
//! | `झ`        | `jh` |                |
//! | `ञ`        | `ñ`  | palatal nasal  |
//! | `ट`        | `ṭ`  | retroflex      |
//! | `ठ`        | `ṭh` |                |
//! | `ड`        | `ḍ`  |                |
//! | `ढ`        | `ḍh` |                |
//! | `ण`        | `ṇ`  | retroflex nasal|
//! | `त`        | `t`  | dental         |
//! | `थ`        | `th` |                |
//! | `द`        | `d`  |                |
//! | `ध`        | `dh` |                |
//! | `न`        | `n`  | dental nasal   |
//! | `प`        | `p`  | labial         |
//! | `फ`        | `ph` |                |
//! | `ब`        | `b`  |                |
//! | `भ`        | `bh` |                |
//! | `म`        | `m`  | labial nasal   |
//! | `य`        | `y`  | semivowel      |
//! | `र`        | `r`  |                |
//! | `ल`        | `l`  |                |
//! | `व`        | `v`  |                |
//! | `श`        | `ś`  | sibilant       |
//! | `ष`        | `ṣ`  |                |
//! | `स`        | `s`  |                |
//! | `ह`        | `h`  |                |
//!
//! ## Nukta letters (Perso-Arabic loans)
//!
//! Both the precomposed forms (single scalars) and the decomposed
//! forms (base + `़`) are handled — the encoder tracks the nukta
//! flag as it walks so a `ज` + `़` sequence encodes the same as
//! precomposed `ज़`.
//!
//! | Devanagari | IAST | Notes                    |
//! |------------|------|--------------------------|
//! | `क़` / `क` + `़` | `q`  | Perso-Arabic q      |
//! | `ख़` / `ख` + `़` | `k͟h` | (rendered as `kh`)  |
//! | `ग़` / `ग` + `़` | `ġ`  | Perso-Arabic ghain  |
//! | `ज़` / `ज` + `़` | `z`  | Persian z           |
//! | `ड़` / `ड` + `़` | `ṛ`  | Hindi retroflex flap|
//! | `ढ़` / `ढ` + `़` | `ṛh` | breathy retroflex flap |
//! | `फ़` / `फ` + `़` | `f`  | Perso-Arabic f      |
//!
//! ## Digits
//!
//! Devanagari digits `०..९` (U+0966..=U+096F) map to ASCII digits
//! `0..9`, matching what the [`crate::normalize::HindiNormalizer`]'s
//! digit-fold option produces.
//!
//! # Byte-length caveat
//!
//! Every Devanagari scalar is **3 bytes** in UTF-8 (U+0900..=U+097F
//! falls in the 3-byte range). The encoder walks characters via
//! [`str::chars`], never raw bytes, so it never risks slicing a
//! scalar apart. The output uses ASCII plus a small set of
//! Latin-with-diacritic scalars (`ā ī ū ṛ ṭ ḍ ṇ ṅ ñ ś ṣ ṃ ṁ ḥ ġ`);
//! those diacritic-carrying scalars are 2 bytes in UTF-8.
//!
//! # Non-Devanagari pass-through
//!
//! ASCII characters, whitespace, punctuation, and any scalar outside
//! the Devanagari block pass through unchanged.

use alloc::string::String;

use stringcheese_lang::LanguagePhoneticEncoder;

/// The Devanagari → IAST transliteration.
///
/// A zero-sized value; construct as [`HindiIast`] and reuse across
/// threads and calls.
///
/// See the [module-level docs](self) for the full mapping table and
/// the discussion of inherent-vowel (schwa) handling.
///
/// # Example
///
/// ```
/// use stringcheese_hi::HindiIast;
///
/// // "namaste" — न + म + स + ् + त + े.
/// // The virama suppresses the schwa on स; the े matra provides
/// // the final vowel on त.
/// assert_eq!(HindiIast.encode("नमस्ते"), "namaste");
/// // "Rama" — र + ा + म. The final म has an inherent schwa.
/// assert_eq!(HindiIast.encode("राम"), "rāma");
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HindiIast;

impl HindiIast {
    /// Encode `text` per the Devanagari → IAST transliteration.
    ///
    /// Every Devanagari scalar in the mapping is replaced by its IAST
    /// counterpart (one or more Latin characters); every scalar
    /// outside the mapping (ASCII, whitespace, other-script letters)
    /// passes through unchanged.
    ///
    /// The encoder honors the inherent-schwa convention: a base
    /// consonant with no following matra or virama emits the
    /// consonant letter followed by `a`; a following matra overrides
    /// the `a`; a following virama suppresses it.
    #[must_use]
    pub fn encode(&self, text: &str) -> String {
        // Preallocate roughly the same size — IAST output for pure
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
        // Tracks the pending base consonant's IAST letter (already
        // encoded, but the `a` hasn't been emitted yet).
        let mut pending_consonant: Option<&'static str> = None;
        // Tracks whether the pending consonant was followed by a
        // nukta scalar — if so, we may need to re-emit it as its
        // nukta-modified counterpart when we resolve the schwa.
        let mut pending_nukta = false;

        while let Some(c) = chars.next() {
            // Peek what's next (to decide how to resolve the pending
            // consonant's schwa).
            let next = chars.peek().copied();

            // A base consonant is queued. Decide whether the current
            // scalar overrides its schwa or not.
            if let Some(base) = pending_consonant.take() {
                // If the current scalar is a nukta and we haven't
                // already applied one, mark it and continue — the
                // nukta modifies the still-pending consonant.
                if c == '\u{093C}' && !pending_nukta {
                    // Re-queue the consonant with nukta applied. We
                    // look at the pending base's IAST letter and swap
                    // it for its nukta counterpart if there is one;
                    // otherwise we pass the consonant through and
                    // drop the nukta.
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

                // Virama suppresses the schwa. Emit the bare
                // consonant, consume the virama, do NOT emit `a`.
                if c == '\u{094D}' {
                    out.push_str(base);
                    continue;
                }

                // A dependent vowel sign (matra) overrides the schwa.
                if let Some(matra) = matra_iast(c) {
                    out.push_str(base);
                    out.push_str(matra);
                    continue;
                }

                // Anusvara / visarga / chandrabindu attach *after*
                // the schwa. Emit `base + a + mark`.
                if let Some(mark) = combining_mark_iast(c) {
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
            if let Some(v) = independent_vowel_iast(c) {
                out.push_str(v);
                continue;
            }

            // Base consonant? Queue it — we need to see the next
            // scalar to decide whether the schwa fires.
            if let Some(cons) = consonant_iast(c) {
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
            if let Some(d) = digit_iast(c) {
                out.push(d);
                continue;
            }

            // Combining mark seen without a pending consonant
            // (unusual — usually preceded by one). Emit the mark's
            // IAST alone.
            if let Some(mark) = combining_mark_iast(c) {
                out.push_str(mark);
                continue;
            }

            // Matra seen without a pending consonant (unusual — the
            // matra normally follows one). Emit its IAST alone.
            if let Some(matra) = matra_iast(c) {
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

/// Map an independent vowel scalar (`अ`..`औ` plus vocalic-r) to its
/// IAST string. Returns `None` for scalars outside this set.
#[must_use]
pub const fn independent_vowel_iast(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0905}' => "a",  // अ
        '\u{0906}' => "ā",  // आ
        '\u{0907}' => "i",  // इ
        '\u{0908}' => "ī",  // ई
        '\u{0909}' => "u",  // उ
        '\u{090A}' => "ū",  // ऊ
        '\u{090B}' => "ṛ",  // ऋ
        '\u{090F}' => "e",  // ए
        '\u{0910}' => "ai", // ऐ
        '\u{0913}' => "o",  // ओ
        '\u{0914}' => "au", // औ
        _ => return None,
    })
}

/// Map a dependent vowel sign (matra) scalar to its IAST string.
/// Returns `None` for scalars outside this set.
#[must_use]
pub const fn matra_iast(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{093E}' => "ā",  // ा
        '\u{093F}' => "i",  // ि
        '\u{0940}' => "ī",  // ी
        '\u{0941}' => "u",  // ु
        '\u{0942}' => "ū",  // ू
        '\u{0943}' => "ṛ",  // ृ
        '\u{0947}' => "e",  // े
        '\u{0948}' => "ai", // ै
        '\u{094B}' => "o",  // ो
        '\u{094C}' => "au", // ौ
        _ => return None,
    })
}

/// Map a combining vowel-modifier scalar (anusvara, chandrabindu,
/// visarga) to its IAST string. Returns `None` for scalars outside
/// this set.
#[must_use]
pub const fn combining_mark_iast(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{0902}' => "ṃ", // ं anusvara
        '\u{0901}' => "m̐", // ँ chandrabindu (m + combining candrabindu)
        '\u{0903}' => "ḥ", // ः visarga
        _ => return None,
    })
}

/// Map a base consonant scalar to its IAST string (without the
/// inherent `a` — the encoder adds the `a` conditionally). Returns
/// `None` for scalars outside this set.
///
/// The `match_same_arms` lint is suppressed because ख (U+0916) and
/// the precomposed nukta variant ख़ (U+0959) *deliberately* map to
/// the same IAST string `kh` — the uvular / non-uvular distinction
/// is not carried in this deterministic mapping (it is a rare
/// Perso-Arabic loan distinction that IAST does not always mark),
/// and merging the two arms would obscure which source scalar we
/// are handling.
#[must_use]
#[allow(clippy::match_same_arms)]
pub const fn consonant_iast(c: char) -> Option<&'static str> {
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
        // Labials.
        '\u{092A}' => "p",  // प
        '\u{092B}' => "ph", // फ
        '\u{092C}' => "b",  // ब
        '\u{092D}' => "bh", // भ
        '\u{092E}' => "m",  // म
        // Semivowels and liquids.
        '\u{092F}' => "y", // य
        '\u{0930}' => "r", // र
        '\u{0932}' => "l", // ल
        '\u{0935}' => "v", // व
        // Sibilants and h.
        '\u{0936}' => "ś", // श
        '\u{0937}' => "ṣ", // ष
        '\u{0938}' => "s", // स
        '\u{0939}' => "h", // ह
        // Precomposed nukta consonants — treated as bare consonants
        // (the nukta modification is baked into the IAST letter).
        '\u{0958}' => "q",  // क़
        '\u{0959}' => "kh", // ख़ (uvular — same IAST as ख; the
        // distinction is not carried in this
        // deterministic mapping)
        '\u{095A}' => "ġ",  // ग़
        '\u{095B}' => "z",  // ज़
        '\u{095C}' => "ṛ",  // ड़
        '\u{095D}' => "ṛh", // ढ़
        '\u{095E}' => "f",  // फ़
        _ => return None,
    })
}

/// If `base` is the IAST form of a base consonant that has a nukta
/// variant, return the nukta variant's IAST form; otherwise `None`.
///
/// This is used when the encoder sees a decomposed `base + ़`
/// sequence (rather than a precomposed nukta letter): the encoder
/// looks up the base consonant's IAST, then swaps it for the
/// nukta-modified IAST when it hits the following `़` U+093C scalar.
#[must_use]
fn nukta_variant(base: &'static str) -> Option<&'static str> {
    match base {
        "k" => Some("q"),   // क + ़ → क़ → q
        "kh" => Some("kh"), // ख + ़ → ख़ → uvular (same IAST here)
        "g" => Some("ġ"),   // ग + ़ → ग़ → ġ
        "j" => Some("z"),   // ज + ़ → ज़ → z
        "ḍ" => Some("ṛ"),   // ड + ़ → ड़ → ṛ (retroflex flap)
        "ḍh" => Some("ṛh"), // ढ + ़ → ढ़ → ṛh
        "ph" => Some("f"),  // फ + ़ → फ़ → f
        _ => None,
    }
}

/// Map a Devanagari digit (`०..९` U+0966..=U+096F) to its ASCII digit
/// counterpart. Returns `None` for scalars outside this set.
#[must_use]
pub const fn digit_iast(c: char) -> Option<char> {
    match c {
        '\u{0966}'..='\u{096F}' => {
            let offset = c as u32 - 0x0966;
            char::from_u32(0x0030 + offset)
        }
        _ => None,
    }
}

/// Adapter that exposes [`HindiIast`] through the object-safe
/// [`LanguagePhoneticEncoder`] trait — this is the type
/// [`Hindi::phonetic_encoder`](crate::Hindi) hands back.
///
/// The adapter always returns `Some((key, None))` — IAST is a
/// single-key encoder — and returns `None` for input with no
/// Devanagari-block content (the transliteration would pass through
/// unchanged, which is not a useful key).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct HindiIastAdapter;

impl LanguagePhoneticEncoder for HindiIastAdapter {
    fn encode(&self, word: &str) -> Option<(String, Option<String>)> {
        if !contains_devanagari(word) {
            return None;
        }
        let key = HindiIast.encode(word);
        if key.is_empty() {
            return None;
        }
        Some((key, None))
    }

    fn name(&self) -> &'static str {
        "iast-hi"
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
        HindiIast.encode(s)
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
        // क + ि → ki (matra overrides).
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
        // सत्य = स + त + ् + य → sat (with schwa on स) + t (virama) + ya = satya.
        assert_eq!(e("सत्य"), "satya");
        // धर्म = ध + र + ् + म → dha + r + ma = dharma.
        assert_eq!(e("धर्म"), "dharma");
    }

    #[test]
    fn sanskrit_style_schwa_retention() {
        // राम = र + ा + म → rāma (final म carries schwa).
        // Modern Hindi pronounces "rām"; IAST convention retains
        // the schwa.
        assert_eq!(e("राम"), "rāma");
        // कमल = क + म + ल → kamala (both म and ल carry schwa).
        assert_eq!(e("कमल"), "kamala");
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
        // हैं = ह + ै + ं. ह + ै → hai; + ं → haiṃ.
        assert_eq!(e("हैं"), "haiṃ");
    }

    #[test]
    fn visarga_encodes_to_dot_h() {
        // अः → aḥ.
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
    // The 33 consonants — spot checks.
    // -------------------------------------------------------------

    #[test]
    fn velar_nasal_encodes_to_dot_n() {
        // ङ → ṅa (with inherent schwa).
        assert_eq!(e("ङ"), "ṅa");
    }

    #[test]
    fn palatal_nasal_encodes_to_tilde_n() {
        // ञ → ña.
        assert_eq!(e("ञ"), "ña");
    }

    #[test]
    fn retroflex_series_encodes_with_dots_below() {
        assert_eq!(e("ट"), "ṭa");
        assert_eq!(e("ठ"), "ṭha");
        assert_eq!(e("ड"), "ḍa");
        assert_eq!(e("ढ"), "ḍha");
        assert_eq!(e("ण"), "ṇa");
    }

    #[test]
    fn sibilants_encode_correctly() {
        assert_eq!(e("श"), "śa");
        assert_eq!(e("ष"), "ṣa");
        assert_eq!(e("स"), "sa");
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

    #[test]
    fn mixed_content_passes_through_non_devanagari() {
        assert_eq!(e("hello राम"), "hello rāma");
    }

    // -------------------------------------------------------------
    // Adapter.
    // -------------------------------------------------------------

    #[test]
    fn adapter_name_is_iast_hi() {
        assert_eq!(HindiIastAdapter.name(), "iast-hi");
    }

    #[test]
    fn adapter_returns_some_for_devanagari() {
        let out = HindiIastAdapter.encode("राम");
        assert_eq!(out, Some((String::from("rāma"), None)));
    }

    #[test]
    fn adapter_returns_none_for_no_devanagari() {
        assert!(HindiIastAdapter.encode("").is_none());
        assert!(HindiIastAdapter.encode("hello").is_none());
        assert!(HindiIastAdapter.encode("123").is_none());
    }
}
