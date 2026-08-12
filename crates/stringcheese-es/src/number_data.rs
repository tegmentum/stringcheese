//! WIT-i18n number-formatting SCUD pack for Spanish.
//!
//! Exposes the compiled `number-es.scud` blob ([`NUMBER_ES_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44 `es.xml`
//! (Spain / `es-ES` conventions) and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.3 for the
//! Phase 3 delivery notes.
//!
//! # Coverage
//!
//! * **Decimal** — group `.` (period), decimal `,` (comma), 0-3
//!   fraction digits (CLDR pattern `#,##0.###`). Spain convention;
//!   Latin American variants like `es-MX` differ (group `,`, decimal
//!   `.`) and are documented deferrals.
//! * **Currency** — EUR `€`, USD `$`, GBP `£`, MXN `MX$` placed
//!   after the value with a space (`1.234,56 €`, CLDR pattern
//!   `#,##0.00 ¤`). MXN included for Latin-American relevance.
//! * **Percent** — symbol `%` after the value with a space
//!   (`50 %`, CLDR pattern `#,##0 %`).

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for Spanish.
pub const NUMBER_ES_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-es.scud"));

/// Wrap [`NUMBER_ES_SCUD`] as a [`NumberPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build artifact.
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_ES_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "es";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "es");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "es", FormattingOptions::default())
                .unwrap(),
            "1.234,5"
        );
        assert_eq!(
            e.format_currency(1234.56, "EUR", "es", FormattingOptions::default())
                .unwrap(),
            "1.234,56 \u{20AC}"
        );
        assert_eq!(
            e.format_percent(0.5, "es", FormattingOptions::default())
                .unwrap(),
            "50 %"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_ES_SCUD.len() < 1024,
            "number-es.scud grew unexpectedly: {} bytes",
            NUMBER_ES_SCUD.len()
        );
    }
}
