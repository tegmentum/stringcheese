//! WIT-i18n date/time-formatting SCUD pack for Russian.
//!
//! Exposes the compiled `datetime-ru.scud` blob
//! ([`DATETIME_RU_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Russian and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `dd.MM.y`, medium `d MMM y г.`,
//!   long `d MMMM y г.`, full `EEEE, d MMMM y г.`. The trailing
//!   `г.` is Russian's era-less year suffix (short for `года`,
//!   "of the year").
//! * **Time patterns** — 24-hour throughout: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month + weekday names** — CLDR `format` (genitive) context
//!   for months (`января`, `февраля`, …); the `stand-alone`
//!   (nominative) variants are a documented follow-up.
//! * **Era names** — `до н. э.` (BC), `н. э.` (AD).

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Russian.
pub const DATETIME_RU_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-ru.scud"));

/// Wrap [`DATETIME_RU_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_RU_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ru";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "ru");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_short() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "ru", DateTimeLength::Short)
                .unwrap(),
            "22.09.2024"
        );
    }

    #[test]
    fn format_time_short_24h() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_time("17:03:04", "ru", DateTimeLength::Short)
                .unwrap(),
            "17:03"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_RU_SCUD.len() < 1024,
            "datetime-ru.scud grew unexpectedly: {} bytes",
            DATETIME_RU_SCUD.len()
        );
    }
}
