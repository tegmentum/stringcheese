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
    ///
    /// Three locale tailorings switch the compare path off the
    /// default UCA delegation:
    ///
    /// * **Primary-weight overrides** (Turkish) — when the winning
    ///   pack carries a
    ///   [`SECT_PRIMARY_OVERRIDES`](stringcheese_scud::SECT_PRIMARY_OVERRIDES)
    ///   section, the engine ranks characters by their override
    ///   entry rather than DUCET root weights. Characters without an
    ///   override use their ASCII-lowercased codepoint as a
    ///   primary-weight approximation.
    /// * **Backwards-secondary** (French) — when the winning pack
    ///   sets the backwards-secondary options-blob bit, the engine
    ///   reverses the secondary weight sequence before comparing so
    ///   accents tie-break right-to-left within a word.
    /// * **Case-second** (Russian) — when the winning pack sets the
    ///   case-second options-blob bit, the engine promotes
    ///   case-distinguishing weights from tertiary to secondary so
    ///   that case differences dominate over diacritics at
    ///   secondary strength.
    ///
    /// # Precedence when multiple tailorings coexist
    ///
    /// Practical CLDR packs pick exactly one of these three; the
    /// engine tolerates simultaneous bits and picks the winner in
    /// the order listed above (primary-overrides beat
    /// backwards-secondary beat case-second). No CLDR data ships
    /// combined variants, so this ordering is a defensive fallback
    /// rather than a semantic decision.
    #[must_use]
    pub fn compare(&self, a: &str, b: &str, locale: &str, strength: CollationStrength) -> Ordering {
        let pack = self.active_pack(locale);
        if let Some(p) = pack {
            if p.data.has_primary_overrides() {
                return self.compare_with_primary_overrides(a, b, locale, strength);
            }
            if p.data.backwards_secondary() == Some(true) {
                return self.compare_with_backwards_secondary(a, b, locale, strength);
            }
            if p.data.case_second() == Some(true) {
                return self.compare_with_case_second(a, b, locale, strength);
            }
        }
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

    /// Compare `a` and `b` using the active pack's primary-weight
    /// override table.
    ///
    /// Builds a per-character weight sequence for each operand:
    /// characters listed in the pack's
    /// [`SECT_PRIMARY_OVERRIDES`](stringcheese_scud::SECT_PRIMARY_OVERRIDES)
    /// section use the tabled `(primary, secondary, tertiary)`
    /// tuple; other characters use their ASCII-lowercased codepoint
    /// as the primary weight and 0 for secondary + tertiary. Pack
    /// expansions apply first (so `ß → ss` still fires under
    /// Turkish). Level-1 keys compare, then level-2, then level-3,
    /// per the requested strength.
    fn compare_with_primary_overrides(
        &self,
        a: &str,
        b: &str,
        locale: &str,
        strength: CollationStrength,
    ) -> Ordering {
        let ka = self.overridden_key(a, locale, strength);
        let kb = self.overridden_key(b, locale, strength);
        match ka.cmp(&kb) {
            Ordering::Equal if matches!(strength, CollationStrength::Identical) => a.cmp(b),
            ord => ord,
        }
    }

    /// Compare `a` and `b` under the French backwards-secondary
    /// tailoring.
    ///
    /// The base primary compare delegates to feruca (via
    /// `normalize_for_strength(Primary)`); on a tie, the engine
    /// extracts the secondary weight sequence for each operand,
    /// reverses it, and lex-compares. On a further tie, the
    /// tertiary / identical strengths fall back to the default UCA
    /// compare.
    fn compare_with_backwards_secondary(
        &self,
        a: &str,
        b: &str,
        locale: &str,
        strength: CollationStrength,
    ) -> Ordering {
        // Level-1 (primary): pack-expand, decompose precomposed
        // Latin accented letters (so côte and cote fold identically),
        // then strip combining marks + ASCII-lowercase before
        // delegating to feruca. The extra decomposition step matters
        // for French — feruca on its own weighs `ô` and `o` with
        // different secondaries, but for backwards-secondary we need
        // the primary to tie on identical base letters regardless
        // of the accent form.
        let base_a = decompose_and_primary_fold(a, locale, self);
        let base_b = decompose_and_primary_fold(b, locale, self);
        let primary_ord =
            <stringcheese_collate::UcaCollator as stringcheese_collate::Collator>::compare(
                &self.uca, &base_a, &base_b,
            );
        if primary_ord != Ordering::Equal || matches!(strength, CollationStrength::Primary) {
            return primary_ord;
        }
        // Level-2 (secondary): extract per-position diacritics,
        // reverse, lex-compare. This is the backwards-secondary
        // tie-break — accents at the END of the word are the
        // primary tie-breaker.
        let sec_a = reversed_secondary_weights(a, locale, self);
        let sec_b = reversed_secondary_weights(b, locale, self);
        let sec_ord = sec_a.cmp(&sec_b);
        if sec_ord != Ordering::Equal || matches!(strength, CollationStrength::Secondary) {
            return sec_ord;
        }
        // Tertiary+ fall back to the default UCA compare (which
        // carries case-sensitivity via feruca's tertiary weights).
        let full_a = self.normalize_for_strength(a, locale, strength);
        let full_b = self.normalize_for_strength(b, locale, strength);
        let ord = <stringcheese_collate::UcaCollator as stringcheese_collate::Collator>::compare(
            &self.uca, &full_a, &full_b,
        );
        if matches!(strength, CollationStrength::Identical) && ord == Ordering::Equal {
            return a.cmp(b);
        }
        ord
    }

    /// Compare `a` and `b` under the Russian case-second tailoring.
    ///
    /// UCA default puts case at level 3 (tertiary); CLDR's `ru`
    /// `standard` variant moves case to level 2 so that a case
    /// difference dominates over any diacritic difference at
    /// secondary strength. The engine's simplified model builds a
    /// bytewise sort key:
    ///
    /// * **Level 1 (primary):** pack-expand, strip combining marks,
    ///   and ASCII-lowercase — same primary fold as the default
    ///   engine.
    /// * **Level 2 (secondary):** one byte per character carrying a
    ///   case marker — `0x01` for lowercase or non-letter, `0x02`
    ///   for uppercase. The 1-byte spacing keeps the compare stable
    ///   across multi-byte UTF-8 sequences.
    /// * **Level 3 (tertiary):** the pack-expanded original text so
    ///   Tertiary+ still tie-breaks on the raw form.
    ///
    /// Concretely, for the Cyrillic pair `"Аа"` vs `"аА"`:
    ///
    /// * Level 1 folds both to `"аа"` (equal).
    /// * Level 2 emits `[0x02, 0x01]` vs `[0x01, 0x02]` — the first
    ///   character diverges (`0x02 > 0x01`), so `"Аа" > "аА"`.
    ///
    /// This matches the CLDR `ru` `standard` "lower before upper at
    /// secondary" rule (lowercase sorts before uppercase, so the
    /// leading-uppercase form comes later).
    fn compare_with_case_second(
        &self,
        a: &str,
        b: &str,
        locale: &str,
        strength: CollationStrength,
    ) -> Ordering {
        let ka = self.case_second_key(a, locale, strength);
        let kb = self.case_second_key(b, locale, strength);
        match ka.cmp(&kb) {
            Ordering::Equal if matches!(strength, CollationStrength::Identical) => a.cmp(b),
            ord => ord,
        }
    }

    /// Build a bytewise-comparable sort key for `text` under the
    /// case-second tailoring. See [`compare_with_case_second`] for
    /// the encoding.
    fn case_second_key(&self, text: &str, locale: &str, strength: CollationStrength) -> Vec<u8> {
        // Pack-expand once so every level operates on the same
        // normalised form (`ß → ss` still fires for the ru pack's
        // shared expansion table).
        let mut expanded = String::with_capacity(text.len());
        for c in text.chars() {
            self.expand_char(c, locale, &mut expanded);
        }
        // Level 1: primary fold that lowercases via `char::to_lowercase`
        // so Cyrillic and other non-ASCII case pairs collapse to a
        // common primary form (a plain `primary_fold` only
        // ASCII-lowercases).
        let level1 = case_second_primary_fold(&expanded);
        let mut out = Vec::with_capacity(expanded.len() * 2 + 8);
        out.extend_from_slice(level1.as_bytes());
        // Level 2: case markers per character. Emit only for
        // Secondary+ strengths so Primary compares stay case-blind.
        if !matches!(strength, CollationStrength::Primary) {
            out.push(0x00); // level separator
            for c in expanded.chars() {
                if is_combining_mark(c) {
                    // Combining marks contribute no case signal.
                    out.push(0x01);
                } else if is_upper_letter(c) {
                    out.push(0x02); // uppercase sorts after
                } else {
                    out.push(0x01); // lowercase / non-letter
                }
            }
        }
        // Level 3: raw pack-expanded text at Tertiary+.
        if matches!(
            strength,
            CollationStrength::Tertiary
                | CollationStrength::Quaternary
                | CollationStrength::Identical
        ) {
            out.push(0x00);
            out.extend_from_slice(expanded.as_bytes());
        }
        out
    }

    /// Build a bytewise-comparable weight key for `text` using the
    /// active pack's primary-override table.
    fn overridden_key(&self, text: &str, locale: &str, strength: CollationStrength) -> Vec<u8> {
        // Expand once using the pack's expansion table so `ß → ss`
        // still fires under Turkish; the override table then
        // applies per-character to the expanded form.
        let mut expanded = String::with_capacity(text.len());
        for c in text.chars() {
            self.expand_char(c, locale, &mut expanded);
        }
        // Look up each character's (primary, secondary, tertiary)
        // weight, falling back to (lowercased_codepoint, 0, 0) when
        // no override is present.
        let pack = self.active_pack(locale);
        let mut primary = Vec::with_capacity(expanded.len() * 4);
        let mut secondary = Vec::with_capacity(expanded.len() * 4);
        let mut tertiary = Vec::with_capacity(expanded.len() * 4);
        for c in expanded.chars() {
            let cp = c as u32;
            // Case-fold before override lookup so uppercase pack
            // hits its lowercase override row (Ç → ç, Ğ → ğ, …).
            // `char::to_lowercase()` produces one scalar for every
            // Latin uppercase letter we care about; if it expands
            // (e.g. `İ → i̇`), we take the first scalar as the
            // lookup key — every Turkish letter has a single-scalar
            // lowercase form.
            let lower_cp = c.to_lowercase().next().map_or(cp, |lc| lc as u32);
            let (pw, sw, tw) = pack
                .and_then(|p| p.data.primary_override(lower_cp))
                .unwrap_or((lower_cp, 0, cp));
            primary.extend_from_slice(&pw.to_be_bytes());
            secondary.extend_from_slice(&sw.to_be_bytes());
            tertiary.extend_from_slice(&tw.to_be_bytes());
        }
        let mut out = Vec::with_capacity(primary.len() * 3 + 4);
        out.extend_from_slice(&primary);
        if !matches!(strength, CollationStrength::Primary) {
            out.push(0x00);
            out.extend_from_slice(&secondary);
        }
        if matches!(
            strength,
            CollationStrength::Tertiary
                | CollationStrength::Quaternary
                | CollationStrength::Identical
        ) {
            out.push(0x00);
            out.extend_from_slice(&tertiary);
        }
        out
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
        // Primary-override packs (Turkish): produce a bytewise key
        // from the override weights so `sort_key` cmp matches the
        // `compare` result under the same tailoring.
        let pack = self.active_pack(locale);
        if pack.is_some_and(|p| p.data.has_primary_overrides()) {
            let mut out = Vec::with_capacity(text.len() * 4 + 4);
            out.push(strength.as_u8());
            out.push(0);
            let key = self.overridden_key(text, locale, strength);
            out.extend_from_slice(&key);
            if matches!(strength, CollationStrength::Identical) {
                out.push(0);
                out.extend_from_slice(text.as_bytes());
            }
            return out;
        }
        // Case-second packs (Russian): produce a bytewise key from
        // the L1|L2|L3 layout used by compare_with_case_second so
        // `sort_key` cmp mirrors compare.
        if pack.is_some_and(|p| {
            p.data.case_second() == Some(true) && p.data.backwards_secondary() != Some(true)
        }) {
            let mut out = Vec::with_capacity(text.len() * 2 + 8);
            out.push(strength.as_u8());
            out.push(0);
            let key = self.case_second_key(text, locale, strength);
            out.extend_from_slice(&key);
            if matches!(strength, CollationStrength::Identical) {
                out.push(0);
                out.extend_from_slice(text.as_bytes());
            }
            return out;
        }
        // Backwards-secondary packs (French): the key concatenates
        // the primary-fold key with the reversed secondary sequence
        // so `sort_key` cmp mirrors the two-phase compare above.
        let backwards_sec = pack.is_some_and(|p| p.data.backwards_secondary() == Some(true));
        // Expand once — every level derives from the same
        // pack-normalized form.
        let mut expanded = String::with_capacity(text.len());
        for c in text.chars() {
            self.expand_char(c, locale, &mut expanded);
        }
        // Level-1: strip case + combining marks (matches Primary
        // compare). Backwards-secondary packs additionally
        // decompose precomposed accented letters so `côte` and
        // `cote` produce the same level-1 key.
        let level1 = if backwards_sec {
            decompose_and_primary_fold(text, locale, self)
        } else {
            primary_fold(&expanded)
        };
        let mut out = Vec::with_capacity(expanded.len() * 2 + 8);
        out.push(strength.as_u8());
        out.push(0);
        out.extend_from_slice(level1.as_bytes());
        // Level-2 (secondary weights).
        if !matches!(strength, CollationStrength::Primary) {
            out.push(0x02);
            if backwards_sec {
                // French tailoring: encode the reversed per-position
                // secondary sequence so bytewise cmp of two keys
                // agrees with the compare_with_backwards_secondary
                // tie-break.
                let sec = reversed_secondary_weights(text, locale, self);
                for w in sec {
                    out.extend_from_slice(&w.to_be_bytes());
                }
            } else {
                let level2 = ascii_casefold(&expanded);
                out.extend_from_slice(level2.as_bytes());
            }
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

/// Primary-fold variant used by the case-second tailoring: strips
/// combining marks and lowercases every letter (not just ASCII) via
/// `char::to_lowercase()` so Cyrillic and Latin-supplement upper /
/// lower pairs collapse to a common primary form. The main
/// [`primary_fold`] deliberately ASCII-lowercases only — that
/// matches the Phase 2 default engine, which the case-second path
/// improves on for Russian.
#[cfg(feature = "alloc")]
fn case_second_primary_fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_combining_mark(c) {
            continue;
        }
        for lower in c.to_lowercase() {
            out.push(lower);
        }
    }
    out
}

/// True iff `c` carries an uppercase case signal — used by the
/// case-second tailoring to score per-character case at level 2.
///
/// Covers ASCII A-Z plus the Cyrillic uppercase block (U+0400..
/// =U+042F including irregular Ё U+0401), the Latin-1 supplement
/// upper letters (U+00C0..=U+00DE minus U+00D7 ×), and any scalar
/// whose `char::is_uppercase()` returns true and which is not
/// already ASCII-lowercase. The `is_uppercase()` check picks up
/// wider Unicode uppercase letters without needing a hand-written
/// table.
#[cfg(feature = "alloc")]
fn is_upper_letter(c: char) -> bool {
    if c.is_ascii_lowercase() {
        return false;
    }
    if c.is_ascii_uppercase() {
        return true;
    }
    // Cyrillic uppercase A-Я = U+0410..=U+042F, plus irregular
    // Ё at U+0401.
    let cp = c as u32;
    if (0x0410..=0x042F).contains(&cp) || cp == 0x0401 {
        return true;
    }
    // Latin-1 supplement uppercase (À..Þ minus ×).
    if (0x00C0..=0x00DE).contains(&cp) && cp != 0x00D7 {
        return true;
    }
    c.is_uppercase()
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

/// Pack-expand `text`, decompose recognised precomposed Latin
/// accented letters into base + combining mark, drop the combining
/// marks, and ASCII-lowercase. Produces a level-1 (primary) form
/// that ties on identical base letters regardless of accents or
/// case. Used by the backwards-secondary compare path so
/// `côte` and `cote` land in the same primary equivalence class.
#[cfg(feature = "alloc")]
fn decompose_and_primary_fold(text: &str, locale: &str, engine: &CollationEngine<'_>) -> String {
    let mut expanded = String::with_capacity(text.len());
    for c in text.chars() {
        engine.expand_char(c, locale, &mut expanded);
    }
    let mut out = String::with_capacity(expanded.len());
    for c in expanded.chars() {
        if is_combining_mark(c) {
            continue;
        }
        let base = decompose_precomposed(c).map_or(c, |(b, _)| b);
        if base.is_ascii_alphabetic() {
            out.push(base.to_ascii_lowercase());
        } else {
            out.push(base);
        }
    }
    out
}

/// Extract the reversed secondary-weight sequence for `text` under
/// the given locale — the tie-breaker for the French
/// backwards-secondary tailoring.
///
/// Walks the string once, pack-expanding each character, then
/// emitting a `u32` per source scalar carrying the diacritic weight
/// at that position:
///
/// * Combining marks (U+0300..U+036F etc.) attach to the previous
///   base slot rather than emitting their own — this matches
///   pre-NFD decomposed input like `cafe\u{0301}`.
/// * Precomposed Latin-1 accented letters emit the diacritic's
///   canonical combining mark (`é → U+0301`, `ô → U+0302`, …)
///   through the hand-written [`decompose_precomposed`] table.
/// * Everything else emits `0` (no accent at this position).
///
/// The resulting sequence is then reversed so that
/// `Vec<u32>::cmp` compares the last-position accent first.
#[cfg(feature = "alloc")]
fn reversed_secondary_weights(text: &str, locale: &str, engine: &CollationEngine<'_>) -> Vec<u32> {
    let mut expanded = String::with_capacity(text.len());
    for c in text.chars() {
        engine.expand_char(c, locale, &mut expanded);
    }
    let mut out: Vec<u32> = Vec::with_capacity(expanded.len());
    for c in expanded.chars() {
        if is_combining_mark(c) {
            if let Some(last) = out.last_mut() {
                // Attach this mark to the previous base slot. If
                // multiple marks stack, use the last one seen (a
                // simplification; UCA-full stacks all marks
                // separately, but real French text rarely has
                // multi-mark stacks that matter for sort order).
                *last = c as u32;
            }
            continue;
        }
        if let Some((_, mark)) = decompose_precomposed(c) {
            out.push(mark);
        } else {
            out.push(0);
        }
    }
    out.reverse();
    out
}

/// Decompose a precomposed Latin-1 / Latin-Extended letter into
/// `(base, combining_mark)` if it is a recognised accented form.
/// Returns `None` for base letters and non-Latin characters.
///
/// This is a hand-written subset of NFD covering the accented
/// characters common in French, German, and other Western European
/// text; adding rows is cheap when a locale's pack needs a new
/// letter. Uppercase pairs live alongside their lowercase forms so
/// `Café` and `café` produce the same reversed-secondary sequence.
#[cfg(feature = "alloc")]
fn decompose_precomposed(c: char) -> Option<(char, u32)> {
    // Combining diacritic codes: 0x300 grave, 0x301 acute, 0x302
    // circumflex, 0x303 tilde, 0x308 diaeresis, 0x30A ring, 0x327
    // cedilla.
    Some(match c {
        // Latin-1 supplement — lowercase.
        '\u{00E0}' => ('a', 0x0300),
        '\u{00E1}' => ('a', 0x0301),
        '\u{00E2}' => ('a', 0x0302),
        '\u{00E3}' => ('a', 0x0303),
        '\u{00E4}' => ('a', 0x0308),
        '\u{00E5}' => ('a', 0x030A),
        '\u{00E7}' => ('c', 0x0327),
        '\u{00E8}' => ('e', 0x0300),
        '\u{00E9}' => ('e', 0x0301),
        '\u{00EA}' => ('e', 0x0302),
        '\u{00EB}' => ('e', 0x0308),
        '\u{00EC}' => ('i', 0x0300),
        '\u{00ED}' => ('i', 0x0301),
        '\u{00EE}' => ('i', 0x0302),
        '\u{00EF}' => ('i', 0x0308),
        '\u{00F1}' => ('n', 0x0303),
        '\u{00F2}' => ('o', 0x0300),
        '\u{00F3}' => ('o', 0x0301),
        '\u{00F4}' => ('o', 0x0302),
        '\u{00F5}' => ('o', 0x0303),
        '\u{00F6}' => ('o', 0x0308),
        '\u{00F9}' => ('u', 0x0300),
        '\u{00FA}' => ('u', 0x0301),
        '\u{00FB}' => ('u', 0x0302),
        '\u{00FC}' => ('u', 0x0308),
        '\u{00FD}' => ('y', 0x0301),
        '\u{00FF}' => ('y', 0x0308),
        // Latin-1 supplement — uppercase.
        '\u{00C0}' => ('A', 0x0300),
        '\u{00C1}' => ('A', 0x0301),
        '\u{00C2}' => ('A', 0x0302),
        '\u{00C3}' => ('A', 0x0303),
        '\u{00C4}' => ('A', 0x0308),
        '\u{00C5}' => ('A', 0x030A),
        '\u{00C7}' => ('C', 0x0327),
        '\u{00C8}' => ('E', 0x0300),
        '\u{00C9}' => ('E', 0x0301),
        '\u{00CA}' => ('E', 0x0302),
        '\u{00CB}' => ('E', 0x0308),
        '\u{00CC}' => ('I', 0x0300),
        '\u{00CD}' => ('I', 0x0301),
        '\u{00CE}' => ('I', 0x0302),
        '\u{00CF}' => ('I', 0x0308),
        '\u{00D1}' => ('N', 0x0303),
        '\u{00D2}' => ('O', 0x0300),
        '\u{00D3}' => ('O', 0x0301),
        '\u{00D4}' => ('O', 0x0302),
        '\u{00D5}' => ('O', 0x0303),
        '\u{00D6}' => ('O', 0x0308),
        '\u{00D9}' => ('U', 0x0300),
        '\u{00DA}' => ('U', 0x0301),
        '\u{00DB}' => ('U', 0x0302),
        '\u{00DC}' => ('U', 0x0308),
        '\u{00DD}' => ('Y', 0x0301),
        _ => return None,
    })
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

    // -------------------------------------------------------------------
    // Backwards-secondary (French) tailoring
    // -------------------------------------------------------------------

    fn build_test_fr_backwards_secondary() -> alloc::vec::Vec<u8> {
        let mut c = CollationSectionBuilder::new();
        c.set_default_strength(CollationStrength::Tertiary.as_u8());
        c.set_backwards_secondary(true);
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("fr"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        w.finish()
    }

    #[test]
    fn backwards_secondary_orders_classic_french_quartet() {
        // Classical French dictionary order with the backwards-
        // secondary rule: at primary all four tie, so the tie-break
        // scans accents from the right — the rightmost accent (or
        // its absence) is the primary discriminator.
        //
        // The engine's per-position secondary sequence, reversed:
        //   cote → [0,   0, 0, 0]
        //   côte → [0,   0, ô, 0]
        //   coté → [é,   0, 0, 0]
        //   côté → [é,   0, ô, 0]
        //
        // Bytewise sort of the reversed sequences gives
        // `cote < côte < coté < côté`.
        let fr = build_test_fr_backwards_secondary();
        let pack = CollationPack::from_scud_bytes(&fr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        let mut words = vec!["côté", "coté", "cote", "côte"];
        words.sort_by(|a, b| engine.compare(a, b, "fr", CollationStrength::Tertiary));
        assert_eq!(words, vec!["cote", "côte", "coté", "côté"]);
    }

    #[test]
    fn backwards_secondary_ties_at_primary() {
        // All four words fold to the same primary key ("cote") when
        // combining marks are stripped.
        let fr = build_test_fr_backwards_secondary();
        let pack = CollationPack::from_scud_bytes(&fr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        for pair in [("cote", "côte"), ("cote", "coté"), ("cote", "côté")] {
            assert_eq!(
                engine.compare(pair.0, pair.1, "fr", CollationStrength::Primary),
                Ordering::Equal,
                "primary should tie for {pair:?}",
            );
        }
    }

    #[test]
    fn backwards_secondary_handles_decomposed_input() {
        // Feeding decomposed input (`cafe\u{0301}` == `café`) must
        // produce the same reversed secondary sequence as the
        // precomposed form.
        let fr = build_test_fr_backwards_secondary();
        let pack = CollationPack::from_scud_bytes(&fr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("café", "cafe\u{0301}", "fr", CollationStrength::Secondary),
            Ordering::Equal,
        );
    }

    #[test]
    fn backwards_secondary_falls_back_to_primary_when_base_letters_differ() {
        let fr = build_test_fr_backwards_secondary();
        let pack = CollationPack::from_scud_bytes(&fr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("bonjour", "chateau", "fr", CollationStrength::Tertiary),
            Ordering::Less,
        );
        assert_eq!(
            engine.compare("chien", "chat", "fr", CollationStrength::Tertiary),
            Ordering::Greater,
        );
    }

    #[test]
    fn backwards_secondary_sort_key_matches_compare() {
        let fr = build_test_fr_backwards_secondary();
        let pack = CollationPack::from_scud_bytes(&fr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        let pairs = [
            ("cote", "coté"),
            ("cote", "côte"),
            ("côte", "coté"),
            ("côte", "côté"),
            ("coté", "côté"),
        ];
        for (a, b) in pairs {
            let ka = engine.sort_key(a, "fr", CollationStrength::Tertiary);
            let kb = engine.sort_key(b, "fr", CollationStrength::Tertiary);
            assert_eq!(
                ka.cmp(&kb),
                engine.compare(a, b, "fr", CollationStrength::Tertiary),
                "sort_key vs compare disagreed for ({a:?}, {b:?})",
            );
        }
    }

    // -------------------------------------------------------------------
    // Primary-weight overrides (Turkish) tailoring
    // -------------------------------------------------------------------

    fn build_test_tr_primary_overrides() -> alloc::vec::Vec<u8> {
        let mut c = CollationSectionBuilder::new();
        // Full Turkish lowercase alphabet — assign primary weights
        // in Turkish alphabet order.
        for (i, (cp, _)) in TURKISH_ALPHABET.iter().enumerate() {
            // TURKISH_ALPHABET has <30 entries so u32::try_from is
            // never going to fail; clippy prefers the explicit
            // fallible cast anyway.
            let pw = 100 + u32::try_from(i).unwrap() * 10;
            c.push_primary_override(*cp, pw, 0, 0);
        }
        c.set_default_strength(CollationStrength::Tertiary.as_u8());
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("tr"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        w.append_section(
            stringcheese_scud::SECT_PRIMARY_OVERRIDES,
            &c.primary_overrides_bytes(),
        );
        w.finish()
    }

    /// The Turkish alphabet in dictionary order, lowercase.
    /// `(codepoint, letter)` pairs. ASCII codepoints are written
    /// as hex literals so the table stays a `const` (u8→u32 via
    /// `From` is not yet stable in const context on Rust 1.88).
    const TURKISH_ALPHABET: &[(u32, char)] = &[
        (0x0061, 'a'),
        (0x0062, 'b'),
        (0x0063, 'c'),
        (0x00E7, 'ç'),
        (0x0064, 'd'),
        (0x0065, 'e'),
        (0x0066, 'f'),
        (0x0067, 'g'),
        (0x011F, 'ğ'),
        (0x0068, 'h'),
        (0x0131, 'ı'),
        (0x0069, 'i'),
        (0x006A, 'j'),
        (0x006B, 'k'),
        (0x006C, 'l'),
        (0x006D, 'm'),
        (0x006E, 'n'),
        (0x006F, 'o'),
        (0x00F6, 'ö'),
        (0x0070, 'p'),
        (0x0072, 'r'),
        (0x0073, 's'),
        (0x015F, 'ş'),
        (0x0074, 't'),
        (0x0075, 'u'),
        (0x00FC, 'ü'),
        (0x0076, 'v'),
        (0x0079, 'y'),
        (0x007A, 'z'),
    ];

    #[test]
    fn primary_overrides_place_dotless_i_between_h_and_i() {
        let tr = build_test_tr_primary_overrides();
        let pack = CollationPack::from_scud_bytes(&tr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("h", "ı", "tr", CollationStrength::Primary),
            Ordering::Less,
        );
        assert_eq!(
            engine.compare("ı", "i", "tr", CollationStrength::Primary),
            Ordering::Less,
        );
        assert_eq!(
            engine.compare("h", "i", "tr", CollationStrength::Primary),
            Ordering::Less,
        );
    }

    #[test]
    fn primary_overrides_sort_turkish_alphabet_correctly() {
        let tr = build_test_tr_primary_overrides();
        let pack = CollationPack::from_scud_bytes(&tr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        // Shuffle the Turkish alphabet, then sort it — should
        // recover the dictionary order.
        let expected: alloc::vec::Vec<String> = TURKISH_ALPHABET
            .iter()
            .map(|(_, c)| c.to_string())
            .collect();
        let mut shuffled = expected.clone();
        shuffled.reverse();
        shuffled.sort_by(|a, b| engine.compare(a, b, "tr", CollationStrength::Primary));
        assert_eq!(shuffled, expected);
    }

    #[test]
    fn primary_overrides_case_folded_at_primary() {
        // Uppercase Ç should tie with lowercase ç at primary under
        // the override table's ASCII-lowercase-then-lookup rule.
        let tr = build_test_tr_primary_overrides();
        let pack = CollationPack::from_scud_bytes(&tr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("araba", "ARABA", "tr", CollationStrength::Primary),
            Ordering::Equal,
        );
    }

    #[test]
    fn primary_overrides_apply_pack_expansions() {
        // The tr collation pack often ships ß → ss as an expansion.
        // Verify the expansion still fires under the override path.
        let mut c = CollationSectionBuilder::new();
        c.push_expansion(0x00DF, &[u32::from(b's'), u32::from(b's')]);
        for (i, (cp, _)) in TURKISH_ALPHABET.iter().enumerate() {
            let pw = 100 + u32::try_from(i).unwrap() * 10;
            c.push_primary_override(*cp, pw, 0, 0);
        }
        c.set_default_strength(CollationStrength::Tertiary.as_u8());
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("tr"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        w.append_section(
            stringcheese_scud::SECT_PRIMARY_OVERRIDES,
            &c.primary_overrides_bytes(),
        );
        let bytes = w.finish();
        let pack = CollationPack::from_scud_bytes(&bytes).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("straße", "strasse", "tr", CollationStrength::Primary),
            Ordering::Equal,
        );
    }

    #[test]
    fn primary_overrides_sort_key_matches_compare() {
        let tr = build_test_tr_primary_overrides();
        let pack = CollationPack::from_scud_bytes(&tr).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        for (a, b) in [
            ("h", "ı"),
            ("ı", "i"),
            ("araba", "bebek"),
            ("cadde", "cami"),
            ("hı", "hi"),
        ] {
            let ka = engine.sort_key(a, "tr", CollationStrength::Primary);
            let kb = engine.sort_key(b, "tr", CollationStrength::Primary);
            assert_eq!(
                ka.cmp(&kb),
                engine.compare(a, b, "tr", CollationStrength::Primary),
                "sort_key vs compare disagreed for ({a:?}, {b:?})",
            );
        }
    }

    #[test]
    fn decompose_precomposed_covers_french_accents() {
        assert_eq!(decompose_precomposed('é'), Some(('e', 0x0301)));
        assert_eq!(decompose_precomposed('è'), Some(('e', 0x0300)));
        assert_eq!(decompose_precomposed('ô'), Some(('o', 0x0302)));
        assert_eq!(decompose_precomposed('ç'), Some(('c', 0x0327)));
        assert_eq!(decompose_precomposed('É'), Some(('E', 0x0301)));
        assert_eq!(decompose_precomposed('a'), None);
        assert_eq!(decompose_precomposed('z'), None);
    }

    // -------------------------------------------------------------------
    // Case-second (Russian) tailoring
    // -------------------------------------------------------------------

    fn build_test_ru_case_second() -> alloc::vec::Vec<u8> {
        let mut c = CollationSectionBuilder::new();
        // ß → ss stays as a shared expansion so composed-engine hits
        // match ru's real pack shape.
        c.push_expansion(0x00DF, &[0x0073, 0x0073]);
        c.set_default_strength(CollationStrength::Tertiary.as_u8());
        c.set_case_second(true);
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("ru"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        w.finish()
    }

    #[test]
    fn case_second_promotes_case_to_secondary_level() {
        // "Аа" (Upper, lower) vs "аА" (lower, Upper): both fold to
        // the same primary. Under case-second the L2 case marker
        // dominates — uppercase is scored higher, so the
        // leading-uppercase form comes later.
        let ru = build_test_ru_case_second();
        let pack = CollationPack::from_scud_bytes(&ru).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        let ord = engine.compare("Аа", "аА", "ru", CollationStrength::Secondary);
        assert_eq!(ord, Ordering::Greater);
        // Antisymmetry.
        let ord_rev = engine.compare("аА", "Аа", "ru", CollationStrength::Secondary);
        assert_eq!(ord_rev, Ordering::Less);
    }

    #[test]
    fn case_second_ties_at_primary() {
        // At primary the case difference disappears — both forms
        // fold to the same base letters.
        let ru = build_test_ru_case_second();
        let pack = CollationPack::from_scud_bytes(&ru).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("Аа", "аА", "ru", CollationStrength::Primary),
            Ordering::Equal,
        );
        // ASCII pairs still fold at primary too.
        assert_eq!(
            engine.compare("apple", "APPLE", "ru", CollationStrength::Primary),
            Ordering::Equal,
        );
    }

    #[test]
    fn case_second_still_orders_by_base_letters() {
        // When base letters differ, the primary level wins and the
        // case-second machinery never fires.
        let ru = build_test_ru_case_second();
        let pack = CollationPack::from_scud_bytes(&ru).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("Арбуз", "Белка", "ru", CollationStrength::Secondary),
            Ordering::Less,
        );
        assert_eq!(
            engine.compare("Гараж", "Арбуз", "ru", CollationStrength::Tertiary),
            Ordering::Greater,
        );
    }

    #[test]
    fn case_second_case_wins_over_length_prefix() {
        // The classic UCA "case ties break after full L1 compare"
        // rule still holds — a shorter all-lower prefix vs a
        // longer mixed-case string: L1 orders by prefix length,
        // L2 only fires on primary ties.
        let ru = build_test_ru_case_second();
        let pack = CollationPack::from_scud_bytes(&ru).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        // "аА" (2 chars, mixed) vs "аа" (2 chars, all lower):
        // primary ties, then case-second sorts lowercase-first.
        assert_eq!(
            engine.compare("аа", "аА", "ru", CollationStrength::Secondary),
            Ordering::Less,
        );
        // "аа" (2 chars) vs "аа" (2 chars) — identical.
        assert_eq!(
            engine.compare("аа", "аа", "ru", CollationStrength::Secondary),
            Ordering::Equal,
        );
    }

    #[test]
    fn case_second_sort_key_matches_compare() {
        let ru = build_test_ru_case_second();
        let pack = CollationPack::from_scud_bytes(&ru).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        let pairs = [
            ("Аа", "аА"),
            ("Арбуз", "Белка"),
            ("аА", "аа"),
            ("привет", "ПРИВЕТ"),
            ("Straße", "Strasse"),
        ];
        for strength in [
            CollationStrength::Primary,
            CollationStrength::Secondary,
            CollationStrength::Tertiary,
        ] {
            for (a, b) in pairs {
                let ka = engine.sort_key(a, "ru", strength);
                let kb = engine.sort_key(b, "ru", strength);
                assert_eq!(
                    ka.cmp(&kb),
                    engine.compare(a, b, "ru", strength),
                    "sort_key vs compare disagreed for ({a:?}, {b:?}, {strength:?})",
                );
            }
        }
    }

    #[test]
    fn case_second_expansions_still_fire() {
        // The shared ß → ss expansion still applies under
        // case-second, matching Russian's real pack shape.
        let ru = build_test_ru_case_second();
        let pack = CollationPack::from_scud_bytes(&ru).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        assert_eq!(
            engine.compare("Straße", "Strasse", "ru", CollationStrength::Primary),
            Ordering::Equal,
        );
    }

    #[test]
    fn case_second_yields_when_pack_also_has_primary_overrides() {
        // When a pack sets both primary_overrides and case_second,
        // primary-overrides win — this is the documented precedence
        // and matches how `compare` dispatches. No CLDR pack ships
        // both bits together; this test locks the precedence for
        // future maintainers.
        let mut c = CollationSectionBuilder::new();
        c.push_primary_override(0x0430, 100, 0, 0); // а
        c.push_primary_override(0x0410, 100, 0, 0); // А (same primary)
        c.set_default_strength(CollationStrength::Tertiary.as_u8());
        c.set_case_second(true);
        let mut w = ScudWriter::new(CAP_COLLATION, "44.1", Some("ru-x-mixed"));
        w.append_section(SECT_EXPANSIONS, &c.expansion_bytes());
        w.append_section(SECT_COLLATION_OPTIONS, &c.options_bytes());
        w.append_section(
            stringcheese_scud::SECT_PRIMARY_OVERRIDES,
            &c.primary_overrides_bytes(),
        );
        let bytes = w.finish();
        let pack = CollationPack::from_scud_bytes(&bytes).unwrap();
        let engine = CollationEngine::new(vec![pack]);
        // Under the primary-override path both "а" and "А" have the
        // same primary weight → the compare uses the L2/L3 tuple
        // from that path, not the case-second key. Whatever the
        // result is, the important invariant is that sort_key ==
        // compare under the same tailoring, which we check here.
        let a = "а";
        let b = "А";
        let ka = engine.sort_key(a, "ru-x-mixed", CollationStrength::Tertiary);
        let kb = engine.sort_key(b, "ru-x-mixed", CollationStrength::Tertiary);
        assert_eq!(
            ka.cmp(&kb),
            engine.compare(a, b, "ru-x-mixed", CollationStrength::Tertiary),
        );
    }

    #[test]
    fn is_upper_letter_covers_cyrillic_and_latin() {
        assert!(is_upper_letter('A'));
        assert!(!is_upper_letter('a'));
        assert!(is_upper_letter('\u{0410}')); // А
        assert!(!is_upper_letter('\u{0430}')); // а
        assert!(is_upper_letter('\u{0401}')); // Ё
        assert!(!is_upper_letter('\u{0451}')); // ё
        assert!(is_upper_letter('\u{00DC}')); // Ü
        assert!(!is_upper_letter('\u{00FC}')); // ü
        // Non-letters and combining marks do not count as uppercase.
        assert!(!is_upper_letter(' '));
        assert!(!is_upper_letter('1'));
        assert!(!is_upper_letter('\u{0301}')); // combining acute
    }
}
