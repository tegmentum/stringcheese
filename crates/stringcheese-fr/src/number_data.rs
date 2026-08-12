//! WIT-i18n number-formatting SCUD pack for French.
//!
//! Exposes the compiled `number-fr.scud` blob ([`NUMBER_FR_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! # Coverage
//!
//! * **Decimal** — group NBSP (U+00A0), decimal `,` (comma),
//!   0-3 fraction digits.
//! * **Currency** — EUR / USD / GBP / CAD / CHF placed after the
//!   value with a space (`1 234,56 €`).
//! * **Percent** — symbol `%` after the value with a space
//!   (`50 %`).

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for French.
pub const NUMBER_FR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-fr.scud"));

/// Wrap [`NUMBER_FR_SCUD`] as a [`NumberPack`].
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_FR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "fr";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "fr");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "fr", FormattingOptions::default())
                .unwrap(),
            "1\u{00A0}234,5"
        );
        assert_eq!(
            e.format_currency(1234.56, "EUR", "fr", FormattingOptions::default())
                .unwrap(),
            "1\u{00A0}234,56 \u{20AC}"
        );
        assert_eq!(
            e.format_percent(0.5, "fr", FormattingOptions::default())
                .unwrap(),
            "50 %"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_FR_SCUD.len() < 1024,
            "number-fr.scud grew unexpectedly: {} bytes",
            NUMBER_FR_SCUD.len()
        );
    }
}
