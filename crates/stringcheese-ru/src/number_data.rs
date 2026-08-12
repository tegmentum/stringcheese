//! WIT-i18n number-formatting SCUD pack for Russian.
//!
//! Exposes the compiled `number-ru.scud` blob ([`NUMBER_RU_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44 `ru.xml`
//! and embedded here via `include_bytes!`. See
//! `docs/design/wit-i18n.md` § 8.3 for the Phase 3 delivery notes.
//!
//! # Coverage
//!
//! * **Decimal** — group U+00A0 (NBSP), decimal `,`, 0-3 fraction
//!   digits (CLDR pattern `#,##0.###`).
//! * **Currency** — RUB `₽`, USD `$`, EUR `€`, GBP `£` placed after
//!   the value with a space (`1 234,56 ₽`, CLDR pattern
//!   `#,##0.00 ¤`).
//! * **Percent** — symbol `%` after the value with a space
//!   (`50 %`, CLDR pattern `#,##0 %`).

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for Russian.
pub const NUMBER_RU_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-ru.scud"));

/// Wrap [`NUMBER_RU_SCUD`] as a [`NumberPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build artifact.
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_RU_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ru";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "ru");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "ru", FormattingOptions::default())
                .unwrap(),
            "1\u{00A0}234,5"
        );
        assert_eq!(
            e.format_currency(1234.56, "RUB", "ru", FormattingOptions::default())
                .unwrap(),
            "1\u{00A0}234,56 \u{20BD}"
        );
        assert_eq!(
            e.format_percent(0.5, "ru", FormattingOptions::default())
                .unwrap(),
            "50 %"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_RU_SCUD.len() < 1024,
            "number-ru.scud grew unexpectedly: {} bytes",
            NUMBER_RU_SCUD.len()
        );
    }
}
