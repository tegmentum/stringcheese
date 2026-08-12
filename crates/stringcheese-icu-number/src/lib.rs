//! Number-formatting capability for the StringCheese ICU-alternative
//! subsystem.
//!
//! Formats an `f64` value as a locale-sensitive decimal, currency,
//! or percent string, using CLDR default patterns supplied through
//! one or more `stringcheese-scud` number packs. Exposes the result
//! through the `tegmentum:i18n-number@0.1.0` WIT world.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 3 of the WIT-i18n design (`docs/design/wit-i18n.md` §
//! 8.3) — the fourth capability delivered on top of the shared
//! `stringcheese-scud` loader, alongside `stringcheese-icu-plural`.
//! Phase 3 covers the CLDR *default* decimal, currency, and
//! percent patterns for ~10-12 locales; the full CLDR pattern
//! grammar (compact / scientific / accounting) is deferred.
//!
//! # WIT surface
//!
//! The WIT file at `component/wit/number/stringcheese-icu-number.wit`
//! defines three exports on the `number-world` world:
//!
//! * `format-decimal(value, locale, options)` — locale-sensitive
//!   decimal formatting.
//! * `format-currency(value, currency, locale, options)` — currency
//!   formatting.
//! * `format-percent(value, locale, options)` — percent formatting.
//!
//! A [`NumberEngine`] implements every export on the Rust side; a
//! future `wit-component`-gated `Guest` implementation lands in a
//! follow-up wave.
//!
//! # Phase 3 deferrals
//!
//! * **Standalone WASM component build.** The WIT interface parses
//!   cleanly under `wit-parser`; the `wit-bindgen` `Guest`
//!   implementation and `cargo build --target wasm32-wasip1
//!   --features wit-component` recipe land in a follow-up wave.
//! * **Compact notation** (`"1.2K"`, `"3.5M"`) — CLDR "short"
//!   patterns. Follow-up.
//! * **Scientific notation** — follow-up.
//! * **Accounting-style currency** (parentheses for negatives) —
//!   follow-up.
//! * **Named-currency database** (ISO 4217 → symbol lookup for
//!   every locale) — this crate reads whatever currencies the pack
//!   ships; a shipping-locale-by-locale currency-symbol lookup
//!   crate lives separately.
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
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use stringcheese_scud::{NumberDataView, ScudFile};

pub use stringcheese_scud::{CurrencyRecord, DecimalPattern, PercentPattern, ScudError};

/// Per-call formatting overrides for a [`NumberEngine`] query.
///
/// Every field is `Option<T>`; an absent field defers to the pack's
/// default. Mirrors the WIT `formatting-options` record.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FormattingOptions {
    /// Minimum fraction digits to render.
    pub min_fraction: Option<u8>,
    /// Maximum fraction digits to render.
    pub max_fraction: Option<u8>,
    /// Whether to use grouping (thousands) separators.
    pub use_grouping: Option<bool>,
}

/// Typed failure modes of the number engine. Mirrors the WIT
/// `number-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberError {
    /// The locale tag was not a well-formed BCP 47 tag.
    InvalidLocale(&'static str),
    /// No pack was loaded for the requested locale.
    LocaleUnavailable(&'static str),
    /// The requested currency code is not covered by the pack.
    UnknownCurrency(&'static str),
    /// The input was non-finite (`NaN`, `+Inf`, `-Inf`).
    NonFinite,
}

/// A loaded number-formatting pack for one BCP 47 locale.
///
/// Wraps a validated [`ScudFile`] whose capability tag is
/// [`stringcheese_scud::CAP_NUMBER`].
#[derive(Debug, Clone, Copy)]
pub struct NumberPack<'a> {
    scud: ScudFile<'a>,
    locale: &'a str,
    data: NumberDataView<'a>,
}

impl<'a> NumberPack<'a> {
    /// Wrap a validated [`ScudFile`] as a number pack.
    pub fn new(scud: ScudFile<'a>) -> Result<Self, ScudError> {
        let data = scud.as_number_data()?;
        let locale = scud.locale().unwrap_or("");
        Ok(Self { scud, locale, data })
    }

    /// Parse `bytes` as a SCUD file and wrap it as a number pack.
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

    /// The zero-copy number-formatting data view.
    #[must_use]
    pub fn data(&self) -> &NumberDataView<'a> {
        &self.data
    }
}

/// Locale-sensitive number-formatting engine.
///
/// Holds a list of [`NumberPack`]s and consults them at query time
/// in BCP 47 fallback order.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct NumberEngine<'a> {
    packs: Vec<NumberPack<'a>>,
}

#[cfg(feature = "alloc")]
impl<'a> NumberEngine<'a> {
    /// Construct a fresh engine backed by the given packs.
    #[must_use]
    pub fn new(packs: Vec<NumberPack<'a>>) -> Self {
        Self { packs }
    }

    /// Every BCP 47 locale tag this engine knows about.
    #[must_use]
    pub fn supported_locales(&self) -> Vec<&'a str> {
        self.packs.iter().map(NumberPack::locale).collect()
    }

    /// True iff a query in the given locale would use a pack
    /// (rather than falling through to root).
    #[must_use]
    pub fn supports(&self, locale: &str) -> bool {
        walk_fallback_chain(locale).any(|tag| self.pack_for(tag).is_some())
    }

    /// Format `value` as a decimal number under the given locale.
    ///
    /// # Errors
    ///
    /// * [`NumberError::LocaleUnavailable`] if no pack covers the
    ///   requested locale (and no fallback matches).
    /// * [`NumberError::NonFinite`] if `value` is `NaN`, `+Inf`, or
    ///   `-Inf`.
    pub fn format_decimal(
        &self,
        value: f64,
        locale: &str,
        options: FormattingOptions,
    ) -> Result<String, NumberError> {
        if !value.is_finite() {
            return Err(NumberError::NonFinite);
        }
        let pack = self
            .active_pack(locale)
            .ok_or(NumberError::LocaleUnavailable(""))?;
        let pattern = pack
            .data
            .decimal_pattern()
            .ok_or(NumberError::LocaleUnavailable(""))?;
        Ok(format_decimal_with(value, &pattern, options))
    }

    /// Format `value` as a currency amount under the given locale.
    ///
    /// # Errors
    ///
    /// Same set as [`format_decimal`](Self::format_decimal), plus
    /// [`NumberError::UnknownCurrency`] if the pack does not
    /// include the requested ISO 4217 code.
    ///
    /// # Negative-value composition
    ///
    /// Negative currencies place the sign *outside* the symbol —
    /// `-$1.00` rather than `$-1.00` — matching CLDR's default
    /// pattern `-¤#,##0.00`. Format the absolute value first, then
    /// compose sign + (symbol + body).
    pub fn format_currency(
        &self,
        value: f64,
        currency: &str,
        locale: &str,
        options: FormattingOptions,
    ) -> Result<String, NumberError> {
        if !value.is_finite() {
            return Err(NumberError::NonFinite);
        }
        let pack = self
            .active_pack(locale)
            .ok_or(NumberError::LocaleUnavailable(""))?;
        let pattern = pack
            .data
            .decimal_pattern()
            .ok_or(NumberError::LocaleUnavailable(""))?;
        let record = pack
            .data
            .currency(currency)
            .ok_or(NumberError::UnknownCurrency(""))?;

        // Currency default: 2 fraction digits when the caller did
        // not override.
        let cur_opts = FormattingOptions {
            min_fraction: Some(options.min_fraction.unwrap_or(2)),
            max_fraction: Some(options.max_fraction.unwrap_or(2)),
            use_grouping: options.use_grouping,
        };
        let negative = value.is_sign_negative() && value != 0.0;
        let body = format_decimal_with(value.abs(), &pattern, cur_opts);
        let composed = compose_symbol(
            &body,
            record.symbol,
            record.symbol_after,
            record.symbol_spaced,
        );
        if negative {
            let mut out = String::with_capacity(composed.len() + 1);
            out.push('-');
            out.push_str(&composed);
            Ok(out)
        } else {
            Ok(composed)
        }
    }

    /// Format `value` as a percentage under the given locale.
    ///
    /// `value` is the fraction (`0.5` → `"50%"`).
    ///
    /// # Errors
    ///
    /// Same set as [`format_decimal`](Self::format_decimal).
    pub fn format_percent(
        &self,
        value: f64,
        locale: &str,
        options: FormattingOptions,
    ) -> Result<String, NumberError> {
        if !value.is_finite() {
            return Err(NumberError::NonFinite);
        }
        let pack = self
            .active_pack(locale)
            .ok_or(NumberError::LocaleUnavailable(""))?;
        let pattern = pack
            .data
            .decimal_pattern()
            .ok_or(NumberError::LocaleUnavailable(""))?;
        let percent = pack
            .data
            .percent_pattern()
            .ok_or(NumberError::LocaleUnavailable(""))?;
        // Percent default: 0 fraction digits when the caller did
        // not override.
        let pct_opts = FormattingOptions {
            min_fraction: Some(options.min_fraction.unwrap_or(0)),
            max_fraction: Some(options.max_fraction.unwrap_or(0)),
            use_grouping: options.use_grouping,
        };
        let body = format_decimal_with(value * 100.0, &pattern, pct_opts);
        Ok(compose_symbol(
            &body,
            percent.symbol,
            percent.symbol_after,
            percent.symbol_spaced,
        ))
    }

    /// The pack that would service a query under `locale`, if any.
    #[must_use]
    pub fn active_pack(&self, locale: &str) -> Option<&NumberPack<'a>> {
        walk_fallback_chain(locale).find_map(|tag| self.pack_for(tag))
    }

    fn pack_for(&self, tag: &str) -> Option<&NumberPack<'a>> {
        self.packs
            .iter()
            .find(|p| p.locale.eq_ignore_ascii_case(tag))
    }
}

/// Format `value` against a decoded decimal pattern.
///
/// Kept as a free function so it can be unit-tested without going
/// through the engine.
///
/// # Rounding
///
/// Rounds to `max_fraction` digits using banker's rounding (`f64::
/// round_ties_even`) — the default behaviour of Rust's `{:.N}`
/// formatter, chosen for consistency with the CLDR reference
/// implementations that also default to half-even.
#[cfg(feature = "alloc")]
#[must_use]
pub fn format_decimal_with(
    value: f64,
    pattern: &DecimalPattern<'_>,
    options: FormattingOptions,
) -> String {
    let min_frac = usize::from(options.min_fraction.unwrap_or(pattern.min_fraction));
    let max_frac = usize::from(options.max_fraction.unwrap_or(pattern.max_fraction));
    let use_grouping = options.use_grouping.unwrap_or(true);

    let negative = value.is_sign_negative() && value != 0.0;
    let abs = value.abs();

    // Round to max_frac digits using `{:.N}` (half-to-even).
    let rounded = alloc::format!("{abs:.max_frac$}");
    let (int_part, mut frac_part) = match rounded.find('.') {
        Some(dot) => (rounded[..dot].to_string(), rounded[dot + 1..].to_string()),
        None => (rounded, String::new()),
    };

    // Trim trailing zeros down to min_frac.
    while frac_part.len() > min_frac
        && frac_part
            .as_bytes()
            .last()
            .copied()
            .is_some_and(|c| c == b'0')
    {
        frac_part.pop();
    }

    // Group the integer part.
    let grouped_int = if use_grouping {
        group_integer_part(
            &int_part,
            pattern.group_separator,
            pattern.primary_grouping,
            pattern.secondary_grouping,
        )
    } else {
        int_part
    };

    let mut out = String::with_capacity(grouped_int.len() + frac_part.len() + 4);
    if negative {
        out.push('-');
    }
    out.push_str(&grouped_int);
    if !frac_part.is_empty() {
        out.push_str(pattern.decimal_separator);
        out.push_str(&frac_part);
    }
    out
}

#[cfg(feature = "alloc")]
fn group_integer_part(int_digits: &str, separator: &str, primary: u8, secondary: u8) -> String {
    let primary = usize::from(primary).max(1);
    let secondary = usize::from(secondary).max(1);
    // Reverse-walk the integer digits, inserting a separator every
    // `primary` digits and then every `secondary` digits.
    let digits: Vec<char> = int_digits.chars().collect();
    let mut out_rev: Vec<char> = Vec::with_capacity(digits.len() + digits.len() / primary);
    let mut consumed = 0usize;
    let mut group_size = primary;
    for (idx, ch) in digits.iter().rev().enumerate() {
        if idx > 0 && consumed == group_size {
            for c in separator.chars().rev() {
                out_rev.push(c);
            }
            consumed = 0;
            group_size = secondary;
        }
        out_rev.push(*ch);
        consumed += 1;
    }
    out_rev.into_iter().rev().collect()
}

#[cfg(feature = "alloc")]
fn compose_symbol(body: &str, symbol: &str, after: bool, spaced: bool) -> String {
    let mut out = String::with_capacity(body.len() + symbol.len() + 1);
    if after {
        out.push_str(body);
        if spaced {
            // U+00A0 non-breaking space would be a more principled
            // choice; CLDR default patterns spell it as ASCII space
            // in the pattern string, so we match that.
            out.push(' ');
        }
        out.push_str(symbol);
    } else {
        out.push_str(symbol);
        if spaced {
            out.push(' ');
        }
        out.push_str(body);
    }
    out
}

/// Walk the CLDR-defined fallback chain for a BCP 47 tag.
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
        CAP_NUMBER, NumberSectionBuilder, SECT_CURRENCY_TABLE, SECT_DECIMAL_PATTERN,
        SECT_PERCENT_PATTERN, ScudFile, ScudWriter,
    };

    fn build_en() -> alloc::vec::Vec<u8> {
        let mut n = NumberSectionBuilder::new();
        // en: comma grouping, dot decimal, 0-3 fraction digits.
        n.set_decimal_pattern(",", ".", 0, 3, 3, 3);
        n.push_currency("USD", "$", false, false);
        n.push_currency("EUR", "\u{20AC}", false, false);
        n.push_currency("GBP", "\u{00A3}", false, false);
        n.set_percent("%", true, false);
        let mut w = ScudWriter::new(CAP_NUMBER, "44.1", Some("en"));
        w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
        w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
        w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
        w.finish()
    }

    fn build_de() -> alloc::vec::Vec<u8> {
        let mut n = NumberSectionBuilder::new();
        // de: dot grouping, comma decimal.
        n.set_decimal_pattern(".", ",", 0, 3, 3, 3);
        n.push_currency("USD", "$", true, true);
        n.push_currency("EUR", "\u{20AC}", true, true);
        n.set_percent("%", true, true);
        let mut w = ScudWriter::new(CAP_NUMBER, "44.1", Some("de"));
        w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
        w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
        w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
        w.finish()
    }

    fn build_fr() -> alloc::vec::Vec<u8> {
        let mut n = NumberSectionBuilder::new();
        // fr: NBSP grouping, comma decimal. Use ASCII space here for
        // simple string equality — the real CLDR spec uses NARROW NO-BREAK
        // SPACE (U+202F) as of CLDR 42; we use a plain space in the test
        // pack to keep golden vectors readable.
        n.set_decimal_pattern(" ", ",", 0, 3, 3, 3);
        n.push_currency("EUR", "\u{20AC}", true, true);
        n.push_currency("USD", "$", true, true);
        n.set_percent("%", true, true);
        let mut w = ScudWriter::new(CAP_NUMBER, "44.1", Some("fr"));
        w.append_section(SECT_DECIMAL_PATTERN, &n.decimal_bytes());
        w.append_section(SECT_CURRENCY_TABLE, &n.currency_bytes());
        w.append_section(SECT_PERCENT_PATTERN, &n.percent_bytes());
        w.finish()
    }

    #[test]
    fn english_decimal_grouping() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_decimal(1234.5, "en", FormattingOptions::default())
                .unwrap(),
            "1,234.5"
        );
        assert_eq!(
            e.format_decimal(1_000_000.0, "en", FormattingOptions::default())
                .unwrap(),
            "1,000,000"
        );
        assert_eq!(
            e.format_decimal(0.0, "en", FormattingOptions::default())
                .unwrap(),
            "0"
        );
    }

    #[test]
    fn german_decimal_grouping() {
        let bytes = build_de();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_decimal(1234.5, "de", FormattingOptions::default())
                .unwrap(),
            "1.234,5"
        );
        assert_eq!(
            e.format_decimal(1_000_000.0, "de", FormattingOptions::default())
                .unwrap(),
            "1.000.000"
        );
    }

    #[test]
    fn french_decimal_grouping() {
        let bytes = build_fr();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_decimal(1234.5, "fr", FormattingOptions::default())
                .unwrap(),
            "1 234,5"
        );
    }

    #[test]
    fn english_currency_placement() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_currency(1234.56, "USD", "en", FormattingOptions::default())
                .unwrap(),
            "$1,234.56"
        );
    }

    #[test]
    fn german_currency_placement() {
        let bytes = build_de();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_currency(1234.56, "EUR", "de", FormattingOptions::default())
                .unwrap(),
            "1.234,56 \u{20AC}"
        );
    }

    #[test]
    fn english_percent() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_percent(0.5, "en", FormattingOptions::default())
                .unwrap(),
            "50%"
        );
        // 12.5 → "12%" under banker's (half-to-even) rounding: 12
        // is even and closer than 14, so ties round to it.
        assert_eq!(
            e.format_percent(0.125, "en", FormattingOptions::default())
                .unwrap(),
            "12%"
        );
        // 13.5 → "14%" — 14 is the even neighbour.
        assert_eq!(
            e.format_percent(0.135, "en", FormattingOptions::default())
                .unwrap(),
            "14%"
        );
    }

    #[test]
    fn german_percent_has_space() {
        let bytes = build_de();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_percent(0.5, "de", FormattingOptions::default())
                .unwrap(),
            "50 %"
        );
    }

    #[test]
    fn negative_decimals() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert_eq!(
            e.format_decimal(-1234.5, "en", FormattingOptions::default())
                .unwrap(),
            "-1,234.5"
        );
    }

    #[test]
    fn options_override_fraction_digits() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        let out = e
            .format_decimal(
                1.0,
                "en",
                FormattingOptions {
                    min_fraction: Some(2),
                    max_fraction: Some(2),
                    use_grouping: None,
                },
            )
            .unwrap();
        assert_eq!(out, "1.00");
    }

    #[test]
    fn no_grouping_option() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        let out = e
            .format_decimal(
                1_234_567.0,
                "en",
                FormattingOptions {
                    min_fraction: None,
                    max_fraction: None,
                    use_grouping: Some(false),
                },
            )
            .unwrap();
        assert_eq!(out, "1234567");
    }

    #[test]
    fn unknown_currency_errors() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert!(matches!(
            e.format_currency(1.0, "XYZ", "en", FormattingOptions::default()),
            Err(NumberError::UnknownCurrency(_))
        ));
    }

    #[test]
    fn non_finite_errors() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert!(matches!(
            e.format_decimal(f64::NAN, "en", FormattingOptions::default()),
            Err(NumberError::NonFinite)
        ));
        assert!(matches!(
            e.format_decimal(f64::INFINITY, "en", FormattingOptions::default()),
            Err(NumberError::NonFinite)
        ));
    }

    #[test]
    fn unknown_locale_errors() {
        let bytes = build_en();
        let pack = NumberPack::from_scud_bytes(&bytes).unwrap();
        let e = NumberEngine::new(vec![pack]);
        assert!(matches!(
            e.format_decimal(1.0, "xx", FormattingOptions::default()),
            Err(NumberError::LocaleUnavailable(_))
        ));
    }

    #[test]
    fn scud_file_wrongly_typed_rejected() {
        let w = ScudWriter::new(*b"CASE", "44.1", Some("en"));
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(NumberPack::new(file).is_err());
    }

    #[test]
    fn engine_reports_supported_locales() {
        let en = build_en();
        let de = build_de();
        let e = NumberEngine::new(vec![
            NumberPack::from_scud_bytes(&en).unwrap(),
            NumberPack::from_scud_bytes(&de).unwrap(),
        ]);
        assert_eq!(e.supported_locales(), vec!["en", "de"]);
        assert!(e.supports("en"));
        assert!(e.supports("en-US"));
        assert!(e.supports("de-DE"));
        assert!(!e.supports("xx"));
    }
}
