//! WIT-i18n number-formatting SCUD pack for Japanese.
//!
//! Exposes the compiled `number-ja.scud` blob ([`NUMBER_JA_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! # Coverage
//!
//! * **Decimal** — group `,`, decimal `.`, 0-3 fraction digits
//!   (CLDR pattern `#,##0.###`).
//! * **Currency** — JPY `¥`, USD `US$`, EUR `€`, CNY `CN¥` all
//!   placed before the value with **no space** (`¥1,234.56`, CLDR
//!   pattern `¤#,##0` for JPY specifically — see the note on
//!   per-currency fraction digits below).
//! * **Percent** — symbol `%` after the value with no space
//!   (`50%`, CLDR pattern `#,##0%`).
//!
//! # Yen fraction digits
//!
//! CLDR ships JPY with **zero** fraction digits (the yen has no
//! sub-unit). The Phase 3 [`NumberEngine::format_currency`] path
//! forces 2 fraction digits by default regardless of the currency
//! code; callers who want the culturally-correct `¥1234` (no
//! fraction) must explicitly pass
//! [`FormattingOptions`] with `min_fraction = Some(0)` and
//! `max_fraction = Some(0)`. Per-currency fraction-digit defaults
//! are a deferred follow-up.
//!
//! [`NumberEngine::format_currency`]:
//!     stringcheese_icu_number::NumberEngine::format_currency
//! [`FormattingOptions`]: stringcheese_icu_number::FormattingOptions

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for Japanese.
pub const NUMBER_JA_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-ja.scud"));

/// Wrap [`NUMBER_JA_SCUD`] as a [`NumberPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_JA_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ja";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "ja");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "ja", FormattingOptions::default())
                .unwrap(),
            "1,234.5"
        );
        assert_eq!(
            e.format_currency(1234.56, "JPY", "ja", FormattingOptions::default())
                .unwrap(),
            "\u{00A5}1,234.56"
        );
        assert_eq!(
            e.format_percent(0.5, "ja", FormattingOptions::default())
                .unwrap(),
            "50%"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_JA_SCUD.len() < 1024,
            "number-ja.scud grew unexpectedly: {} bytes",
            NUMBER_JA_SCUD.len()
        );
    }
}
