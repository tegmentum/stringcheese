//! WIT-i18n number-formatting SCUD pack for Arabic.
//!
//! Exposes the compiled `number-ar.scud` blob ([`NUMBER_AR_SCUD`])
//! plus [`number_pack`], a helper that wraps it as a
//! [`stringcheese_icu_number::NumberPack`] ready to hand to a
//! [`stringcheese_icu_number::NumberEngine`].
//!
//! # Coverage
//!
//! * **Decimal** — group `,`, decimal `.`, 0-3 fraction digits
//!   (CLDR pattern `#,##0.###` under the `latn` numbering system).
//! * **Currency** — SAR `ر.س.` (Saudi riyal), USD `US$`,
//!   EUR `€`, AED `د.إ.` placed **before** the value with a space
//!   (`ر.س. 1,234.56`, CLDR pattern `¤ #,##0.00`).
//! * **Percent** — symbol `%` after the value with **no space**
//!   (`50%`, CLDR pattern `#,##0%`).
//!
//! # Deferred
//!
//! * **Arabic-Indic digits.** The `arab` (U+0660..U+0669) and
//!   `arabext` (U+06F0..U+06F9) numbering systems ship separate
//!   digit shapes; this pack uses the default `latn` (Western
//!   `0..9`) digits. A follow-up SCUD extension will carry
//!   alternate digit tables.
//! * **RTL bidi.** The output preserves logical byte order; a
//!   downstream shaper is responsible for the visual reversal
//!   Arabic-context consumers require.
//! * **Localized percent sign.** U+066A (Arabic percent sign) is
//!   the culturally-correct choice for Arabic contexts; this pack
//!   emits ASCII `%` to match every other shipped locale.

use stringcheese_icu_number::{NumberPack, ScudError};

/// The compiled number-formatting SCUD pack for Arabic.
pub const NUMBER_AR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/number-ar.scud"));

/// Wrap [`NUMBER_AR_SCUD`] as a [`NumberPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn number_pack() -> Result<NumberPack<'static>, ScudError> {
    NumberPack::from_scud_bytes(NUMBER_AR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ar";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_number::{FormattingOptions, NumberEngine};

    #[test]
    fn pack_loads() {
        let pack = number_pack().unwrap();
        assert_eq!(pack.locale(), "ar");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn decimal_uses_latn_digits() {
        let e = NumberEngine::new(alloc::vec![number_pack().unwrap()]);
        assert_eq!(
            e.format_decimal(1234.5, "ar", FormattingOptions::default())
                .unwrap(),
            "1,234.5"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            NUMBER_AR_SCUD.len() < 1024,
            "number-ar.scud grew unexpectedly: {} bytes",
            NUMBER_AR_SCUD.len()
        );
    }
}
