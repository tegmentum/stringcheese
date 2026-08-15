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
    /// Two-tier fast path:
    ///
    /// 1. **Bulk `str::is_ascii()` first** (SIMD-accelerated in
    ///    std). All-ASCII inputs skip the per-byte boundary
    ///    check entirely and run a straight loop over the
    ///    128-entry `ASCII_CLASS` table — one lookup + six
    ///    bit-tests per byte, no `>= 0x80` branch in the hot
    ///    loop. Common inputs (log lines, identifiers, source
    ///    code) are pure ASCII and land here.
    /// 2. **Mixed inputs** fall through to a per-byte dispatch
    ///    loop that keeps the ASCII fast path for the ASCII
    ///    bytes it encounters and only calls
    ///    `get_general_category` on the non-ASCII scalars.
    ///
    /// The ASCII table packs all six classification bits into
    /// one `u8`. Bench-driven redesign (2026-08-09) introduced
    /// the table; the bulk `is_ascii()` upfront dispatch
    /// followed on 2026-08-15 to lift the pure-ASCII throughput
    /// past the branch-tracked variant.
    ///
    /// The pure-ASCII path returns byte-for-byte identical
    /// counts to the mixed path on any all-ASCII input — see the
    /// `ascii_paths_match_mixed_path` differential test.
    ///
    /// # Panics
    ///
    /// The mixed-path branch expects `text[i..]` to start with a
    /// valid non-ASCII scalar; this holds by construction (we
    /// only enter the branch when `bytes[i] >= 0x80` and `text`
    /// is a valid `&str`). The `expect` there is
    /// defense-in-depth and cannot fire on any input
    /// `Ratios::of` accepts.
    #[must_use]
    pub fn of(text: &str) -> Self {
        if text.is_ascii() {
            return Self::of_ascii(text.as_bytes());
        }
        Self::of_mixed(text)
    }

    /// Pure-ASCII path. Straight loop over `ASCII_CLASS`, no
    /// per-byte boundary check. Caller guarantees the slice is
    /// valid ASCII (all bytes `< 0x80`); this holds when reached
    /// via [`Self::of`] because the entry point gates on
    /// `str::is_ascii()`.
    fn of_ascii(bytes: &[u8]) -> Self {
        let mut printable = 0u64;
        let mut control = 0u64;
        let mut whitespace = 0u64;
        let mut digit = 0u64;
        let mut alphabetic = 0u64;
        let mut punctuation = 0u64;

        for &b in bytes {
            // Mask to 7 bits so the compiler can drop the bounds
            // check on the 128-entry table (`is_ascii()` gate
            // already guarantees `b < 0x80`, but the mask makes
            // that visible at codegen time).
            let flags = ASCII_CLASS[(b & 0x7F) as usize];
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
        }

        let total = bytes.len() as u64;
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

    /// Mixed-input path. Per-byte dispatch: ASCII bytes take the
    /// table fast path, non-ASCII bytes parse one scalar and hit
    /// the general-category path.
    fn of_mixed(text: &str) -> Self {
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

    /// Differential test: on any all-ASCII input the pure-ASCII
    /// path (`of_ascii`) must return byte-for-byte identical
    /// counts to the mixed-path (`of_mixed`) walker. Guards the
    /// invariant the bulk `is_ascii()` gate relies on.
    #[test]
    fn ascii_paths_match_mixed_path() {
        // Enumerate every ASCII byte so the differential covers
        // control codes, printables, punctuation, digits, and
        // letters together — nothing left implicit.
        let all_ascii: String = (0u8..128).map(|b| b as char).collect();
        let a = Ratios::of_ascii(all_ascii.as_bytes());
        let b = Ratios::of_mixed(&all_ascii);
        assert_eq!(a, b, "ASCII fast path diverged from mixed path");

        // A few representative ASCII prose / mixed-class inputs
        // in addition to the exhaustive one — cheap belt & braces.
        for s in [
            "",
            "hello",
            "hi, world",
            "12345",
            "a\x07b",
            "The quick brown fox jumps over the lazy dog.\n",
            "\t\r\n ",
        ] {
            let a = Ratios::of_ascii(s.as_bytes());
            let b = Ratios::of_mixed(s);
            assert_eq!(a, b, "ASCII fast path diverged on {s:?}");
        }
    }

    /// Confirm the public entry `of` picks the fast path for
    /// all-ASCII input and the mixed path otherwise, and both
    /// dispatches produce the same visible result as a direct
    /// call to `of_mixed`.
    #[test]
    fn public_of_matches_mixed_walker_on_all_inputs() {
        for s in [
            "",
            "hello",
            "12345",
            "café",   // one non-ASCII scalar
            "日本語", // pure non-ASCII
            "abc日本",
        ] {
            let via_public = Ratios::of(s);
            let via_mixed = Ratios::of_mixed(s);
            assert_eq!(via_public, via_mixed, "public path diverged on {s:?}");
        }
    }
}
