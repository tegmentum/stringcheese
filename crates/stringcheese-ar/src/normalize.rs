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
//! - **Strip tatweel (kashida, `ـ` U+0640).** The purely-orthographic
//!   letter-stretcher used for justification carries no semantic
//!   content but appears in copied-from-rendered-document input. Off
//!   by default in the plain [`ArabicNormalizer::builder`] (which
//!   respects the input the caller gave it); on by default in the
//!   [`ArabicNormalizer::DEFAULT_FOR_SEARCH`] preset (search and
//!   comparison contexts always want it gone).
//!
//! - **Digit normalization.** Arabic text mixes three digit blocks:
//!   Western Arabic digits (`0123456789`, U+0030..=U+0039), Eastern
//!   Arabic-Indic digits (`٠١٢٣٤٥٦٧٨٩`, U+0660..=U+0669), and Extended
//!   Arabic-Indic digits (`۰۱۲۳۴۵۶۷۸۹`, U+06F0..=U+06F9) used in
//!   Persian, Urdu, and Pashto. [`ArabicNormalizer::with_western_digits`]
//!   folds both Eastern blocks down to Western digits (numeric-search
//!   use-case); [`ArabicNormalizer::with_eastern_digits`] does the
//!   reverse (rendering-parity use-case). Both flags on at once is
//!   *undefined-order* — see [`ArabicNormalizer::with_eastern_digits`]
//!   for the note; typical usage picks exactly one direction.
//!
//! # What is *not* normalized
//!
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
//! (Both digit-direction flags on at the same time is the documented
//! undefined-order case and does not carry the idempotence guarantee.)
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

/// Configurable Arabic normalizer.
///
/// A zero-sized-ish value (four bool flags packed by the compiler);
/// construct via [`ArabicNormalizer::default`] or
/// [`ArabicNormalizer::new`] and reuse across threads and calls, or use
/// [`ArabicNormalizer::builder`] to opt in to the extra folds.
///
/// See the [module-level docs](self) for the list of rules and the
/// rationale for each opt-in flag.
// The four flags are genuinely independent orthographic knobs — a
// state machine would obscure the API rather than clarify it, so we
// silence the excessive-bools lint.
#[allow(clippy::struct_excessive_bools)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArabicNormalizer {
    /// If `true`, fold teh marbuta (`ة`) to plain heh (`ه`). Off by
    /// default; see the module-level docs for the trade-off.
    fold_teh_marbuta: bool,
    /// If `true`, strip tatweel (`ـ` U+0640) — the purely-orthographic
    /// letter-stretcher — from the input. Off in the plain builder,
    /// on in [`ArabicNormalizer::DEFAULT_FOR_SEARCH`].
    strip_tatweel: bool,
    /// If `true`, fold Eastern Arabic-Indic digits (U+0660..=U+0669)
    /// and Extended Arabic-Indic digits (U+06F0..=U+06F9) down to
    /// Western Arabic digits (U+0030..=U+0039). Off by default.
    fold_to_western_digits: bool,
    /// If `true`, fold Western Arabic digits (U+0030..=U+0039) up to
    /// Eastern Arabic-Indic digits (U+0660..=U+0669). Off by default.
    fold_to_eastern_digits: bool,
}

impl ArabicNormalizer {
    /// Construct the default normalizer — strips diacritics, folds
    /// alef variants, folds `ى` → `ي`. Teh marbuta is *not* folded,
    /// tatweel is preserved, and digits pass through unchanged.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fold_teh_marbuta: false,
            strip_tatweel: false,
            fold_to_western_digits: false,
            fold_to_eastern_digits: false,
        }
    }

    /// Enter the builder form — chain `.with_teh_marbuta_folding(true)`,
    /// `.with_strip_tatweel(true)`, `.with_western_digits(true)`, or
    /// `.with_eastern_digits(true)` to opt in to the extra rewrites.
    #[inline]
    #[must_use]
    pub const fn builder() -> Self {
        Self::new()
    }

    /// Preset for **search and comparison** contexts.
    ///
    /// Extends the plain default with tatweel stripping — the
    /// letter-stretcher `ـ` (U+0640) has no semantic content, and any
    /// pipeline that compares strings or builds a search index should
    /// collapse `مـحـمـد` and `محمد` to the same key. The digit
    /// direction flags stay off; if the caller wants a specific digit
    /// direction they layer it on:
    ///
    /// ```
    /// use stringcheese_ar::normalize::ArabicNormalizer;
    ///
    /// let n = ArabicNormalizer::DEFAULT_FOR_SEARCH.with_western_digits(true);
    /// assert_eq!(n.normalize("مـحـمـد ٢٠٢٤"), "محمد 2024");
    /// ```
    pub const DEFAULT_FOR_SEARCH: Self = Self::new().with_strip_tatweel(true);

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

    /// Toggle stripping of tatweel (`ـ` U+0640). Off by default in the
    /// plain builder; on in [`ArabicNormalizer::DEFAULT_FOR_SEARCH`].
    ///
    /// Tatweel is a decorative elongation character used for
    /// justification; it carries no semantic content, so stripping it
    /// makes tatweel-decorated variants (`مـحـمـد`) collapse to their
    /// undecorated form (`محمد`).
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

    /// Toggle folding of Eastern digits to Western digits.
    ///
    /// When on, translates Arabic-Indic digits `٠..٩`
    /// (U+0660..=U+0669) and Extended Arabic-Indic digits `۰..۹`
    /// (U+06F0..=U+06F9) — the latter used in Persian, Urdu, and
    /// Pashto — to Western Arabic digits `0..9` (U+0030..=U+0039). Off
    /// by default. Turn this on for numeric-search use-cases where a
    /// document written with Eastern digits should match a query typed
    /// with Western digits.
    ///
    /// Both this flag and [`with_eastern_digits`](Self::with_eastern_digits)
    /// on at the same time is *undefined-order*: the current
    /// implementation applies Eastern → Western first and then Western
    /// → Eastern, so the net effect is that everything ends up
    /// Eastern, but callers should not rely on that ordering. Typical
    /// usage picks exactly one direction.
    #[inline]
    #[must_use]
    pub const fn with_western_digits(mut self, on: bool) -> Self {
        self.fold_to_western_digits = on;
        self
    }

    /// Returns whether Eastern → Western digit folding is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_to_western_digits(self) -> bool {
        self.fold_to_western_digits
    }

    /// Toggle folding of Western digits to Eastern digits.
    ///
    /// When on, translates Western Arabic digits `0..9`
    /// (U+0030..=U+0039) to Arabic-Indic digits `٠..٩`
    /// (U+0660..=U+0669). Off by default. Turn this on to emit
    /// numeric output in the traditional Arabic look — the
    /// rendering-parity use-case.
    ///
    /// Note that this rewrite *grows* the output: an ASCII digit is
    /// one UTF-8 byte, while an Arabic-Indic digit is two. See
    /// [`with_western_digits`](Self::with_western_digits) for the
    /// note about both flags on at once.
    #[inline]
    #[must_use]
    pub const fn with_eastern_digits(mut self, on: bool) -> Self {
        self.fold_to_eastern_digits = on;
        self
    }

    /// Returns whether Western → Eastern digit folding is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_to_eastern_digits(self) -> bool {
        self.fold_to_eastern_digits
    }

    /// Normalize `text` under this configuration.
    ///
    /// Returns an owned [`String`]. In every configuration *except*
    /// `with_eastern_digits(true)`, the output byte length is bounded
    /// by the input's — the harakat / tatweel rules delete bytes, and
    /// the alef / yeh / teh-marbuta / eastern-digit rewrites are all
    /// same-length substitutions in UTF-8. With
    /// `with_eastern_digits(true)`, ASCII digits (1 byte) expand to
    /// Arabic-Indic digits (2 bytes each) and the output can grow.
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        // Worst-case size: input.len() bytes, plus one extra byte per
        // ASCII digit if we're folding to Eastern. Cheap upper bound.
        let cap = if self.fold_to_eastern_digits {
            // ASCII digits are 1 byte each; Arabic-Indic digits are 2.
            // Overestimating by input.len() (i.e. doubling) is safe and
            // spares a per-char scan. Use saturating_mul because the
            // input can be huge on 32-bit platforms — the arithmetic
            // trap would be a real hazard.
            text.len().saturating_mul(2)
        } else {
            text.len()
        };
        let mut out = String::with_capacity(cap);
        for c in text.chars() {
            // Strip harakat and dagger alef.
            if is_harakat(c) {
                continue;
            }
            // Strip tatweel (opt-in).
            if self.strip_tatweel && c == '\u{0640}' {
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
            // Fold Eastern digits to Western (opt-in).
            let c = if self.fold_to_western_digits {
                fold_to_western_digit(c)
            } else {
                c
            };
            // Fold Western digits to Eastern (opt-in). See
            // `with_western_digits` for the note about both flags on
            // at once.
            let c = if self.fold_to_eastern_digits {
                fold_to_eastern_digit(c)
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
/// *preserved*, tatweel *preserved*, digits pass through).
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
/// // Tatweel is *preserved* under the default configuration (search
/// // pipelines should reach for ArabicNormalizer::DEFAULT_FOR_SEARCH).
/// assert_eq!(normalize("مـحـمـد"), "مـحـمـد");
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

/// Fold an Eastern Arabic-Indic digit (`٠..٩` U+0660..=U+0669) or an
/// Extended Arabic-Indic digit (`۰..۹` U+06F0..=U+06F9) to the
/// corresponding Western Arabic digit (`0..9` U+0030..=U+0039).
/// Returns the input unchanged for other scalars.
#[inline]
#[must_use]
pub const fn fold_to_western_digit(c: char) -> char {
    match c {
        '\u{0660}'..='\u{0669}' => {
            // U+0660..=U+0669 → U+0030..=U+0039.
            let offset = c as u32 - 0x0660;
            // SAFETY-free: offset is in 0..=9, so 0x30+offset is a
            // valid ASCII digit.
            match char::from_u32(0x0030 + offset) {
                Some(d) => d,
                None => c,
            }
        }
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

/// Fold a Western Arabic digit (`0..9` U+0030..=U+0039) to the
/// corresponding Eastern Arabic-Indic digit (`٠..٩`
/// U+0660..=U+0669). Returns the input unchanged for other scalars.
///
/// Note that the Western → Eastern direction *grows* the UTF-8 output
/// (1 byte → 2 bytes per digit) — see
/// [`ArabicNormalizer::with_eastern_digits`] for the note.
#[inline]
#[must_use]
pub const fn fold_to_eastern_digit(c: char) -> char {
    match c {
        '0'..='9' => {
            let offset = c as u32 - 0x0030;
            match char::from_u32(0x0660 + offset) {
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
    // Tatweel (opt-in).
    // -------------------------------------------------------------

    #[test]
    fn tatweel_preserved_by_default() {
        // Plain builder preserves tatweel.
        assert_eq!(normalize("مـحـمـد"), "مـحـمـد");
        assert_eq!(normalize("ـ"), "ـ");
    }

    #[test]
    fn tatweel_stripped_when_opted_in() {
        let n = ArabicNormalizer::builder().with_strip_tatweel(true);
        // Tatweel-in-the-middle-of-a-word.
        assert_eq!(n.normalize("مـحـمـد"), "محمد");
        // Word-initial tatweel (rare but possible in decorated text).
        assert_eq!(n.normalize("ـعلي"), "علي");
        // Word-final tatweel.
        assert_eq!(n.normalize("عليـ"), "علي");
        // Multiple runs of tatweel.
        assert_eq!(n.normalize("مـــحـــمـــد"), "محمد");
        // Bare tatweel.
        assert_eq!(n.normalize("ـ"), "");
    }

    #[test]
    fn tatweel_stripping_flag_reads_back() {
        let n = ArabicNormalizer::builder().with_strip_tatweel(true);
        assert!(n.strips_tatweel());
        let m = ArabicNormalizer::default();
        assert!(!m.strips_tatweel());
    }

    #[test]
    fn search_preset_strips_tatweel() {
        let n = ArabicNormalizer::DEFAULT_FOR_SEARCH;
        assert!(n.strips_tatweel());
        assert!(!n.folds_teh_marbuta());
        assert!(!n.folds_to_western_digits());
        assert!(!n.folds_to_eastern_digits());
        assert_eq!(n.normalize("مـحـمـد"), "محمد");
    }

    /// Tatweel-decorated variants collapse to the same key under the
    /// search preset — the whole point of the flag.
    #[test]
    fn tatweel_decorated_variants_collapse_to_same_key() {
        let n = ArabicNormalizer::DEFAULT_FOR_SEARCH;
        let bare = n.normalize("محمد");
        for decorated in [
            "مـحمد",
            "محـمد",
            "محمـد",
            "مـحـمـد",
            "مـــحـــمـــد",
            "ـمحمدـ",
        ] {
            assert_eq!(
                n.normalize(decorated),
                bare,
                "tatweel-decorated {decorated:?} did not collapse to bare form"
            );
        }
    }

    // -------------------------------------------------------------
    // Digit normalization — Eastern → Western.
    // -------------------------------------------------------------

    /// Boundary and reference pairs for Arabic-Indic → Western.
    #[test]
    fn folds_arabic_indic_digits_to_western() {
        let n = ArabicNormalizer::builder().with_western_digits(true);
        // Ten reference pairs covering both boundaries (0 and 9) and
        // interior digits.
        let pairs = [
            ("٠", "0"),
            ("١", "1"),
            ("٢", "2"),
            ("٣", "3"),
            ("٤", "4"),
            ("٥", "5"),
            ("٦", "6"),
            ("٧", "7"),
            ("٨", "8"),
            ("٩", "9"),
        ];
        for (input, expected) in pairs {
            assert_eq!(n.normalize(input), expected, "fold failed on {input:?}");
        }
        // Multi-digit numbers.
        assert_eq!(n.normalize("٢٠٢٤"), "2024");
        assert_eq!(n.normalize("١٩٩٥"), "1995");
    }

    /// Extended Arabic-Indic (Persian / Urdu) digits also fold to Western.
    #[test]
    fn folds_extended_arabic_indic_digits_to_western() {
        let n = ArabicNormalizer::builder().with_western_digits(true);
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
    }

    #[test]
    fn western_digits_untouched_when_folding_east_to_west() {
        let n = ArabicNormalizer::builder().with_western_digits(true);
        assert_eq!(n.normalize("2024"), "2024");
        assert_eq!(n.normalize("hello 123"), "hello 123");
    }

    // -------------------------------------------------------------
    // Digit normalization — Western → Eastern.
    // -------------------------------------------------------------

    #[test]
    fn folds_western_digits_to_arabic_indic() {
        let n = ArabicNormalizer::builder().with_eastern_digits(true);
        // Ten reference pairs covering both boundaries and interior.
        let pairs = [
            ("0", "٠"),
            ("1", "١"),
            ("2", "٢"),
            ("3", "٣"),
            ("4", "٤"),
            ("5", "٥"),
            ("6", "٦"),
            ("7", "٧"),
            ("8", "٨"),
            ("9", "٩"),
        ];
        for (input, expected) in pairs {
            assert_eq!(n.normalize(input), expected, "fold failed on {input:?}");
        }
        assert_eq!(n.normalize("2024"), "٢٠٢٤");
    }

    #[test]
    fn arabic_indic_digits_untouched_when_folding_west_to_east() {
        let n = ArabicNormalizer::builder().with_eastern_digits(true);
        assert_eq!(n.normalize("٢٠٢٤"), "٢٠٢٤");
    }

    #[test]
    fn digit_flags_read_back() {
        let n = ArabicNormalizer::builder().with_western_digits(true);
        assert!(n.folds_to_western_digits());
        assert!(!n.folds_to_eastern_digits());
        let m = ArabicNormalizer::builder().with_eastern_digits(true);
        assert!(m.folds_to_eastern_digits());
        assert!(!m.folds_to_western_digits());
        let plain = ArabicNormalizer::default();
        assert!(!plain.folds_to_western_digits());
        assert!(!plain.folds_to_eastern_digits());
    }

    #[test]
    fn digits_untouched_when_flags_off() {
        // The plain default preserves all three digit blocks.
        assert_eq!(normalize("2024"), "2024");
        assert_eq!(normalize("٢٠٢٤"), "٢٠٢٤");
        assert_eq!(normalize("۲۰۲۴"), "۲۰۲۴");
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

    #[test]
    fn idempotent_with_strip_tatweel_on() {
        let n = ArabicNormalizer::builder().with_strip_tatweel(true);
        for w in ["مـحـمـد", "مـــحـــمـــد", "علي", "ـبيتـ", "ـ"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_western_digits_on() {
        let n = ArabicNormalizer::builder().with_western_digits(true);
        for w in ["٢٠٢٤", "۲۰۲۴", "2024", "٠٩", "٩٠", "hello ٤٢"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_eastern_digits_on() {
        let n = ArabicNormalizer::builder().with_eastern_digits(true);
        for w in ["2024", "٢٠٢٤", "0", "9", "hello 42"] {
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

    #[test]
    fn fold_to_western_digit_covers_advertised_ranges() {
        for cp in 0x0660u32..=0x0669 {
            let c = char::from_u32(cp).unwrap();
            let out = fold_to_western_digit(c);
            assert_eq!(out as u32, 0x0030 + (cp - 0x0660));
        }
        for cp in 0x06F0u32..=0x06F9 {
            let c = char::from_u32(cp).unwrap();
            let out = fold_to_western_digit(c);
            assert_eq!(out as u32, 0x0030 + (cp - 0x06F0));
        }
        // Adjacent scalars pass through.
        assert_eq!(fold_to_western_digit('\u{065F}'), '\u{065F}');
        assert_eq!(fold_to_western_digit('\u{066A}'), '\u{066A}');
        assert_eq!(fold_to_western_digit('\u{06EF}'), '\u{06EF}');
        assert_eq!(fold_to_western_digit('\u{06FA}'), '\u{06FA}');
        // Non-digits pass through.
        assert_eq!(fold_to_western_digit('a'), 'a');
        assert_eq!(fold_to_western_digit('ا'), 'ا');
    }

    #[test]
    fn fold_to_eastern_digit_covers_ascii_range() {
        for cp in 0x0030u32..=0x0039 {
            let c = char::from_u32(cp).unwrap();
            let out = fold_to_eastern_digit(c);
            assert_eq!(out as u32, 0x0660 + (cp - 0x0030));
        }
        // Adjacent scalars pass through.
        assert_eq!(fold_to_eastern_digit('/'), '/');
        assert_eq!(fold_to_eastern_digit(':'), ':');
        // Non-digits pass through.
        assert_eq!(fold_to_eastern_digit('a'), 'a');
        assert_eq!(fold_to_eastern_digit('ا'), 'ا');
    }
}
