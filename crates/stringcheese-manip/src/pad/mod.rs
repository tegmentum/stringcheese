//! Pad a string to a target width.
//!
//! Every function in this module returns an owned `String` that is at
//! least as long as the input. Each function names the *width unit* it
//! measures against:
//!
//! - **scalars** (default) — width is the [`chars`](str::chars) count.
//!   This is the least-surprising default for programmers who think of
//!   a string as a sequence of code points. Functions: [`pad_left`],
//!   [`pad_right`], [`center`].
//! - **bytes** — width is the raw UTF-8 byte length. Useful for
//!   fixed-width byte-aligned output. Functions: [`pad_left_bytes`],
//!   [`pad_right_bytes`], [`center_bytes`].
//! - **display columns** — width is the terminal-cell count per
//!   Unicode East Asian Width. **Deferred**: these variants are not
//!   shipped in this wave. The `stringcheese-unicode` crate does not
//!   yet expose a display-width API and this crate refuses to reach
//!   past it to `unicode-width` directly. Track the follow-up in
//!   `docs/DESIGN.md`.
//!
//! # If the input is already at or over the target
//!
//! Every function returns a clone of the input verbatim — never
//! truncated. Padding is an *additive* operation; callers who need to
//! shorten output first should use [`crate::slice`] to trim, then pad.
//!
//! # Center rule
//!
//! [`center`] and [`center_bytes`] distribute padding evenly. When the
//! required padding is odd, the extra unit goes on the **right**:
//! centering `"x"` into width 4 with fill `'.'` produces `".x.."`, not
//! `"..x."`. This matches Python `str.center` and Rust's own
//! `fill`/`center`-style formatting conventions widely used elsewhere.
//!
//! # `no_std`
//!
//! Every item in this module is gated on `feature = "alloc"` because the
//! outputs are owned `String`s.

#![cfg(feature = "alloc")]

use alloc::string::String;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------
// Scalar-width padding (default).
// ---------------------------------------------------------------------

/// Pads `s` on the **left** with `fill` so the returned string contains
/// at least `target` Unicode scalar values.
///
/// If `s` is already `target` scalars or longer, `s` is returned
/// unchanged (as an owned `String`).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pad;
///
/// assert_eq!(pad::pad_left("hi", 5, ' '), "   hi");
/// assert_eq!(pad::pad_left("hi", 2, ' '), "hi");
/// assert_eq!(pad::pad_left("hi", 1, ' '), "hi");
/// // Fill can be any scalar.
/// assert_eq!(pad::pad_left("42", 5, '0'), "00042");
/// // Empty input padded to target is all fill.
/// assert_eq!(pad::pad_left("", 3, 'x'), "xxx");
/// ```
#[must_use]
pub fn pad_left(s: &str, target: usize, fill: char) -> String {
    let cur = s.chars().count();
    if cur >= target {
        return String::from(s);
    }
    let pad = target - cur;
    let mut out = String::with_capacity(s.len() + fill.len_utf8() * pad);
    for _ in 0..pad {
        out.push(fill);
    }
    out.push_str(s);
    out
}

/// Pads `s` on the **right** with `fill` so the returned string contains
/// at least `target` Unicode scalar values.
///
/// If `s` is already `target` scalars or longer, `s` is returned
/// unchanged.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pad;
///
/// assert_eq!(pad::pad_right("hi", 5, ' '), "hi   ");
/// assert_eq!(pad::pad_right("hi", 2, ' '), "hi");
/// assert_eq!(pad::pad_right("", 3, 'x'), "xxx");
/// ```
#[must_use]
pub fn pad_right(s: &str, target: usize, fill: char) -> String {
    let cur = s.chars().count();
    if cur >= target {
        return String::from(s);
    }
    let pad = target - cur;
    let mut out = String::with_capacity(s.len() + fill.len_utf8() * pad);
    out.push_str(s);
    for _ in 0..pad {
        out.push(fill);
    }
    out
}

/// Pads `s` on **both sides** with `fill` so the returned string contains
/// at least `target` Unicode scalar values.
///
/// When the required padding is odd, the extra unit goes on the right —
/// see the [module documentation](self#center-rule).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pad;
///
/// assert_eq!(pad::center("hi", 6, ' '), "  hi  ");
/// // Odd padding: extra unit on the right.
/// assert_eq!(pad::center("x", 4, '.'), ".x..");
/// // Already wide enough is passed through.
/// assert_eq!(pad::center("hello", 3, ' '), "hello");
/// ```
#[must_use]
pub fn center(s: &str, target: usize, fill: char) -> String {
    let cur = s.chars().count();
    if cur >= target {
        return String::from(s);
    }
    let pad = target - cur;
    let left = pad / 2;
    let right = pad - left; // extra unit lands on the right for odd totals
    let mut out = String::with_capacity(s.len() + fill.len_utf8() * pad);
    for _ in 0..left {
        out.push(fill);
    }
    out.push_str(s);
    for _ in 0..right {
        out.push(fill);
    }
    out
}

// ---------------------------------------------------------------------
// Byte-width padding.
// ---------------------------------------------------------------------

/// Pads `s` on the **left** with `fill` so the returned string is at
/// least `target` **UTF-8 bytes** long.
///
/// Because a fill character may itself be multi-byte, the output can
/// overshoot `target` by up to `fill.len_utf8() - 1` bytes. The number of
/// fill characters inserted is `ceil((target - s.len()) / fill.len_utf8())`
/// — never fewer, so the target is always met or exceeded.
///
/// If `s` is already `target` bytes or longer, `s` is returned unchanged.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pad;
///
/// assert_eq!(pad::pad_left_bytes("hi", 5, ' '), "   hi");
/// // Multi-byte fill: 'é' is 2 bytes, so padding to 5 with 'é' where
/// // "hi" is 2 bytes needs at least 2 more bytes → one 'é' is not
/// // enough, two 'é's overshoots to 6.
/// let out = pad::pad_left_bytes("hi", 5, 'é');
/// assert!(out.len() >= 5);
/// assert!(out.ends_with("hi"));
/// ```
#[must_use]
pub fn pad_left_bytes(s: &str, target: usize, fill: char) -> String {
    let cur = s.len();
    if cur >= target {
        return String::from(s);
    }
    let needed = target - cur;
    let fill_len = fill.len_utf8();
    // ceil-div so we always meet or exceed `target`.
    let pad = needed.div_ceil(fill_len);
    let mut out = String::with_capacity(cur + fill_len * pad);
    for _ in 0..pad {
        out.push(fill);
    }
    out.push_str(s);
    out
}

/// Pads `s` on the **right** with `fill` so the returned string is at
/// least `target` **UTF-8 bytes** long.
///
/// The overshoot rule matches [`pad_left_bytes`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pad;
///
/// assert_eq!(pad::pad_right_bytes("hi", 5, ' '), "hi   ");
/// ```
#[must_use]
pub fn pad_right_bytes(s: &str, target: usize, fill: char) -> String {
    let cur = s.len();
    if cur >= target {
        return String::from(s);
    }
    let needed = target - cur;
    let fill_len = fill.len_utf8();
    let pad = needed.div_ceil(fill_len);
    let mut out = String::with_capacity(cur + fill_len * pad);
    out.push_str(s);
    for _ in 0..pad {
        out.push(fill);
    }
    out
}

/// Pads `s` on **both sides** with `fill` so the returned string is at
/// least `target` **UTF-8 bytes** long.
///
/// The pad units are split as evenly as possible; when the pad count is
/// odd, the extra unit goes on the right. Because fill characters may be
/// multi-byte, the actual byte total can exceed `target` — this is the
/// same overshoot rule as [`pad_left_bytes`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pad;
///
/// assert_eq!(pad::center_bytes("hi", 6, ' '), "  hi  ");
/// // Odd padding: extra unit on the right.
/// assert_eq!(pad::center_bytes("x", 4, '.'), ".x..");
/// ```
#[must_use]
pub fn center_bytes(s: &str, target: usize, fill: char) -> String {
    let cur = s.len();
    if cur >= target {
        return String::from(s);
    }
    let needed = target - cur;
    let fill_len = fill.len_utf8();
    let pad_units = needed.div_ceil(fill_len);
    let left = pad_units / 2;
    let right = pad_units - left;
    let mut out = String::with_capacity(cur + fill_len * pad_units);
    for _ in 0..left {
        out.push(fill);
    }
    out.push_str(s);
    for _ in 0..right {
        out.push(fill);
    }
    out
}
