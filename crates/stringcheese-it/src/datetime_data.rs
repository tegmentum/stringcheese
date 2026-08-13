//! WIT-i18n date/time-formatting SCUD pack for Italian.
//!
//! Exposes the compiled `datetime-it.scud` blob
//! ([`DATETIME_IT_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Italian and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `dd/MM/y`, medium `d MMM y`, long
//!   `d MMMM y`, full `EEEE d MMMM y`.
//! * **Time patterns** — 24-hour: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month + weekday names** — CLDR default (lowercase months
//!   `gennaio`..`dicembre`; weekdays `luned\u{00EC}`..`domenica`).
//! * **Era names** — `a.C.` (BC), `d.C.` (AD).

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Italian.
pub const DATETIME_IT_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-it.scud"));

/// Wrap [`DATETIME_IT_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_IT_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "it";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "it");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_long() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "it", DateTimeLength::Long)
                .unwrap(),
            "22 settembre 2024"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_IT_SCUD.len() < 1024,
            "datetime-it.scud grew unexpectedly: {} bytes",
            DATETIME_IT_SCUD.len()
        );
    }
}
