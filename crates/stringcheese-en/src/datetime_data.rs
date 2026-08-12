//! WIT-i18n date/time-formatting SCUD pack for English.
//!
//! Exposes the compiled `datetime-en.scud` blob
//! ([`DATETIME_EN_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`] ready
//! to hand to a [`stringcheese_icu_datetime::DateTimeEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for English and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `M/d/y`, medium `MMM d, y`, long
//!   `MMMM d, y`, full `EEEE, MMMM d, y`.
//! * **Time patterns** — short `h:mm a`, medium/long/full
//!   `h:mm:ss a`.
//! * **Month + weekday names** — CLDR default (English).
//! * **AM/PM markers** — `AM`, `PM`.
//! * **Era names** — `BC`, `AD`.

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for English.
pub const DATETIME_EN_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-en.scud"));

/// Wrap [`DATETIME_EN_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build
/// artifact.
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_EN_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "en";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "en");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_medium() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "en", DateTimeLength::Medium)
                .unwrap(),
            "Sep 22, 2024"
        );
    }

    #[test]
    fn format_time_short() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_time("17:03:04", "en", DateTimeLength::Short)
                .unwrap(),
            "5:03 PM"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_EN_SCUD.len() < 1024,
            "datetime-en.scud grew unexpectedly: {} bytes",
            DATETIME_EN_SCUD.len()
        );
    }
}
