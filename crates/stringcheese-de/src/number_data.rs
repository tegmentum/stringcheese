//! WIT-i18n number-formatting SCUD pack for German.
//!
//! Exposes the compiled `number-de.scud` blob ([`NUMBER_DE_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! # Coverage
//!
//! * **Decimal** — group `.`, decimal `,`, 0-3 fraction digits.
//! * **Currency** — EUR / USD / GBP / CHF placed after the value
//!   with a space (`1.234,56 €`).
//! * **Percent** — symbol `%` after the value with a space (`50 %`).

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for German.
pub const NUMBER_DE_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-de.scud"));

/// Wrap [`NUMBER_DE_SCUD`] as a [`NumberPack`].
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_DE_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "de";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "de");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "de", FormattingOptions::default())
                .unwrap(),
            "1.234,5"
        );
        assert_eq!(
            e.format_currency(1234.56, "EUR", "de", FormattingOptions::default())
                .unwrap(),
            "1.234,56 \u{20AC}"
        );
        assert_eq!(
            e.format_percent(0.5, "de", FormattingOptions::default())
                .unwrap(),
            "50 %"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_DE_SCUD.len() < 1024,
            "number-de.scud grew unexpectedly: {} bytes",
            NUMBER_DE_SCUD.len()
        );
    }
}
