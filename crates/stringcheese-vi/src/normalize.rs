//! Vietnamese-specific text normalization.
//!
//! Vietnamese orthography carries a **two-layer diacritic system**
//! (letter modifiers `ă â đ ê ô ơ ư` plus five tone marks) that IR
//! pipelines fold to varying degrees depending on the use case. This
//! module ships a [`normalize`] one-shot function for the common case
//! and a [`VietnameseNormalizer`] builder for callers who need to
//! configure the more controversial folds.
//!
//! # What always happens (defaults)
//!
//! [`normalize`] and [`VietnameseNormalizer::default`] both apply:
//!
//! - **NFC canonicalization.** The web overwhelmingly delivers
//!   Vietnamese text in NFC (precomposed vowel-plus-diacritic
//!   scalars), but individual Unicode inputs — copy-paste from
//!   older PDFs, poorly-configured text processors, macOS clipboard
//!   round-trips — sometimes come through in NFD (base letter plus
//!   combining marks). NFC canonicalization is the workspace-wide
//!   convention for Latin-script diacritic-carrying languages and
//!   is the lossless baseline every downstream stage assumes.
//!
//! # What is opt-in
//!
//! - **Strip tone marks.** Removes the five Vietnamese tone marks
//!   (grave, acute, hook-above, tilde, dot-below) while
//!   **preserving letter modifiers** (`ă`, `â`, `đ`, `ê`, `ô`, `ơ`,
//!   `ư` all survive). Reduces the six-tone syllable inventory to
//!   one written form; useful for fuzzy search where the six tone
//!   variants of a syllable should collapse to a single key. Off by
//!   default — tone marks distinguish real Vietnamese words and
//!   stripping them is lossy.
//! - **Strip all diacritics.** Removes both tone marks *and* letter
//!   modifiers, folding every Vietnamese character to plain ASCII
//!   (`ằ → a`, `đ → d`, `ệ → e`, `ự → u`). Useful for cross-script
//!   fuzzy search or when the target index is ASCII-only. Off by
//!   default — this is the most aggressive fold and destroys
//!   information the letter-modifier system carries. Enabling this
//!   flag subsumes the tone-mark strip; enabling both is redundant
//!   but well-defined.
//! - **NFC canonicalization.** On by default; can be turned off for
//!   callers whose input has already been normalized upstream and
//!   who want to avoid the second pass.
//!
//! # Distinguishing "tone mark" from "letter modifier"
//!
//! The distinction is real Vietnamese linguistics and is applied
//! consistently across the normalizer:
//!
//! * **Letter modifiers** — the seven characters `ă â đ ê ô ơ ư` and
//!   their uppercase counterparts. These are **distinct letters** in
//!   the Vietnamese alphabet; removing them changes the segmental
//!   phoneme. In NFD form they decompose as base + combining breve
//!   (U+0306, for `ă`), combining circumflex (U+0302, for `â` / `ê`
//!   / `ô`), combining horn (U+031B, for `ơ` / `ư`), or a full
//!   replacement (`đ` has no NFD decomposition — it is a distinct
//!   letter, not a modified `d`). The [`with_strip_tone_marks`]
//!   flag preserves these.
//! * **Tone marks** — the five combining marks that encode
//!   suprasegmental pitch:
//!   * `U+0300` combining grave accent (huyền, "falling"): `à`
//!   * `U+0301` combining acute accent (sắc, "rising"): `á`
//!   * `U+0309` combining hook above (hỏi, "dipping"): `ả`
//!   * `U+0303` combining tilde (ngã, "creaky-rising"): `ã`
//!   * `U+0323` combining dot below (nặng, "heavy"): `ạ`
//!
//! The classification is stable: any combining mark that is *not* on
//! the five-tone-mark list above stays put under
//! [`with_strip_tone_marks`], and every diacritic (tone marks +
//! combining breve + combining circumflex + combining horn + the
//! `đ → d` fold) drops under [`with_strip_all_diacritics`].
//!
//! [`with_strip_tone_marks`]: VietnameseNormalizer::with_strip_tone_marks
//! [`with_strip_all_diacritics`]: VietnameseNormalizer::with_strip_all_diacritics
//!
//! # Idempotence
//!
//! The normalizer is idempotent for every single-flag configuration:
//! `normalize(normalize(x)) == normalize(x)`. NFC is idempotent by
//! construction (the second pass is a no-op), the tone-mark strip
//! and full-diacritic strip both produce output that contains none
//! of the marks they strip, and enabling multiple flags composes
//! them in a fixed order that stays a strict rewrite. See the crate's
//! property-test module for the machine-checked assertion.

use alloc::string::String;

use unicode_normalization::UnicodeNormalization;

/// Configurable Vietnamese normalizer.
///
/// Three bool flags packed by the compiler; construct via
/// [`VietnameseNormalizer::default`] or [`VietnameseNormalizer::new`]
/// and reuse across threads and calls, or use
/// [`VietnameseNormalizer::builder`] to opt in to the extra folds.
///
/// See the [module-level docs](self) for the rules and the rationale.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VietnameseNormalizer {
    /// If `true`, precompose the output via Unicode Normalization
    /// Form C (NFC). On by default.
    nfc: bool,
    /// If `true`, strip the five Vietnamese tone marks (grave,
    /// acute, hook-above, tilde, dot-below) while preserving letter
    /// modifiers. Off by default.
    strip_tone_marks: bool,
    /// If `true`, strip all diacritics — tone marks *and* letter
    /// modifiers — folding to plain ASCII. Off by default.
    strip_all_diacritics: bool,
}

impl Default for VietnameseNormalizer {
    /// The default Vietnamese normalizer applies NFC canonicalization
    /// only; tone marks and letter modifiers are preserved.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl VietnameseNormalizer {
    /// Construct the default normalizer — NFC only.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nfc: true,
            strip_tone_marks: false,
            strip_all_diacritics: false,
        }
    }

    /// Enter the builder form — chain
    /// [`with_strip_tone_marks`](Self::with_strip_tone_marks),
    /// [`with_strip_all_diacritics`](Self::with_strip_all_diacritics),
    /// or [`with_nfc`](Self::with_nfc) to configure the rewrites.
    #[inline]
    #[must_use]
    pub const fn builder() -> Self {
        Self::new()
    }

    /// Toggle NFC precomposition. On by default.
    ///
    /// Turn off when the caller has already NFC-normalized the input
    /// upstream and wants to avoid the second pass.
    #[inline]
    #[must_use]
    pub const fn with_nfc(mut self, on: bool) -> Self {
        self.nfc = on;
        self
    }

    /// Returns whether NFC precomposition is enabled.
    #[inline]
    #[must_use]
    pub const fn precomposes_nfc(self) -> bool {
        self.nfc
    }

    /// Toggle stripping of the five Vietnamese tone marks (grave,
    /// acute, hook-above, tilde, dot-below). Off by default.
    ///
    /// Preserves letter modifiers (`ă`, `â`, `đ`, `ê`, `ô`, `ơ`, `ư`
    /// all survive). Enable when the caller wants the six tone
    /// variants of a Vietnamese syllable to collapse to a single
    /// fuzzy-search key (e.g. `bàn / bán / bản / bãn / bạn` all fold
    /// to `ban`).
    #[inline]
    #[must_use]
    pub const fn with_strip_tone_marks(mut self, on: bool) -> Self {
        self.strip_tone_marks = on;
        self
    }

    /// Returns whether tone-mark stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_tone_marks(self) -> bool {
        self.strip_tone_marks
    }

    /// Toggle stripping of *all* diacritics — tone marks and letter
    /// modifiers. Off by default.
    ///
    /// Produces plain ASCII output for every Vietnamese scalar:
    /// `ằ → a`, `đ → d`, `ệ → e`, `ự → u`. Enable when the caller
    /// wants an ASCII-only key (e.g. for a URL slug, an ASCII-only
    /// database column, or a phonetic index). Subsumes the
    /// tone-mark strip; enabling both is redundant but well-defined.
    #[inline]
    #[must_use]
    pub const fn with_strip_all_diacritics(mut self, on: bool) -> Self {
        self.strip_all_diacritics = on;
        self
    }

    /// Returns whether full-diacritic stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_all_diacritics(self) -> bool {
        self.strip_all_diacritics
    }

    /// Normalize `text` under this configuration.
    ///
    /// Returns an owned [`String`]. The output byte length is bounded
    /// by roughly the input's — NFC never grows the output (canonical
    /// composition is either a no-op or a length-preserving-or-
    /// shrinking rewrite), tone-mark stripping shrinks, and
    /// full-diacritic stripping shrinks (each multi-byte Vietnamese
    /// scalar folds to a single ASCII byte).
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        // -----------------------------------------------------------------
        // Full-diacritic strip subsumes the tone-mark strip.
        //
        // The strip works on NFD (canonically-decomposed) input so we
        // can walk combining marks explicitly. We decompose, drop the
        // marks (and fold `đ → d`), and — if NFC is on — recompose.
        // If NFC is off we still leave the base ASCII letters
        // uncomposed (there is no composition to do since combining
        // marks are gone).
        // -----------------------------------------------------------------
        if self.strip_all_diacritics {
            let stripped: String = text
                .nfd()
                .filter_map(|c| {
                    if is_any_combining_diacritic(c) {
                        None
                    } else {
                        Some(fold_letter_modifier_no_combining(c))
                    }
                })
                .collect();
            // Output is ASCII in the Vietnamese subset — no composition
            // is available. NFC pass is a no-op.
            return stripped;
        }

        // -----------------------------------------------------------------
        // Tone-mark strip (keeps letter modifiers).
        //
        // Decompose to NFD so tone marks become independent combining
        // scalars, drop the five tone-mark code points, and recompose
        // if NFC is on. Combining marks that are *not* tone marks
        // (combining breve U+0306, combining circumflex U+0302,
        // combining horn U+031B) stay put so `ă`, `â`, `ê`, `ô`, `ơ`,
        // `ư` survive.
        // -----------------------------------------------------------------
        if self.strip_tone_marks {
            let stripped_iter = text.nfd().filter(|&c| !is_tone_mark(c));
            return if self.nfc {
                stripped_iter.nfc().collect()
            } else {
                stripped_iter.collect()
            };
        }

        // -----------------------------------------------------------------
        // No stripping — NFC-only path.
        // -----------------------------------------------------------------
        if self.nfc {
            text.nfc().collect()
        } else {
            // Passthrough — allocate an owned copy for API symmetry.
            String::from(text)
        }
    }
}

/// One-shot normalization with the default configuration (NFC only,
/// tone marks preserved, letter modifiers preserved).
///
/// Equivalent to `VietnameseNormalizer::new().normalize(text)`. See the
/// [module-level docs](self) for what each rule does.
///
/// # Examples
///
/// ```
/// use stringcheese_vi::normalize::normalize;
///
/// // NFC canonicalization: decomposed "e" + combining circumflex +
/// // combining dot-below (three scalars) → precomposed "ệ" (one
/// // scalar).
/// assert_eq!(normalize("e\u{0302}\u{0323}"), "ệ");
/// // Tone marks and letter modifiers are preserved by default.
/// assert_eq!(normalize("Học sinh đọc sách."), "Học sinh đọc sách.");
/// ```
#[must_use]
pub fn normalize(text: &str) -> String {
    VietnameseNormalizer::new().normalize(text)
}

/// Is `c` one of the five Vietnamese tone-mark combining code points?
///
/// * U+0300 combining grave (huyền)
/// * U+0301 combining acute (sắc)
/// * U+0309 combining hook above (hỏi)
/// * U+0303 combining tilde (ngã)
/// * U+0323 combining dot below (nặng)
///
/// Returns `false` for the letter-modifier combining marks (breve
/// U+0306, circumflex U+0302, horn U+031B) — those survive the
/// tone-mark strip.
#[inline]
#[must_use]
pub const fn is_tone_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}' | '\u{0301}' | '\u{0303}' | '\u{0309}' | '\u{0323}'
    )
}

/// Is `c` any combining diacritic that Vietnamese uses?
///
/// The five tone marks plus the three letter-modifier combining
/// marks (breve U+0306, circumflex U+0302, horn U+031B). Used by the
/// full-diacritic strip.
#[inline]
#[must_use]
const fn is_any_combining_diacritic(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'
            | '\u{0301}'
            | '\u{0302}'
            | '\u{0303}'
            | '\u{0306}'
            | '\u{0309}'
            | '\u{031B}'
            | '\u{0323}'
    )
}

/// Fold non-combining Vietnamese letters to their ASCII base.
///
/// After NFD decomposition, most Vietnamese vowel-plus-diacritic
/// combinations split into `<base ASCII vowel> + <combining marks>`
/// (dropped by the filter). But **`đ` and `Đ` do not decompose** —
/// they are distinct letters in Unicode with no canonical
/// decomposition. So the full-diacritic strip needs an explicit fold
/// for those two scalars, and this helper handles it.
///
/// All other characters pass through unchanged.
#[inline]
#[must_use]
const fn fold_letter_modifier_no_combining(c: char) -> char {
    match c {
        'đ' => 'd',
        'Đ' => 'D',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------
    // Defaults — NFC canonicalization only.
    // -------------------------------------------------------------

    #[test]
    fn default_precomposes_nfd_input() {
        // `e` + combining circumflex (U+0302) + combining dot-below
        // (U+0323) → precomposed `ệ` (U+1EC7).
        assert_eq!(normalize("e\u{0302}\u{0323}"), "ệ");
        // `a` + combining breve (U+0306) + combining grave (U+0300)
        // → precomposed `ằ` (U+1EB1).
        assert_eq!(normalize("a\u{0306}\u{0300}"), "ằ");
    }

    #[test]
    fn default_preserves_nfc_input_unchanged() {
        assert_eq!(normalize("Học sinh đọc sách."), "Học sinh đọc sách.");
        assert_eq!(normalize("và"), "và");
        assert_eq!(normalize("được"), "được");
    }

    #[test]
    fn default_preserves_tone_marks() {
        for w in ["và", "cá", "hỏi", "mã", "nặng"] {
            assert_eq!(normalize(w), w, "tone mark stripped by default on {w:?}");
        }
    }

    #[test]
    fn default_preserves_letter_modifiers() {
        for w in ["ăn", "cầu", "đường", "bến", "cột", "cơm", "nước"] {
            assert_eq!(
                normalize(w),
                w,
                "letter modifier stripped by default on {w:?}"
            );
        }
    }

    // -------------------------------------------------------------
    // Tone-mark strip: strips five tone marks, keeps letter modifiers.
    // -------------------------------------------------------------

    #[test]
    fn tone_mark_strip_removes_five_tone_marks() {
        let n = VietnameseNormalizer::builder().with_strip_tone_marks(true);
        // grave `à` → `a`
        assert_eq!(n.normalize("à"), "a");
        // acute `á` → `a`
        assert_eq!(n.normalize("á"), "a");
        // hook-above `ả` → `a`
        assert_eq!(n.normalize("ả"), "a");
        // tilde `ã` → `a`
        assert_eq!(n.normalize("ã"), "a");
        // dot-below `ạ` → `a`
        assert_eq!(n.normalize("ạ"), "a");
        // Six tone variants of `ban` all collapse to `ban`.
        for w in ["ban", "bàn", "bán", "bản", "bãn", "bạn"] {
            assert_eq!(
                n.normalize(w),
                "ban",
                "tone variant {w:?} did not fold to `ban`"
            );
        }
    }

    #[test]
    fn tone_mark_strip_preserves_letter_modifiers() {
        let n = VietnameseNormalizer::builder().with_strip_tone_marks(true);
        // `ă` (a-breve, letter modifier) survives.
        assert_eq!(n.normalize("ă"), "ă");
        // `â` (a-circumflex, letter modifier) survives.
        assert_eq!(n.normalize("â"), "â");
        // `đ` (d-with-stroke, letter modifier) survives.
        assert_eq!(n.normalize("đ"), "đ");
        // `ê` survives; `ô` survives; `ơ` survives; `ư` survives.
        assert_eq!(n.normalize("ê"), "ê");
        assert_eq!(n.normalize("ô"), "ô");
        assert_eq!(n.normalize("ơ"), "ơ");
        assert_eq!(n.normalize("ư"), "ư");
    }

    #[test]
    fn tone_mark_strip_removes_tone_from_modified_vowel() {
        let n = VietnameseNormalizer::builder().with_strip_tone_marks(true);
        // `ằ` = ă + grave → `ă` (tone stripped, breve kept).
        assert_eq!(n.normalize("ằ"), "ă");
        // `ầ` = â + grave → `â`.
        assert_eq!(n.normalize("ầ"), "â");
        // `ộ` = ô + dot-below → `ô`.
        assert_eq!(n.normalize("ộ"), "ô");
        // `ự` = ư + dot-below → `ư`.
        assert_eq!(n.normalize("ự"), "ư");
    }

    // -------------------------------------------------------------
    // Full-diacritic strip: strips everything to plain ASCII.
    // -------------------------------------------------------------

    #[test]
    fn strip_all_removes_tone_marks() {
        let n = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
        for w in ["à", "á", "ả", "ã", "ạ"] {
            assert_eq!(n.normalize(w), "a", "tone variant {w:?} not folded to a");
        }
    }

    #[test]
    fn strip_all_removes_letter_modifiers() {
        let n = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
        assert_eq!(n.normalize("ă"), "a");
        assert_eq!(n.normalize("â"), "a");
        assert_eq!(n.normalize("đ"), "d");
        assert_eq!(n.normalize("ê"), "e");
        assert_eq!(n.normalize("ô"), "o");
        assert_eq!(n.normalize("ơ"), "o");
        assert_eq!(n.normalize("ư"), "u");
        // Uppercase Đ folds to D.
        assert_eq!(n.normalize("Đ"), "D");
    }

    #[test]
    fn strip_all_folds_stacked_diacritics() {
        let n = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
        // `ằ` = ă + grave → `a`.
        assert_eq!(n.normalize("ằ"), "a");
        // `ệ` = ê + dot-below → `e`.
        assert_eq!(n.normalize("ệ"), "e");
        // `ự` = ư + dot-below → `u`.
        assert_eq!(n.normalize("ự"), "u");
        // Full sentence.
        assert_eq!(n.normalize("Học sinh đọc sách."), "Hoc sinh doc sach.");
    }

    // -------------------------------------------------------------
    // NFC flag.
    // -------------------------------------------------------------

    #[test]
    fn nfc_flag_off_preserves_decomposition() {
        let n = VietnameseNormalizer::builder().with_nfc(false);
        // NFD input passes through unchanged.
        assert_eq!(n.normalize("e\u{0302}\u{0323}"), "e\u{0302}\u{0323}");
        // NFC input passes through unchanged too.
        assert_eq!(n.normalize("ệ"), "ệ");
    }

    #[test]
    fn tone_strip_with_nfc_off_leaves_letter_modifier_combining() {
        // Tone strip on, NFC off — the letter modifier's combining
        // mark (circumflex, U+0302) survives but does not get
        // recomposed into `ê`.
        let n = VietnameseNormalizer::builder()
            .with_strip_tone_marks(true)
            .with_nfc(false);
        // `ệ` decomposes to `e` + circumflex + dot-below. Strip drops
        // the dot-below. Result: `e` + combining circumflex.
        assert_eq!(n.normalize("ệ"), "e\u{0302}");
    }

    #[test]
    fn tone_strip_with_nfc_on_recomposes_letter_modifier() {
        let n = VietnameseNormalizer::builder()
            .with_strip_tone_marks(true)
            .with_nfc(true);
        // `ệ` → `ê` (letter modifier recomposed).
        assert_eq!(n.normalize("ệ"), "ê");
    }

    // -------------------------------------------------------------
    // Combining both flags.
    // -------------------------------------------------------------

    #[test]
    fn combining_flags_is_well_defined() {
        // The full-diacritic strip subsumes the tone-mark strip; the
        // combined output matches strip-all alone.
        let both = VietnameseNormalizer::builder()
            .with_strip_tone_marks(true)
            .with_strip_all_diacritics(true);
        let all = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
        for w in ["và", "được", "học sinh đọc sách", "ệ", "ằ"] {
            assert_eq!(both.normalize(w), all.normalize(w));
        }
    }

    // -------------------------------------------------------------
    // Idempotence.
    // -------------------------------------------------------------

    #[test]
    fn default_idempotent_on_typical_inputs() {
        for w in [
            "và",
            "được",
            "học sinh",
            "e\u{0302}\u{0323}",
            "Việt Nam",
            "hello",
            "",
        ] {
            let once = normalize(w);
            let twice = normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn tone_strip_is_idempotent() {
        let n = VietnameseNormalizer::builder().with_strip_tone_marks(true);
        for w in ["và", "được", "học sinh", "ệ", "ằ", "hello"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "tone-strip not idempotent on {w:?}");
        }
    }

    #[test]
    fn strip_all_is_idempotent() {
        let n = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
        for w in ["và", "được", "học sinh", "ệ", "ằ", "hello"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "strip-all not idempotent on {w:?}");
        }
    }

    // -------------------------------------------------------------
    // Non-Vietnamese input.
    // -------------------------------------------------------------

    #[test]
    fn passes_through_ascii() {
        assert_eq!(normalize("hello world"), "hello world");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("123"), "123");
    }

    #[test]
    fn preserves_ascii_around_vietnamese() {
        assert_eq!(normalize("Hello Việt Nam!"), "Hello Việt Nam!");
    }

    // -------------------------------------------------------------
    // Helper predicates.
    // -------------------------------------------------------------

    #[test]
    fn is_tone_mark_covers_five_tone_scalars() {
        for cp in [0x0300u32, 0x0301, 0x0303, 0x0309, 0x0323] {
            let c = char::from_u32(cp).unwrap();
            assert!(is_tone_mark(c), "U+{cp:04X} not classified as tone mark");
        }
        // Non-tone combining marks pass.
        for cp in [0x0302u32, 0x0306, 0x031B] {
            let c = char::from_u32(cp).unwrap();
            assert!(
                !is_tone_mark(c),
                "U+{cp:04X} incorrectly classified as tone mark"
            );
        }
        // ASCII does not classify as tone mark.
        assert!(!is_tone_mark('a'));
    }
}
