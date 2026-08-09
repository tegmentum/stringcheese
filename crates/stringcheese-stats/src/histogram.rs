//! Unicode general-category histogram.
//!
//! Walks a string once and tallies every code point by its Unicode
//! general category (Lu, Ll, Nd, Po, …). Feeds decisions like
//! "should I normalize this to NFC before tokenizing?" or "is this
//! payload code-heavy vs prose-heavy?".
//!
//! The full 30-way category tally is [`Histogram::by_category`];
//! the coarse 7-way roll-up (Letter / Mark / Number / Punctuation /
//! Symbol / Separator / Other) is [`Histogram::by_major`].
//!
//! ## Unit
//!
//! Code points. Multibyte scalars count as one entry each.

#[cfg(feature = "alloc")]
use alloc::collections::BTreeMap;

use hashbrown::HashMap;
use unicode_general_category::{GeneralCategory, get_general_category};

/// A per-code-point tally of Unicode general categories.
///
/// Cheap to construct — one linear scan of the input. Access the
/// raw counts via [`Self::by_category`] or roll them up to
/// [`MajorCategory`] with [`Self::by_major`].
#[cfg(feature = "alloc")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Histogram {
    /// Total code points scanned. Equal to
    /// `text.chars().count()` for the input the histogram was built
    /// from.
    pub total: u64,
    // `GeneralCategory` isn't `Ord`, so a BTreeMap is out — use a
    // HashMap (hashbrown so `alloc`-only builds work).
    counts: HashMap<GeneralCategory, u64>,
}

/// The distinct `GeneralCategory` values that ASCII code points
/// can fall into. Order matters: `ASCII_CATEGORY_SLOT` indexes
/// into this array.
const ASCII_CATEGORY_SLOTS: &[GeneralCategory] = &[
    GeneralCategory::UppercaseLetter,
    GeneralCategory::LowercaseLetter,
    GeneralCategory::DecimalNumber,
    GeneralCategory::SpaceSeparator,
    GeneralCategory::Control,
    GeneralCategory::ConnectorPunctuation,
    GeneralCategory::DashPunctuation,
    GeneralCategory::OpenPunctuation,
    GeneralCategory::ClosePunctuation,
    GeneralCategory::OtherPunctuation,
    GeneralCategory::MathSymbol,
    GeneralCategory::CurrencySymbol,
    GeneralCategory::ModifierSymbol,
];

/// Per-byte lookup — for each ASCII byte, the index into
/// `ASCII_CATEGORY_SLOTS` giving its general category.
const ASCII_CATEGORY_SLOT: [u8; 128] = build_ascii_slot_table();

const fn build_ascii_slot_table() -> [u8; 128] {
    // Slot indices into ASCII_CATEGORY_SLOTS. Keep in sync!
    const UP: u8 = 0;
    const LO: u8 = 1;
    const ND: u8 = 2;
    const ZS: u8 = 3;
    const CC: u8 = 4;
    const PC: u8 = 5;
    const PD: u8 = 6;
    const PS: u8 = 7;
    const PE: u8 = 8;
    const PO: u8 = 9;
    const SM: u8 = 10;
    const SC: u8 = 11;
    const SK: u8 = 12;

    let mut t = [PO; 128];
    let mut b = 0u8;
    while b < 128 {
        t[b as usize] = if b <= 0x1F || b == 0x7F {
            CC
        } else if b >= b'0' && b <= b'9' {
            ND
        } else if b >= b'A' && b <= b'Z' {
            UP
        } else if b >= b'a' && b <= b'z' {
            LO
        } else if b == b' ' {
            ZS
        } else {
            // ASCII punctuation and symbols split by Unicode
            // sub-category. Table verbatim from UnicodeData.txt.
            match b {
                b'_' => PC,
                b'-' => PD,
                b'(' | b'[' | b'{' => PS,
                b')' | b']' | b'}' => PE,
                b'+' | b'<' | b'=' | b'>' | b'|' | b'~' => SM,
                b'$' => SC,
                b'^' | b'`' => SK,
                // Every remaining ASCII printable is a "other
                // punctuation" — the explicit list would be
                // `! " # % & ' * , . / : ; ? @ \` plus the
                // `_ = _` catch-all. Clippy's identical-arm
                // detector rejects listing them separately from
                // the wildcard, so use the wildcard alone.
                _ => PO,
            }
        };
        b += 1;
    }
    t
}

#[cfg(feature = "alloc")]
impl Histogram {
    /// Build a histogram by walking every code point in `text`.
    ///
    /// ## Implementation
    ///
    /// Bench-driven redesign (2026-08-09): hot path is a
    /// byte-oriented ASCII scan that accumulates counts into a
    /// small fixed-size array indexed by the ASCII category
    /// (Lu/Ll/Nd/Po/... — only the subset that appears in
    /// ASCII). Non-ASCII scalars fall back to
    /// `get_general_category` + `HashMap::entry`. At the end
    /// the ASCII array merges into the `HashMap`. Avoids per-
    /// byte hashing + probing on the common all-ASCII case.
    ///
    /// # Panics
    ///
    /// The internal `.chars().next().expect(...)` on the
    /// non-ASCII branch cannot fire — the branch is only
    /// reached when `bytes[i] >= 0x80`, guaranteeing
    /// `text[i..]` starts with a non-empty non-ASCII scalar in
    /// any valid `&str`.
    #[must_use]
    pub fn of(text: &str) -> Self {
        let mut h = Self::default();
        // Small fixed-size accumulator for ASCII categories.
        // Only the categories that appear in ASCII need slots —
        // enumerated below via `ASCII_CATEGORY_SLOTS`.
        let mut ascii_counts = [0u64; ASCII_CATEGORY_SLOTS.len()];

        let bytes = text.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            if b < 0x80 {
                let slot = ASCII_CATEGORY_SLOT[b as usize] as usize;
                ascii_counts[slot] += 1;
                h.total += 1;
                i += 1;
            } else {
                let c = text[i..]
                    .chars()
                    .next()
                    .expect("bytes[i] >= 0x80 guarantees a non-empty non-ASCII prefix");
                i += c.len_utf8();
                *h.counts.entry(get_general_category(c)).or_insert(0) += 1;
                h.total += 1;
            }
        }

        // Merge the ASCII slot totals into the main counts map.
        for (slot, &count) in ascii_counts.iter().enumerate() {
            if count > 0 {
                *h.counts.entry(ASCII_CATEGORY_SLOTS[slot]).or_insert(0) += count;
            }
        }
        h
    }

    /// The raw per-category counts. Categories with zero
    /// occurrences aren't present in the map.
    #[must_use]
    pub fn by_category(&self) -> &HashMap<GeneralCategory, u64> {
        &self.counts
    }

    /// Count for one category. Returns 0 when the category never
    /// appeared.
    #[must_use]
    pub fn count(&self, cat: GeneralCategory) -> u64 {
        self.counts.get(&cat).copied().unwrap_or(0)
    }

    /// Rollup to the seven Unicode major categories (Letter /
    /// Mark / Number / Punctuation / Symbol / Separator / Other).
    #[must_use]
    pub fn by_major(&self) -> BTreeMap<MajorCategory, u64> {
        let mut out: BTreeMap<MajorCategory, u64> = BTreeMap::new();
        for (&cat, &count) in &self.counts {
            *out.entry(MajorCategory::of(cat)).or_insert(0) += count;
        }
        out
    }
}

/// The seven Unicode major general categories, matching the
/// letter-prefix grouping (`L`, `M`, `N`, `P`, `S`, `Z`, `C`).
///
/// Useful when the 30-way tally is too granular — "is this a
/// letters-and-numbers identifier?" is easier to answer against
/// [`MajorCategory::Letter`] + [`MajorCategory::Number`] than
/// against the full 8-variant letter subset.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MajorCategory {
    /// `L` — every `Lu`/`Ll`/`Lt`/`Lm`/`Lo` variant.
    Letter,
    /// `M` — combining / spacing / enclosing marks.
    Mark,
    /// `N` — decimal / letter / other numbers.
    Number,
    /// `P` — every punctuation subcategory.
    Punctuation,
    /// `S` — math / currency / modifier / other symbols.
    Symbol,
    /// `Z` — space / line / paragraph separators.
    Separator,
    /// `C` — control / format / surrogate / private-use /
    /// unassigned.
    Other,
}

impl MajorCategory {
    /// The major category a general category falls under.
    #[must_use]
    pub fn of(cat: GeneralCategory) -> Self {
        match cat {
            GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter => Self::Letter,

            GeneralCategory::NonspacingMark
            | GeneralCategory::SpacingMark
            | GeneralCategory::EnclosingMark => Self::Mark,

            GeneralCategory::DecimalNumber
            | GeneralCategory::LetterNumber
            | GeneralCategory::OtherNumber => Self::Number,

            GeneralCategory::ConnectorPunctuation
            | GeneralCategory::DashPunctuation
            | GeneralCategory::OpenPunctuation
            | GeneralCategory::ClosePunctuation
            | GeneralCategory::InitialPunctuation
            | GeneralCategory::FinalPunctuation
            | GeneralCategory::OtherPunctuation => Self::Punctuation,

            GeneralCategory::MathSymbol
            | GeneralCategory::CurrencySymbol
            | GeneralCategory::ModifierSymbol
            | GeneralCategory::OtherSymbol => Self::Symbol,

            GeneralCategory::SpaceSeparator
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator => Self::Separator,

            // `GeneralCategory` is `#[non_exhaustive]`. Fold every
            // remaining variant (Control, Format, Surrogate,
            // PrivateUse, Unassigned, and anything new upstream)
            // into `Other` — matches the Unicode `C` major class's
            // catch-all role.
            _ => Self::Other,
        }
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn empty_string_has_zero_total() {
        let h = Histogram::of("");
        assert_eq!(h.total, 0);
        assert!(h.by_category().is_empty());
    }

    #[test]
    fn ascii_word_is_all_letters() {
        let h = Histogram::of("hello");
        assert_eq!(h.total, 5);
        assert_eq!(h.count(GeneralCategory::LowercaseLetter), 5);
        assert_eq!(*h.by_major().get(&MajorCategory::Letter).unwrap(), 5);
    }

    #[test]
    fn mixed_letters_split_case() {
        let h = Histogram::of("Hello");
        assert_eq!(h.count(GeneralCategory::UppercaseLetter), 1);
        assert_eq!(h.count(GeneralCategory::LowercaseLetter), 4);
        assert_eq!(*h.by_major().get(&MajorCategory::Letter).unwrap(), 5);
    }

    #[test]
    fn digits_and_punctuation_land_correctly() {
        let h = Histogram::of("Hi, 42!");
        // H, i are letters (2), space is separator (1),
        // 4, 2 are decimals (2), comma and ! are punctuation (2).
        assert_eq!(h.total, 7);
        let major = h.by_major();
        assert_eq!(*major.get(&MajorCategory::Letter).unwrap(), 2);
        assert_eq!(*major.get(&MajorCategory::Number).unwrap(), 2);
        assert_eq!(*major.get(&MajorCategory::Punctuation).unwrap(), 2);
        assert_eq!(*major.get(&MajorCategory::Separator).unwrap(), 1);
    }

    #[test]
    fn cjk_scalars_are_letters() {
        // 日 and 本 are Other_Letter (Lo). Kana / kanji all fall
        // under the letter major class.
        let h = Histogram::of("日本");
        assert_eq!(h.count(GeneralCategory::OtherLetter), 2);
        assert_eq!(*h.by_major().get(&MajorCategory::Letter).unwrap(), 2);
    }
}
