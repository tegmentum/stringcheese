//! WIT-i18n number-formatting SCUD pack for English.
//!
//! Exposes the compiled `number-en.scud` blob ([`NUMBER_EN_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1 `en.xml`
//! and embedded here via `include_bytes!`. See
//! `docs/design/wit-i18n.md` § 8.3 for the Phase 3 delivery notes.
//!
//! # Coverage
//!
//! * **Decimal** — group `,`, decimal `.`, 0-3 fraction digits
//!   (CLDR default `#,##0.###`).
//! * **Currency** — USD `$`, EUR `€`, GBP `£`, JPY `¥`, CAD `CA$`,
//!   AUD `A$` all placed before the value (`$1.00`).
//! * **Percent** — symbol `%` after the value with no space
//!   (`50%`).

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for English.
pub const NUMBER_EN_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-en.scud"));

/// Wrap [`NUMBER_EN_SCUD`] as a [`NumberPack`].
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_EN_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "en";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "en");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "en", FormattingOptions::default())
                .unwrap(),
            "1,234.5"
        );
        assert_eq!(
            e.format_currency(1234.56, "USD", "en", FormattingOptions::default())
                .unwrap(),
            "$1,234.56"
        );
        assert_eq!(
            e.format_percent(0.5, "en", FormattingOptions::default())
                .unwrap(),
            "50%"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_EN_SCUD.len() < 1024,
            "number-en.scud grew unexpectedly: {} bytes",
            NUMBER_EN_SCUD.len()
        );
    }
}
