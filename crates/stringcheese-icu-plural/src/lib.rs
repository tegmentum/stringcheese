//! Plural-rules capability for the StringCheese ICU-alternative
//! subsystem.
//!
//! Classifies a number under CLDR plural rules (cardinal + ordinal)
//! using per-locale data supplied through one or more
//! `stringcheese-scud` plural packs, and exposes the result through
//! the `tegmentum:i18n-plural@0.1.0` WIT world. Callers construct a
//! [`PluralEngine`] from a slice of loaded [`PluralPack`]s and issue
//! [`plural_cardinal`](PluralEngine::plural_cardinal) /
//! [`plural_ordinal`](PluralEngine::plural_ordinal) queries; the
//! engine walks the BCP 47 fallback chain (`pt-BR → pt → ""`) at
//! query time, then evaluates each pack rule in order against the
//! CLDR operand tuple.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 3 of the WIT-i18n design (`docs/design/wit-i18n.md`
//! section 8.3) — the third capability delivered on top of the
//! shared `stringcheese-scud` loader (Phase 1) after case-mapping
//! (Phase 1) and collation (Phase 2). Phase 3 hand-encodes ~10-12
//! locales worth of CLDR predicates and defers the full ~200
//! locale set to a follow-up wave. See [`PluralRuleId`] for the
//! predicate opcodes shipped in this wave.
//!
//! # WIT surface
//!
//! The WIT file at `component/wit/plural/stringcheese-icu-plural.wit`
//! defines three exports on the `plural-world` world:
//!
//! * `plural-cardinal(n, locale)` — cardinal classification.
//! * `plural-ordinal(n, locale)` — ordinal classification.
//! * `supported-locales()` — introspection.
//!
//! A [`PluralEngine`] implements every export on the Rust side; a
//! future `wit-component`-gated `Guest` implementation lands in a
//! follow-up wave.
//!
//! # Phase 3 deferrals
//!
//! * **Standalone WASM component build.** The WIT interface is in
//!   place and parses cleanly under `wit-parser` (see the smoke
//!   test in `tests/wit_parse.rs`); the `wit-bindgen` `Guest`
//!   implementation and the `cargo build --target wasm32-wasip1
//!   --features wit-component` recipe land in a follow-up wave.
//! * **Full ~200-locale CLDR plural rules.** Phase 3 hand-encodes
//!   ~10-12 locales' predicates (see [`PluralRuleId`]); the rest
//!   ship as follow-up packs.
//! * **`c` / `e` (compact/exponent) operands.** Phase 3 evaluates
//!   only the `n / i / v / w / f / t` operands per CLDR UTS #35 §
//!   5.1. Compact notation (`1.2K → 1200`) will need `e ≠ 0`
//!   handling in a follow-up.
//!
//! # Trust model
//!
//! Inherited from `stringcheese-scud`: SCUD packs are trusted
//! input. This crate does not defend against maliciously crafted
//! packs.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use stringcheese_scud::{PluralDataView, ScudFile};

// Re-export the SCUD-side types so language packs can name them
// without adding a direct `stringcheese-scud` dependency.
pub use stringcheese_scud::{PluralCategory, ScudError};

/// The set of CLDR plural predicate opcodes hand-encoded by Phase 3.
///
/// The wire format stores a `u8 rule_id` in each pack entry; the
/// engine dispatches on that id to one of the predicates below.
/// Language packs push entries via
/// `PluralSectionBuilder::push_cardinal(category, rule_id.as_u8())`
/// — the enum keeps the ids symbolic rather than magic numbers.
///
/// The predicates cover ~11 locales' cardinal + ordinal rules
/// exactly:
///
/// | Rule id                             | Value | Covers                              |
/// | ----------------------------------- | -----:| ----------------------------------- |
/// | [`IEq1AndVEq0`](Self::IEq1AndVEq0)  |     1 | en/de cardinal `one`                |
/// | [`NMod10Eq1NotMod100Eq11`](Self::NMod10Eq1NotMod100Eq11) |     2 | en ordinal `one`   |
/// | [`NMod10Eq2NotMod100Eq12`](Self::NMod10Eq2NotMod100Eq12) |     3 | en ordinal `two`   |
/// | [`NMod10Eq3NotMod100Eq13`](Self::NMod10Eq3NotMod100Eq13) |     4 | en ordinal `few`   |
/// | [`IIn01`](Self::IIn01)              |     5 | fr/pt cardinal `one`                |
/// | [`NEq1`](Self::NEq1)                |     6 | es/it ordinal, ar/others `one`      |
/// | [`RuOne`](Self::RuOne)              |     7 | ru cardinal `one`                   |
/// | [`SlavFew`](Self::SlavFew)          |     8 | ru/pl cardinal `few`                |
/// | [`PlMany`](Self::PlMany)            |     9 | pl cardinal `many`                  |
/// | [`RuMany`](Self::RuMany)            |    10 | ru cardinal `many`                  |
/// | [`NEq0`](Self::NEq0)                |    11 | ar cardinal `zero`                  |
/// | [`NEq2`](Self::NEq2)                |    12 | ar cardinal `two`                   |
/// | [`ArFew`](Self::ArFew)              |    13 | ar cardinal `few`                   |
/// | [`ArMany`](Self::ArMany)            |    14 | ar cardinal `many`                  |
/// | [`IEq1`](Self::IEq1)                |    15 | integer `one` (v ignored)           |
///
/// Rule ids outside the shipped set evaluate to `false`, so an
/// unknown pack entry silently falls through to the next rule and
/// ultimately to [`PluralCategory::Other`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PluralRuleId {
    /// `i = 1 and v = 0` — English / German cardinal `one`.
    IEq1AndVEq0 = 1,
    /// `n % 10 = 1 and n % 100 != 11` — English ordinal `one`.
    NMod10Eq1NotMod100Eq11 = 2,
    /// `n % 10 = 2 and n % 100 != 12` — English ordinal `two`.
    NMod10Eq2NotMod100Eq12 = 3,
    /// `n % 10 = 3 and n % 100 != 13` — English ordinal `few`.
    NMod10Eq3NotMod100Eq13 = 4,
    /// `i = 0..1` — French / Portuguese cardinal `one`.
    IIn01 = 5,
    /// `n = 1` — Spanish / Italian ordinal `one` and the
    /// integer-only shorthand for other locales.
    NEq1 = 6,
    /// `v = 0 and i % 10 = 1 and i % 100 != 11` — Russian cardinal
    /// `one`.
    RuOne = 7,
    /// `v = 0 and i % 10 in 2..4 and i % 100 not in 12..14` —
    /// Russian / Polish cardinal `few`.
    SlavFew = 8,
    /// `v = 0 and ((i != 1 and i % 10 in 0..1) or i % 10 in 5..9
    /// or i % 100 in 12..14)` — Polish cardinal `many`.
    PlMany = 9,
    /// `v = 0 and (i % 10 = 0 or i % 10 in 5..9 or i % 100 in
    /// 11..14)` — Russian cardinal `many`.
    RuMany = 10,
    /// `n = 0` — Arabic cardinal `zero`.
    NEq0 = 11,
    /// `n = 2` — Arabic cardinal `two`.
    NEq2 = 12,
    /// `n % 100 in 3..10` — Arabic cardinal `few`.
    ArFew = 13,
    /// `n % 100 in 11..99` — Arabic cardinal `many`.
    ArMany = 14,
    /// `i = 1` — English-like `one` when the caller does not care
    /// about `v`.
    IEq1 = 15,
}

impl PluralRuleId {
    /// The wire-encoded `u8` value for this rule id. Matches what
    /// `PluralSectionBuilder::push_cardinal(_, id.as_u8())` writes.
    #[must_use]
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// The CLDR operand tuple extracted from a numeric input.
///
/// Only the operands used by the Phase 3 predicates are carried;
/// `c` / `e` (compact/exponent) land in a follow-up.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PluralOperands {
    /// The absolute value of the source number.
    pub n: f64,
    /// The integer digits of `n` (i.e. `n.trunc().abs() as u64`).
    pub i: u64,
    /// Number of visible fraction digits (with trailing zeros).
    pub v: u32,
    /// Number of visible fraction digits (without trailing zeros).
    pub w: u32,
    /// Visible fraction digits as an integer (with trailing zeros).
    pub f: u64,
    /// Visible fraction digits as an integer (without trailing
    /// zeros).
    pub t: u64,
}

impl PluralOperands {
    /// Extract the CLDR operand tuple from an `f64` input.
    ///
    /// Uses Rust's default `f64` formatting to recover the visible
    /// fraction-digit shape: `1.0` gives v = 0, `1.5` gives v = w = 1,
    /// `1.50` (indistinguishable from `1.5` as an `f64`) gives v = w = 1
    /// — a documented Phase 3 simplification.
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        let n_abs = value.abs();
        // Non-finite values reduce to the `other` category via a
        // caller-side guard in `PluralEngine`; we return a zeroed
        // operand here so downstream predicates never see NaN in
        // arithmetic.
        if !n_abs.is_finite() {
            return Self {
                n: 0.0,
                i: 0,
                v: 0,
                w: 0,
                f: 0,
                t: 0,
            };
        }
        let integer = int_part(n_abs);
        let frac = n_abs - n_abs.trunc();
        if frac == 0.0 {
            return Self {
                n: n_abs,
                i: integer,
                v: 0,
                w: 0,
                f: 0,
                t: 0,
            };
        }
        // Rust's default f64 formatting emits the shortest
        // round-tripping decimal representation. Split on the
        // decimal point and take the trailing digits as `f`.
        let formatted = alloc::format!("{n_abs}");
        if let Some(dot) = formatted.find('.') {
            let frac_str = &formatted[dot + 1..];
            let v_digits = u32::try_from(frac_str.len()).unwrap_or(u32::MAX);
            let f_digits = frac_str.parse::<u64>().unwrap_or(0);
            let trimmed = frac_str.trim_end_matches('0');
            let w_digits = u32::try_from(trimmed.len()).unwrap_or(u32::MAX);
            let t_digits = if trimmed.is_empty() {
                0
            } else {
                trimmed.parse::<u64>().unwrap_or(0)
            };
            Self {
                n: n_abs,
                i: integer,
                v: v_digits,
                w: w_digits,
                f: f_digits,
                t: t_digits,
            }
        } else {
            Self {
                n: n_abs,
                i: integer,
                v: 0,
                w: 0,
                f: 0,
                t: 0,
            }
        }
    }

    /// `#[cfg(not(feature = "alloc"))]` fallback: integer-only.
    ///
    /// Without `alloc` we cannot format the value to extract
    /// fraction digits; instead we round toward zero and treat the
    /// result as an integer. Callers on embedded targets who need
    /// non-integer plural classification enable `alloc`.
    #[cfg(not(feature = "alloc"))]
    #[must_use]
    pub fn from_f64(value: f64) -> Self {
        let n_abs = value.abs();
        if !n_abs.is_finite() {
            return Self {
                n: 0.0,
                i: 0,
                v: 0,
                w: 0,
                f: 0,
                t: 0,
            };
        }
        Self {
            n: n_abs,
            i: int_part(n_abs),
            v: 0,
            w: 0,
            f: 0,
            t: 0,
        }
    }
}

/// Truncate a non-negative finite `f64` to a `u64`. Saturates on
/// overflow; caller guarantees `n` is finite and non-negative.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn int_part(n: f64) -> u64 {
    let truncated = n.trunc();
    if truncated < 0.0 {
        0
    } else if truncated >= u64::MAX as f64 {
        u64::MAX
    } else {
        truncated as u64
    }
}

/// Evaluate a single rule opcode against the operand tuple.
///
/// Returns `true` when the predicate matches. Unknown opcodes
/// return `false` so a forward-compatible pack (a newer minor
/// version that defined a new rule) does not misclassify — the
/// engine simply falls through to the next rule or to
/// [`PluralCategory::Other`].
#[allow(clippy::float_cmp)]
#[must_use]
pub fn evaluate_rule(rule_id: u8, op: &PluralOperands) -> bool {
    // `op.n` is non-negative and finite (guaranteed by
    // `PluralOperands::from_f64`); use the saturating conversion so
    // extreme inputs do not panic and unusual values just fall to
    // the `other` bucket.
    let n_int = int_part(op.n);
    // Small-integer equality on `f64` is exact when the operand
    // originated from `PluralOperands::from_f64` (which stores the
    // caller's f64 as-is, then extracts the fraction shape). The
    // callers we care about compare against 0, 1, 2 — all exactly
    // representable — so `float_cmp` is allowed for the whole
    // function.
    match rule_id {
        1 => op.i == 1 && op.v == 0,
        2 => n_int % 10 == 1 && n_int % 100 != 11 && op.v == 0,
        3 => n_int % 10 == 2 && n_int % 100 != 12 && op.v == 0,
        4 => n_int % 10 == 3 && n_int % 100 != 13 && op.v == 0,
        5 => (op.i == 0 || op.i == 1) && op.v == 0,
        6 => op.n == 1.0,
        7 => op.v == 0 && op.i % 10 == 1 && op.i % 100 != 11,
        8 => op.v == 0 && (2..=4).contains(&(op.i % 10)) && !(12..=14).contains(&(op.i % 100)),
        9 => {
            op.v == 0
                && ((op.i != 1 && (op.i.is_multiple_of(10) || op.i % 10 == 1))
                    || (5..=9).contains(&(op.i % 10))
                    || (12..=14).contains(&(op.i % 100)))
        }
        10 => {
            op.v == 0
                && (op.i.is_multiple_of(10)
                    || (5..=9).contains(&(op.i % 10))
                    || (11..=14).contains(&(op.i % 100)))
        }
        11 => op.n == 0.0,
        12 => op.n == 2.0 && op.v == 0,
        13 => op.v == 0 && (3..=10).contains(&(n_int % 100)),
        14 => op.v == 0 && (11..=99).contains(&(n_int % 100)),
        15 => op.i == 1,
        _ => false,
    }
}

/// A loaded plural-rules pack for one BCP 47 locale.
///
/// Wraps a validated [`ScudFile`] whose capability tag is
/// [`stringcheese_scud::CAP_PLURAL`]. Cheap to clone — the
/// underlying SCUD bytes are borrowed by the [`ScudFile`], and this
/// wrapper carries only the parsed header plus the locale tag.
#[derive(Debug, Clone, Copy)]
pub struct PluralPack<'a> {
    scud: ScudFile<'a>,
    locale: &'a str,
    data: PluralDataView<'a>,
}

impl<'a> PluralPack<'a> {
    /// Wrap a validated [`ScudFile`] as a plural pack.
    ///
    /// Returns an error if the SCUD file's capability tag is not
    /// [`stringcheese_scud::CAP_PLURAL`].
    pub fn new(scud: ScudFile<'a>) -> Result<Self, ScudError> {
        let data = scud.as_plural_data()?;
        let locale = scud.locale().unwrap_or("");
        Ok(Self { scud, locale, data })
    }

    /// Parse `bytes` as a SCUD file and wrap it as a plural pack.
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

    /// The zero-copy plural-rules data view.
    #[must_use]
    pub fn data(&self) -> &PluralDataView<'a> {
        &self.data
    }
}

/// Locale-sensitive plural-rules engine.
///
/// Holds a list of [`PluralPack`]s and consults them at query time
/// in BCP 47 fallback order.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct PluralEngine<'a> {
    packs: Vec<PluralPack<'a>>,
}

/// Typed failure modes of the plural engine. Mirrors the WIT
/// `plural-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluralError {
    /// The locale tag was not a well-formed BCP 47 tag.
    InvalidLocale(&'static str),
    /// No pack was loaded for the requested locale.
    LocaleUnavailable(&'static str),
}

#[cfg(feature = "alloc")]
impl<'a> PluralEngine<'a> {
    /// Construct a fresh engine backed by the given packs.
    #[must_use]
    pub fn new(packs: Vec<PluralPack<'a>>) -> Self {
        Self { packs }
    }

    /// Every BCP 47 locale tag this engine knows about.
    #[must_use]
    pub fn supported_locales(&self) -> Vec<&'a str> {
        self.packs.iter().map(PluralPack::locale).collect()
    }

    /// True iff a query in the given locale would use a pack
    /// (rather than falling through to root).
    #[must_use]
    pub fn supports(&self, locale: &str) -> bool {
        walk_fallback_chain(locale).any(|tag| self.pack_for(tag).is_some())
    }

    /// Classify `n` as a cardinal under the given locale.
    ///
    /// Returns [`PluralCategory::Other`] when no rule matches — the
    /// CLDR-guaranteed fallback bucket every locale defines. Only
    /// the most-specific matching pack in the fallback chain is
    /// consulted; a pack that ships no matching rule falls to
    /// `Other` rather than mixing rule tables across dialects.
    #[must_use]
    pub fn plural_cardinal(&self, n: f64, locale: &str) -> PluralCategory {
        if !n.is_finite() {
            return PluralCategory::Other;
        }
        let op = PluralOperands::from_f64(n);
        let Some(pack) = self.packs_for_locale(locale).next() else {
            return PluralCategory::Other;
        };
        for (cat, rule_id) in pack.data.cardinal_rules() {
            if evaluate_rule(rule_id, &op) {
                return cat;
            }
        }
        PluralCategory::Other
    }

    /// Classify `n` as an ordinal under the given locale.
    #[must_use]
    pub fn plural_ordinal(&self, n: f64, locale: &str) -> PluralCategory {
        if !n.is_finite() {
            return PluralCategory::Other;
        }
        let op = PluralOperands::from_f64(n);
        let Some(pack) = self.packs_for_locale(locale).next() else {
            return PluralCategory::Other;
        };
        for (cat, rule_id) in pack.data.ordinal_rules() {
            if evaluate_rule(rule_id, &op) {
                return cat;
            }
        }
        PluralCategory::Other
    }

    /// The pack that would service a query under `locale`, if any.
    #[must_use]
    pub fn active_pack(&self, locale: &str) -> Option<&PluralPack<'a>> {
        walk_fallback_chain(locale).find_map(|tag| self.pack_for(tag))
    }

    fn pack_for(&self, tag: &str) -> Option<&PluralPack<'a>> {
        self.packs
            .iter()
            .find(|p| p.locale.eq_ignore_ascii_case(tag))
    }

    fn packs_for_locale<'e>(&'e self, locale: &'e str) -> impl Iterator<Item = &'e PluralPack<'a>> {
        walk_fallback_chain(locale).filter_map(move |tag| self.pack_for(tag))
    }
}

/// Walk the CLDR-defined fallback chain for a BCP 47 tag.
///
/// Same shape as
/// [`stringcheese_icu_case::walk_fallback_chain`](https://docs.rs/stringcheese-icu-case)
/// and [`stringcheese_icu_collation::walk_fallback_chain`](https://docs.rs/stringcheese-icu-collation)
/// — the chain strips subtags one at a time from the right,
/// terminating with the empty string (root).
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
// Utility: language-pack builders
// -----------------------------------------------------------------------

/// Utilities language-pack `build.rs` scripts use to construct the
/// per-locale plural SCUD packs shipped by this crate's callers.
///
/// Every helper writes into a caller-supplied
/// [`stringcheese_scud::PluralSectionBuilder`]; the caller finalises
/// the pack via `ScudWriter::finish`.
#[cfg(feature = "alloc")]
pub mod builder {
    use stringcheese_scud::{PluralCategory, PluralSectionBuilder};

    use super::PluralRuleId;

    /// English cardinal rules: `one` (i = 1, v = 0), else `other`.
    ///
    /// Also serves German cardinals — the two locales share this
    /// exact predicate structure.
    pub fn english_cardinals(b: &mut PluralSectionBuilder) {
        b.push_cardinal(PluralCategory::One, PluralRuleId::IEq1AndVEq0.as_u8());
    }

    /// English ordinal rules: `one` (n % 10 = 1, n % 100 ≠ 11),
    /// `two` (n % 10 = 2, n % 100 ≠ 12), `few` (n % 10 = 3, n %
    /// 100 ≠ 13), else `other`.
    pub fn english_ordinals(b: &mut PluralSectionBuilder) {
        b.push_ordinal(
            PluralCategory::One,
            PluralRuleId::NMod10Eq1NotMod100Eq11.as_u8(),
        );
        b.push_ordinal(
            PluralCategory::Two,
            PluralRuleId::NMod10Eq2NotMod100Eq12.as_u8(),
        );
        b.push_ordinal(
            PluralCategory::Few,
            PluralRuleId::NMod10Eq3NotMod100Eq13.as_u8(),
        );
    }

    /// German cardinal rules: `one` (i = 1, v = 0), else `other`.
    pub fn german_cardinals(b: &mut PluralSectionBuilder) {
        b.push_cardinal(PluralCategory::One, PluralRuleId::IEq1AndVEq0.as_u8());
    }

    /// German ordinals: everything is `other`. The helper pushes
    /// nothing.
    pub fn german_ordinals(_b: &mut PluralSectionBuilder) {}

    /// French cardinal rules: `one` (i in 0..1), else `other`.
    ///
    /// (`many` compact-notation bucket is deferred — Phase 3 does
    /// not evaluate the `e` operand.)
    pub fn french_cardinals(b: &mut PluralSectionBuilder) {
        b.push_cardinal(PluralCategory::One, PluralRuleId::IIn01.as_u8());
    }

    /// French ordinals: `one` (n = 1), else `other`.
    pub fn french_ordinals(b: &mut PluralSectionBuilder) {
        b.push_ordinal(PluralCategory::One, PluralRuleId::NEq1.as_u8());
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;
    use stringcheese_scud::{
        CAP_PLURAL, PluralSectionBuilder, SECT_CARDINAL_RULES, SECT_ORDINAL_RULES, ScudFile,
        ScudWriter,
    };

    fn build_en() -> alloc::vec::Vec<u8> {
        let mut b = PluralSectionBuilder::new();
        builder::english_cardinals(&mut b);
        builder::english_ordinals(&mut b);
        let mut w = ScudWriter::new(CAP_PLURAL, "44.1", Some("en"));
        w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
        w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
        w.finish()
    }

    fn build_fr() -> alloc::vec::Vec<u8> {
        let mut b = PluralSectionBuilder::new();
        builder::french_cardinals(&mut b);
        builder::french_ordinals(&mut b);
        let mut w = ScudWriter::new(CAP_PLURAL, "44.1", Some("fr"));
        w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
        w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
        w.finish()
    }

    fn build_ar() -> alloc::vec::Vec<u8> {
        let mut b = PluralSectionBuilder::new();
        b.push_cardinal(PluralCategory::Zero, PluralRuleId::NEq0.as_u8());
        b.push_cardinal(PluralCategory::One, PluralRuleId::NEq1.as_u8());
        b.push_cardinal(PluralCategory::Two, PluralRuleId::NEq2.as_u8());
        b.push_cardinal(PluralCategory::Few, PluralRuleId::ArFew.as_u8());
        b.push_cardinal(PluralCategory::Many, PluralRuleId::ArMany.as_u8());
        let mut w = ScudWriter::new(CAP_PLURAL, "44.1", Some("ar"));
        w.append_section(SECT_CARDINAL_RULES, &b.cardinal_bytes());
        w.append_section(SECT_ORDINAL_RULES, &b.ordinal_bytes());
        w.finish()
    }

    #[test]
    fn english_cardinals_basic() {
        let bytes = build_en();
        let pack = PluralPack::from_scud_bytes(&bytes).unwrap();
        let e = PluralEngine::new(vec![pack]);
        assert_eq!(e.plural_cardinal(1.0, "en"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(0.0, "en"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(2.0, "en"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(1.5, "en"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(1.0, "en-US"), PluralCategory::One);
    }

    #[test]
    fn english_ordinals_basic() {
        let bytes = build_en();
        let pack = PluralPack::from_scud_bytes(&bytes).unwrap();
        let e = PluralEngine::new(vec![pack]);
        // 1st, 21st, 101st, 121st → one
        for n in [1.0, 21.0, 101.0, 121.0] {
            assert_eq!(e.plural_ordinal(n, "en"), PluralCategory::One, "n={n}");
        }
        // 2nd, 22nd, 102nd → two
        for n in [2.0, 22.0, 102.0] {
            assert_eq!(e.plural_ordinal(n, "en"), PluralCategory::Two, "n={n}");
        }
        // 3rd, 23rd, 103rd → few
        for n in [3.0, 23.0, 103.0] {
            assert_eq!(e.plural_ordinal(n, "en"), PluralCategory::Few, "n={n}");
        }
        // 11th, 12th, 13th → other (teens exception)
        for n in [11.0, 12.0, 13.0] {
            assert_eq!(e.plural_ordinal(n, "en"), PluralCategory::Other, "n={n}");
        }
        // 111th → other
        assert_eq!(e.plural_ordinal(111.0, "en"), PluralCategory::Other);
    }

    #[test]
    fn french_cardinals_treat_zero_as_one() {
        let bytes = build_fr();
        let pack = PluralPack::from_scud_bytes(&bytes).unwrap();
        let e = PluralEngine::new(vec![pack]);
        // French: 0 and 1 both use singular.
        assert_eq!(e.plural_cardinal(0.0, "fr"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(1.0, "fr"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "fr"), PluralCategory::Other);
    }

    #[test]
    fn arabic_full_category_set() {
        let bytes = build_ar();
        let pack = PluralPack::from_scud_bytes(&bytes).unwrap();
        let e = PluralEngine::new(vec![pack]);
        assert_eq!(e.plural_cardinal(0.0, "ar"), PluralCategory::Zero);
        assert_eq!(e.plural_cardinal(1.0, "ar"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "ar"), PluralCategory::Two);
        assert_eq!(e.plural_cardinal(3.0, "ar"), PluralCategory::Few);
        assert_eq!(e.plural_cardinal(10.0, "ar"), PluralCategory::Few);
        assert_eq!(e.plural_cardinal(11.0, "ar"), PluralCategory::Many);
        assert_eq!(e.plural_cardinal(99.0, "ar"), PluralCategory::Many);
        assert_eq!(e.plural_cardinal(100.0, "ar"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(103.0, "ar"), PluralCategory::Few);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn operands_from_f64_integer() {
        let op = PluralOperands::from_f64(5.0);
        assert_eq!(op.n, 5.0);
        assert_eq!(op.i, 5);
        assert_eq!(op.v, 0);
        assert_eq!(op.f, 0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn operands_from_f64_fractional() {
        let op = PluralOperands::from_f64(1.5);
        assert_eq!(op.n, 1.5);
        assert_eq!(op.i, 1);
        assert_eq!(op.v, 1);
        assert_eq!(op.f, 5);
    }

    #[test]
    fn engine_reports_supported_locales() {
        let en = build_en();
        let fr = build_fr();
        let e = PluralEngine::new(vec![
            PluralPack::from_scud_bytes(&en).unwrap(),
            PluralPack::from_scud_bytes(&fr).unwrap(),
        ]);
        assert!(e.supports("en"));
        assert!(e.supports("fr"));
        assert!(e.supports("en-US"));
        assert!(e.supports("fr-CA"));
        assert!(!e.supports("de"));
    }

    #[test]
    fn non_finite_returns_other() {
        let bytes = build_en();
        let pack = PluralPack::from_scud_bytes(&bytes).unwrap();
        let e = PluralEngine::new(vec![pack]);
        assert_eq!(e.plural_cardinal(f64::NAN, "en"), PluralCategory::Other);
        assert_eq!(
            e.plural_cardinal(f64::INFINITY, "en"),
            PluralCategory::Other
        );
    }

    #[test]
    fn scud_file_wrongly_typed_rejected() {
        let w = ScudWriter::new(*b"CASE", "44.1", Some("en"));
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(PluralPack::new(file).is_err());
    }

    #[test]
    fn negative_numbers_treated_as_absolute_value() {
        let bytes = build_en();
        let pack = PluralPack::from_scud_bytes(&bytes).unwrap();
        let e = PluralEngine::new(vec![pack]);
        // -1 counts as one because CLDR n is absolute.
        assert_eq!(e.plural_cardinal(-1.0, "en"), PluralCategory::One);
    }

    #[test]
    fn fallback_chain_walks_correctly() {
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("pt-BR").collect();
        assert_eq!(chain, ["pt-BR", "pt", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("en").collect();
        assert_eq!(chain, ["en", ""]);
    }
}
