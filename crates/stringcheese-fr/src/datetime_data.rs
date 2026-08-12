//! WIT-i18n date/time-formatting SCUD pack for French.
//!
//! Exposes the compiled `datetime-fr.scud` blob
//! ([`DATETIME_FR_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! # Coverage
//!
//! * **Date patterns** — short `dd/MM/y`, medium `d MMM y`, long
//!   `d MMMM y`, full `EEEE d MMMM y`.
//! * **Time patterns** — 24-hour throughout: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month + weekday names** — CLDR French (all lowercase per
//!   the CLDR data: `janvier`, `dimanche`, …).
//! * **Era names** — `av. J.-C.`, `ap. J.-C.`.

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for French.
pub const DATETIME_FR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-fr.scud"));

/// Wrap [`DATETIME_FR_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_FR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "fr";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "fr");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_medium() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "fr", DateTimeLength::Medium)
                .unwrap(),
            "22 sept. 2024"
        );
    }

    #[test]
    fn format_date_full_uses_lowercase_weekday() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "fr", DateTimeLength::Full)
                .unwrap(),
            "dimanche 22 septembre 2024"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_FR_SCUD.len() < 1024,
            "datetime-fr.scud grew unexpectedly: {} bytes",
            DATETIME_FR_SCUD.len()
        );
    }
}
