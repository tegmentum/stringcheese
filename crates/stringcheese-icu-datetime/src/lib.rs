//! Date/time-formatting capability for the StringCheese
//! ICU-alternative subsystem.
//!
//! Renders Proleptic-Gregorian dates, times, and combined date-times
//! against CLDR short/medium/long/full patterns using per-locale data
//! supplied through one or more `stringcheese-scud` datetime packs,
//! and exposes the result through the
//! `tegmentum:i18n-datetime@0.1.0` WIT world. Callers construct a
//! [`DateTimeEngine`] from a slice of loaded [`DateTimePack`]s and
//! issue [`format_date`](DateTimeEngine::format_date) /
//! [`format_time`](DateTimeEngine::format_time) /
//! [`format_datetime`](DateTimeEngine::format_datetime) queries; the
//! engine walks the BCP 47 fallback chain (`fr-CA → fr → ""`) at
//! query time and interprets the pack-supplied pattern token-by-token.
//!
//! # Position in the WIT-i18n subsystem
//!
//! Phase 4 of the WIT-i18n design (`docs/design/wit-i18n.md` § 8.4)
//! — the fourth capability delivered on top of the shared
//! `stringcheese-scud` loader (Phase 1) after case-mapping (Phase 1),
//! collation (Phase 2), and plural + number (Phase 3). Phase 4 covers
//! the Gregorian calendar for `en` / `de` / `fr` (matching Phase 3's
//! initial locale set); other calendars, skeleton-based formatting,
//! relative time, and interval formatting are deferred.
//!
//! # WIT surface
//!
//! The WIT file at `component/wit/datetime/stringcheese-icu-datetime.wit`
//! defines four exports on the `datetime-world` world:
//!
//! * `format-date(iso-date, locale, length)` — locale-sensitive
//!   date formatting.
//! * `format-time(iso-time, locale, length)` — time-of-day
//!   formatting.
//! * `format-datetime(iso-datetime, locale, date-length, time-length)`
//!   — combined date/time formatting.
//! * `supported-locales()` — introspection.
//!
//! A [`DateTimeEngine`] implements every export on the Rust side; a
//! `wit-component`-gated `Guest` implementation lives in the
//! sibling `stringcheese-icu-datetime-component` crate.
//!
//! # Time-zone naivety
//!
//! Phase 4 is time-zone-naive by design. Whatever offset (or absence
//! thereof) the input ISO string carries is **discarded** by the
//! formatter — the pattern renders the wall-clock components exactly
//! as parsed. Zone-aware operations are a follow-up phase.
//!
//! # Date arithmetic
//!
//! Weekday computation uses Zeller's congruence
//! (<https://en.wikipedia.org/wiki/Zeller%27s_congruence>) for the
//! Proleptic Gregorian calendar; leap-year handling follows the
//! standard Gregorian rule (divisible by 4, not by 100 unless also
//! by 400). No external date/time crate (`chrono`, `jiff`, `time`)
//! is pulled in: the algorithm is self-contained so the wasm
//! component stays as lean as the icu-case and icu-collation
//! reference builds.
//!
//! # Phase 4 deferrals
//!
//! * **Non-Gregorian calendars** (Buddhist, Hebrew, Islamic,
//!   Japanese). The wire format leaves room for calendar-specific
//!   month-name tables under future capability tags.
//! * **Skeleton-based formatting** — CLDR skeletons (`"yMMMd"`
//!   → auto-picked pattern) will land alongside the follow-up
//!   locale sweep.
//! * **Relative time formatting** ("3 hours ago") and **interval
//!   formatting** ("Mar 3 – Mar 5") are separate concerns handled
//!   by different WIT interfaces.
//! * **Timezone conversion** — see the "Time-zone naivety" section
//!   above.
//!
//! # Trust model
//!
//! Inherited from `stringcheese-scud`: SCUD packs are trusted
//! input. This crate does not defend against maliciously crafted
//! packs.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use stringcheese_scud::{DateTimeDataView, ScudFile};

pub use stringcheese_scud::{DateTimeLength, ScudError};

// -----------------------------------------------------------------------
// Typed error surface
// -----------------------------------------------------------------------

/// Typed failure modes of the datetime engine. Mirrors the WIT
/// `datetime-error` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateTimeError {
    /// The locale tag was not a well-formed BCP 47 tag.
    InvalidLocale(&'static str),
    /// No pack was loaded for the requested locale.
    LocaleUnavailable(&'static str),
    /// The `iso-date`, `iso-time`, or `iso-datetime` input was
    /// syntactically malformed.
    InvalidInput(&'static str),
    /// The parsed input carried an out-of-range field
    /// (`2024-02-30`, `25:00:00`, etc.).
    OutOfRange(&'static str),
}

// -----------------------------------------------------------------------
// Civil date and time structs
// -----------------------------------------------------------------------

/// A civil (calendar) date in the Proleptic Gregorian calendar.
///
/// No timezone. Year is signed so a BC year is representable as
/// `year <= 0` (astronomical convention: year 0 = 1 BC, year -1 =
/// 2 BC). The month is `1..=12`; day is `1..=days_in_month`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CivilDate {
    /// The astronomical year (`0` = 1 BC, `1` = 1 AD).
    pub year: i32,
    /// Month, `1..=12`.
    pub month: u8,
    /// Day of month, `1..=days_in_month(year, month)`.
    pub day: u8,
}

/// A civil (wall-clock) time-of-day.
///
/// Sub-second precision is discarded before construction so the
/// engine can render a stable `HH:MM:SS`. No timezone.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CivilTime {
    /// Hour, `0..=23`.
    pub hour: u8,
    /// Minute, `0..=59`.
    pub minute: u8,
    /// Second, `0..=59` (leap seconds unmodelled — a leap second
    /// second-value of 60 is rejected as out-of-range).
    pub second: u8,
}

/// A civil date-time.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CivilDateTime {
    /// The civil date component.
    pub date: CivilDate,
    /// The civil time-of-day component.
    pub time: CivilTime,
}

// -----------------------------------------------------------------------
// Parsing helpers (ISO-8601)
// -----------------------------------------------------------------------

impl CivilDate {
    /// Parse a strict `"YYYY-MM-DD"` string.
    ///
    /// # Errors
    ///
    /// * [`DateTimeError::InvalidInput`] if the string is not four
    ///   digits + `-` + two digits + `-` + two digits.
    /// * [`DateTimeError::OutOfRange`] if the month or day is
    ///   outside the Gregorian civil calendar's range for that year.
    pub fn parse_iso(input: &str) -> Result<Self, DateTimeError> {
        parse_iso_date(input)
    }
}

impl CivilTime {
    /// Parse a strict `"HH:MM:SS"` or `"HH:MM:SS.sss"` string.
    ///
    /// Fractional seconds are accepted syntactically but discarded
    /// (Phase 4 does not render sub-second precision).
    ///
    /// # Errors
    ///
    /// See [`CivilDate::parse_iso`].
    pub fn parse_iso(input: &str) -> Result<Self, DateTimeError> {
        parse_iso_time(input)
    }
}

impl CivilDateTime {
    /// Parse an ISO-8601 combined date-time.
    ///
    /// Accepted forms (all with `T` as the separator; a bare space
    /// is not accepted for strictness):
    ///
    /// * `"YYYY-MM-DDTHH:MM:SS"` (naive local)
    /// * `"YYYY-MM-DDTHH:MM:SS.sss"` (naive local, fractional
    ///   seconds discarded)
    /// * `"YYYY-MM-DDTHH:MM:SSZ"` (UTC — the `Z` is accepted and
    ///   ignored)
    /// * `"YYYY-MM-DDTHH:MM:SS+HH:MM"` or `"-HH:MM"` (offset
    ///   accepted and ignored)
    ///
    /// # Errors
    ///
    /// See [`CivilDate::parse_iso`].
    pub fn parse_iso(input: &str) -> Result<Self, DateTimeError> {
        // Split on the mandatory 'T' separator.
        let bytes = input.as_bytes();
        if bytes.len() < 19 {
            return Err(DateTimeError::InvalidInput("iso-datetime too short"));
        }
        if bytes[10] != b'T' {
            return Err(DateTimeError::InvalidInput(
                "iso-datetime missing 'T' separator",
            ));
        }
        let date = CivilDate::parse_iso(&input[..10])?;
        // Trim any trailing timezone marker before handing to the
        // time parser. Accepted terminators: 'Z', '+', or '-' after
        // the seconds field (i.e. at or past position 8 within the
        // time substring).
        let time_str = &input[11..];
        let time_trimmed = trim_tz_suffix(time_str);
        let time = CivilTime::parse_iso(time_trimmed)?;
        Ok(Self { date, time })
    }
}

fn parse_iso_date(input: &str) -> Result<CivilDate, DateTimeError> {
    let bytes = input.as_bytes();
    // Support optional leading '+'/'-' sign on the year field so
    // ISO-8601 extended years survive round-trip. Common case:
    // "YYYY-MM-DD" (10 bytes, no sign).
    if bytes.len() != 10 {
        return Err(DateTimeError::InvalidInput("iso-date wrong length"));
    }
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(DateTimeError::InvalidInput("iso-date missing separators"));
    }
    let year_str = &input[..4];
    let month_str = &input[5..7];
    let day_str = &input[8..10];
    let year: i32 = parse_int(year_str).ok_or(DateTimeError::InvalidInput("iso-date year"))?;
    let month: u8 =
        parse_int::<u8>(month_str).ok_or(DateTimeError::InvalidInput("iso-date month"))?;
    let day: u8 = parse_int::<u8>(day_str).ok_or(DateTimeError::InvalidInput("iso-date day"))?;
    if !(1..=12).contains(&month) {
        return Err(DateTimeError::OutOfRange("iso-date month"));
    }
    let max = days_in_month(year, month);
    if day < 1 || day > max {
        return Err(DateTimeError::OutOfRange("iso-date day"));
    }
    Ok(CivilDate { year, month, day })
}

fn parse_iso_time(input: &str) -> Result<CivilTime, DateTimeError> {
    let bytes = input.as_bytes();
    if bytes.len() < 8 {
        return Err(DateTimeError::InvalidInput("iso-time too short"));
    }
    if bytes[2] != b':' || bytes[5] != b':' {
        return Err(DateTimeError::InvalidInput("iso-time missing separators"));
    }
    // Sub-second portion must start with '.' if present; discard
    // it (Phase 4 renders no sub-second precision).
    if bytes.len() > 8 && bytes[8] != b'.' {
        return Err(DateTimeError::InvalidInput(
            "iso-time trailing garbage after seconds",
        ));
    }
    if bytes.len() > 8 {
        // Every char after '.' must be a digit.
        for &b in &bytes[9..] {
            if !b.is_ascii_digit() {
                return Err(DateTimeError::InvalidInput(
                    "iso-time non-digit in fractional-seconds",
                ));
            }
        }
    }
    let hour: u8 = parse_int(&input[..2]).ok_or(DateTimeError::InvalidInput("iso-time hour"))?;
    let minute: u8 =
        parse_int(&input[3..5]).ok_or(DateTimeError::InvalidInput("iso-time minute"))?;
    let second: u8 =
        parse_int(&input[6..8]).ok_or(DateTimeError::InvalidInput("iso-time second"))?;
    if hour > 23 {
        return Err(DateTimeError::OutOfRange("iso-time hour"));
    }
    if minute > 59 {
        return Err(DateTimeError::OutOfRange("iso-time minute"));
    }
    if second > 59 {
        return Err(DateTimeError::OutOfRange("iso-time second"));
    }
    Ok(CivilTime {
        hour,
        minute,
        second,
    })
}

fn trim_tz_suffix(time_str: &str) -> &str {
    let bytes = time_str.as_bytes();
    // Find the first '+', '-', or 'Z' at or after position 8
    // (i.e. after "HH:MM:SS"). Anything before 8 is part of the
    // wall-clock or the fractional-seconds portion.
    for (i, &b) in bytes.iter().enumerate().skip(8) {
        if b == b'Z' || b == b'+' || b == b'-' {
            return &time_str[..i];
        }
    }
    time_str
}

/// Minimal integer parser — the standard library's `str::parse`
/// requires `FromStr` and drags allocation in for negative-number
/// error paths. This version handles the strict ASCII case and
/// optional leading `-`.
fn parse_int<T>(s: &str) -> Option<T>
where
    T: TryFrom<i64>,
{
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut acc: i64 = 0;
    let (start, sign) = if bytes[0] == b'-' {
        (1usize, -1i64)
    } else {
        (0usize, 1i64)
    };
    if start >= bytes.len() {
        return None;
    }
    for &b in &bytes[start..] {
        if !b.is_ascii_digit() {
            return None;
        }
        acc = acc.checked_mul(10)?.checked_add(i64::from(b - b'0'))?;
    }
    T::try_from(acc.checked_mul(sign)?).ok()
}

// -----------------------------------------------------------------------
// Gregorian calendar math
// -----------------------------------------------------------------------

/// True iff `year` is a Gregorian leap year.
#[must_use]
pub fn is_leap_year(year: i32) -> bool {
    if year % 4 != 0 {
        return false;
    }
    if year % 100 != 0 {
        return true;
    }
    year % 400 == 0
}

/// The number of days in `(year, month)` under the Gregorian
/// calendar.
///
/// Returns 0 for `month` outside `1..=12`.
#[must_use]
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Weekday for a Gregorian civil date, expressed as `0..=6` where
/// `0 = Sunday`, `1 = Monday`, …, `6 = Saturday`.
///
/// Uses Zeller's congruence
/// (<https://en.wikipedia.org/wiki/Zeller%27s_congruence>) with the
/// classic Sunday-first convention baked in via a fixed offset. The
/// SCUD pack's weekday name table is indexed by the same 0..=6 range.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn weekday(date: CivilDate) -> u8 {
    let (mut m, mut y) = (i32::from(date.month), date.year);
    // Zeller treats January and February as months 13 and 14 of the
    // previous year, so shift.
    if m < 3 {
        m += 12;
        y -= 1;
    }
    let k = y.rem_euclid(100);
    let j = y.div_euclid(100);
    let q = i32::from(date.day);
    // Zeller's formula: h = (q + (13(m+1))/5 + K + K/4 + J/4 - 2J) mod 7
    // where h is 0=Saturday. Convert to 0=Sunday afterwards.
    let h = (q + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j).rem_euclid(7);
    // h: 0=Sat, 1=Sun, 2=Mon, ..., 6=Fri. Shift to 0=Sun.
    // (h - 1) mod 7 → 0=Sun, 1=Mon, ..., 6=Sat.
    // Guaranteed non-negative because rem_euclid produced 0..=6, and
    // the truncated cast is safe within u8.
    let sun_first = (h + 6).rem_euclid(7);
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let out = sun_first as u8;
    out
}

// -----------------------------------------------------------------------
// Pattern interpreter
// -----------------------------------------------------------------------

/// Interpret a CLDR pattern against a date, a time, or both, using
/// the supplied [`DateTimeDataView`] for locale-specific names.
///
/// The pattern grammar is a subset of CLDR's tr35 pattern language,
/// sized for the short/medium/long/full defaults. Literal runs
/// between token clusters are copied verbatim; the `'` character
/// escapes a literal (CLDR's own convention). Unknown letters
/// (`A..Z` / `a..z`) that are not in the recognised token set fall
/// through as literals — a conservative choice for forward
/// compatibility with future pattern additions.
///
/// # Tokens recognised
///
/// | Token   | Field                                              |
/// | ------- | -------------------------------------------------- |
/// | `y`     | Year, minimum digits (no zero-pad)                 |
/// | `yy`    | Year, two-digit (2024 → "24")                      |
/// | `yyy+`  | Year, zero-padded to width                         |
/// | `M`     | Month, no zero-pad                                 |
/// | `MM`    | Month, two-digit zero-pad                          |
/// | `MMM`   | Month, abbreviated name (`SECT_MONTH_ABBR`)        |
/// | `MMMM`  | Month, full name (`SECT_MONTH_NAMES`)              |
/// | `d`     | Day, no zero-pad                                   |
/// | `dd`    | Day, two-digit zero-pad                            |
/// | `E`/`EE`/`EEE` | Weekday, abbreviated (`SECT_WEEKDAY_ABBR`)  |
/// | `EEEE`  | Weekday, full (`SECT_WEEKDAY_NAMES`)               |
/// | `H`     | 24-hour, no zero-pad                               |
/// | `HH`    | 24-hour, two-digit zero-pad                        |
/// | `h`     | 12-hour, no zero-pad                               |
/// | `hh`    | 12-hour, two-digit zero-pad                        |
/// | `m`     | Minute, no zero-pad                                |
/// | `mm`    | Minute, two-digit zero-pad                         |
/// | `s`     | Second, no zero-pad                                |
/// | `ss`    | Second, two-digit zero-pad                         |
/// | `a`     | AM/PM marker                                       |
/// | `G`     | Era, abbreviated (BC/AD)                           |
///
/// Any other pattern letter is copied literally.
#[cfg(feature = "alloc")]
#[allow(clippy::similar_names)]
pub fn render_pattern(
    pattern: &str,
    date: Option<CivilDate>,
    time: Option<CivilTime>,
    data: &DateTimeDataView<'_>,
) -> String {
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            // CLDR quoted literal — copy contents up to the next '.
            // A doubled '' emits a single '.
            if chars.peek() == Some(&'\'') {
                chars.next();
                out.push('\'');
                continue;
            }
            for inner in chars.by_ref() {
                if inner == '\'' {
                    break;
                }
                out.push(inner);
            }
            continue;
        }
        if !ch.is_ascii_alphabetic() {
            out.push(ch);
            continue;
        }
        // Count consecutive identical letters.
        let mut count = 1usize;
        while chars.peek() == Some(&ch) {
            chars.next();
            count += 1;
        }
        emit_token(ch, count, date, time, data, &mut out);
    }
    out
}

#[cfg(feature = "alloc")]
#[allow(clippy::similar_names)]
fn emit_token(
    ch: char,
    count: usize,
    date: Option<CivilDate>,
    time: Option<CivilTime>,
    data: &DateTimeDataView<'_>,
    out: &mut String,
) {
    match ch {
        'y' => {
            if let Some(d) = date {
                emit_year(d.year, count, out);
            } else {
                fill_literal(ch, count, out);
            }
        }
        'M' => {
            if let Some(d) = date {
                emit_month(d.month, count, data, out);
            } else {
                fill_literal(ch, count, out);
            }
        }
        'd' => {
            if let Some(d) = date {
                emit_padded_u32(u32::from(d.day), count, out);
            } else {
                fill_literal(ch, count, out);
            }
        }
        'E' => {
            if let Some(d) = date {
                let wd = weekday(d);
                let name = if count >= 4 {
                    data.weekday_name(wd)
                } else {
                    data.weekday_abbreviation(wd)
                };
                out.push_str(name.unwrap_or(""));
            } else {
                fill_literal(ch, count, out);
            }
        }
        'H' => {
            if let Some(t) = time {
                emit_padded_u32(u32::from(t.hour), count, out);
            } else {
                fill_literal(ch, count, out);
            }
        }
        'h' => {
            if let Some(t) = time {
                let h12 = to_12h(t.hour);
                emit_padded_u32(u32::from(h12), count, out);
            } else {
                fill_literal(ch, count, out);
            }
        }
        'm' => {
            if let Some(t) = time {
                emit_padded_u32(u32::from(t.minute), count, out);
            } else {
                fill_literal(ch, count, out);
            }
        }
        's' => {
            if let Some(t) = time {
                emit_padded_u32(u32::from(t.second), count, out);
            } else {
                fill_literal(ch, count, out);
            }
        }
        'a' => {
            if let Some(t) = time {
                let marker = if t.hour < 12 { data.am() } else { data.pm() };
                out.push_str(marker.unwrap_or(""));
            } else {
                fill_literal(ch, count, out);
            }
        }
        'G' => {
            if let Some(d) = date {
                let era = if d.year > 0 {
                    data.era_ad()
                } else {
                    data.era_bc()
                };
                out.push_str(era.unwrap_or(""));
            } else {
                fill_literal(ch, count, out);
            }
        }
        _ => {
            // Unknown pattern letter — copy verbatim so a forward-
            // compatible pack does not silently swallow the run.
            fill_literal(ch, count, out);
        }
    }
}

#[cfg(feature = "alloc")]
fn emit_year(year: i32, count: usize, out: &mut String) {
    // CLDR treats BC years as their absolute value for pattern
    // rendering; the era token is what distinguishes.
    let abs_year: u32 = year.unsigned_abs();
    match count {
        2 => {
            let last_two = abs_year % 100;
            let _ = fmt_padded(out, last_two, 2);
        }
        _ => {
            let _ = fmt_padded(out, abs_year, count);
        }
    }
}

#[cfg(feature = "alloc")]
fn emit_month(month: u8, count: usize, data: &DateTimeDataView<'_>, out: &mut String) {
    match count {
        1 | 2 => emit_padded_u32(u32::from(month), count, out),
        3 => out.push_str(data.month_abbreviation(month).unwrap_or("")),
        _ => out.push_str(data.month_name(month).unwrap_or("")),
    }
}

#[cfg(feature = "alloc")]
fn emit_padded_u32(v: u32, count: usize, out: &mut String) {
    let _ = fmt_padded(out, v, count);
}

#[cfg(feature = "alloc")]
fn fmt_padded(out: &mut String, v: u32, width: usize) -> core::fmt::Result {
    use core::fmt::Write as _;
    write!(out, "{v:0width$}")
}

#[cfg(feature = "alloc")]
fn fill_literal(ch: char, count: usize, out: &mut String) {
    for _ in 0..count {
        out.push(ch);
    }
}

fn to_12h(h24: u8) -> u8 {
    match h24 {
        0 => 12,
        1..=12 => h24,
        _ => h24 - 12,
    }
}

// -----------------------------------------------------------------------
// Pack + engine
// -----------------------------------------------------------------------

/// A loaded datetime-formatting pack for one BCP 47 locale.
///
/// Wraps a validated [`ScudFile`] whose capability tag is
/// [`stringcheese_scud::CAP_DATETIME`]. Cheap to clone — the
/// underlying SCUD bytes are borrowed by the [`ScudFile`], and this
/// wrapper carries only the parsed header plus the locale tag.
#[derive(Debug, Clone, Copy)]
pub struct DateTimePack<'a> {
    scud: ScudFile<'a>,
    locale: &'a str,
    data: DateTimeDataView<'a>,
}

impl<'a> DateTimePack<'a> {
    /// Wrap a validated [`ScudFile`] as a datetime pack.
    ///
    /// # Errors
    ///
    /// Returns [`ScudError::CapabilityMismatch`] if the file's
    /// capability tag is not [`stringcheese_scud::CAP_DATETIME`].
    pub fn new(scud: ScudFile<'a>) -> Result<Self, ScudError> {
        let data = scud.as_datetime_data()?;
        let locale = scud.locale().unwrap_or("");
        Ok(Self { scud, locale, data })
    }

    /// Parse `bytes` as a SCUD file and wrap it as a datetime pack.
    ///
    /// # Errors
    ///
    /// See [`ScudFile::from_slice`] and [`Self::new`].
    pub fn from_scud_bytes(bytes: &'a [u8]) -> Result<Self, ScudError> {
        let scud = ScudFile::from_slice(bytes)?;
        Self::new(scud)
    }

    /// The BCP 47 locale tag associated with this pack.
    #[must_use]
    pub fn locale(&self) -> &'a str {
        self.locale
    }

    /// The CLDR version the pack was generated from.
    #[must_use]
    pub fn cldr_version(&self) -> &'a str {
        self.scud.cldr_version()
    }

    /// Total byte length of the underlying SCUD file.
    #[must_use]
    pub fn scud_bytes_len(&self) -> usize {
        self.scud.len()
    }

    /// The zero-copy datetime data view.
    #[must_use]
    pub fn data(&self) -> &DateTimeDataView<'a> {
        &self.data
    }
}

/// Locale-sensitive datetime-formatting engine.
///
/// Holds a list of [`DateTimePack`]s and consults them at query time
/// in BCP 47 fallback order.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct DateTimeEngine<'a> {
    packs: Vec<DateTimePack<'a>>,
}

#[cfg(feature = "alloc")]
impl<'a> DateTimeEngine<'a> {
    /// Construct a fresh engine backed by the given packs.
    #[must_use]
    pub fn new(packs: Vec<DateTimePack<'a>>) -> Self {
        Self { packs }
    }

    /// Every BCP 47 locale tag this engine knows about.
    #[must_use]
    pub fn supported_locales(&self) -> Vec<&'a str> {
        self.packs.iter().map(DateTimePack::locale).collect()
    }

    /// True iff a query in the given locale would use a pack
    /// (rather than falling through to root).
    #[must_use]
    pub fn supports(&self, locale: &str) -> bool {
        walk_fallback_chain(locale).any(|tag| self.pack_for(tag).is_some())
    }

    /// Format an ISO-8601 date-only string.
    ///
    /// # Errors
    ///
    /// * [`DateTimeError::LocaleUnavailable`] when no pack covers
    ///   the requested locale.
    /// * [`DateTimeError::InvalidInput`] / [`DateTimeError::OutOfRange`]
    ///   for parser failures.
    pub fn format_date(
        &self,
        iso_date: &str,
        locale: &str,
        length: DateTimeLength,
    ) -> Result<String, DateTimeError> {
        let pack = self
            .active_pack(locale)
            .ok_or(DateTimeError::LocaleUnavailable(""))?;
        let date = CivilDate::parse_iso(iso_date)?;
        let pattern = pack
            .data
            .date_pattern(length)
            .ok_or(DateTimeError::LocaleUnavailable(""))?;
        Ok(render_pattern(pattern, Some(date), None, &pack.data))
    }

    /// Format an ISO-8601 time-of-day string.
    ///
    /// # Errors
    ///
    /// See [`Self::format_date`].
    pub fn format_time(
        &self,
        iso_time: &str,
        locale: &str,
        length: DateTimeLength,
    ) -> Result<String, DateTimeError> {
        let pack = self
            .active_pack(locale)
            .ok_or(DateTimeError::LocaleUnavailable(""))?;
        let time = CivilTime::parse_iso(iso_time)?;
        let pattern = pack
            .data
            .time_pattern(length)
            .ok_or(DateTimeError::LocaleUnavailable(""))?;
        Ok(render_pattern(pattern, None, Some(time), &pack.data))
    }

    /// Format a combined ISO-8601 date-time string.
    ///
    /// The output is `"<date> <time>"`. Timezone information
    /// carried by the input is parsed but discarded.
    ///
    /// # Errors
    ///
    /// See [`Self::format_date`].
    pub fn format_datetime(
        &self,
        iso_datetime: &str,
        locale: &str,
        date_length: DateTimeLength,
        time_length: DateTimeLength,
    ) -> Result<String, DateTimeError> {
        let pack = self
            .active_pack(locale)
            .ok_or(DateTimeError::LocaleUnavailable(""))?;
        let dt = CivilDateTime::parse_iso(iso_datetime)?;
        let date_pattern = pack
            .data
            .date_pattern(date_length)
            .ok_or(DateTimeError::LocaleUnavailable(""))?;
        let time_pattern = pack
            .data
            .time_pattern(time_length)
            .ok_or(DateTimeError::LocaleUnavailable(""))?;
        let date_out = render_pattern(date_pattern, Some(dt.date), None, &pack.data);
        let time_out = render_pattern(time_pattern, None, Some(dt.time), &pack.data);
        let mut out = String::with_capacity(date_out.len() + 1 + time_out.len());
        out.push_str(&date_out);
        out.push(' ');
        out.push_str(&time_out);
        Ok(out)
    }

    /// The pack that would service a query under `locale`, if any.
    #[must_use]
    pub fn active_pack(&self, locale: &str) -> Option<&DateTimePack<'a>> {
        walk_fallback_chain(locale).find_map(|tag| self.pack_for(tag))
    }

    fn pack_for(&self, tag: &str) -> Option<&DateTimePack<'a>> {
        self.packs
            .iter()
            .find(|p| p.locale.eq_ignore_ascii_case(tag))
    }
}

/// Walk the CLDR-defined fallback chain for a BCP 47 tag.
///
/// Same shape as
/// [`stringcheese_icu_plural::walk_fallback_chain`](https://docs.rs/stringcheese-icu-plural)
/// and [`stringcheese_icu_number::walk_fallback_chain`](https://docs.rs/stringcheese-icu-number).
pub fn walk_fallback_chain(locale: &str) -> impl Iterator<Item = &str> {
    let mut current = Some(locale);
    let mut emitted_root = false;
    core::iter::from_fn(move || {
        if let Some(tag) = current.take() {
            if tag.is_empty() {
                if emitted_root {
                    None
                } else {
                    emitted_root = true;
                    Some("")
                }
            } else {
                let next = tag.rfind('-').map_or("", |idx| &tag[..idx]);
                current = Some(next);
                Some(tag)
            }
        } else {
            None
        }
    })
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;
    use stringcheese_scud::{
        CAP_DATETIME, DateTimeSectionBuilder, SECT_AM_PM, SECT_DATE_PATTERNS, SECT_ERA_NAMES,
        SECT_MONTH_ABBR, SECT_MONTH_NAMES, SECT_TIME_PATTERNS, SECT_WEEKDAY_ABBR,
        SECT_WEEKDAY_NAMES, ScudFile, ScudWriter,
    };

    fn build_en() -> alloc::vec::Vec<u8> {
        let mut d = DateTimeSectionBuilder::new();
        d.set_date_pattern(DateTimeLength::Short, "M/d/y");
        d.set_date_pattern(DateTimeLength::Medium, "MMM d, y");
        d.set_date_pattern(DateTimeLength::Long, "MMMM d, y");
        d.set_date_pattern(DateTimeLength::Full, "EEEE, MMMM d, y");
        d.set_time_pattern(DateTimeLength::Short, "h:mm a");
        d.set_time_pattern(DateTimeLength::Medium, "h:mm:ss a");
        d.set_time_pattern(DateTimeLength::Long, "h:mm:ss a");
        d.set_time_pattern(DateTimeLength::Full, "h:mm:ss a");
        d.set_month_names([
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ]);
        d.set_month_abbreviations([
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]);
        d.set_weekday_names([
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ]);
        d.set_weekday_abbreviations(["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]);
        d.set_am_pm("AM", "PM");
        d.set_eras("BC", "AD");
        let mut w = ScudWriter::new(CAP_DATETIME, "44.1", Some("en"));
        w.append_section(SECT_DATE_PATTERNS, &d.date_patterns_bytes());
        w.append_section(SECT_TIME_PATTERNS, &d.time_patterns_bytes());
        w.append_section(SECT_MONTH_NAMES, &d.month_names_bytes());
        w.append_section(SECT_MONTH_ABBR, &d.month_abbr_bytes());
        w.append_section(SECT_WEEKDAY_NAMES, &d.weekday_names_bytes());
        w.append_section(SECT_WEEKDAY_ABBR, &d.weekday_abbr_bytes());
        w.append_section(SECT_AM_PM, &d.am_pm_bytes());
        w.append_section(SECT_ERA_NAMES, &d.era_names_bytes());
        w.finish()
    }

    #[test]
    fn parse_iso_date_ok() {
        let d = CivilDate::parse_iso("2024-09-22").unwrap();
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 9);
        assert_eq!(d.day, 22);
    }

    #[test]
    fn parse_iso_date_rejects_bad_shape() {
        assert!(matches!(
            CivilDate::parse_iso("2024/09/22"),
            Err(DateTimeError::InvalidInput(_))
        ));
        assert!(matches!(
            CivilDate::parse_iso("2024-9-22"),
            Err(DateTimeError::InvalidInput(_))
        ));
    }

    #[test]
    fn parse_iso_date_out_of_range() {
        assert!(matches!(
            CivilDate::parse_iso("2024-02-30"),
            Err(DateTimeError::OutOfRange(_))
        ));
        assert!(matches!(
            CivilDate::parse_iso("2024-13-01"),
            Err(DateTimeError::OutOfRange(_))
        ));
        // 2023 was not a leap year.
        assert!(matches!(
            CivilDate::parse_iso("2023-02-29"),
            Err(DateTimeError::OutOfRange(_))
        ));
        // 2024 is a leap year; the 29th is valid.
        assert!(CivilDate::parse_iso("2024-02-29").is_ok());
        // 2000 is a leap year (÷400).
        assert!(CivilDate::parse_iso("2000-02-29").is_ok());
        // 1900 is NOT a leap year (÷100, not ÷400).
        assert!(matches!(
            CivilDate::parse_iso("1900-02-29"),
            Err(DateTimeError::OutOfRange(_))
        ));
    }

    #[test]
    fn parse_iso_time_ok() {
        let t = CivilTime::parse_iso("17:03:04").unwrap();
        assert_eq!(t.hour, 17);
        assert_eq!(t.minute, 3);
        assert_eq!(t.second, 4);
    }

    #[test]
    fn parse_iso_time_fractional_discarded() {
        let t = CivilTime::parse_iso("17:03:04.567").unwrap();
        assert_eq!(t.second, 4);
    }

    #[test]
    fn parse_iso_time_out_of_range() {
        assert!(matches!(
            CivilTime::parse_iso("25:00:00"),
            Err(DateTimeError::OutOfRange(_))
        ));
        assert!(matches!(
            CivilTime::parse_iso("12:60:00"),
            Err(DateTimeError::OutOfRange(_))
        ));
    }

    #[test]
    fn parse_iso_datetime_ok_with_tz() {
        // Z suffix
        let dt = CivilDateTime::parse_iso("2024-09-22T17:03:04Z").unwrap();
        assert_eq!(dt.date.day, 22);
        assert_eq!(dt.time.hour, 17);
        // +HH:MM suffix
        let dt = CivilDateTime::parse_iso("2024-09-22T17:03:04+02:00").unwrap();
        assert_eq!(dt.time.hour, 17);
        // -HH:MM suffix
        let dt = CivilDateTime::parse_iso("2024-09-22T17:03:04-05:00").unwrap();
        assert_eq!(dt.time.hour, 17);
        // Fractional seconds
        let dt = CivilDateTime::parse_iso("2024-09-22T17:03:04.123Z").unwrap();
        assert_eq!(dt.time.second, 4);
    }

    #[test]
    fn parse_iso_datetime_rejects_missing_t() {
        assert!(matches!(
            CivilDateTime::parse_iso("2024-09-22 17:03:04"),
            Err(DateTimeError::InvalidInput(_))
        ));
    }

    #[test]
    fn weekday_known_dates() {
        // 2024-09-22 was a Sunday.
        assert_eq!(
            weekday(CivilDate {
                year: 2024,
                month: 9,
                day: 22
            }),
            0
        );
        // 2024-01-01 was a Monday.
        assert_eq!(
            weekday(CivilDate {
                year: 2024,
                month: 1,
                day: 1
            }),
            1
        );
        // 2000-02-29 was a Tuesday.
        assert_eq!(
            weekday(CivilDate {
                year: 2000,
                month: 2,
                day: 29
            }),
            2
        );
        // 1969-07-20 was a Sunday (moon landing).
        assert_eq!(
            weekday(CivilDate {
                year: 1969,
                month: 7,
                day: 20
            }),
            0
        );
        // 1776-07-04 was a Thursday (Declaration of Independence).
        assert_eq!(
            weekday(CivilDate {
                year: 1776,
                month: 7,
                day: 4
            }),
            4
        );
    }

    #[test]
    fn leap_year_matrix() {
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(2023));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2100));
        assert!(is_leap_year(2400));
    }

    #[test]
    fn days_in_month_matrix() {
        assert_eq!(days_in_month(2024, 1), 31);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2024, 4), 30);
        assert_eq!(days_in_month(2024, 12), 31);
        assert_eq!(days_in_month(2024, 0), 0);
        assert_eq!(days_in_month(2024, 13), 0);
    }

    #[test]
    fn format_date_short_medium_long_full() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let e = DateTimeEngine::new(vec![pack]);
        assert_eq!(
            e.format_date("2024-09-22", "en", DateTimeLength::Short)
                .unwrap(),
            "9/22/2024"
        );
        assert_eq!(
            e.format_date("2024-09-22", "en", DateTimeLength::Medium)
                .unwrap(),
            "Sep 22, 2024"
        );
        assert_eq!(
            e.format_date("2024-09-22", "en", DateTimeLength::Long)
                .unwrap(),
            "September 22, 2024"
        );
        assert_eq!(
            e.format_date("2024-09-22", "en", DateTimeLength::Full)
                .unwrap(),
            "Sunday, September 22, 2024"
        );
    }

    #[test]
    fn format_time_am_pm() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let e = DateTimeEngine::new(vec![pack]);
        assert_eq!(
            e.format_time("17:03:04", "en", DateTimeLength::Medium)
                .unwrap(),
            "5:03:04 PM"
        );
        assert_eq!(
            e.format_time("17:03:04", "en", DateTimeLength::Short)
                .unwrap(),
            "5:03 PM"
        );
        // Midnight.
        assert_eq!(
            e.format_time("00:00:00", "en", DateTimeLength::Short)
                .unwrap(),
            "12:00 AM"
        );
        // Noon.
        assert_eq!(
            e.format_time("12:00:00", "en", DateTimeLength::Short)
                .unwrap(),
            "12:00 PM"
        );
        // Just after noon.
        assert_eq!(
            e.format_time("12:30:00", "en", DateTimeLength::Short)
                .unwrap(),
            "12:30 PM"
        );
    }

    #[test]
    fn format_datetime_composes_date_and_time() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let e = DateTimeEngine::new(vec![pack]);
        assert_eq!(
            e.format_datetime(
                "2024-09-22T17:03:04Z",
                "en",
                DateTimeLength::Medium,
                DateTimeLength::Short
            )
            .unwrap(),
            "Sep 22, 2024 5:03 PM"
        );
    }

    #[test]
    fn locale_fallback_chain() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let e = DateTimeEngine::new(vec![pack]);
        // en-US falls back to en.
        assert_eq!(
            e.format_date("2024-09-22", "en-US", DateTimeLength::Short)
                .unwrap(),
            "9/22/2024"
        );
        assert!(e.supports("en"));
        assert!(e.supports("en-US"));
        assert!(!e.supports("de"));
    }

    #[test]
    fn unknown_locale_errors() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let e = DateTimeEngine::new(vec![pack]);
        assert!(matches!(
            e.format_date("2024-09-22", "xx", DateTimeLength::Short),
            Err(DateTimeError::LocaleUnavailable(_))
        ));
    }

    #[test]
    fn invalid_input_errors() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let e = DateTimeEngine::new(vec![pack]);
        assert!(matches!(
            e.format_date("not-a-date", "en", DateTimeLength::Short),
            Err(DateTimeError::InvalidInput(_))
        ));
        assert!(matches!(
            e.format_date("2024-02-30", "en", DateTimeLength::Short),
            Err(DateTimeError::OutOfRange(_))
        ));
    }

    #[test]
    fn scud_file_wrongly_typed_rejected() {
        let w = ScudWriter::new(*b"CASE", "44.1", Some("en"));
        let bytes = w.finish();
        let file = ScudFile::from_slice(&bytes).unwrap();
        assert!(DateTimePack::new(file).is_err());
    }

    #[test]
    fn engine_reports_supported_locales() {
        let en = build_en();
        let e = DateTimeEngine::new(vec![DateTimePack::from_scud_bytes(&en).unwrap()]);
        assert_eq!(e.supported_locales(), vec!["en"]);
    }

    #[test]
    fn render_pattern_quoted_literals() {
        // CLDR quoted literals: 'abc' → abc, '' → '
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let out = render_pattern(
            "yyyy 'year:' MMMM",
            Some(CivilDate {
                year: 2024,
                month: 9,
                day: 22,
            }),
            None,
            pack.data(),
        );
        assert_eq!(out, "2024 year: September");
    }

    #[test]
    fn render_pattern_unknown_letter_falls_through() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        // 'Q' (quarter) is not in the recognised token set; it should
        // fall through as a literal.
        let out = render_pattern(
            "QQQQ y",
            Some(CivilDate {
                year: 2024,
                month: 9,
                day: 22,
            }),
            None,
            pack.data(),
        );
        assert_eq!(out, "QQQQ 2024");
    }

    #[test]
    fn year_two_digit_uses_last_two() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let out = render_pattern(
            "yy",
            Some(CivilDate {
                year: 2024,
                month: 1,
                day: 1,
            }),
            None,
            pack.data(),
        );
        assert_eq!(out, "24");
    }

    #[test]
    fn era_token_renders_ad_bc() {
        let bytes = build_en();
        let pack = DateTimePack::from_scud_bytes(&bytes).unwrap();
        let ad = render_pattern(
            "y G",
            Some(CivilDate {
                year: 2024,
                month: 1,
                day: 1,
            }),
            None,
            pack.data(),
        );
        assert_eq!(ad, "2024 AD");
        let bc = render_pattern(
            "y G",
            Some(CivilDate {
                year: -44,
                month: 3,
                day: 15,
            }),
            None,
            pack.data(),
        );
        assert_eq!(bc, "44 BC");
    }

    #[test]
    fn fallback_chain_walks_correctly() {
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("fr-CA").collect();
        assert_eq!(chain, ["fr-CA", "fr", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("en").collect();
        assert_eq!(chain, ["en", ""]);
        let chain: alloc::vec::Vec<&str> = walk_fallback_chain("").collect();
        assert_eq!(chain, [""]);
    }
}
