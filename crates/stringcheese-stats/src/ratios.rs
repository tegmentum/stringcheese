//! Per-code-point character-class ratios.
//!
//! Six ratios in `[0.0, 1.0]` from one linear scan: printable /
//! control / whitespace / digit / alphabetic / punctuation. Useful
//! for quickly deciding "is this a binary blob?" (low printable
//! ratio, high control), "does this look like a hex string?" (high
//! digit + limited alpha), or "is this natural language?" (high
//! alpha + moderate punctuation).
//!
//! ## Definitions
//!
//! - `printable` — any code point that is NOT a control / format /
//!   surrogate / unassigned. Whitespace counts as printable.
//! - `control` — Unicode general category `Cc` (ASCII control-code
//!   range and its 8-bit continuation).
//! - `whitespace` — Rust's `char::is_whitespace` (a Unicode
//!   whitespace superset of ASCII whitespace).
//! - `digit` — `char::is_ascii_digit` (0-9 only). For Unicode
//!   decimal digits, use [`crate::histogram::Histogram::count`]
//!   with `DecimalNumber` instead.
//! - `alphabetic` — `char::is_alphabetic` (Unicode-aware; covers
//!   Latin, Cyrillic, CJK, etc.).
//! - `punctuation` — Unicode major category
//!   [`crate::MajorCategory::Punctuation`].
//!
//! All ratios are `count / total` where `total` is the number of
//! code points in the input. An empty string yields every ratio at
//! 0.0 — no denominator, no signal.

use unicode_general_category::{GeneralCategory, get_general_category};

use crate::histogram::MajorCategory;

/// Character-class ratios, all in `[0.0, 1.0]`.
///
/// Cheap to construct; one scan of the input.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Ratios {
    /// Fraction of code points that are printable
    /// (non-control / non-format / non-surrogate / non-unassigned).
    pub printable: f64,
    /// Fraction of code points classified as `Cc` (control).
    pub control: f64,
    /// Fraction of code points matching `char::is_whitespace`.
    pub whitespace: f64,
    /// Fraction of code points that are ASCII digits (0-9).
    pub digit: f64,
    /// Fraction of code points matching `char::is_alphabetic`.
    pub alphabetic: f64,
    /// Fraction of code points in Unicode major category `P`
    /// (any punctuation subcategory).
    pub punctuation: f64,
}

impl Ratios {
    /// Compute every ratio in one pass over `text`.
    ///
    /// ## Implementation
    ///
    /// Bench-driven redesign (2026-08-09): the ASCII fast path
    /// dispatches through a static 128-entry lookup table that
    /// packs all six classification bits into one `u8`. For each
    /// ASCII byte the hot path is one table lookup + six
    /// bit-tests, no external calls. Non-ASCII scalars still
    /// take the general-category path. Common inputs (log lines,
    /// identifiers, source code) are ASCII-dominated so this is
    /// the common case.
    ///
    /// # Panics
    ///
    /// The internal non-ASCII branch expects `text[i..]` to
    /// start with a valid non-ASCII scalar; this holds by
    /// construction (we only enter the branch when
    /// `bytes[i] >= 0x80` and `text` is a valid `&str`).
    /// The `expect` there is defense-in-depth and cannot fire
    /// on any input `Ratios::of` accepts.
    #[must_use]
    pub fn of(text: &str) -> Self {
        let mut total: u64 = 0;
        let mut printable = 0u64;
        let mut control = 0u64;
        let mut whitespace = 0u64;
        let mut digit = 0u64;
        let mut alphabetic = 0u64;
        let mut punctuation = 0u64;

        let bytes = text.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            if b < 0x80 {
                // ASCII fast path — one lookup, six bit-tests.
                let flags = ASCII_CLASS[b as usize];
                total += 1;
                if flags & F_PRINTABLE != 0 {
                    printable += 1;
                }
                if flags & F_CONTROL != 0 {
                    control += 1;
                }
                if flags & F_WHITESPACE != 0 {
                    whitespace += 1;
                }
                if flags & F_DIGIT != 0 {
                    digit += 1;
                }
                if flags & F_ALPHABETIC != 0 {
                    alphabetic += 1;
                }
                if flags & F_PUNCTUATION != 0 {
                    punctuation += 1;
                }
                i += 1;
            } else {
                // Non-ASCII — fall back to the general-category
                // path. The `text.as_bytes()[i..]` prefix is a
                // valid `&str` up to the next char boundary; parse
                // one scalar off it and advance.
                let rest = &text[i..];
                let c = rest.chars().next().expect("non-empty non-ASCII prefix");
                let clen = c.len_utf8();
                total += 1;
                let cat = get_general_category(c);
                if is_printable(cat) {
                    printable += 1;
                }
                if matches!(cat, GeneralCategory::Control) {
                    control += 1;
                }
                if c.is_whitespace() {
                    whitespace += 1;
                }
                // `is_ascii_digit` is always false for non-ASCII;
                // skip the check.
                if c.is_alphabetic() {
                    alphabetic += 1;
                }
                if matches!(MajorCategory::of(cat), MajorCategory::Punctuation) {
                    punctuation += 1;
                }
                i += clen;
            }
        }

        if total == 0 {
            return Self::default();
        }
        let t = total as f64;
        Self {
            printable: printable as f64 / t,
            control: control as f64 / t,
            whitespace: whitespace as f64 / t,
            digit: digit as f64 / t,
            alphabetic: alphabetic as f64 / t,
            punctuation: punctuation as f64 / t,
        }
    }
}

// ---------------------------------------------------------------------
// ASCII classification table.
// ---------------------------------------------------------------------

const F_PRINTABLE: u8 = 1 << 0;
const F_CONTROL: u8 = 1 << 1;
const F_WHITESPACE: u8 = 1 << 2;
const F_DIGIT: u8 = 1 << 3;
const F_ALPHABETIC: u8 = 1 << 4;
const F_PUNCTUATION: u8 = 1 << 5;

const ASCII_CLASS: [u8; 128] = build_ascii_class();

const fn build_ascii_class() -> [u8; 128] {
    let mut table = [0u8; 128];
    let mut b = 0u8;
    while b < 128 {
        let mut f = 0u8;
        // Control: 0x00..=0x1F and 0x7F.
        let is_control = b <= 0x1F || b == 0x7F;
        // Printable: everything not control/format/surrogate/
        // unassigned. In ASCII, "printable" = "not control".
        if !is_control {
            f |= F_PRINTABLE;
        }
        if is_control {
            f |= F_CONTROL;
        }
        // Whitespace (ASCII portion of char::is_whitespace):
        // 0x09..=0x0D and 0x20.
        if matches!(b, 0x09..=0x0D | 0x20) {
            f |= F_WHITESPACE;
        }
        // Digit: '0'..='9'.
        if b >= b'0' && b <= b'9' {
            f |= F_DIGIT;
        }
        // Alphabetic (ASCII portion): 'A'..='Z' and 'a'..='z'.
        if (b >= b'A' && b <= b'Z') || (b >= b'a' && b <= b'z') {
            f |= F_ALPHABETIC;
        }
        // Punctuation (ASCII portion): the six Unicode Po/Pd/Pc/
        // Ps/Pe/Pi/Pf variants that live in ASCII. Enumerated
        // rather than range-tested because ASCII punctuation is
        // scattered across three non-contiguous blocks.
        if matches!(
            b,
            b'!' | b'"'
                | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b'-'
                | b'.'
                | b'/'
                | b':'
                | b';'
                | b'?'
                | b'@'
                | b'['
                | b'\\'
                | b']'
                | b'_'
                | b'{'
                | b'}'
        ) {
            f |= F_PUNCTUATION;
        }
        table[b as usize] = f;
        b += 1;
    }
    table
}

fn is_printable(cat: GeneralCategory) -> bool {
    !matches!(
        cat,
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::Surrogate
            | GeneralCategory::Unassigned
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_all_zero() {
        let r = Ratios::of("");
        assert_eq!(r.printable, 0.0);
        assert_eq!(r.alphabetic, 0.0);
    }

    #[test]
    fn pure_ascii_letters() {
        let r = Ratios::of("hello");
        assert_eq!(r.printable, 1.0);
        assert_eq!(r.control, 0.0);
        assert_eq!(r.alphabetic, 1.0);
        assert_eq!(r.digit, 0.0);
        assert_eq!(r.punctuation, 0.0);
    }

    #[test]
    fn control_char_registers_as_control_not_printable() {
        // `\x07` is ASCII BEL — a control character.
        let r = Ratios::of("a\x07b");
        assert!((r.control - 1.0 / 3.0).abs() < 1e-9);
        assert!((r.printable - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn punctuation_and_whitespace_land_right() {
        let r = Ratios::of("hi, world");
        // 9 chars: h, i, ',', ' ', w, o, r, l, d
        // punctuation: 1 (comma)
        // whitespace: 1 (space)
        assert!((r.punctuation - 1.0 / 9.0).abs() < 1e-9);
        assert!((r.whitespace - 1.0 / 9.0).abs() < 1e-9);
    }

    #[test]
    fn ascii_digit_only() {
        let r = Ratios::of("12345");
        assert_eq!(r.digit, 1.0);
        assert_eq!(r.alphabetic, 0.0);
    }

    #[test]
    fn cjk_is_alphabetic_but_not_ascii_digit() {
        let r = Ratios::of("日本語");
        assert_eq!(r.alphabetic, 1.0);
        assert_eq!(r.digit, 0.0);
    }
}
