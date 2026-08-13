//! WIT-i18n date/time-formatting SCUD pack for Arabic.
//!
//! Exposes the compiled `datetime-ar.scud` blob
//! ([`DATETIME_AR_SCUD`]) plus [`datetime_pack`], a helper that
//! wraps it as a [`stringcheese_icu_datetime::DateTimePack`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `gregorian.json` for Arabic and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.4 for the
//! Phase 4 delivery notes.
//!
//! # Coverage
//!
//! * **Date patterns** — short `d‏/M‏/y`, medium `dd‏/MM‏/y`
//!   (both carry U+200F RTL MARK between numeric fields and the
//!   slash separators), long `d MMMM y`, full `EEEE، d MMMM y`
//!   (Arabic comma U+060C).
//! * **Time patterns** — 12-hour with `ص`/`م` AM/PM markers:
//!   short `h:mm a`, medium/long/full `h:mm:ss a`.
//! * **Month names** — Arabic (`يناير`, `فبراير`, …). CLDR ships
//!   the same strings for full and abbreviated forms.
//! * **Weekday names** — `الأحد` (Sunday-first) through `السبت`.
//! * **Era names** — `ق.م` (BC), `م` (AD).
//!
//! # Deferred: RTL bidi rendering
//!
//! The pattern strings include the RTL marks CLDR ships; the
//! formatter emits them verbatim into the output so a downstream
//! bidi-aware shaper produces the culturally-correct visual
//! result. stringcheese does not itself perform bidi shaping.

use stringcheese_icu_datetime::{DateTimePack, ScudError};

/// The compiled datetime SCUD pack for Arabic.
pub const DATETIME_AR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/datetime-ar.scud"));

/// Wrap [`DATETIME_AR_SCUD`] as a [`DateTimePack`].
///
/// # Errors
///
/// See [`DateTimePack::from_scud_bytes`].
pub fn datetime_pack() -> Result<DateTimePack<'static>, ScudError> {
    DateTimePack::from_scud_bytes(DATETIME_AR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ar";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_datetime::{DateTimeEngine, DateTimeLength};

    #[test]
    fn pack_loads() {
        let pack = datetime_pack().unwrap();
        assert_eq!(pack.locale(), "ar");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn format_time_uses_arabic_am_pm() {
        let e = DateTimeEngine::new(alloc::vec![datetime_pack().unwrap()]);
        assert_eq!(
            e.format_time("09:30:00", "ar", DateTimeLength::Short)
                .unwrap(),
            "9:30 \u{0635}"
        );
        assert_eq!(
            e.format_time("17:03:04", "ar", DateTimeLength::Short)
                .unwrap(),
            "5:03 \u{0645}"
        );
    }

    #[test]
    fn pack_is_small() {
        assert!(
            DATETIME_AR_SCUD.len() < 1024,
            "datetime-ar.scud grew unexpectedly: {} bytes",
            DATETIME_AR_SCUD.len()
        );
    }
}
