//! WIT-i18n date/time-formatting SCUD pack for German.
//!
//! Exposes the compiled `datetime-de.scud` blob
//! ([`DATETIME_DE_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! # Coverage
//!
//! * **Date patterns** — short/medium `dd.MM.y`, long
//!   `d. MMMM y`, full `EEEE, d. MMMM y`.
//! * **Time patterns** — 24-hour throughout: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month + weekday names** — CLDR German (`März`, `Sonntag`, …).
//! * **Era names** — `v. Chr.`, `n. Chr.`.

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for German.
pub const DATETIME_DE_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-de.scud"));

/// Wrap [`DATETIME_DE_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_DE_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "de";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "de");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_short() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "de", DateTimeLength::Short)
                .unwrap(),
            "22.09.2024"
        );
    }

    #[test]
    fn format_time_short_24h() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_time("17:03:04", "de", DateTimeLength::Short)
                .unwrap(),
            "17:03"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_DE_SCUD.len() < 1024,
            "datetime-de.scud grew unexpectedly: {} bytes",
            DATETIME_DE_SCUD.len()
        );
    }
}
