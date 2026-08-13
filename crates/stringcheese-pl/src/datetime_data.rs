//! WIT-i18n date/time-formatting SCUD pack for Polish.
//!
//! Exposes the compiled `datetime-pl.scud` blob
//! ([`DATETIME_PL_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Polish and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `dd.MM.y`, medium `d MMM y`, long
//!   `d MMMM y`, full `EEEE, d MMMM y`.
//! * **Time patterns** — 24-hour throughout: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month + weekday names** — CLDR `format` (genitive) context
//!   for months (`stycznia`, `lutego`, …); Polish weekdays
//!   include the ł/ś diacritics in `poniedziałek` / `środa`.
//! * **Era names** — `p.n.e.` (BC), `n.e.` (AD).

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Polish.
pub const DATETIME_PL_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-pl.scud"));

/// Wrap [`DATETIME_PL_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_PL_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "pl";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "pl");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_short() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "pl", DateTimeLength::Short)
                .unwrap(),
            "22.09.2024"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_PL_SCUD.len() < 1024,
            "datetime-pl.scud grew unexpectedly: {} bytes",
            DATETIME_PL_SCUD.len()
        );
    }
}
