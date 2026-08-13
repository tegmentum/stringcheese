//! WIT-i18n date/time-formatting SCUD pack for Spanish.
//!
//! Exposes the compiled `datetime-es.scud` blob
//! ([`DATETIME_ES_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Spanish and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `d/M/y`, medium `d MMM y`, long
//!   `d 'de' MMMM 'de' y`, full `EEEE, d 'de' MMMM 'de' y`.
//! * **Time patterns** — 24-hour (Spain default): short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month + weekday names** — CLDR default lowercase form.
//! * **Era names** — `a. C.` (BC), `d. C.` (AD).

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Spanish.
pub const DATETIME_ES_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-es.scud"));

/// Wrap [`DATETIME_ES_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_ES_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "es";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "es");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_long_uses_de_literal() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "es", DateTimeLength::Long)
                .unwrap(),
            "22 de septiembre de 2024"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_ES_SCUD.len() < 1024,
            "datetime-es.scud grew unexpectedly: {} bytes",
            DATETIME_ES_SCUD.len()
        );
    }
}
