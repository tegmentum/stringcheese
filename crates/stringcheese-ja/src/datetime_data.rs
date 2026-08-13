//! WIT-i18n date/time-formatting SCUD pack for Japanese.
//!
//! Exposes the compiled `datetime-ja.scud` blob
//! ([`DATETIME_JA_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Japanese and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short/medium `y/MM/dd`, long
//!   `y年M月d日`, full `y年M月d日EEEE`.
//! * **Time patterns** — 24-hour throughout: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month names** — numeric-with-suffix `1月`..`12月`.
//! * **Weekday names** — `日曜日` (Sunday-first) through `土曜日`.
//! * **Era names** — `紀元前` (BC), `西暦` (AD). The Japanese
//!   Imperial calendar (Reiwa / Heisei / …) is a documented
//!   follow-up.

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Japanese.
pub const DATETIME_JA_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-ja.scud"));

/// Wrap [`DATETIME_JA_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_JA_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ja";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "ja");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_short() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "ja", DateTimeLength::Short)
                .unwrap(),
            "2024/09/22"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_JA_SCUD.len() < 1024,
            "datetime-ja.scud grew unexpectedly: {} bytes",
            DATETIME_JA_SCUD.len()
        );
    }
}
