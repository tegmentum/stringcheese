//! Hindi-specific text normalization.
//!
//! Hindi orthography carries a couple of *frequently-mishandled* traits
//! that an IR pipeline usually wants to configure explicitly. This
//! module ships a [`normalize`] one-shot function for the common case
//! and a [`HindiNormalizer`] builder for callers who need to configure
//! the more controversial folds.
//!
//! # What always happens (defaults)
//!
//! [`normalize`] and [`HindiNormalizer::default`] both apply **no
//! transformations by default** — the identity function on Hindi input.
//! The two folds this normalizer offers are both opt-in because both
//! are lossy in ways that some Hindi IR pipelines want to preserve:
//!
//! # What is opt-in
//!
//! - **Fold Devanagari digits to ASCII digits.** Hindi text may write
//!   numerals in either the Devanagari digit block `०..९`
//!   (U+0966..=U+096F) or in ASCII `0..9`; folding the Devanagari
//!   forms to their ASCII counterparts makes a query typed with ASCII
//!   digits match a document written with Devanagari digits (and vice
//!   versa, if the caller normalizes both sides). Off by default —
//!   the digit forms are visually distinct and some pipelines want
//!   to preserve them as separate lexical items.
//! - **Strip nukta (`़` U+093C).** The nukta combining mark modifies
//!   base consonants to produce Perso-Arabic loans: `क़` (`qa`, from
//!   `क` + `़`), `ख़` (`kha`, uvular), `ग़` (`ġa`), `ज़` (`za`, from
//!   `ज` + `़`), `ड़` (retroflex flap), `ढ़` (breathy retroflex flap),
//!   `फ़` (`fa`). Nukta is **semantically meaningful** — `ज` (`ja`)
//!   and `ज़` (`za`) are different phonemes — so this fold is off
//!   by default. Turn it on when a query typed without a nukta
//!   should match a document written with one (e.g. `जरूरी` "necessary"
//!   is commonly written with `ज` even though a strict romanization
//!   uses `ज़`).
//!
//! # What is *not* normalized
//!
//! - **Anusvara vs. nasal consonant.** Hindi has two ways to write a
//!   nasal before a stop: with an explicit nasal consonant
//!   (`अंग` "limb" written `अ` `ं` `ग`) or with an anusvara alone (both
//!   collapse phonetically to `[əŋg]`). Some corpora normalize
//!   anusvara to the appropriate nasal consonant using look-ahead
//!   rules; the fold is context-dependent and out of scope for this
//!   normalizer. Callers who want it should apply it themselves.
//! - **Chandrabindu vs. anusvara.** Chandrabindu `ँ` (U+0901) marks
//!   pure vowel nasalization; anusvara `ं` (U+0902) marks a nasal
//!   consonant. Many pipelines fold `ँ → ं`, but the distinction is
//!   preserved in careful orthography and this normalizer leaves it
//!   alone.
//! - **Composed vs. decomposed forms.** The Devanagari letters `क़`,
//!   `ख़`, `ग़`, `ज़`, `ड़`, `ढ़`, `फ़`, `य़` have precomposed forms
//!   in Unicode (U+0958..=U+095F and U+095E) *and* decomposed forms
//!   (base + nukta). NFC / NFD normalization handles this; this
//!   crate does not duplicate that work. Callers whose input mixes
//!   precomposed and decomposed forms should apply NFC first (via
//!   `unicode_normalization` or the WHATWG string-normalize API).
//!
//! # Idempotence
//!
//! The normalizer is idempotent for every configuration:
//! `normalize(normalize(x)) == normalize(x)`. All rules are strict
//! rewrites and none produce a character that a later rule matches.
//! See the crate's property-test module for the machine-checked
//! assertion.

use alloc::string::String;

/// Configurable Hindi normalizer.
///
/// Two bool flags packed by the compiler; construct via
/// [`HindiNormalizer::default`] or [`HindiNormalizer::new`] and reuse
/// across threads and calls, or use [`HindiNormalizer::builder`] to
/// opt in to the folds.
///
/// See the [module-level docs](self) for the list of rules and the
/// rationale for each flag.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct HindiNormalizer {
    /// If `true`, fold Devanagari digits (`०..९` U+0966..=U+096F) to
    /// ASCII digits (U+0030..=U+0039). Off by default.
    fold_devanagari_digits: bool,
    /// If `true`, strip nukta combining mark (`़` U+093C) from the
    /// input. Off by default; nukta is semantically meaningful and
    /// distinguishes different phonemes.
    strip_nukta: bool,
}

impl Default for HindiNormalizer {
    /// The default Hindi normalizer performs no transformations —
    /// digits pass through unchanged, nukta is preserved.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl HindiNormalizer {
    /// Construct the default normalizer — no transformations applied.
    /// Callers who want the Devanagari-digit fold or nukta stripping
    /// should chain [`with_devanagari_digit_folding`](Self::with_devanagari_digit_folding)
    /// and [`with_nukta_stripping`](Self::with_nukta_stripping).
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fold_devanagari_digits: false,
            strip_nukta: false,
        }
    }

    /// Enter the builder form — chain
    /// [`with_devanagari_digit_folding`](Self::with_devanagari_digit_folding)
    /// or [`with_nukta_stripping`](Self::with_nukta_stripping) to
    /// configure the rewrites.
    #[inline]
    #[must_use]
    pub const fn builder() -> Self {
        Self::new()
    }

    /// Toggle folding of Devanagari digits (`०..९` U+0966..=U+096F) to
    /// ASCII digits (`0..9`). Off by default. Turn this on to make a
    /// document written with Devanagari digits match a query typed
    /// with ASCII digits.
    #[inline]
    #[must_use]
    pub const fn with_devanagari_digit_folding(mut self, on: bool) -> Self {
        self.fold_devanagari_digits = on;
        self
    }

    /// Returns whether Devanagari-to-ASCII digit folding is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_devanagari_digits(self) -> bool {
        self.fold_devanagari_digits
    }

    /// Toggle stripping of the nukta combining mark (`़` U+093C). Off
    /// by default.
    ///
    /// The nukta modifies a base consonant to produce a distinct
    /// phoneme (`ज` "ja" vs. `ज़` "za"), so stripping it collapses two
    /// distinct sounds. Turn this on only when the caller wants
    /// nukta-decorated and nukta-free variants of the same word to
    /// collapse to a single key (e.g. for a fuzzy search index over
    /// text where a nukta is commonly omitted in casual writing).
    ///
    /// Note that this only strips **standalone** nukta scalars from
    /// the byte sequence. The precomposed nukta letters `क़` `ख़`
    /// `ग़` `ज़` `ड़` `ढ़` `फ़` `य़` (U+0958..=U+095F and U+095E) are
    /// **not** decomposed by this fold — they are single Unicode
    /// scalars that pass through unchanged. Callers who need those
    /// decomposed should apply NFD normalization first, then this
    /// fold.
    #[inline]
    #[must_use]
    pub const fn with_nukta_stripping(mut self, on: bool) -> Self {
        self.strip_nukta = on;
        self
    }

    /// Returns whether nukta stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_nukta(self) -> bool {
        self.strip_nukta
    }

    /// Normalize `text` under this configuration.
    ///
    /// Returns an owned [`String`]. The output byte length is bounded
    /// by the input's — the digit fold replaces 3-byte Devanagari
    /// scalars with 1-byte ASCII (shrinks), and nukta stripping
    /// deletes bytes.
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            // Strip nukta (opt-in). U+093C is the standalone nukta
            // combining mark; the precomposed nukta letters
            // (U+0958..=U+095F, U+095E) are unaffected.
            if self.strip_nukta && c == '\u{093C}' {
                continue;
            }
            // Fold Devanagari digit → ASCII digit (opt-in).
            let c = if self.fold_devanagari_digits {
                fold_devanagari_digit(c)
            } else {
                c
            };
            out.push(c);
        }
        out
    }
}

/// One-shot normalization with the default configuration (no
/// transformations — identity on Hindi input).
///
/// Equivalent to `HindiNormalizer::new().normalize(text)`. Callers who
/// want the digit fold or nukta stripping should use
/// [`HindiNormalizer::builder`] and chain the desired flags.
///
/// # Examples
///
/// ```
/// use stringcheese_hi::normalize::normalize;
///
/// // Default: no transformations.
/// assert_eq!(normalize("२०२६"), "२०२६");
/// assert_eq!(normalize("ज़रूरी"), "ज़रूरी");
/// ```
#[must_use]
pub fn normalize(text: &str) -> String {
    HindiNormalizer::new().normalize(text)
}

/// Fold a Devanagari digit (`०..९` U+0966..=U+096F) to the
/// corresponding ASCII digit (`0..9` U+0030..=U+0039). Returns the
/// input unchanged for other scalars.
#[inline]
#[must_use]
pub const fn fold_devanagari_digit(c: char) -> char {
    match c {
        '\u{0966}'..='\u{096F}' => {
            let offset = c as u32 - 0x0966;
            match char::from_u32(0x0030 + offset) {
                Some(d) => d,
                None => c,
            }
        }
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------
    // Defaults — identity.
    // -------------------------------------------------------------

    #[test]
    fn default_is_identity_on_hindi() {
        assert_eq!(normalize("मैं"), "मैं");
        assert_eq!(normalize("२०२६"), "२०२६");
        assert_eq!(normalize("ज़रूरी"), "ज़रूरी");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn default_is_identity_on_ascii() {
        assert_eq!(normalize("hello"), "hello");
        assert_eq!(normalize("123"), "123");
    }

    // -------------------------------------------------------------
    // Devanagari digit folding.
    // -------------------------------------------------------------

    #[test]
    fn devanagari_digit_folding_opt_in() {
        let n = HindiNormalizer::builder().with_devanagari_digit_folding(true);
        let pairs = [
            ("०", "0"),
            ("१", "1"),
            ("२", "2"),
            ("३", "3"),
            ("४", "4"),
            ("५", "5"),
            ("६", "6"),
            ("७", "7"),
            ("८", "8"),
            ("९", "9"),
        ];
        for (input, expected) in pairs {
            assert_eq!(n.normalize(input), expected, "fold failed on {input:?}");
        }
        assert_eq!(n.normalize("२०२६"), "2026");
        assert!(n.folds_devanagari_digits());
    }

    #[test]
    fn digit_folding_preserves_letters() {
        let n = HindiNormalizer::builder().with_devanagari_digit_folding(true);
        // Letters pass through even when the digit fold is on.
        assert_eq!(n.normalize("साल २०२६"), "साल 2026");
    }

    // -------------------------------------------------------------
    // Nukta stripping.
    // -------------------------------------------------------------

    #[test]
    fn nukta_stripping_opt_in() {
        let n = HindiNormalizer::builder().with_nukta_stripping(true);
        // ज + ़ + र + ू + र + ी → ज + र + ू + र + ी (nukta stripped).
        assert_eq!(n.normalize("ज़रूरी"), "जरूरी");
        assert!(n.strips_nukta());
    }

    #[test]
    fn nukta_stripping_leaves_precomposed_letters_alone() {
        let n = HindiNormalizer::builder().with_nukta_stripping(true);
        // U+0958 क़ (qa) is a precomposed single scalar; the standalone
        // nukta at U+093C is not present, so nothing is stripped.
        assert_eq!(n.normalize("\u{0958}"), "\u{0958}");
    }

    #[test]
    fn nukta_stripping_preserved_default() {
        // Nukta is preserved by default.
        assert_eq!(normalize("ज़रूरी"), "ज़रूरी");
    }

    // -------------------------------------------------------------
    // Idempotence.
    // -------------------------------------------------------------

    #[test]
    fn idempotent_default() {
        for w in ["मैं", "२०२६", "ज़रूरी", "hello"] {
            let once = normalize(w);
            let twice = normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_digit_folding() {
        let n = HindiNormalizer::builder().with_devanagari_digit_folding(true);
        for w in ["२०२६", "2026", "साल २०२६", "००९"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_nukta_stripping() {
        let n = HindiNormalizer::builder().with_nukta_stripping(true);
        for w in ["ज़रूरी", "जरूरी", "क़", "\u{093C}"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_both_folds() {
        let n = HindiNormalizer::builder()
            .with_devanagari_digit_folding(true)
            .with_nukta_stripping(true);
        for w in ["ज़रूरी २०२६", "क़ १०"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    // -------------------------------------------------------------
    // Helper predicate.
    // -------------------------------------------------------------

    #[test]
    fn fold_devanagari_digit_covers_advertised_range() {
        for cp in 0x0966u32..=0x096F {
            let c = char::from_u32(cp).unwrap();
            let out = fold_devanagari_digit(c);
            assert_eq!(out as u32, 0x0030 + (cp - 0x0966));
        }
        // Adjacent scalars pass through.
        assert_eq!(fold_devanagari_digit('\u{0965}'), '\u{0965}');
        assert_eq!(fold_devanagari_digit('\u{0970}'), '\u{0970}');
        // Non-digits pass through.
        assert_eq!(fold_devanagari_digit('क'), 'क');
        assert_eq!(fold_devanagari_digit('a'), 'a');
    }
}
