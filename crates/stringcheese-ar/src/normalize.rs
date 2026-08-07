//! Arabic-specific text normalization.
//!
//! Arabic orthography carries several *optional* marks and several
//! *distinguished-but-often-conflated* letter variants that an IR
//! pipeline usually wants collapsed before matching. This module ships
//! a [`normalize`] one-shot function for the common case and an
//! [`ArabicNormalizer`] builder for callers who need to configure the
//! more controversial folds.
//!
//! # What always happens
//!
//! [`normalize`] and [`ArabicNormalizer::default`] both apply:
//!
//! - **Strip harakat (short-vowel diacritics).** The Qur'anic
//!   vocalization set plus a couple of surrounding orthographic marks:
//!
//!   | Code point | Name                 | Symbol |
//!   |------------|----------------------|--------|
//!   | U+064B     | Fathatan (tanween)   | `ً`   |
//!   | U+064C     | Dammatan (tanween)   | `ٌ`   |
//!   | U+064D     | Kasratan (tanween)   | `ٍ`   |
//!   | U+064E     | Fatha                | `َ`   |
//!   | U+064F     | Damma                | `ُ`   |
//!   | U+0650     | Kasra                | `ِ`   |
//!   | U+0651     | Shadda               | `ّ`   |
//!   | U+0652     | Sukun                | `ْ`   |
//!   | U+0653     | Maddah above         | `ٓ`   |
//!   | U+0654     | Hamza above          | `ٔ`   |
//!   | U+0655     | Hamza below          | `ٕ`   |
//!   | U+0670     | Dagger alef (superscript alef) | `ٰ` |
//!
//!   These are combining marks that carry pronunciation information but
//!   not lexical identity; newswire and web text almost universally
//!   drop them.
//!
//! - **Normalize alef variants → plain alef (`ا` U+0627).**
//!   `أ` (hamza above alef, U+0623), `إ` (hamza below alef, U+0625),
//!   and `آ` (madda alef, U+0622) all collapse to plain alef. Callers
//!   who need to preserve the hamza distinction (Qur'anic scholarship,
//!   pedagogical corpora) should skip this normalizer.
//!
//! - **Normalize yeh variants → plain yeh (`ي` U+064A).**
//!   `ى` (alef maqsura, U+0649) — the final-position substitute for
//!   yeh — is folded to plain yeh. This is the single most common
//!   orthographic conflation in modern Arabic text; it happens
//!   effectively by accident across a huge fraction of digital input.
//!
//! # What is opt-in
//!
//! - **Teh marbuta → heh (`ة` U+0629 → `ه` U+0647).**
//!   Teh marbuta is a *feminine* ending pronounced `t` in construct
//!   state and `h` in pausal state; folding it to plain heh collapses
//!   the two orthographic variants that authors frequently confuse
//!   (`مدرسة` vs. `مدرسه`), which is often what an IR pipeline wants
//!   — but it also erases a real grammatical distinction that a
//!   morphological analyzer relies on. The normalizer offers this as
//!   a builder flag so callers can pick.
//!
//! # What is *not* normalized
//!
//! - **Tatweel (kashida, `ـ` U+0640).** The purely-orthographic
//!   letter-stretcher is *not* stripped here — a caller who needs to
//!   normalize display-elongated text should filter it out
//!   themselves before calling. (Rationale: tatweel appears only in
//!   presentation-form input and never in properly-encoded logical
//!   text; adding it to the strip list would surprise callers whose
//!   text is already clean.)
//! - **Presentation forms.** U+FB50..=U+FDFF and U+FE70..=U+FEFF —
//!   the Arabic presentation forms A and B blocks — are display-only
//!   ligature codepoints and are out of scope for this normalizer.
//!   Text in those blocks should be reduced to logical form via a
//!   Unicode normalization pass (NFKC) before this normalizer runs.
//! - **Digit normalization.** Arabic uses both Eastern Arabic digits
//!   (`٠١٢٣٤٥٦٧٨٩`, U+0660..=U+0669) and Western Arabic digits
//!   (`0123456789`). This normalizer does not fold between them.
//!
//! # Idempotence
//!
//! The normalizer is idempotent: `normalize(normalize(x)) ==
//! normalize(x)`. All rules are strict rewrites and none produce a
//! character that a later rule matches. See the crate's property-test
//! module for the machine-checked assertion.
//!
//! # RTL note
//!
//! Normalization operates on **logical order** — the byte order in
//! UTF-8 — not on display order. A caller who has already passed the
//! text through a bidi algorithm for rendering must reset it to
//! logical order before normalizing (which is what
//! [`String::from`]-of-a-`&str` naturally does; only presentation-form
//! copy-paste from a rendered document requires the extra step).

use alloc::string::String;

/// Configurable Arabic normalizer.
///
/// A zero-sized value; construct via [`ArabicNormalizer::default`] or
/// [`ArabicNormalizer::new`] and reuse across threads and calls, or use
/// [`ArabicNormalizer::builder`] to opt in to teh-marbuta folding.
///
/// See the [module-level docs](self) for the list of rules and the
/// rationale for each opt-in flag.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArabicNormalizer {
    /// If `true`, fold teh marbuta (`ة`) to plain heh (`ه`). Off by
    /// default; see the module-level docs for the trade-off.
    fold_teh_marbuta: bool,
}

impl ArabicNormalizer {
    /// Construct the default normalizer — strips diacritics, folds
    /// alef variants, folds `ى` → `ي`. Teh marbuta is *not* folded.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fold_teh_marbuta: false,
        }
    }

    /// Enter the builder form — chain `.with_teh_marbuta_folding(true)`
    /// to opt in to the extra fold.
    #[inline]
    #[must_use]
    pub const fn builder() -> Self {
        Self::new()
    }

    /// Toggle teh-marbuta → heh folding. Off by default (see the
    /// module-level docs).
    #[inline]
    #[must_use]
    pub const fn with_teh_marbuta_folding(mut self, on: bool) -> Self {
        self.fold_teh_marbuta = on;
        self
    }

    /// Returns whether teh-marbuta folding is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_teh_marbuta(self) -> bool {
        self.fold_teh_marbuta
    }

    /// Normalize `text` under this configuration.
    ///
    /// Returns an owned [`String`]. The output byte length is bounded
    /// by the input's — every rule either deletes bytes (harakat
    /// stripping) or is a same-length substitution (`أ` and `ا` are
    /// both 2 UTF-8 bytes; `ي`, `ى`, `ة`, `ه` are all 2 UTF-8 bytes).
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            // Strip harakat and dagger alef.
            if is_harakat(c) {
                continue;
            }
            // Fold alef variants.
            let c = fold_alef(c);
            // Fold yeh variants (alef maqsura → yeh).
            let c = fold_yeh(c);
            // Fold teh marbuta (opt-in).
            let c = if self.fold_teh_marbuta {
                fold_teh_marbuta(c)
            } else {
                c
            };
            out.push(c);
        }
        out
    }
}

/// One-shot normalization with the default configuration (diacritics
/// stripped, alef variants folded, yeh variants folded, teh marbuta
/// *preserved*).
///
/// Equivalent to `ArabicNormalizer::new().normalize(text)`. See the
/// [module-level docs](self) for what each rule does.
///
/// # Examples
///
/// ```
/// use stringcheese_ar::normalize::normalize;
///
/// // Harakat stripping.
/// assert_eq!(normalize("مُحَمَّد"), "محمد");
/// // Alef variant folding: أحمد → احمد.
/// assert_eq!(normalize("أحمد"), "احمد");
/// // Yeh variant folding: على → علي ("Ali", commonly written with the
/// // final alef maqsura in the wild).
/// assert_eq!(normalize("على"), "علي");
/// // Teh marbuta is *preserved* under the default configuration.
/// assert_eq!(normalize("مدرسة"), "مدرسة");
/// ```
#[must_use]
pub fn normalize(text: &str) -> String {
    ArabicNormalizer::new().normalize(text)
}

/// Is `c` an Arabic harakat / tanween / dagger-alef combining mark?
///
/// Returns `true` for U+064B..=U+0655 and U+0670. These are the marks
/// the normalizer strips unconditionally.
#[inline]
#[must_use]
pub const fn is_harakat(c: char) -> bool {
    matches!(
        c,
        // Tanween (U+064B..=U+064D), fatha, damma, kasra, shadda, sukun.
        '\u{064B}'..='\u{0652}'
        // Maddah above, hamza above, hamza below.
        | '\u{0653}'..='\u{0655}'
        // Superscript "dagger" alef.
        | '\u{0670}'
    )
}

/// Fold the alef variants (`أ` U+0623, `إ` U+0625, `آ` U+0622) to plain
/// alef (`ا` U+0627). Returns the input unchanged for other scalars.
#[inline]
#[must_use]
pub const fn fold_alef(c: char) -> char {
    match c {
        '\u{0622}' | '\u{0623}' | '\u{0625}' => '\u{0627}',
        _ => c,
    }
}

/// Fold alef maqsura (`ى` U+0649) to plain yeh (`ي` U+064A). Returns
/// the input unchanged for other scalars.
#[inline]
#[must_use]
pub const fn fold_yeh(c: char) -> char {
    match c {
        '\u{0649}' => '\u{064A}',
        _ => c,
    }
}

/// Fold teh marbuta (`ة` U+0629) to heh (`ه` U+0647). Returns the
/// input unchanged for other scalars.
///
/// This fold is controversial — see the [module-level docs](self) for
/// the trade-off. Callers should reach for [`ArabicNormalizer::builder`]
/// with `.with_teh_marbuta_folding(true)` rather than calling this
/// directly.
#[inline]
#[must_use]
pub const fn fold_teh_marbuta(c: char) -> char {
    match c {
        '\u{0629}' => '\u{0647}',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------
    // Harakat stripping.
    // -------------------------------------------------------------

    #[test]
    fn strips_fatha_kasra_damma() {
        assert_eq!(normalize("مُحَمَّد"), "محمد");
    }

    #[test]
    fn strips_all_harakat_codepoints() {
        // Build a string with every harakat scalar bookended by a
        // letter, then verify normalize wipes them.
        let mut input = String::from("ا");
        for cp in 0x064Bu32..=0x0655 {
            input.push(char::from_u32(cp).unwrap());
        }
        input.push('\u{0670}'); // dagger alef
        input.push('ب');
        assert_eq!(normalize(&input), "اب");
    }

    #[test]
    fn strips_tanween() {
        // شكرًا (thank you) — the alif ends with fathatan U+064B.
        assert_eq!(normalize("شكرًا"), "شكرا");
    }

    // -------------------------------------------------------------
    // Alef variants.
    // -------------------------------------------------------------

    #[test]
    fn folds_hamza_above_alef() {
        assert_eq!(normalize("أحمد"), "احمد");
    }

    #[test]
    fn folds_hamza_below_alef() {
        assert_eq!(normalize("إبراهيم"), "ابراهيم");
    }

    #[test]
    fn folds_madda_alef() {
        assert_eq!(normalize("آدم"), "ادم");
    }

    // -------------------------------------------------------------
    // Yeh variants.
    // -------------------------------------------------------------

    #[test]
    fn folds_alef_maqsura_to_yeh() {
        // The name "Ali" is commonly written with a final alef maqsura
        // in the wild; normalization folds it to plain yeh.
        assert_eq!(normalize("على"), "علي");
    }

    #[test]
    fn folds_alef_maqsura_in_isolation() {
        assert_eq!(normalize("ى"), "ي");
    }

    // -------------------------------------------------------------
    // Teh marbuta (opt-in).
    // -------------------------------------------------------------

    #[test]
    fn teh_marbuta_preserved_by_default() {
        assert_eq!(normalize("مدرسة"), "مدرسة");
    }

    #[test]
    fn teh_marbuta_folded_when_opted_in() {
        let n = ArabicNormalizer::builder().with_teh_marbuta_folding(true);
        assert_eq!(n.normalize("مدرسة"), "مدرسه");
    }

    #[test]
    fn teh_marbuta_flag_reads_back() {
        let n = ArabicNormalizer::builder().with_teh_marbuta_folding(true);
        assert!(n.folds_teh_marbuta());
        let m = ArabicNormalizer::default();
        assert!(!m.folds_teh_marbuta());
    }

    // -------------------------------------------------------------
    // Idempotence.
    // -------------------------------------------------------------

    #[test]
    fn idempotent_on_typical_inputs() {
        for w in ["مُحَمَّد", "أحمد", "إبراهيم", "آدم", "على", "مدرسة", "شكرًا"]
        {
            let once = normalize(w);
            let twice = normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_teh_marbuta_folding_on() {
        let n = ArabicNormalizer::builder().with_teh_marbuta_folding(true);
        for w in ["مدرسة", "طالبة", "مكتبة"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    // -------------------------------------------------------------
    // Non-Arabic input.
    // -------------------------------------------------------------

    #[test]
    fn passes_through_non_arabic() {
        assert_eq!(normalize("hello world"), "hello world");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("123"), "123");
    }

    #[test]
    fn preserves_ascii_around_arabic() {
        assert_eq!(normalize("Hello مُحَمَّد!"), "Hello محمد!");
    }

    // -------------------------------------------------------------
    // Helper predicates.
    // -------------------------------------------------------------

    #[test]
    fn is_harakat_covers_the_advertised_range() {
        for cp in 0x064Bu32..=0x0655 {
            assert!(is_harakat(char::from_u32(cp).unwrap()));
        }
        assert!(is_harakat('\u{0670}'));
        // Adjacent scalars should not be classified as harakat.
        assert!(!is_harakat('\u{064A}')); // yeh
        assert!(!is_harakat('\u{0656}')); // first scalar past the range
        assert!(!is_harakat('\u{0671}')); // scalar after dagger alef
    }
}
