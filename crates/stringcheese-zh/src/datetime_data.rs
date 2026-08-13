//! WIT-i18n date/time-formatting SCUD pack for Chinese (Simplified).
//!
//! Exposes the compiled `datetime-zh.scud` blob
//! ([`DATETIME_ZH_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Chinese and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `y/M/d`, medium/long
//!   `y年M月d日`, full `y年M月d日EEEE`.
//! * **Time patterns** — 24-hour throughout: short `HH:mm`,
//!   medium/long/full `HH:mm:ss`.
//! * **Month names** — wide `一月`..`十二月`, abbreviated
//!   `1月`..`12月`.
//! * **Weekday names** — `星期日` (Sunday-first) through `星期六`.
//! * **Era names** — `公元前` (BC), `公元` (AD).

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Chinese (Simplified).
pub const DATETIME_ZH_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-zh.scud"));

/// Wrap [`DATETIME_ZH_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_ZH_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "zh";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "zh");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_date_medium() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_date("2024-09-22", "zh", DateTimeLength::Medium)
                .unwrap(),
            "2024\u{5E74}9\u{6708}22\u{65E5}"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_ZH_SCUD.len() < 1024,
            "datetime-zh.scud grew unexpectedly: {} bytes",
            DATETIME_ZH_SCUD.len()
        );
    }
}
