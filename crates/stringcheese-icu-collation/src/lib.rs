//! Collation capability for the StringCheese ICU-alternative
//! subsystem.
//!
//! Wraps the existing native [`stringcheese_collate::UcaCollator`]
//! (a `feruca`-backed CLDR-root Unicode Collation Algorithm
//! implementation) with locale-tailoring data from one or more
//! `stringcheese-scud` collation packs, and exposes the composed
//! surface through the `stringcheese:icu-collation@0.1.0` WIT
//! world. Callers construct a [`CollationEngine`] from a slice of
//! loaded [`CollationPack`]s and issue [`compare`](CollationEngine::compare) /
//! [`sort_key`](CollationEngine::sort_key) queries; the engine
//! walks the BCP 47 fallback chain (`de-DE → de → ""`) at query
//! time, applies the winning pack's expansion table to both
//! operands, then delegates to the UCA oracle for the actual
//! comparison at the requested strength.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 2 of the WIT-i18n design (`docs/design/wit-i18n.md` §
//! 8.2) — the second capability delivered on top of the shared
//! `stringcheese-scud` loader (Phase 1) and the existing
//! `stringcheese-collate` UCA implementation. Phase 2 is
//! specifically about *wrapping* the native collator behind a
//! SCUD-shaped data plane and a WIT-shaped API plane; it does
//! not reimplement UCA.
//!
//! # WIT surface
//!
//! The WIT file at `component/wit/collation/stringcheese-icu-collation.wit`
//! defines three exports on the `collation-world` world:
//!
//! * `compare(a, b, locale, strength)` — locale-sensitive
//!   ordering.
//! * `sort-key(text, locale, strength)` — bytewise-comparable
//!   sort key.
//! * `get-capabilities()` — introspection.
//!
//! A [`CollationEngine`] implements every export on the Rust
//! side; a future `wit-component`-gated `Guest` implementation
//! (matching the `stringcheese-tokenizer-component` pattern)
//! will bridge the two sides so this crate can ship as a
//! standalone WASM component.
//!
//! # Strength handling
//!
//! Phase 2 approximates the UCA strength levels by pre-folding
//! before delegating to the underlying `feruca::Collator`
//! (via [`stringcheese_collate::UcaCollator`]) which itself
//! walks primary → tertiary weights with a byte tiebreak:
//!
//! * **Primary** — pack-normalize both sides, then strip
//!   ASCII case and combining marks before comparing. Two
//!   strings that differ only in case or diacritic mark
//!   compare equal.
//! * **Secondary** — pack-normalize, ASCII-casefold, then
//!   compare. Two strings that differ only in case compare
//!   equal; diacritics are significant.
//! * **Tertiary** — pack-normalize only, then compare. Case
//!   and diacritics are both significant.
//! * **Quaternary** — same as tertiary in Phase 2 (feruca's
//!   default shifted mode is already
//!   quaternary-aware for variable-weight punctuation).
//! * **Identical** — tertiary compare with a full-codepoint
//!   tiebreak on equal.
//!
//! # Phase 2 deferrals
//!
//! * **Standalone WASM component build.** The WIT interface is
//!   in place and parses cleanly under `wit-parser` (see the
//!   smoke test in `tests/wit_parse.rs`); the `wit-bindgen`
//!   `Guest` implementation and the `cargo build --target
//!   wasm32-wasip1 --features wit-component` recipe land in a
//!   follow-up wave.
//! * **Full ~200 000-entry `CollationTest.txt` conformance.**
//!   The design commits to a subset in Phase 2; the follow-up
//!   wave runs the whole file against the engine.
//! * **Cross-locale composition.** Phase 2 loads one primary
//!   pack per query; the engine already walks the pack list in
//!   fallback order, but the "load en + de and switch
//!   per-string" story is a Phase 3 concern.
//!
//! # Trust model
//!
//! Inherited from `stringcheese-scud`: SCUD packs are trusted
//! input. This crate does not defend against maliciously
//! crafted packs.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::cmp::Ordering;

use stringcheese_scud::{CollationDataView, ScudFile};

// Re-export the SCUD-side error surface so downstream language
// packs depending on `stringcheese-icu-collation` do not have to
// add a direct `stringcheese-scud` dependency just to name the
// error type. Matches the shape used by `stringcheese-icu-case`.
pub use stringcheese_scud::ScudError;

/// A loaded collation pack for one BCP 47 locale.
///
/// Wraps a validated [`ScudFile`] whose capability tag is
/// [`stringcheese_scud::CAP_COLLATION`]. Cheap to clone — the
/// underlying SCUD bytes are borrowed by the [`ScudFile`], and
/// this wrapper carries only the parsed header plus the locale
/// tag pulled from it.
#[derive(Debug, Clone, Copy)]
pub struct CollationPack<'a> {
    scud: ScudFile<'a>,
    locale: &'a str,
    data: CollationDataView<'a>,
}

impl<'a> CollationPack<'a> {
    /// Wrap a validated [`ScudFile`] as a collation pack.
    ///
    /// Returns an error if the SCUD file's capability tag is not
    /// `CAP_COLLATION` or if the file's body cannot be parsed as
    /// collation data.
    pub fn new(scud: ScudFile<'a>) -> Result<Self, ScudError> {
        let data = scud.as_collation_data()?;
        let locale = scud.locale().unwrap_or("");
        Ok(Self { scud, locale, data })
    }

    /// Parse `bytes` as a SCUD file and wrap it as a collation
    /// pack.
    ///
    /// Convenience constructor for pack crates that embed their
    /// SCUD blob as an `include_bytes!` constant.
    pub fn from_scud_bytes(bytes: &'a [u8]) -> Result<Self, ScudError> {
        let scud = ScudFile::from_slice(bytes)?;
        Self::new(scud)
    }

    /// The BCP 47 locale tag associated with this pack.
    #[must_use]
    pub fn locale(&self) -> &'a str {
        self.locale
    }

    /// The CLDR version the pack was generated from.
    #[must_use]
    pub fn cldr_version(&self) -> &'a str {
        self.scud.cldr_version()
    }

    /// Total byte length of the underlying SCUD file.
    #[must_use]
    pub fn scud_bytes_len(&self) -> usize {
        self.scud.len()
    }

    /// The zero-copy collation data view.
    #[must_use]
    pub fn data(&self) -> &CollationDataView<'a> {
        &self.data
    }
}

/// UCA collation strength. Mirrors the WIT `collation-strength`
/// enum.
///
/// See UTS #10 § 1.1 for the primary/secondary/tertiary/
/// quaternary/identical breakdown.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CollationStrength {
    /// Base characters only. `a = A = á = ä`.
    Primary,
    /// Base characters + diacritics. `a = A`, `a < á`.
    Secondary,
    /// Base characters + diacritics + case. `a < A < á`.
    #[default]
    Tertiary,
    /// Adds variable-weight (punctuation) awareness.
    Quaternary,
    /// Full code-point tiebreak on top of tertiary.
    Identical,
}

impl CollationStrength {
    /// Round-trip a `u8` strength back into the typed enum.
    ///
    /// Uses the wire encoding shared with SCUD's
    /// [`stringcheese_scud::SECT_COLLATION_OPTIONS`] section.
    #[must_use]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Primary),
            1 => Some(Self::Secondary),
            2 => Some(Self::Tertiary),
            3 => Some(Self::Quaternary),
            4 => Some(Self::Identical),
            _ => None,
        }
    }

    /// The wire-encoded `u8` value for this strength.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
            Self::Tertiary => 2,
            Self::Quaternary => 3,
            Self::Identical => 4,
        }
    }
}

/// Typed failure modes of the collation engine. Mirrors the WIT
/// `collation-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollationError {
    /// The locale tag was not a well-formed BCP 47 tag.
    InvalidLocale(&'static str),
    /// No pack was loaded for the requested locale.
    LocaleUnavailable(&'static str),
    /// The requested strength is not implemented.
    UnsupportedStrength(&'static str),
}

/// Locale-sensitive collation engine.
///
/// Holds a list of [`CollationPack`]s and consults them at query
/// time in BCP 47 fallback order. A caller who wants German and
/// French constructs a [`CollationEngine`] from `[german_pack,
/// french_pack]`; queries under `de-DE` walk `de-DE → de → ""`,
/// queries under `fr-CA` walk `fr-CA → fr → ""`.
///
/// The engine is `Send + Sync` and cheap to store in a `static`
/// slot so long as the underlying SCUD bytes are `'static`.
#[cfg(feature = "alloc")]
pub struct CollationEngine<'a> {
    packs: Vec<CollationPack<'a>>,
    // `stringcheese_collate::UcaCollator` wraps a `feruca::Collator`
    // in a `RefCell` because feruca's `collate` takes `&mut self`
    // for its internal buffers. We reuse that wrapper directly so
    // the collator caches its work across queries.
    uca: stringcheese_collate::UcaCollator,
}

#[cfg(feature = "alloc")]
impl core::fmt::Debug for CollationEngine<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CollationEngine")
            .field("pack_count", &self.packs.len())
            .finish()
    }
}

#[cfg(feature = "alloc")]
impl<'a> CollationEngine<'a> {
    /// Construct a fresh engine backed by the given packs.
    ///
    /// Pack order is *not* significant — the engine indexes by
    /// BCP 47 tag at query time. A later pack with the same tag
    /// as an earlier one overrides it.
    #[must_use]
    pub fn new(packs: Vec<CollationPack<'a>>) -> Self {
        Self {
            packs,
            uca: stringcheese_collate::UcaCollator::new(),
        }
    }

    /// Every BCP 47 locale tag this engine knows about.
    #[must_use]
    pub fn supported_locales(&self) -> Vec<&'a str> {
        self.packs.iter().map(CollationPack::locale).collect()
    }

    /// True iff a query in the given locale would use a pack
    /// (rather than falling through to root DUCET).
    #[must_use]
    pub fn supports(&self, locale: &str) -> bool {
        walk_fallback_chain(locale).any(|tag| self.pack_for(tag).is_some())
    }

    /// Compare `a` and `b` under the given locale and strength.
    ///
    /// Walks the CLDR fallback chain: the pack with the most
    /// specific tag matching `locale` wins, then progressively
    /// less-specific ancestors, then no pack (root DUCET only).
    #[must_use]
    pub fn compare(&self, a: &str, b: &str, locale: &str, strength: CollationStrength) -> Ordering {
        let expanded_a = self.normalize_for_strength(a, locale, strength);
        let expanded_b = self.normalize_for_strength(b, locale, strength);
        let ord = <stringcheese_collate::UcaCollator as stringcheese_collate::Collator>::compare(
            &self.uca,
            &expanded_a,
            &expanded_b,
        );
        if matches!(strength, CollationStrength::Identical) && ord == Ordering::Equal {
            return a.cmp(b);
        }
        ord
    }

    /// Produce a bytewise-comparable sort key for `text` under the
    /// given locale and strength.
    ///
    /// The returned key encodes the pack-normalized string with a
    /// per-strength folding, followed (at Tertiary and above) by
    /// a level-3 tail carrying the case information. Two keys
    /// `ka` and `kb` compare under `[u8]::cmp` in the same
    /// order the two input strings compare through
    /// [`compare`](Self::compare) *at the same strength*, up to
    /// the ordering imposed by the underlying UCA table.
    ///
    /// # Encoding
    ///
    /// ```text
    ///    strength-byte | 0x00 |
    ///    level-1 (case-folded, diacritic-stripped) |
    ///    0x02 | level-2 (case-folded, with diacritics)? |
    ///    0x02 | level-3 (case-marker bits)?           |
    ///    0x00 | raw input (Identical only)?
    /// ```
    ///
    /// Level-2 and level-3 tails only appear at Secondary and
    /// Tertiary+ strengths respectively; the level-3 tail flips
    /// ASCII casing so a bytewise compare of two Tertiary keys
    /// agrees with CLDR-root tertiary weights (lowercase sorts
    /// before uppercase for the same base letter).
    #[must_use]
    pub fn sort_key(&self, text: &str, locale: &str, strength: CollationStrength) -> Vec<u8> {
        // Expand once — every level derives from the same
        // pack-normalized form.
        let mut expanded = String::with_capacity(text.len());
        for c in text.chars() {
            self.expand_char(c, locale, &mut expanded);
        }
        // Level-1: strip case + combining marks (matches Primary
        // compare).
        let level1 = primary_fold(&expanded);
        let mut out = Vec::with_capacity(expanded.len() * 2 + 8);
        out.push(strength.as_u8());
        out.push(0);
        out.extend_from_slice(level1.as_bytes());
        // Level-2 (case-folded, diacritics preserved).
        if !matches!(strength, CollationStrength::Primary) {
            out.push(0x02);
            let level2 = ascii_casefold(&expanded);
            out.extend_from_slice(level2.as_bytes());
        }
        // Level-3 (case marker). Tertiary+ needs to encode "which
        // characters were uppercase in the original" so bytewise
        // compare matches CLDR-root tertiary (lowercase before
        // uppercase for the same base letter).
        if matches!(
            strength,
            CollationStrength::Tertiary
                | CollationStrength::Quaternary
                | CollationStrength::Identical
        ) {
            out.push(0x02);
            for c in expanded.chars() {
                if c.is_ascii_uppercase() {
                    out.push(0x02); // uppercase marker
                } else if c.is_ascii_lowercase() {
                    out.push(0x01); // lowercase marker
                } else {
                    out.push(0x01); // non-letter treated as low
                }
            }
        }
        if matches!(strength, CollationStrength::Identical) {
            // Append the raw input so bit-for-bit differences
            // survive the sort_key comparison.
            out.push(0);
            out.extend_from_slice(text.as_bytes());
        }
        out
    }

    /// The pack that would service a query under `locale`, if any.
    #[must_use]
    pub fn active_pack(&self, locale: &str) -> Option<&CollationPack<'a>> {
        walk_fallback_chain(locale).find_map(|tag| self.pack_for(tag))
    }

    /// Look up the first-matching pack for a locale tag.
    fn pack_for(&self, tag: &str) -> Option<&CollationPack<'a>> {
        self.packs
            .iter()
            .find(|p| p.locale.eq_ignore_ascii_case(tag))
    }

    /// Apply the winning pack's expansion table plus strength-
    /// dependent case / diacritic folding to `text`.
    fn normalize_for_strength(
        &self,
        text: &str,
        locale: &str,
        strength: CollationStrength,
    ) -> String {
        let mut expanded = String::with_capacity(text.len());
        for c in text.chars() {
            self.expand_char(c, locale, &mut expanded);
        }
        match strength {
            CollationStrength::Primary => primary_fold(&expanded),
            CollationStrength::Secondary => ascii_casefold(&expanded),
            CollationStrength::Tertiary
            | CollationStrength::Quaternary
            | CollationStrength::Identical => expanded,
        }
    }

    /// Push the pack-expanded form of `c` into `out`.
    fn expand_char(&self, c: char, locale: &str, out: &mut String) {
        let src = c as u32;
        for pack in walk_fallback_chain(locale).filter_map(|tag| self.pack_for(tag)) {
            if let Some(mapping) = pack.data.expansion(src) {
                for ch in mapping.chars() {
                    out.push(ch);
                }
                return;
            }
        }
        out.push(c);
    }
}

/// Strip ASCII case and combining marks — the Phase 2 primary
/// approximation. Non-ASCII base letters are preserved; combining
/// marks (U+0300..U+036F, U+1AB0..U+1AFF, U+1DC0..U+1DFF,
/// U+20D0..U+20FF, U+FE20..U+FE2F) are dropped.
#[cfg(feature = "alloc")]
fn primary_fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_combining_mark(c) {
            continue;
        }
        // Casefold ASCII; leave everything else alone. Phase 2
        // does not decompose precomposed accented characters (that
        // needs `unicode-normalization` which we deliberately do
        // not depend on at this layer); the pack's expansion
        // table handles the DE `ä → ae` case, and root DUCET
        // handles most others.
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(feature = "alloc")]
fn ascii_casefold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// True iff `c` sits in one of the Unicode combining-mark
/// ranges. Kept as a small hand-written table rather than
/// depending on `unicode-normalization` at this layer — the
/// ranges are stable across the Unicode versions the WIT-i18n
/// design supports.
#[cfg(feature = "alloc")]
fn is_combining_mark(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        0x0300..=0x036F   // Combining Diacritical Marks
            | 0x1AB0..=0x1AFF  // Combining Diacritical Marks Extended
            | 0x1DC0..=0x1DFF  // Combining Diacritical Marks Supplement
            | 0x20D0..=0x20FF  // Combining Diacritical Marks for Symbols
            | 0xFE20..=0xFE2F  // Combining Half Marks
    )
}

/// Walk the CLDR-defined fallback chain for a BCP 47 tag.
///
/// The chain strips subtags one at a time from the right,
/// terminating with the empty string (root). Examples:
///
/// * `pt-BR` → `pt-BR`, `pt`, `""`
/// * `de-DE` → `de-DE`, `de`, `""`
/// * `en` → `en`, `""`
/// * `""` → `""`
pub fn walk_fallback_chain(locale: &str) -> impl Iterator<Item = &str> {
    let mut current = Some(locale);
    let mut emitted_root = false;
    core::iter::from_fn(move || {
        if let Some(tag) = current.take() {
            if tag.is_empty() {
                if emitted_root {
                    None
                } else {
                    emitted_root = true;
                    Some("")
                }
            } else {
                let next = tag.rfind('-').map_or("", |idx| &tag[..idx]);
                current = Some(next);
                Some(tag)
            }
        } else {
            None
        }
    })
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;
    use stringcheese_scud::{
        CAP_COLLATION, CollationSectionBuilder, SECT_COLLATION_OPTIONS, SECT_EXPANSIONS, ScudFile,
        ScudWriter,
    };

    fn build_test_de_phonebook() -> alloc::vec::Vec<u8> {
        let mut c = CollationSectionBuilder::new();
        c.push_expansion(0x00DF, &[0x0073, 0x0073]); // ß → ss
        c.push_expansion(0x00E4, &[0x0061, 0x0065]); // ä → ae
        c.push_expansion(0x00C4, &[0x0041, 0x0045]); // Ä → AE
        c.push_expansion(0x00F6, &[0x006F, 0x0065]); // ö → oe
        c.push_expansion(0x00D6, &[0x004F, 0x0045]); // Ö → OE
        c.push_expansion(0x00FC, &[0x0075, 0x0065]); // ü → ue
        c.push_expansion(0x00DC, &[0x0055, 0x0045]); // Ü → UE
        c.set_default_strength(CollationStrength::Tertiary.as_u8());
        c.set_case_insensitive(true);
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("de"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        w.finish()
    }

    fn build_test_en_root() -> alloc::vec::Vec<u8> {
        // English uses DUCET-root behaviour — no character
        // expansions. The pack still ships so the fallback chain
        // returns "the en pack was used" for introspection.
        let c = CollationSectionBuilder::new();
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("en"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.finish()
    }

    #[test]
    fn engine_reports_supported_locales() {
        let en = build_test_en_root();
        let de = build_test_de_phonebook();
        let en_pack = CollationPack::from_scud_bytes(&en).unwrap();
        let de_pack = CollationPack::from_scud_bytes(&de).unwrap();
        let engine = CollationEngine::new(vec![en_pack, de_pack]);
        assert_eq!(engine.supported_locales(), vec!["en", "de"]);
        assert!(engine.supports("en"));
        assert!(engine.supports("de-DE"));
        assert!(!engine.supports("xx"));
    }

    #[test]
    fn de_phonebook_expands_umlauts() {
        let de = build_test_de_phonebook();
        let de_pack = CollationPack::from_scud_bytes(&de).unwrap();
        let engine = CollationEngine::new(vec![de_pack]);
        // Bär → Baer under DE phonebook; Baer sorts before Bar
        // ('e' < 'r' at position 2).
        assert_eq!(
            engine.compare("Bär", "Baer", "de", CollationStrength::Tertiary),
            Ordering::Equal,
        );
        assert_eq!(
            engine.compare("Bär", "Bar", "de", CollationStrength::Tertiary),
            Ordering::Less,
        );
    }

    #[test]
    fn de_phonebook_expands_sharp_s() {
        let de = build_test_de_phonebook();
        let de_pack = CollationPack::from_scud_bytes(&de).unwrap();
        let engine = CollationEngine::new(vec![de_pack]);
        assert_eq!(
            engine.compare("Straße", "Strasse", "de", CollationStrength::Tertiary),
            Ordering::Equal,
        );
    }

    #[test]
    fn primary_strength_ignores_case_and_diacritics() {
        let en = build_test_en_root();
        let en_pack = CollationPack::from_scud_bytes(&en).unwrap();
        let engine = CollationEngine::new(vec![en_pack]);
        // Under primary, case is folded and simple ASCII differences
        // survive.
        assert_eq!(
            engine.compare("apple", "APPLE", "en", CollationStrength::Primary),
            Ordering::Equal,
        );
        // Under tertiary, case matters — feruca sorts lowercase
        // before uppercase in DUCET tertiary weights.
        let ord_tert = engine.compare("apple", "APPLE", "en", CollationStrength::Tertiary);
        assert_ne!(ord_tert, Ordering::Equal);
    }

    #[test]
    fn secondary_ignores_case_but_keeps_diacritics() {
        let en = build_test_en_root();
        let en_pack = CollationPack::from_scud_bytes(&en).unwrap();
        let engine = CollationEngine::new(vec![en_pack]);
        assert_eq!(
            engine.compare("cafe", "CAFE", "en", CollationStrength::Secondary),
            Ordering::Equal,
        );
    }

    #[test]
    fn sort_key_compares_consistently_with_compare() {
        let de = build_test_de_phonebook();
        let de_pack = CollationPack::from_scud_bytes(&de).unwrap();
        let engine = CollationEngine::new(vec![de_pack]);
        let strength = CollationStrength::Tertiary;
        for (a, b) in [
            ("Bär", "Baer"),
            ("Straße", "Strasse"),
            ("Muller", "Munk"),
            ("apple", "banana"),
        ] {
            let ka = engine.sort_key(a, "de", strength);
            let kb = engine.sort_key(b, "de", strength);
            let key_ord = ka.cmp(&kb);
            let compare_ord = engine.compare(a, b, "de", strength);
            assert_eq!(
                key_ord, compare_ord,
                "sort_key vs compare disagreed for ({a:?}, {b:?})"
            );
        }
    }

    #[test]
    fn identical_strength_survives_ties() {
        let en = build_test_en_root();
        let en_pack = CollationPack::from_scud_bytes(&en).unwrap();
        let engine = CollationEngine::new(vec![en_pack]);
        // "abc" and "abc" are equal at identical.
        assert_eq!(
            engine.compare("abc", "abc", "en", CollationStrength::Identical),
            Ordering::Equal,
        );
        // At tertiary, "cafe" and "CAFE" may compare unequal — but
        // at identical they still bytewise-differ.
        let ord = engine.compare("cafe", "CAFE", "en", CollationStrength::Identical);
        assert_ne!(ord, Ordering::Equal);
    }

    #[test]
    fn fallback_chain_walks_correctly() {
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("de-DE").collect();
        assert_eq!(chain, ["de-DE", "de", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("zh-Hant-HK").collect();
        assert_eq!(chain, ["zh-Hant-HK", "zh-Hant", "zh", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("en").collect();
        assert_eq!(chain, ["en", ""]);
    }

    #[test]
    fn strength_round_trips_through_u8() {
        for s in [
            CollationStrength::Primary,
            CollationStrength::Secondary,
            CollationStrength::Tertiary,
            CollationStrength::Quaternary,
            CollationStrength::Identical,
        ] {
            assert_eq!(CollationStrength::from_u8(s.as_u8()), Some(s));
        }
        assert_eq!(CollationStrength::from_u8(99), None);
    }

    #[test]
    fn active_pack_returns_de_for_de_ch() {
        let de = build_test_de_phonebook();
        let de_pack = CollationPack::from_scud_bytes(&de).unwrap();
        let engine = CollationEngine::new(vec![de_pack]);
        let pack = engine.active_pack("de-CH").expect("de-CH falls back to de");
        assert_eq!(pack.locale(), "de");
    }

    #[test]
    fn active_pack_returns_none_for_unknown_locale() {
        let en = build_test_en_root();
        let en_pack = CollationPack::from_scud_bytes(&en).unwrap();
        let engine = CollationEngine::new(vec![en_pack]);
        assert!(engine.active_pack("zz-ZZ").is_none());
    }

    #[test]
    fn scud_file_wrongly_typed_rejected() {
        let w = ScudWriter::new(*b"CASE", "44.1", Some("en"));
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(CollationPack::new(file).is_err());
    }

    #[test]
    fn empty_input_produces_short_sort_key() {
        let en = build_test_en_root();
        let en_pack = CollationPack::from_scud_bytes(&en).unwrap();
        let engine = CollationEngine::new(vec![en_pack]);
        // Tertiary key on empty input: strength + sep + (empty
        // level1) + level2-sep + (empty level2) + level3-sep +
        // (empty level3) = 5 bytes.
        let key = engine.sort_key("", "en", CollationStrength::Tertiary);
        assert_eq!(key[0], CollationStrength::Tertiary.as_u8());
        assert!(key.len() >= 2);
        // Primary key on empty: strength + sep + (empty) = 2
        // bytes.
        let key = engine.sort_key("", "en", CollationStrength::Primary);
        assert_eq!(key.len(), 2);
    }

    #[test]
    fn is_combining_mark_recognises_common_ranges() {
        assert!(is_combining_mark('\u{0301}')); // Combining acute
        assert!(is_combining_mark('\u{0308}')); // Combining diaeresis
        assert!(is_combining_mark('\u{1DC0}')); // Combining dotted grave
        assert!(!is_combining_mark('a'));
        assert!(!is_combining_mark('ä'));
    }
}
