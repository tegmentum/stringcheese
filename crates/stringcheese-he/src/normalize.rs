//! Hebrew-specific text normalization.
//!
//! Hebrew orthography carries several *optional* marks (niqqud vowel
//! points, te'amim cantillation) and several *distinguished-but-often-
//! interchangeable* letter forms (five final-position variants of `כ מ נ פ צ`)
//! that an IR pipeline usually wants collapsed before matching. This
//! module ships a [`normalize`] one-shot function for the common case
//! and a [`HebrewNormalizer`] builder for callers who need to configure
//! the more controversial folds.
//!
//! # What is on by default
//!
//! [`normalize`] and [`HebrewNormalizer::default`] both apply:
//!
//! - **Strip niqqud (vowel points).** The full modern niqqud set:
//!
//!   | Code point | Name                      | Symbol |
//!   |------------|---------------------------|--------|
//!   | U+05B0     | Sheva                     | `ְ`   |
//!   | U+05B1     | Hataf Segol               | `ֱ`   |
//!   | U+05B2     | Hataf Patah               | `ֲ`   |
//!   | U+05B3     | Hataf Qamats              | `ֳ`   |
//!   | U+05B4     | Hiriq                     | `ִ`   |
//!   | U+05B5     | Tsere                     | `ֵ`   |
//!   | U+05B6     | Segol                     | `ֶ`   |
//!   | U+05B7     | Patah                     | `ַ`   |
//!   | U+05B8     | Qamats                    | `ָ`   |
//!   | U+05B9     | Holam                     | `ֹ`   |
//!   | U+05BA     | Holam Haser for Vav       | `ֺ`   |
//!   | U+05BB     | Qubuts                    | `ֻ`   |
//!   | U+05BC     | Dagesh (or Shuruq)        | `ּ`   |
//!   | U+05BE     | Maqaf (see note below)    | `־`   |
//!   | U+05BF     | Rafe                      | `ֿ`   |
//!   | U+05C1     | Shin Dot                  | `ׁ`   |
//!   | U+05C2     | Sin Dot                   | `ׂ`   |
//!   | U+05C4     | Upper Dot                 | `ׄ`   |
//!   | U+05C5     | Lower Dot                 | `ׅ`   |
//!   | U+05C7     | Qamats Qatan              | `ׇ`   |
//!
//!   These are combining marks (except maqaf) that carry vowel or
//!   distinguishing-dot information but not lexical identity; newswire
//!   and web text almost universally omit them.
//!
//!   **Maqaf note.** The task specification lists U+05BE (maqaf, the
//!   Hebrew hyphen) under the niqqud range. Maqaf is *not* a niqqud —
//!   it's an orthographic punctuation character that joins compound
//!   words like `בית־ספר` "school". Stripping it under
//!   [`with_strip_niqqud`](HebrewNormalizer::with_strip_niqqud) will
//!   collapse the compound to `ביתספר`, which is *not* what a tokenizer
//!   downstream from this normalizer wants. Two safer options for
//!   callers who care about compound-word integrity: (a) tokenize
//!   *before* normalizing (the maqaf is preserved by
//!   [`HebrewTokenizer`](crate::tokenizer::HebrewTokenizer) as a
//!   word-internal joiner), or (b) leave niqqud stripping on and let
//!   the compound halves be re-joined by the search analyzer. The
//!   dedicated [`with_strip_hebrew_punctuation`](HebrewNormalizer::with_strip_hebrew_punctuation)
//!   flag also removes maqaf when a caller wants an explicit knob.
//!
//! - **Strip te'amim (cantillation marks).** U+0591..=U+05AF — the
//!   ~30 combining marks used in Biblical Hebrew to annotate chant.
//!   Almost never appear in modern text; stripping is unconditional
//!   for search / comparison contexts. See
//!   [`with_strip_cantillation`](HebrewNormalizer::with_strip_cantillation).
//!
//! # What is opt-in
//!
//! - **Final-form folding (`ך → כ`, `ם → מ`, `ן → נ`, `ף → פ`, `ץ → צ`).**
//!   Off by default. Hebrew orthography distinguishes non-final and
//!   final positions strictly — writing `מלכ` (with medial kaf) at the
//!   end of a word instead of `מלך` (final kaf) is a spelling error, so
//!   the final forms are *semantically meaningful position markers*
//!   that most callers should preserve. Turn this on for
//!   consonantal-skeleton indexing (where the position marker is
//!   noise) or for cross-matching against sloppy input. Note that the
//!   phonetic encoder (see [`crate::phonetic`]) *always* folds finals
//!   to base forms regardless of this flag; the flag controls the
//!   surface form only.
//!
//! - **Strip Hebrew punctuation.** Off by default. Removes maqaf
//!   (`־` U+05BE), geresh (`׳` U+05F3), and gershayim (`״` U+05F4).
//!   Geresh appears in abbreviated single-letter names (`מ׳` short
//!   for `מר` "Mr."); gershayim marks acronyms (`ד״ר` "Dr.",
//!   `צה״ל` "IDF"). Stripping them collapses `ד״ר` and `דר` under one
//!   key.
//!
//! # What is *not* normalized
//!
//! - **Alternate letter forms (final vs. base).** Handled by the
//!   opt-in [`with_final_form_folding`](HebrewNormalizer::with_final_form_folding)
//!   flag.
//! - **Yiddish digraphs (`װ ױ ײ` U+05F0..=U+05F2) and Hebrew letter
//!   alternatives.** These belong to Yiddish orthography; the
//!   normalizer preserves them for downstream `stringcheese-yi` handling.
//! - **Presentation forms.** U+FB1D..=U+FB4F — the Hebrew presentation
//!   forms block — is display-only precomposed-with-niqqud codepoints
//!   and is out of scope for this normalizer. Text in that block
//!   should be reduced to logical form via a Unicode normalization
//!   pass (NFC or NFKC) before this normalizer runs.
//!
//! # Idempotence
//!
//! The normalizer is idempotent for every single-flag configuration:
//! `normalize(normalize(x)) == normalize(x)`. All rules are strict
//! deletions or same-length substitutions; none produce a character
//! that a later rule matches. See the crate's property-test module for
//! the machine-checked assertion.
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

/// Configurable Hebrew normalizer.
///
/// A zero-sized-ish value (four bool flags packed by the compiler);
/// construct via [`HebrewNormalizer::default`] or
/// [`HebrewNormalizer::new`] and reuse across threads and calls, or use
/// [`HebrewNormalizer::builder`] to opt in to the extra folds.
///
/// See the [module-level docs](self) for the list of rules and the
/// rationale for each opt-in flag.
// The four flags are genuinely independent orthographic knobs — a
// state machine would obscure the API rather than clarify it, so we
// silence the excessive-bools lint.
#[allow(clippy::struct_excessive_bools)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct HebrewNormalizer {
    /// If `true`, strip niqqud (vowel points and related dots). On by
    /// default — modern Hebrew text is unpointed and the marks are
    /// noise for IR.
    strip_niqqud: bool,
    /// If `true`, strip te'amim (cantillation marks, U+0591..=U+05AF).
    /// On by default — cantillation is Biblical-only.
    strip_cantillation: bool,
    /// If `true`, fold the five final letter forms (`ך ם ן ף ץ`) to
    /// their base forms (`כ מ נ פ צ`). Off by default (see the
    /// module-level docs — final forms are semantically meaningful
    /// position markers).
    fold_final_forms: bool,
    /// If `true`, strip Hebrew punctuation — maqaf (`־` U+05BE),
    /// geresh (`׳` U+05F3), and gershayim (`״` U+05F4). Off by default.
    strip_hebrew_punctuation: bool,
}

impl Default for HebrewNormalizer {
    /// The default normalizer: niqqud stripped, cantillation stripped,
    /// final forms preserved, Hebrew punctuation preserved.
    fn default() -> Self {
        Self::new()
    }
}

impl HebrewNormalizer {
    /// Construct the default normalizer — strips niqqud and
    /// cantillation, preserves final forms, preserves Hebrew
    /// punctuation.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            strip_niqqud: true,
            strip_cantillation: true,
            fold_final_forms: false,
            strip_hebrew_punctuation: false,
        }
    }

    /// Enter the builder form — chain `.with_final_form_folding(true)`
    /// or `.with_strip_hebrew_punctuation(true)` to opt in to the
    /// extra rewrites; call `.with_strip_niqqud(false)` /
    /// `.with_strip_cantillation(false)` to turn off the defaults.
    #[inline]
    #[must_use]
    pub const fn builder() -> Self {
        Self::new()
    }

    /// Toggle niqqud stripping. On by default (see the module-level
    /// docs — modern Hebrew is unpointed).
    #[inline]
    #[must_use]
    pub const fn with_strip_niqqud(mut self, on: bool) -> Self {
        self.strip_niqqud = on;
        self
    }

    /// Returns whether niqqud stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_niqqud(self) -> bool {
        self.strip_niqqud
    }

    /// Toggle cantillation-mark stripping. On by default.
    #[inline]
    #[must_use]
    pub const fn with_strip_cantillation(mut self, on: bool) -> Self {
        self.strip_cantillation = on;
        self
    }

    /// Returns whether cantillation stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_cantillation(self) -> bool {
        self.strip_cantillation
    }

    /// Toggle final-form → base-form folding
    /// (`ך → כ`, `ם → מ`, `ן → נ`, `ף → פ`, `ץ → צ`).
    ///
    /// Off by default; see the module-level docs for the trade-off.
    /// Note that the phonetic encoder (see [`crate::phonetic`]) *always*
    /// folds final forms regardless of this flag — this knob controls
    /// the surface form the normalizer emits.
    #[inline]
    #[must_use]
    pub const fn with_final_form_folding(mut self, on: bool) -> Self {
        self.fold_final_forms = on;
        self
    }

    /// Returns whether final-form folding is enabled.
    #[inline]
    #[must_use]
    pub const fn folds_final_forms(self) -> bool {
        self.fold_final_forms
    }

    /// Toggle stripping of Hebrew punctuation — maqaf (`־` U+05BE),
    /// geresh (`׳` U+05F3), and gershayim (`״` U+05F4). Off by default.
    ///
    /// Turn this on to collapse `ד״ר` (`Dr.`) and `דר` under one
    /// stopword-key candidate, or to remove the maqaf compound-word
    /// joiner when downstream code expects a clean whitespace split.
    #[inline]
    #[must_use]
    pub const fn with_strip_hebrew_punctuation(mut self, on: bool) -> Self {
        self.strip_hebrew_punctuation = on;
        self
    }

    /// Returns whether Hebrew-punctuation stripping is enabled.
    #[inline]
    #[must_use]
    pub const fn strips_hebrew_punctuation(self) -> bool {
        self.strip_hebrew_punctuation
    }

    /// Normalize `text` under this configuration.
    ///
    /// Returns an owned [`String`]. The output byte length is bounded
    /// by the input's — every rule either deletes bytes (niqqud,
    /// cantillation, punctuation stripping) or performs a same-length
    /// UTF-8 substitution (final-form folding — both variants are
    /// 2-byte scalars in the U+05Dx range).
    #[must_use]
    pub fn normalize(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            // Strip niqqud (opt-out — on by default).
            if self.strip_niqqud && is_niqqud(c) {
                continue;
            }
            // Strip cantillation marks (opt-out — on by default).
            if self.strip_cantillation && is_cantillation(c) {
                continue;
            }
            // Strip Hebrew punctuation (opt-in — off by default).
            if self.strip_hebrew_punctuation && is_hebrew_punctuation(c) {
                continue;
            }
            // Fold final forms (opt-in — off by default).
            let c = if self.fold_final_forms {
                fold_final_form(c)
            } else {
                c
            };
            out.push(c);
        }
        out
    }
}

/// One-shot normalization with the default configuration (niqqud
/// stripped, cantillation stripped, final forms *preserved*, Hebrew
/// punctuation *preserved*).
///
/// Equivalent to `HebrewNormalizer::new().normalize(text)`. See the
/// [module-level docs](self) for what each rule does.
///
/// # Examples
///
/// ```
/// use stringcheese_he::normalize::normalize;
///
/// // Niqqud stripping.
/// assert_eq!(normalize("שָׁלוֹם"), "שלום");
/// // Final forms are preserved by default.
/// assert_eq!(normalize("מלך"), "מלך");
/// // Maqaf gets stripped by default because the task spec places
/// // U+05BE inside the niqqud set — see the module-level docs for
/// // the reasoning and the caller's escape hatches for compound-word
/// // preservation.
/// assert_eq!(normalize("בית־ספר"), "ביתספר");
/// ```
#[must_use]
pub fn normalize(text: &str) -> String {
    HebrewNormalizer::new().normalize(text)
}

/// Is `c` a Hebrew niqqud (vowel-point / dagesh / rafe / sin-dot /
/// shin-dot) scalar?
///
/// Returns `true` for the codepoints listed in the module-level docs.
/// Follows the task specification's set, which includes U+05BE (maqaf);
/// see the module docs for the reasoning and the caller's escape
/// hatches for compound-word preservation.
#[inline]
#[must_use]
pub const fn is_niqqud(c: char) -> bool {
    matches!(
        c,
        // The vowel-point / dagesh range.
        '\u{05B0}'
            ..='\u{05BC}'
        // Maqaf (per spec; see module docs — this is the punctuation
        // hyphen, not a diacritic, but the spec groups it here).
        | '\u{05BE}'
        // Rafe.
        | '\u{05BF}'
        // Shin dot, sin dot.
        | '\u{05C1}' | '\u{05C2}'
        // Upper dot, lower dot.
        | '\u{05C4}' | '\u{05C5}'
        // Qamats qatan.
        | '\u{05C7}'
    )
}

/// Is `c` a Hebrew cantillation (te'amim) mark?
///
/// Returns `true` for U+0591..=U+05AF — the full cantillation range.
#[inline]
#[must_use]
pub const fn is_cantillation(c: char) -> bool {
    matches!(c, '\u{0591}'..='\u{05AF}')
}

/// Is `c` a Hebrew punctuation scalar the
/// [`HebrewNormalizer::with_strip_hebrew_punctuation`] flag removes?
///
/// Returns `true` for maqaf (U+05BE), geresh (U+05F3), and gershayim
/// (U+05F4).
#[inline]
#[must_use]
pub const fn is_hebrew_punctuation(c: char) -> bool {
    matches!(c, '\u{05BE}' | '\u{05F3}' | '\u{05F4}')
}

/// Fold a Hebrew final-form letter to its base form:
/// `ך → כ` (U+05DA → U+05DB),
/// `ם → מ` (U+05DD → U+05DE),
/// `ן → נ` (U+05DF → U+05E0),
/// `ף → פ` (U+05E3 → U+05E4),
/// `ץ → צ` (U+05E5 → U+05E6).
///
/// Returns the input unchanged for other scalars.
#[inline]
#[must_use]
pub const fn fold_final_form(c: char) -> char {
    match c {
        '\u{05DA}' => '\u{05DB}', // ך → כ
        '\u{05DD}' => '\u{05DE}', // ם → מ
        '\u{05DF}' => '\u{05E0}', // ן → נ
        '\u{05E3}' => '\u{05E4}', // ף → פ
        '\u{05E5}' => '\u{05E6}', // ץ → צ
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------
    // Niqqud stripping.
    // -------------------------------------------------------------

    #[test]
    fn strips_shalom_niqqud() {
        // שָׁלוֹם — the fully-pointed spelling of "shalom".
        assert_eq!(normalize("שָׁלוֹם"), "שלום");
    }

    #[test]
    fn strips_all_niqqud_codepoints() {
        // Build a string bookended by two letters with every niqqud
        // scalar between them, then verify normalize wipes them.
        let mut input = String::from("א");
        for cp in 0x05B0u32..=0x05BC {
            input.push(char::from_u32(cp).unwrap());
        }
        input.push('\u{05BF}');
        input.push('\u{05C1}');
        input.push('\u{05C2}');
        input.push('\u{05C4}');
        input.push('\u{05C5}');
        input.push('\u{05C7}');
        input.push('ב');
        assert_eq!(normalize(&input), "אב");
    }

    #[test]
    fn niqqud_stripping_can_be_disabled() {
        let n = HebrewNormalizer::builder().with_strip_niqqud(false);
        assert_eq!(n.normalize("שָׁלוֹם"), "שָׁלוֹם");
    }

    #[test]
    fn niqqud_flag_reads_back() {
        assert!(HebrewNormalizer::default().strips_niqqud());
        let off = HebrewNormalizer::builder().with_strip_niqqud(false);
        assert!(!off.strips_niqqud());
    }

    // -------------------------------------------------------------
    // Cantillation stripping.
    // -------------------------------------------------------------

    #[test]
    fn strips_cantillation_range() {
        // Build a string with several cantillation marks and verify
        // normalize wipes them.
        let mut input = String::from("א");
        for cp in 0x0591u32..=0x05AF {
            input.push(char::from_u32(cp).unwrap());
        }
        input.push('ב');
        assert_eq!(normalize(&input), "אב");
    }

    #[test]
    fn cantillation_stripping_can_be_disabled() {
        let n = HebrewNormalizer::builder().with_strip_cantillation(false);
        // An etnahta between two letters passes through.
        let input = "א\u{0591}ב";
        assert_eq!(n.normalize(input), input);
    }

    #[test]
    fn cantillation_flag_reads_back() {
        assert!(HebrewNormalizer::default().strips_cantillation());
        let off = HebrewNormalizer::builder().with_strip_cantillation(false);
        assert!(!off.strips_cantillation());
    }

    // -------------------------------------------------------------
    // Final-form folding (opt-in).
    // -------------------------------------------------------------

    #[test]
    fn final_forms_preserved_by_default() {
        // מלך (king) has final kaf; ends unchanged under the default.
        assert_eq!(normalize("מלך"), "מלך");
        assert_eq!(normalize("שלום"), "שלום"); // final mem
        assert_eq!(normalize("כן"), "כן"); // final nun
        assert_eq!(normalize("סוף"), "סוף"); // final pe
        assert_eq!(normalize("ארץ"), "ארץ"); // final tsadi
    }

    #[test]
    fn final_forms_folded_when_opted_in() {
        let n = HebrewNormalizer::builder().with_final_form_folding(true);
        assert_eq!(n.normalize("מלך"), "מלכ"); // ך → כ
        assert_eq!(n.normalize("שלום"), "שלומ"); // ם → מ
        assert_eq!(n.normalize("כן"), "כנ"); // ן → נ
        assert_eq!(n.normalize("סוף"), "סופ"); // ף → פ
        assert_eq!(n.normalize("ארץ"), "ארצ"); // ץ → צ
    }

    #[test]
    fn final_form_folding_flag_reads_back() {
        let n = HebrewNormalizer::builder().with_final_form_folding(true);
        assert!(n.folds_final_forms());
        assert!(!HebrewNormalizer::default().folds_final_forms());
    }

    #[test]
    fn all_five_final_forms_covered() {
        // Every fold_final_form output is the correct base scalar.
        assert_eq!(fold_final_form('\u{05DA}'), '\u{05DB}');
        assert_eq!(fold_final_form('\u{05DD}'), '\u{05DE}');
        assert_eq!(fold_final_form('\u{05DF}'), '\u{05E0}');
        assert_eq!(fold_final_form('\u{05E3}'), '\u{05E4}');
        assert_eq!(fold_final_form('\u{05E5}'), '\u{05E6}');
        // Non-final letters pass through.
        assert_eq!(fold_final_form('\u{05DB}'), '\u{05DB}');
        assert_eq!(fold_final_form('א'), 'א');
    }

    // -------------------------------------------------------------
    // Hebrew-punctuation stripping (opt-in).
    // -------------------------------------------------------------

    #[test]
    fn hebrew_punctuation_default_preservation() {
        // Under the plain default, geresh and gershayim pass through.
        assert_eq!(normalize("ד״ר"), "ד״ר");
        assert_eq!(normalize("מ׳"), "מ׳");
        // Note: maqaf gets stripped under default because it is
        // included in the niqqud set per the task spec. See the
        // module-level docs.
    }

    #[test]
    fn hebrew_punctuation_stripped_when_opted_in() {
        let n = HebrewNormalizer::builder().with_strip_hebrew_punctuation(true);
        assert_eq!(n.normalize("ד״ר"), "דר");
        assert_eq!(n.normalize("מ׳"), "מ");
    }

    #[test]
    fn hebrew_punctuation_flag_reads_back() {
        let n = HebrewNormalizer::builder().with_strip_hebrew_punctuation(true);
        assert!(n.strips_hebrew_punctuation());
        assert!(!HebrewNormalizer::default().strips_hebrew_punctuation());
    }

    // -------------------------------------------------------------
    // Maqaf — the delicate case.
    // -------------------------------------------------------------

    #[test]
    fn maqaf_stripped_by_default_niqqud_flag() {
        // The task spec includes U+05BE in the niqqud set; the default
        // normalizer strips it. Callers who want compound-word integrity
        // should tokenize before normalizing.
        assert_eq!(normalize("בית־ספר"), "ביתספר");
    }

    #[test]
    fn maqaf_preserved_when_niqqud_stripping_off() {
        let n = HebrewNormalizer::builder().with_strip_niqqud(false);
        assert_eq!(n.normalize("בית־ספר"), "בית־ספר");
    }

    // -------------------------------------------------------------
    // Idempotence.
    // -------------------------------------------------------------

    #[test]
    fn idempotent_on_typical_inputs() {
        for w in ["שָׁלוֹם", "מלך", "בית", "אני", "ד״ר"] {
            let once = normalize(w);
            let twice = normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_final_form_folding_on() {
        let n = HebrewNormalizer::builder().with_final_form_folding(true);
        for w in ["מלך", "שלום", "כן", "סוף", "ארץ"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    #[test]
    fn idempotent_with_hebrew_punctuation_stripping_on() {
        let n = HebrewNormalizer::builder().with_strip_hebrew_punctuation(true);
        for w in ["ד״ר", "מ׳", "בית־ספר", "צה״ל"] {
            let once = n.normalize(w);
            let twice = n.normalize(&once);
            assert_eq!(once, twice, "normalize not idempotent on {w:?}");
        }
    }

    // -------------------------------------------------------------
    // Non-Hebrew input.
    // -------------------------------------------------------------

    #[test]
    fn passes_through_non_hebrew() {
        assert_eq!(normalize("hello world"), "hello world");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("123"), "123");
    }

    #[test]
    fn preserves_ascii_around_hebrew() {
        assert_eq!(normalize("Hello שָׁלוֹם!"), "Hello שלום!");
    }

    // -------------------------------------------------------------
    // Helper predicates.
    // -------------------------------------------------------------

    #[test]
    fn is_niqqud_covers_the_advertised_scalars() {
        for cp in 0x05B0u32..=0x05BC {
            assert!(is_niqqud(char::from_u32(cp).unwrap()));
        }
        assert!(is_niqqud('\u{05BE}'));
        assert!(is_niqqud('\u{05BF}'));
        assert!(is_niqqud('\u{05C1}'));
        assert!(is_niqqud('\u{05C2}'));
        assert!(is_niqqud('\u{05C4}'));
        assert!(is_niqqud('\u{05C5}'));
        assert!(is_niqqud('\u{05C7}'));
        // Not niqqud: base letters, boundaries, and the excluded gaps.
        assert!(!is_niqqud('א'));
        assert!(!is_niqqud('\u{05C0}')); // paseq (punctuation)
        assert!(!is_niqqud('\u{05C3}')); // sof pasuq (punctuation)
        assert!(!is_niqqud('\u{05C6}')); // nun hafukha
        assert!(!is_niqqud('\u{05BD}')); // meteg (a distinct diacritic)
    }

    #[test]
    fn is_cantillation_covers_the_advertised_range() {
        for cp in 0x0591u32..=0x05AF {
            assert!(is_cantillation(char::from_u32(cp).unwrap()));
        }
        assert!(!is_cantillation('\u{0590}'));
        assert!(!is_cantillation('\u{05B0}'));
    }

    #[test]
    fn is_hebrew_punctuation_covers_the_three_marks() {
        assert!(is_hebrew_punctuation('\u{05BE}'));
        assert!(is_hebrew_punctuation('\u{05F3}'));
        assert!(is_hebrew_punctuation('\u{05F4}'));
        assert!(!is_hebrew_punctuation('א'));
        assert!(!is_hebrew_punctuation('\u{05C0}')); // paseq — separate class
    }
}
