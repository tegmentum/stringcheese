//! Persian-specific text normalization.
//!
//! Persian orthography carries several *conflated-in-the-wild* letter
//! variants and one width-zero joiner that an IR pipeline usually wants
//! to fold or preserve deliberately. This module ships a [`normalize`]
//! one-shot function for the common case and a [`PersianNormalizer`]
//! builder for callers who need to configure the more controversial
//! folds.
//!
//! # What always happens (defaults)
//!
//! [`normalize`] and [`PersianNormalizer::default`] both apply:
//!
//! - **Arabic yeh (`ي` U+064A) → Persian yeh (`ی` U+06CC).** The two
//!   scalars display identically in nearly every font; text on the web
//!   substitutes one for the other silently. Persian keyboards emit
//!   the Persian form; Arabic keyboards emit the Arabic form; the
//!   result is that Persian corpora contain both. Folding to the
//!   Persian form gives a single canonical spelling.
//! - **Arabic kaf (`ك` U+0643) → Persian kaf (`ک` U+06A9).** Same
//!   story — different scalars, same appearance in every Persian font.
//!   Folding to the Persian form is the norm.
//! - **Strip tatweel (`ـ` U+0640).** The Arabic decorative
//!   letter-stretcher used in Persian typography for justification;
//!   carries no semantic content. Dropped by default.
//!
//! # What is opt-in
//!
//! - **Strip zero-width non-joiner (ZWNJ, U+200C).** Persian compound
//!   words (`می‌روم` "I go", `کتاب‌ها` "books") use a ZWNJ between
//!   morphemes to prevent them from ligating while keeping them a
//!   single orthographic word. ZWNJ is **semantically meaningful**;
//!   stripping it collapses compounds that should stay separated at
//!   the character level. Off by default. Callers who want compound
//!   halves to *match each other in search* turn this on (e.g.
//!   `می‌روم` vs. `میروم` collapse to the same key).
//! - **Fold Extended Arabic-Indic digits to Western digits.** Persian
//!   text uses the Extended Arabic-Indic block `۰..۹`
//!   (U+06F0..=U+06F9), which is *distinct* from the Eastern
//!   Arabic-Indic block `٠..٩` (U+0660..=U+0669) that Arabic uses. Off
//!   by default. Turn this on when a query typed with Western digits
//!   should match a document written with Persian digits.
//! - **Decompose heh with yeh above (`ۀ` U+06C0 → heh + yeh, `ه` `ی`).**
//!   The ezafeh-carrying spelling `ۀ` collapses in some pipelines to
//!   the plain `ه` (U+0647) followed by a Persian yeh (U+06CC), which
//!   is how the ezafeh chain is usually rendered when the marker is
//!   spelled out. Off by default — the composed form is the
//!   canonical Unicode representation and callers who want it
//!   preserved should leave the flag off.
//!
//! # What is *not* normalized
//!
//! - **Diacritics.** Persian, like Arabic, has optional short-vowel
//!   marks (fatha U+064E, kasra U+0650, damma U+064F, sukun U+0652)
//!   and tashdid (shadda U+0651). These are rare in Persian outside
//!   pedagogical corpora. The normalizer does not strip them by
//!   default; callers whose input carries diacritics should reach
//!   for [`crate::normalize::normalize`] on top of a
//!   [`crate::normalize::PersianNormalizer::builder`] with any extra
//!   folds they want. (Stripping is a follow-up.)
//! - **Presentation forms.** U+FB50..=U+FDFF and U+FE70..=U+FEFF —
//!   the Arabic presentation forms A and B blocks — are display-only
//!   ligature codepoints and are out of scope for this normalizer.
//!   Text in those blocks should be reduced to logical form via a
//!   Unicode normalization pass (NFKC) before this normalizer runs.
//!
//! # Idempotence
//!
//! The normalizer is idempotent for every single-flag configuration:
//! `normalize(normalize(x)) == normalize(x)`. All rules are strict
//! rewrites and none produce a character that a later rule matches.
//! See the crate's property-test module for the machine-checked
//! assertion.
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

/// Configurable Persian normalizer.
///
/// Six bool flags packed by the compiler; construct via
/// [`PersianNormalizer::default`] or [`PersianNormalizer::new`] and
/// reuse across threads and calls, or use
/// [`PersianNormalizer::builder`] to opt in to the extra folds.
///
/// See the [module-level docs](self) for the list of rules and the
/// rationale for each flag.
// The six flags are genuinely independent orthographic knobs — a state
// machine would obscure the API rather than clarify it, so we silence
// the excessive-bools lint.
#[allow(clippy::struct_excessive_bools)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PersianNormalizer {
    /// If `true`, fold Arabic yeh (`ي` U+064A) to Persian yeh
    /// (`ی` U+06CC). On by default.
    fold_arabic_yeh: bool,
    /// If `true`, fold Arabic kaf (`ك` U+0643) to Persian kaf
    /// (`ک` U+06A9). On by default.
    fold_arabic_kaf: bool,
    /// If `true`, fold Extended Arabic-Indic digits (U+06F0..=U+06F9)
    /// to Western Arabic digits (U+0030..=U+0039). Off by default.
    fold_to_western_digits: bool,
    /// If `true`, strip zero-width non-joiner (ZWNJ, U+200C) from the
    /// input. Off by default; ZWNJ is semantically meaningful in
    /// Persian compound words.
    strip_zwnj: bool,
    /// If `true`, strip tatweel (`ـ` U+0640) — the purely-orthographic
    /// letter-stretcher — from the input. On by default.
    strip_tatweel: bool,
    /// If `true`, decompose heh with yeh above (`ۀ` U+06C0) to plain
    /// heh (`ه` U+0647) followed by Persian yeh (`ی` U+06CC). Off by
    /// default.
    decompose_heh_yeh: bool,
}

impl Default for PersianNormalizer {
    /// The default Persian normalizer folds Arabic yeh / kaf to their
    /// Persian variants and strips tatweel; ZWNJ is preserved, digits
    /// pass through unchanged, and the heh-yeh (`ۀ`) composed form is
    /// preserved.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl PersianNormalizer {
    /// Construct the default normalizer — folds Arabic yeh → Persian
    /// yeh, Arabic kaf → Persian kaf, and strips tatweel. ZWNJ is
    /// preserved, Extended Arabic-Indic digits pass through, and the
    /// heh-yeh (`ۀ`) composed form is preserved.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fold_arabic_yeh: true,
            fold_arabic_kaf: true,
            fold_to_western_digits: false,
            strip_zwnj: false,
            strip_tatweel: true,
            decompose_heh_yeh: false,
        }
    }

    /// Enter the builder form — chain `.with_western_digits(true)`,
    /// `.with_strip_zwnj(true)`, `.with_heh_yeh_normalization(true)`,
    /// or the two Arabic-fold toggles to configure the rewrites.
    #[inline]
    #[must_use]
    pub const fn builder() -> Self {
        Self::new()
    }

    /// Toggle Arabic yeh (`ي` U+064A) → Persian yeh (`ی` U+06CC)
    /// folding. On by default.
    #[inline]
    #[must_use]
    pub const fn with_arabic_yeh_to_persian(mut self, on: bool) -> Self {
        self.fold_arabic_yeh = on;
        self
    }

    /// Returns whether Arabic-yeh → Persian-yeh folding is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_arabic_yeh_to_persian(self) -> bool {
        self.fold_arabic_yeh
    }

    /// Toggle Arabic kaf (`ك` U+0643) → Persian kaf (`ک` U+06A9)
    /// folding. On by default.
    #[inline]
    #[must_use]
    pub const fn with_arabic_kaf_to_persian(mut self, on: bool) -> Self {
        self.fold_arabic_kaf = on;
        self
    }

    /// Returns whether Arabic-kaf → Persian-kaf folding is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_arabic_kaf_to_persian(self) -> bool {
        self.fold_arabic_kaf
    }

    /// Toggle folding of Extended Arabic-Indic digits (`۰..۹`
    /// U+06F0..=U+06F9) — Persian's digit block — to Western Arabic
    /// digits (`0..9`). Off by default. Turn this on to make a
    /// document written with Persian digits match a query typed with
    /// Western digits.
    ///
    /// Note that only the Extended Arabic-Indic block is folded here.
    /// The Eastern Arabic-Indic block (`٠..٩` U+0660..=U+0669) that
    /// Arabic uses is left alone — Persian corpora normally do not
    /// contain those. If your input mixes both blocks, run the Arabic
    /// pack's normalizer first (which folds *both* Eastern and
    /// Extended down to Western), then this one.
    #[inline]
    #[must_use]
    pub const fn with_western_digits(mut self, on: bool) -> Self {
        self.fold_to_western_digits = on;
        self
    }

    /// Returns whether Extended-Arabic-Indic → Western digit folding
    /// is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_to_western_digits(self) -> bool {
        self.fold_to_western_digits
    }

    /// Toggle stripping of the zero-width non-joiner (ZWNJ, U+200C).
    /// Off by default.
    ///
    /// ZWNJ is semantically meaningful in Persian compound words —
    /// `می‌روم` (imperfective marker `می` + `روم` "I go") is a single
    /// orthographic word held apart by a ZWNJ. Stripping it yields
    /// `میروم`, which is *not* wrong per se but changes the surface
    /// form. Turn this flag on when the caller wants ZWNJ-decorated
    /// and ZWNJ-free variants of the same word to collapse to a
    /// single key (e.g. for a fuzzy search index).
    #[inline]
    #[must_use]
    pub const fn with_strip_zwnj(mut self, on: bool) -> Self {
        self.strip_zwnj = on;
        self
    }

    /// Returns whether ZWNJ stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_zwnj(self) -> bool {
        self.strip_zwnj
    }

    /// Toggle stripping of tatweel (`ـ` U+0640). On by default.
    ///
    /// Tatweel is a decorative elongation character used for
    /// justification in both Arabic and Persian typography; it carries
    /// no semantic content, so stripping it makes tatweel-decorated
    /// variants (`مـحمد`) collapse to their undecorated form (`محمد`).
    #[inline]
    #[must_use]
    pub const fn with_strip_tatweel(mut self, on: bool) -> Self {
        self.strip_tatweel = on;
        self
    }

    /// Returns whether tatweel stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_tatweel(self) -> bool {
        self.strip_tatweel
    }

    /// Toggle decomposition of heh with yeh above (`ۀ` U+06C0) into
    /// plain heh (`ه` U+0647) plus Persian yeh (`ی` U+06CC). Off by
    /// default.
    ///
    /// The composed `ۀ` scalar is one Unicode codepoint; the decomposed
    /// form is two. Which is "canonical" depends on the pipeline — the
    /// Unicode canonical decomposition (`NFD`) of `ۀ` is *itself* (it
    /// has no canonical decomposition), so this fold is a
    /// Persian-specific convention rather than a Unicode-mandated
    /// normalization. Turn it on when the downstream index expects
    /// the decomposed form.
    #[inline]
    #[must_use]
    pub const fn with_heh_yeh_normalization(mut self, on: bool) -> Self {
        self.decompose_heh_yeh = on;
        self
    }

    /// Returns whether heh-with-yeh-above decomposition is enabled.
    #[inline]
    #[must_use]
    pub const fn decomposes_heh_yeh(self) -> bool {
        self.decompose_heh_yeh
    }

    /// Normalize `text` under this configuration.
    ///
    /// Returns an owned [`String`]. The output byte length is bounded
    /// by the input's in every configuration except the heh-yeh
    /// decomposition flag, which expands one 2-byte scalar into two
    /// 2-byte scalars (`ۀ` → `ه` + `ی`) and can grow the output.
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        // Worst-case size: input.len() bytes, plus one extra scalar (2
        // bytes) per `ۀ` (also 2 bytes) if we're decomposing heh-yeh.
        // Doubling is the cheap upper bound.
        let cap = if self.decompose_heh_yeh {
            text.len().saturating_mul(2)
        } else {
            text.len()
        };
        let mut out = String::with_capacity(cap);
        for c in text.chars() {
            // Strip tatweel (default on).
            if self.strip_tatweel && c == '\u{0640}' {
                continue;
            }
            // Strip ZWNJ (opt-in).
            if self.strip_zwnj && c == '\u{200C}' {
                continue;
            }
            // Decompose heh-yeh (opt-in). Two-scalar expansion — the
            // rule fires *before* the yeh-fold, so the emitted Persian
            // yeh does not need further rewriting.
            if self.decompose_heh_yeh && c == '\u{06C0}' {
                out.push('\u{0647}'); // heh
                out.push('\u{06CC}'); // Persian yeh
                continue;
            }
            // Fold Arabic yeh → Persian yeh (default on).
            let c = if self.fold_arabic_yeh {
                fold_arabic_yeh(c)
            } else {
                c
            };
            // Fold Arabic kaf → Persian kaf (default on).
            let c = if self.fold_arabic_kaf {
                fold_arabic_kaf(c)
            } else {
                c
            };
            // Fold Extended Arabic-Indic digits → Western (opt-in).
            let c = if self.fold_to_western_digits {
                fold_to_western_digit(c)
            } else {
                c
            };
            out.push(c);
        }
        out
    }
}

/// One-shot normalization with the default configuration (Arabic yeh
/// folded to Persian yeh, Arabic kaf folded to Persian kaf, tatweel
/// stripped, ZWNJ preserved, digits pass through, `ۀ` preserved).
///
/// Equivalent to `PersianNormalizer::new().normalize(text)`. See the
/// [module-level docs](self) for what each rule does.
///
/// # Examples
///
/// ```
/// use stringcheese_fa::normalize::normalize;
///
/// // Arabic yeh folded to Persian yeh (ي → ی).
/// assert_eq!(normalize("علي"), "علی");
/// // Arabic kaf folded to Persian kaf (ك → ک).
/// assert_eq!(normalize("كتاب"), "کتاب");
/// // Tatweel stripped.
/// assert_eq!(normalize("مـحمد"), "محمد");
/// // ZWNJ preserved (semantically meaningful in compound words).
/// assert_eq!(normalize("می\u{200C}روم"), "می\u{200C}روم");
/// ```
#[must_use]
pub fn normalize(text: &str) -> String {
    PersianNormalizer::new().normalize(text)
}

/// Fold Arabic yeh (`ي` U+064A) to Persian yeh (`ی` U+06CC). Returns
/// the input unchanged for other scalars.
///
/// This also folds Arabic alef maqsura (`ى` U+0649) to Persian yeh —
/// alef maqsura displays identically to the yeh forms and is a common
/// mis-typing in Persian text. Callers who need to preserve the alef
/// maqsura distinction (Persian text quoting Qur'anic Arabic) should
/// skip the yeh fold and handle it themselves.
#[inline]
#[must_use]
pub const fn fold_arabic_yeh(c: char) -> char {
    match c {
        '\u{064A}' | '\u{0649}' => '\u{06CC}',
        _ => c,
    }
}

/// Fold Arabic kaf (`ك` U+0643) to Persian kaf (`ک` U+06A9). Returns
/// the input unchanged for other scalars.
#[inline]
#[must_use]
pub const fn fold_arabic_kaf(c: char) -> char {
    match c {
        '\u{0643}' => '\u{06A9}',
        _ => c,
    }
}

/// Fold an Extended Arabic-Indic digit (`۰..۹` U+06F0..=U+06F9) to the
/// corresponding Western Arabic digit (`0..9` U+0030..=U+0039). Returns
/// the input unchanged for other scalars.
#[inline]
#[must_use]
pub const fn fold_to_western_digit(c: char) -> char {
    match c {
        '\u{06F0}'..='\u{06F9}' => {
            let offset = c as u32 - 0x06F0;
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
    // Defaults — Arabic yeh / kaf folding, tatweel stripping.
    // -------------------------------------------------------------

    #[test]
    fn folds_arabic_yeh_by_default() {
        // The name "Ali" is often typed with Arabic yeh (U+064A) on
        // Arabic keyboards; the Persian norm is U+06CC.
        assert_eq!(normalize("علي"), "علی");
    }

    #[test]
    fn folds_arabic_kaf_by_default() {
        // The word "book" — Arabic kaf U+0643 → Persian kaf U+06A9.
        assert_eq!(normalize("كتاب"), "کتاب");
    }

    #[test]
    fn folds_alef_maqsura_to_persian_yeh() {
        // ى U+0649 → ی U+06CC.
        assert_eq!(normalize("ى"), "ی");
    }

    #[test]
    fn strips_tatweel_by_default() {
        assert_eq!(normalize("مـحمد"), "محمد");
        // Multiple runs.
        assert_eq!(normalize("مـــحـــمد"), "محمد");
        // Bare tatweel.
        assert_eq!(normalize("ـ"), "");
    }

    #[test]
    fn zwnj_preserved_by_default() {
        // می‌روم — imperfective compound word held apart by a ZWNJ.
        let input = "می\u{200C}روم";
        assert_eq!(normalize(input), input);
    }

    #[test]
    fn digits_preserved_by_default() {
        // Persian digits pass through unchanged with the default.
        assert_eq!(normalize("۲۰۲۴"), "۲۰۲۴");
        assert_eq!(normalize("2024"), "2024");
    }

    #[test]
    fn heh_yeh_preserved_by_default() {
        // The composed ۀ is preserved with the default.
        assert_eq!(normalize("خانۀ"), "خانۀ");
    }

    // -------------------------------------------------------------
    // Individual flag toggles.
    // -------------------------------------------------------------

    #[test]
    fn arabic_yeh_folding_can_be_disabled() {
        let n = PersianNormalizer::builder().with_arabic_yeh_to_persian(false);
        assert_eq!(n.normalize("علي"), "علي");
        assert!(!n.folds_arabic_yeh_to_persian());
    }

    #[test]
    fn arabic_kaf_folding_can_be_disabled() {
        let n = PersianNormalizer::builder().with_arabic_kaf_to_persian(false);
        assert_eq!(n.normalize("كتاب"), "كتاب");
        assert!(!n.folds_arabic_kaf_to_persian());
    }

    #[test]
    fn tatweel_stripping_can_be_disabled() {
        let n = PersianNormalizer::builder().with_strip_tatweel(false);
        assert_eq!(n.normalize("مـحمد"), "مـحمد");
        assert!(!n.strips_tatweel());
    }

    #[test]
    fn zwnj_stripping_opt_in() {
        let n = PersianNormalizer::builder().with_strip_zwnj(true);
        let input = "می\u{200C}روم";
        assert_eq!(n.normalize(input), "میروم");
        assert!(n.strips_zwnj());
    }

    #[test]
    fn western_digits_opt_in() {
        let n = PersianNormalizer::builder().with_western_digits(true);
        let pairs = [
            ("۰", "0"),
            ("۱", "1"),
            ("۲", "2"),
            ("۳", "3"),
            ("۴", "4"),
            ("۵", "5"),
            ("۶", "6"),
            ("۷", "7"),
            ("۸", "8"),
            ("۹", "9"),
        ];
        for (input, expected) in pairs {
            assert_eq!(n.normalize(input), expected, "fold failed on {input:?}");
        }
        assert_eq!(n.normalize("۲۰۲۴"), "2024");
        assert!(n.folds_to_western_digits());
    }

    #[test]
    fn heh_yeh_decomposition_opt_in() {
        let n = PersianNormalizer::builder().with_heh_yeh_normalization(true);
        // ۀ (U+06C0) → ه (U+0647) + ی (U+06CC).
        assert_eq!(n.normalize("ۀ"), "\u{0647}\u{06CC}");
        assert_eq!(n.normalize("خانۀ"), "خانه\u{06CC}");
        assert!(n.decomposes_heh_yeh());
    }

    // -------------------------------------------------------------
    // Idempotence.
    // -------------------------------------------------------------

    #[test]
    fn idempotent_on_typical_inputs() {
        for w in ["علي", "كتاب", "مـحمد", "می\u{200C}روم", "خانۀ", "hello"] {
            let once = normalize(w);
            let twice = normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_strip_zwnj_on() {
        let n = PersianNormalizer::builder().with_strip_zwnj(true);
        for w in ["می\u{200C}روم", "کتاب\u{200C}ها", "\u{200C}", "علی"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_western_digits_on() {
        let n = PersianNormalizer::builder().with_western_digits(true);
        for w in ["۲۰۲۴", "2024", "۰۹", "hello ۴۲"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_heh_yeh_on() {
        let n = PersianNormalizer::builder().with_heh_yeh_normalization(true);
        for w in ["ۀ", "خانۀ", "\u{0647}\u{06CC}"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    // -------------------------------------------------------------
    // Non-Persian input.
    // -------------------------------------------------------------

    #[test]
    fn passes_through_non_persian() {
        assert_eq!(normalize("hello world"), "hello world");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("123"), "123");
    }

    #[test]
    fn preserves_ascii_around_persian() {
        // Arabic yeh in an Arabic name embedded in English — folds.
        assert_eq!(normalize("Ali علي!"), "Ali علی!");
    }

    // -------------------------------------------------------------
    // Helper predicates.
    // -------------------------------------------------------------

    #[test]
    fn fold_to_western_digit_covers_advertised_range() {
        for cp in 0x06F0u32..=0x06F9 {
            let c = char::from_u32(cp).unwrap();
            let out = fold_to_western_digit(c);
            assert_eq!(out as u32, 0x0030 + (cp - 0x06F0));
        }
        // Adjacent scalars pass through.
        assert_eq!(fold_to_western_digit('\u{06EF}'), '\u{06EF}');
        assert_eq!(fold_to_western_digit('\u{06FA}'), '\u{06FA}');
        // Non-digits pass through.
        assert_eq!(fold_to_western_digit('a'), 'a');
        assert_eq!(fold_to_western_digit('ک'), 'ک');
    }

    #[test]
    fn fold_arabic_yeh_covers_both_source_scalars() {
        assert_eq!(fold_arabic_yeh('\u{064A}'), '\u{06CC}');
        assert_eq!(fold_arabic_yeh('\u{0649}'), '\u{06CC}');
        assert_eq!(fold_arabic_yeh('\u{06CC}'), '\u{06CC}');
        // Non-yeh passes through.
        assert_eq!(fold_arabic_yeh('a'), 'a');
    }

    #[test]
    fn fold_arabic_kaf_covers_only_arabic_kaf() {
        assert_eq!(fold_arabic_kaf('\u{0643}'), '\u{06A9}');
        assert_eq!(fold_arabic_kaf('\u{06A9}'), '\u{06A9}');
        // Non-kaf passes through.
        assert_eq!(fold_arabic_kaf('a'), 'a');
    }
}
