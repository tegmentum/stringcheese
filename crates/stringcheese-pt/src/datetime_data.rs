//! WIT-i18n date/time-formatting SCUD pack for Portuguese.
//!
//! Exposes the compiled `datetime-pt.scud` blob
//! ([`DATETIME_PT_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Portuguese and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `dd/MM/y`, medium
//!   `d 'de' MMM 'de' y`, long `d 'de' MMMM 'de' y`, full
//!   `EEEE, d 'de' MMMM 'de' y`.
//! * **Time patterns** — 24-hour: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month + weekday names** — CLDR default lowercase form
//!   (`janeiro`, `segunda-feira`, …).
//! * **Era names** — `a.C.` (BC), `d.C.` (AD).

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Portuguese.
pub const DATETIME_PT_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-pt.scud"));

/// Wrap [`DATETIME_PT_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_PT_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "pt";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "pt");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_long() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "pt", DateTimeLength::Long)
                .unwrap(),
            "22 de setembro de 2024"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_PT_SCUD.len() < 1024,
            "datetime-pt.scud grew unexpectedly: {} bytes",
            DATETIME_PT_SCUD.len()
        );
    }
}
