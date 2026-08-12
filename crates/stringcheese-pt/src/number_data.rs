//! WIT-i18n number-formatting SCUD pack for Portuguese.
//!
//! Exposes the compiled `number-pt.scud` blob ([`NUMBER_PT_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44 `pt.xml`
//! and embedded here via `include_bytes!`. See
//! `docs/design/wit-i18n.md` § 8.3 for the Phase 3 delivery notes.
//!
//! # Coverage
//!
//! * **Decimal** — group `.` (period), decimal `,` (comma), 0-3
//!   fraction digits (CLDR pattern `#,##0.###`). Both pt-PT and
//!   pt-BR share these separator conventions.
//! * **Currency** — EUR `€` (pt-PT), BRL `R$` (pt-BR), USD `US$`,
//!   GBP `£` placed after the value with a space (`1.234,56 €`,
//!   CLDR pattern `#,##0.00 ¤`).
//! * **Percent** — symbol `%` after the value with a space
//!   (`50 %`, CLDR pattern `#,##0 %`).

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for Portuguese.
pub const NUMBER_PT_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-pt.scud"));

/// Wrap [`NUMBER_PT_SCUD`] as a [`NumberPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build artifact.
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_PT_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "pt";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "pt");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_grouping_and_currency() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "pt", FormattingOptions::default())
                .unwrap(),
            "1.234,5"
        );
        assert_eq!(
            e.format_currency(1234.56, "EUR", "pt", FormattingOptions::default())
                .unwrap(),
            "1.234,56 \u{20AC}"
        );
        assert_eq!(
            e.format_currency(1234.56, "BRL", "pt", FormattingOptions::default())
                .unwrap(),
            "1.234,56 R$"
        );
        assert_eq!(
            e.format_percent(0.5, "pt", FormattingOptions::default())
                .unwrap(),
            "50 %"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_PT_SCUD.len() < 1024,
            "number-pt.scud grew unexpectedly: {} bytes",
            NUMBER_PT_SCUD.len()
        );
    }
}
