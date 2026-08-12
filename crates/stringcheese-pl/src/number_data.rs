//! WIT-i18n number-formatting SCUD pack for Polish.
//!
//! Exposes the compiled `number-pl.scud` blob ([`NUMBER_PL_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! # Coverage
//!
//! * **Decimal** — group U+00A0 (NBSP), decimal `,`, 0-3 fraction
//!   digits (CLDR pattern `#,##0.###`).
//! * **Currency** — PLN `zł`, USD `$`, EUR `€`, GBP `£` placed after
//!   the value with a space (`1 234,56 zł`, CLDR pattern
//!   `#,##0.00 ¤`).
//! * **Percent** — symbol `%` after the value with **no space**
//!   (`50%`, CLDR pattern `#,##0%`). Polish is the odd shipped
//!   Phase 3 locale here — every other locale uses a space
//!   separator; Polish does not.

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for Polish.
pub const NUMBER_PL_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-pl.scud"));

/// Wrap [`NUMBER_PL_SCUD`] as a [`NumberPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_PL_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "pl";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "pl");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "pl", FormattingOptions::default())
                .unwrap(),
            "1\u{00A0}234,5"
        );
        assert_eq!(
            e.format_currency(1234.56, "PLN", "pl", FormattingOptions::default())
                .unwrap(),
            "1\u{00A0}234,56 z\u{0142}"
        );
        assert_eq!(
            e.format_percent(0.5, "pl", FormattingOptions::default())
                .unwrap(),
            "50%"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_PL_SCUD.len() < 1024,
            "number-pl.scud grew unexpectedly: {} bytes",
            NUMBER_PL_SCUD.len()
        );
    }
}
