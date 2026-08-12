//! WIT-i18n number-formatting SCUD pack for Chinese (Simplified).
//!
//! Exposes the compiled `number-zh.scud` blob ([`NUMBER_ZH_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! # Coverage
//!
//! * **Decimal** — group `,`, decimal `.`, 0-3 fraction digits
//!   (CLDR pattern `#,##0.###`).
//! * **Currency** — CNY `¥`, USD `US$`, EUR `€`, HKD `HK$` all
//!   placed before the value with **no space** (`¥1,234.56`, CLDR
//!   pattern `¤#,##0.00`).
//! * **Percent** — symbol `%` after the value with no space
//!   (`50%`, CLDR pattern `#,##0%`).

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for Chinese.
pub const NUMBER_ZH_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-zh.scud"));

/// Wrap [`NUMBER_ZH_SCUD`] as a [`NumberPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_ZH_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "zh";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "zh");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "zh", FormattingOptions::default())
                .unwrap(),
            "1,234.5"
        );
        assert_eq!(
            e.format_currency(1234.56, "CNY", "zh", FormattingOptions::default())
                .unwrap(),
            "\u{00A5}1,234.56"
        );
        assert_eq!(
            e.format_percent(0.5, "zh", FormattingOptions::default())
                .unwrap(),
            "50%"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_ZH_SCUD.len() < 1024,
            "number-zh.scud grew unexpectedly: {} bytes",
            NUMBER_ZH_SCUD.len()
        );
    }
}
