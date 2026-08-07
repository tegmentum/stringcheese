//! Word-wrap and reflow text at UAX #14 line-break opportunities.
//!
//! This module lets a caller ask *"how do I fit this text into a column
//! budget?"* — the layout-engine counterpart to
//! [`lines`](crate::lines)'s "how do I iterate the lines of this text?"
//! Break decisions defer to the Unicode Line Breaking Algorithm (UAX
//! #14) via [`stringcheese_unicode::line_breaks()`], so wraps land after a
//! space, after a hyphen, between two CJK ideographs, and around
//! zero-width space — and never inside a non-breaking-space run,
//! immediately before closing punctuation, or between the components of
//! a numeric expression such as `3.14`.
//!
//! # Width unit
//!
//! Column width is measured with [`unicode_width`], the widely-used
//! implementation of [Unicode Standard Annex #11][UAX11] *East Asian
//! Width*. A CJK ideograph counts as two columns, a Latin letter as
//! one, a combining mark or zero-width joiner as zero. This matches
//! what a fixed-cell terminal renderer expects. Callers who care about
//! byte or scalar counts specifically should use [`crate::pad`] or
//! [`crate::inspect`] instead.
//!
//! Some characters (notably ANSI SGR escape sequences and terminal
//! hyperlinks) count as zero columns for display but are not zero-width
//! per UAX #11 — this module does *not* strip or interpret them. A
//! caller that mixes SGR codes into wrapped text should either strip
//! them before wrapping or add width-tracking of its own.
//!
//! # Break semantics
//!
//! - **Mandatory breaks** ([`stringcheese_unicode::LineBreak::Mandatory`]
//!   — `\n`, `\r\n`, `\r`, `U+2028`, `U+2029`, `U+0085`, form feed,
//!   vertical tab) are **preserved** by [`wrap_at_width`] — a paragraph
//!   the caller intentionally split with `\n\n` stays split. To *undo*
//!   author-supplied hard breaks and re-flow only around soft breaks,
//!   use [`reflow`].
//! - **Allowed breaks** ([`stringcheese_unicode::LineBreak::Allowed`])
//!   are the candidate wrap points; the wrapper picks greedily
//!   first-fit — the *largest* allowed break offset whose column width
//!   stays within budget starts the next line.
//!
//! # Oversized words
//!
//! When a single UAX #14 segment is wider than the budget (a URL, a
//! `CamelCase` identifier, an over-long token) the default behavior is
//! to emit that segment on its own line and let it overflow the width
//! — matching Python's `textwrap` default. Callers who need a strict
//! never-exceed-width guarantee can opt in to force-breaking at
//! character boundaries via [`WrapOptions::break_words`]; the wrapper
//! then subdivides oversized segments so every output line's column
//! width is `≤ width` (up to the width of a single wide grapheme,
//! which is by definition indivisible).
//!
//! # Trailing whitespace
//!
//! Every output line has its trailing whitespace stripped — the space
//! that ends a soft-wrapped line, the `\n` that ends a mandatory-broken
//! line, and any run of ASCII / Unicode `White_Space` characters
//! preceding either. This matches `textwrap`, `fmt(1)`, and Python's
//! `textwrap.wrap`.
//!
//! [UAX11]: https://www.unicode.org/reports/tr11/

#![cfg(feature = "line-breaking")]

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use stringcheese_unicode::{LineBreak, line_breaks};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// ---------------------------------------------------------------------
// Public free functions.
// ---------------------------------------------------------------------

/// Wraps `text` so every returned line fits in `width` display columns,
/// returning owned lines.
///
/// Splits `text` at UAX #14 line-break opportunities using
/// [`stringcheese_unicode::line_breaks()`]. Mandatory breaks (`\n`,
/// `\r\n`, `U+2028`, ...) are preserved as line separators — the caller
/// gets one output entry per hard-broken line. Allowed breaks are used
/// greedily first-fit within each hard-broken segment.
///
/// Each output line has its trailing whitespace stripped. Oversized
/// tokens (a single word wider than `width`) are emitted on their own
/// line and allowed to exceed the budget; to force character-level
/// splits use [`WrapOptions::break_words`].
///
/// Returns an empty `Vec` for empty input.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::wrap;
///
/// let lines = wrap::wrap_at_width("The quick brown fox jumps.", 10);
/// assert_eq!(lines, vec!["The quick", "brown fox", "jumps."]);
///
/// // Mandatory breaks are preserved.
/// let lines = wrap::wrap_at_width("first line\nsecond", 20);
/// assert_eq!(lines, vec!["first line", "second"]);
/// ```
#[must_use]
pub fn wrap_at_width(text: &str, width: usize) -> Vec<String> {
    wrap_at_width_borrowed(text, width)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

/// Wraps `text` so every returned slice fits in `width` display
/// columns, returning **borrowed** sub-slices of `text`.
///
/// The zero-copy sibling of [`wrap_at_width`]. Every returned `&str` is
/// a sub-slice of the input, so callers that can retain the source
/// string pay no per-line `String` allocation. The one small allocation
/// this function *does* perform is the spine `Vec` holding the slice
/// pointers.
///
/// Semantics for break selection, oversized-token handling, and
/// trailing-whitespace stripping are identical to [`wrap_at_width`].
///
/// Force-breaking oversized tokens (i.e., the `break_words` option) is
/// *available* here as well — the character-level splits it produces
/// also fall on valid UTF-8 boundaries, so the returned slices remain
/// valid `&str`s. Reach through [`WrapOptions::wrap_borrowed`] for
/// that.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::wrap;
///
/// let text = "hello wide world";
/// let lines = wrap::wrap_at_width_borrowed(text, 8);
/// assert_eq!(lines, vec!["hello", "wide", "world"]);
/// // Slices point into the original string.
/// assert_eq!(lines[0].as_ptr(), text.as_ptr());
/// ```
#[must_use]
pub fn wrap_at_width_borrowed(text: &str, width: usize) -> Vec<&str> {
    let opts = WrapOptions::new(width);
    wrap_borrowed_impl(text, &opts)
}

/// Joins the output of [`wrap_at_width`] with `\n`, returning a single
/// wrapped `String`.
///
/// Equivalent to `wrap_at_width(text, width).join("\n")`, only without
/// the intermediate `Vec<String>` allocation.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::wrap;
///
/// let out = wrap::fill("The quick brown fox jumps.", 10);
/// assert_eq!(out, "The quick\nbrown fox\njumps.");
/// ```
#[must_use]
pub fn fill(text: &str, width: usize) -> String {
    join_lines(&wrap_at_width_borrowed(text, width))
}

/// Undoes existing single line breaks and re-wraps `text` at `width`
/// columns, preserving paragraph boundaries.
///
/// A **paragraph boundary** is any run of two or more consecutive line
/// terminators (`\n`, `\r\n`, or `\r`); intra-paragraph line breaks are
/// collapsed to a single space and the paragraph is then re-wrapped
/// greedily at `width`. Adjacent whitespace inside a paragraph is
/// collapsed to a single ASCII space.
///
/// Output paragraphs are joined with `\n\n`, matching the input's
/// paragraph-boundary marker.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::wrap;
///
/// let src = "Line one\nline two of the same paragraph.\n\nA second paragraph here.";
/// let out = wrap::reflow(src, 30);
/// assert_eq!(
///     out,
///     "Line one line two of the same\nparagraph.\n\nA second paragraph here.",
/// );
/// ```
#[must_use]
pub fn reflow(text: &str, width: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    let paragraphs = split_paragraphs(text);
    let mut out = String::new();
    for (i, para) in paragraphs.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        let wrapped = fill(para, width);
        out.push_str(&wrapped);
    }
    out
}

// ---------------------------------------------------------------------
// WrapOptions builder.
// ---------------------------------------------------------------------

/// Configuration for [`wrap_at_width`]-style operations.
///
/// A builder that composes the tunables the free functions do not
/// expose: forcing character-level splits inside oversized tokens
/// ([`break_words`](Self::break_words)) and prepending per-line indents
/// ([`initial_indent`](Self::initial_indent) /
/// [`subsequent_indent`](Self::subsequent_indent)).
///
/// # Indent widths
///
/// Both indent strings' column widths count *against* the wrap budget
/// — a caller who wants "wrap at 80 with a 4-space indent on
/// continuation lines" and gets 76 columns of usable text on those
/// lines. The initial indent applies only to the first output line;
/// the subsequent indent applies to every line after that.
///
/// The `initial_indent` and `subsequent_indent` slices are typed
/// `&'static str` so `WrapOptions` implements `Copy` and can be reused
/// across many `wrap()` / `fill()` calls without allocating. Callers
/// who need dynamic indents should build owned indent strings and use
/// them at the call site.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::wrap::WrapOptions;
///
/// let opts = WrapOptions::new(15)
///     .initial_indent("- ")
///     .subsequent_indent("  ");
/// let out = opts.fill("hello world how are you today");
/// assert_eq!(out, "- hello world\n  how are you\n  today");
///
/// // With break_words a super-long token is force-split at char
/// // boundaries; the result never exceeds `width` (except for a
/// // single indivisible grapheme wider than the budget).
/// let opts = WrapOptions::new(5).break_words(true);
/// let out = opts.wrap("superlongword");
/// assert_eq!(out, vec!["super", "longw", "ord"]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrapOptions {
    width: usize,
    break_words: bool,
    initial_indent: &'static str,
    subsequent_indent: &'static str,
}

impl WrapOptions {
    /// Creates a fresh `WrapOptions` with the given column budget and
    /// every other setting at its default: `break_words = false`, no
    /// initial or subsequent indent.
    #[must_use]
    pub const fn new(width: usize) -> Self {
        Self {
            width,
            break_words: false,
            initial_indent: "",
            subsequent_indent: "",
        }
    }

    /// Sets the target column width.
    #[must_use]
    pub const fn width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    /// Sets whether oversized tokens are force-broken at character
    /// boundaries.
    ///
    /// - `false` (default) — a token wider than the width budget is
    ///   emitted on its own line and allowed to overflow, matching
    ///   Python `textwrap`'s default.
    /// - `true` — the token is force-split at character boundaries so
    ///   every output line stays `≤ width` (up to a single indivisible
    ///   wide grapheme).
    #[must_use]
    pub const fn break_words(mut self, on: bool) -> Self {
        self.break_words = on;
        self
    }

    /// Sets the prefix prepended to the first output line.
    ///
    /// Counts against the wrap budget on the first line only.
    #[must_use]
    pub const fn initial_indent(mut self, indent: &'static str) -> Self {
        self.initial_indent = indent;
        self
    }

    /// Sets the prefix prepended to every output line **except** the
    /// first.
    ///
    /// Counts against the wrap budget on every continuation line.
    #[must_use]
    pub const fn subsequent_indent(mut self, indent: &'static str) -> Self {
        self.subsequent_indent = indent;
        self
    }

    /// Wraps `text` per the current options, returning owned lines with
    /// indents applied.
    #[must_use]
    pub fn wrap(&self, text: &str) -> Vec<String> {
        let borrowed = wrap_borrowed_impl(text, self);
        if self.initial_indent.is_empty() && self.subsequent_indent.is_empty() {
            return borrowed.into_iter().map(ToOwned::to_owned).collect();
        }
        let mut out = Vec::with_capacity(borrowed.len());
        for (i, line) in borrowed.iter().enumerate() {
            let indent = if i == 0 {
                self.initial_indent
            } else {
                self.subsequent_indent
            };
            let mut s = String::with_capacity(indent.len() + line.len());
            s.push_str(indent);
            s.push_str(line);
            out.push(s);
        }
        out
    }

    /// Wraps `text` per the current options and returns borrowed
    /// sub-slices of `text`.
    ///
    /// This bypasses the initial / subsequent indent settings — the
    /// returned slices are pure sub-slices of `text`, which is
    /// incompatible with prepending an indent that isn't itself part of
    /// `text`. Callers who need indented output should use
    /// [`wrap`](Self::wrap) or [`fill`](Self::fill) instead.
    #[must_use]
    pub fn wrap_borrowed<'a>(&self, text: &'a str) -> Vec<&'a str> {
        wrap_borrowed_impl(text, self)
    }

    /// Wraps `text` per the current options and joins the result with
    /// `\n`.
    #[must_use]
    pub fn fill(&self, text: &str) -> String {
        let lines = self.wrap(text);
        join_owned_lines(&lines)
    }
}

impl Default for WrapOptions {
    /// Returns a builder with `width = 80`, the traditional terminal
    /// column count, and every other setting at its default.
    fn default() -> Self {
        Self::new(80)
    }
}

// ---------------------------------------------------------------------
// Internal — core wrap engine.
// ---------------------------------------------------------------------

/// Column width of `s` per Unicode East Asian Width.
#[inline]
fn col_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Column width of a single `char` per Unicode East Asian Width.
///
/// Combining marks and other zero-width scalars return 0, so a control
/// character counted here does not consume budget — this is what a
/// wrapping caller wants (the caller pays for what the terminal will
/// draw).
#[inline]
fn char_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Strips trailing whitespace and terminator characters from `s`,
/// returning a sub-slice.
///
/// Uses [`str::trim_end`] (Unicode `White_Space`) so `\n`, `\r`, `\t`,
/// U+2028, U+2029, and the whole run of end-of-line and space
/// characters that a soft break or mandatory break may have left dangling
/// are consumed. The returned slice is a sub-slice of `s` — no
/// allocation.
#[inline]
fn trim_trailing(s: &str) -> &str {
    s.trim_end()
}

/// Core greedy first-fit wrap over UAX #14 break opportunities.
///
/// Returns borrowed sub-slices of `text`. Applied unconditionally by
/// every public entry point; the indent / owned-`String` transforms
/// happen in wrappers above this.
fn wrap_borrowed_impl<'a>(text: &'a str, opts: &WrapOptions) -> Vec<&'a str> {
    let mut out: Vec<&'a str> = Vec::new();
    if text.is_empty() {
        return out;
    }

    let breaks: Vec<(usize, LineBreak)> = line_breaks(text).collect();
    // Break offsets partition the input into UAX #14 segments; the
    // wrapper composes segments into lines.
    let n = breaks.len();
    let mut i = 0usize;
    let mut cur_start = 0usize;

    // Column budget: the first output line uses `initial_indent`'s
    // width, subsequent lines use `subsequent_indent`'s. Indents are
    // prepended by the owned-`String` wrapper; the borrowed engine only
    // needs to know how much budget they consume so break decisions
    // still land on slices that fit.
    let mut used_lines = 0usize;

    while i < n {
        let indent_w = if used_lines == 0 {
            col_width(opts.initial_indent)
        } else {
            col_width(opts.subsequent_indent)
        };
        let budget = opts.width.saturating_sub(indent_w);

        // Find the largest break offset from `cur_start` whose trimmed
        // slice fits within `budget` columns. Also stop immediately on
        // a mandatory break — mandatory breaks close the current line
        // unconditionally, even if the slice up to them overflows.
        let mut chosen_j: Option<usize> = None;
        let mut j = i;
        while j < n {
            let (off, kind) = breaks[j];
            let slice = &text[cur_start..off];
            let trimmed = trim_trailing(slice);
            let w = col_width(trimmed);
            if kind == LineBreak::Mandatory {
                if w <= budget || chosen_j.is_none() {
                    // Either the slice fits, or nothing has fit yet —
                    // in the latter case we still consume the mandatory
                    // break (the segment is oversized and we handle it
                    // in the emit path below).
                    chosen_j = Some(j);
                }
                break;
            }
            if w <= budget {
                chosen_j = Some(j);
                j += 1;
            } else {
                break;
            }
        }

        let cj = chosen_j.unwrap_or(i);
        let off = breaks[cj].0;
        let slice = &text[cur_start..off];
        let trimmed = trim_trailing(slice);
        if opts.break_words && !trimmed.is_empty() && col_width(trimmed) > budget {
            // Oversized segment: either the caller opted in to
            // `break_words` and we force-split at char boundaries so
            // every output line stays within budget, or the branch
            // above is skipped and we emit the oversized slice as-is
            // (matching Python `textwrap`'s default).
            push_force_broken(&mut out, trimmed, budget, &mut used_lines);
        } else {
            out.push(trimmed);
            used_lines += 1;
        }
        cur_start = off;
        i = cj + 1;
    }

    out
}

/// Force-breaks `s` at UTF-8 char boundaries so every pushed sub-slice
/// has column width `≤ budget`.
///
/// A single grapheme wider than `budget` — e.g. an emoji wider than a
/// one-column target — cannot be subdivided; the pushed slice will
/// contain that one grapheme and slightly exceed the budget. The
/// alternative (drop the grapheme) would silently lose data.
fn push_force_broken<'a>(
    out: &mut Vec<&'a str>,
    s: &'a str,
    budget: usize,
    used_lines: &mut usize,
) {
    if budget == 0 {
        // Degenerate — every char exceeds the budget. Emit one char
        // per line so at least the output is stable and lossless.
        for (i, ch) in s.char_indices() {
            let end = i + ch.len_utf8();
            out.push(&s[i..end]);
            *used_lines += 1;
        }
        return;
    }
    let mut chunk_start = 0usize;
    let mut chunk_w = 0usize;
    let mut last_end = 0usize;
    for (i, ch) in s.char_indices() {
        let w = char_width(ch);
        if i > chunk_start && chunk_w + w > budget {
            out.push(&s[chunk_start..i]);
            *used_lines += 1;
            chunk_start = i;
            chunk_w = 0;
        }
        chunk_w += w;
        last_end = i + ch.len_utf8();
    }
    if chunk_start < last_end {
        out.push(&s[chunk_start..last_end]);
        *used_lines += 1;
    }
}

/// Joins borrowed line slices with `\n`.
fn join_lines(lines: &[&str]) -> String {
    let total: usize = lines.iter().map(|l| l.len()).sum::<usize>() + lines.len().saturating_sub(1);
    let mut out = String::with_capacity(total);
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(l);
    }
    out
}

/// Joins owned line strings with `\n`.
fn join_owned_lines(lines: &[String]) -> String {
    let total: usize = lines.iter().map(String::len).sum::<usize>() + lines.len().saturating_sub(1);
    let mut out = String::with_capacity(total);
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(l);
    }
    out
}

// ---------------------------------------------------------------------
// Internal — paragraph split for reflow.
// ---------------------------------------------------------------------

/// Splits `text` into paragraphs at runs of two or more consecutive
/// line terminators, collapsing intra-paragraph line breaks and
/// runs of internal whitespace into single ASCII spaces.
///
/// Recognized terminators: `\n`, `\r\n`, and bare `\r`. A run of any
/// mix of these — with `\r\n` counted as one terminator — of length ≥ 2
/// starts a new paragraph.
fn split_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();
    // Number of terminators seen since the last non-terminator, non-
    // whitespace character. Used to detect the "two or more consecutive
    // newlines = paragraph break" rule.
    let mut newlines_since_content = 0usize;

    let mut iter = text.char_indices().peekable();
    while let Some((_, ch)) = iter.next() {
        // Recognize CRLF as a single terminator; a bare \r or a bare
        // \n each count as one.
        if ch == '\r' {
            // Peek next; if \n, consume it too.
            if let Some(&(_, next)) = iter.peek() {
                if next == '\n' {
                    iter.next();
                }
            }
            newlines_since_content += 1;
            if newlines_since_content >= 2 && !current.is_empty() {
                flush_paragraph(&mut paragraphs, &mut current);
            }
            continue;
        }
        if ch == '\n' {
            newlines_since_content += 1;
            if newlines_since_content >= 2 && !current.is_empty() {
                flush_paragraph(&mut paragraphs, &mut current);
            }
            continue;
        }
        // U+2028 LINE SEPARATOR and U+2029 PARAGRAPH SEPARATOR are also
        // hard terminators per UAX #14. For reflow they behave the same
        // as `\n` — one is a soft-to-space, two-or-more start a new
        // paragraph.
        if ch == '\u{2028}' || ch == '\u{2029}' {
            newlines_since_content += 1;
            if newlines_since_content >= 2 && !current.is_empty() {
                flush_paragraph(&mut paragraphs, &mut current);
            }
            continue;
        }
        if ch.is_whitespace() {
            // Collapse a run of internal whitespace to a single ASCII
            // space, but only inside a paragraph (not at its start).
            if !current.is_empty() && !current.ends_with(' ') {
                current.push(' ');
            }
            // If we had exactly one newline pending and now see other
            // whitespace, the pending newline has already been absorbed
            // as a space by the branch above — so reset the counter to
            // 1 so a follow-up newline still triggers a paragraph
            // boundary if we've seen two overall.
            continue;
        }
        // Non-terminator, non-whitespace: append to current paragraph.
        if newlines_since_content == 1 && !current.is_empty() && !current.ends_with(' ') {
            // Single pending newline inside a paragraph collapses to a
            // space between the surrounding tokens.
            current.push(' ');
        }
        newlines_since_content = 0;
        current.push(ch);
    }
    if !current.is_empty() {
        // Strip any trailing space we may have accumulated.
        while current.ends_with(' ') {
            current.pop();
        }
        if !current.is_empty() {
            paragraphs.push(current);
        }
    }
    paragraphs
}

/// Flushes `current` into `paragraphs`, stripping trailing spaces and
/// resetting `current` to empty.
fn flush_paragraph(paragraphs: &mut Vec<String>, current: &mut String) {
    while current.ends_with(' ') {
        current.pop();
    }
    if !current.is_empty() {
        // Push by clone-then-clear so `current`'s capacity is retained.
        paragraphs.push(core::mem::take(current));
    }
}

// ---------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    // -------- empty / trivial --------

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(wrap_at_width("", 10), Vec::<String>::new());
        assert_eq!(wrap_at_width_borrowed("", 10), Vec::<&str>::new());
        assert_eq!(fill("", 10), "");
        assert_eq!(reflow("", 10), "");
    }

    #[test]
    fn short_input_returns_single_line() {
        assert_eq!(wrap_at_width("hello", 10), vec!["hello".to_owned()]);
    }

    #[test]
    fn borrowed_lines_alias_the_input() {
        let text = "hello wide world";
        let lines = wrap_at_width_borrowed(text, 8);
        // The first slice should point into `text`, not a fresh alloc.
        assert_eq!(lines[0], "hello");
        let text_start = text.as_ptr() as usize;
        let slice_start = lines[0].as_ptr() as usize;
        assert!(slice_start >= text_start && slice_start < text_start + text.len());
    }

    // -------- basic ASCII greedy wrap --------

    #[test]
    fn basic_ascii_wrap() {
        let out = wrap_at_width("The quick brown fox jumps over the lazy dog.", 15);
        assert_eq!(
            out,
            vec![
                "The quick brown".to_owned(),
                "fox jumps over".to_owned(),
                "the lazy dog.".to_owned(),
            ],
        );
    }

    #[test]
    fn wrap_at_exact_width() {
        // "hello" is exactly 5 columns, wrap at 5 gives one line.
        assert_eq!(wrap_at_width("hello", 5), vec!["hello".to_owned()]);
        // Two 5-char words at width 5 wrap to two lines.
        assert_eq!(
            wrap_at_width("hello world", 5),
            vec!["hello".to_owned(), "world".to_owned()],
        );
    }

    #[test]
    fn wrap_at_width_gt_input_returns_single_line() {
        assert_eq!(
            wrap_at_width("The quick brown fox.", 200),
            vec!["The quick brown fox.".to_owned()],
        );
    }

    // -------- oversized tokens --------

    #[test]
    fn word_longer_than_width_default_overflows() {
        // Default: emit the oversized word on its own line and let it
        // overflow.
        let out = wrap_at_width("hi superlongword ok", 5);
        assert_eq!(
            out,
            vec!["hi".to_owned(), "superlongword".to_owned(), "ok".to_owned(),],
        );
    }

    #[test]
    fn word_longer_than_width_break_words_true_splits() {
        let opts = WrapOptions::new(5).break_words(true);
        let out = opts.wrap("superlongword");
        assert_eq!(
            out,
            vec!["super".to_owned(), "longw".to_owned(), "ord".to_owned()],
        );
    }

    #[test]
    fn break_words_respects_char_boundaries_utf8() {
        // A word of 6 accented letters at width 3, break_words=true,
        // must split at UTF-8 char boundaries — never mid-codepoint.
        let opts = WrapOptions::new(3).break_words(true);
        let out = opts.wrap("ééééé");
        // 5 é's at width 3 → ["ééé", "éé"].
        assert_eq!(out, vec!["ééé".to_owned(), "éé".to_owned()]);
        for line in &out {
            // Reconstructing the &str proves the byte slice is on a
            // valid UTF-8 boundary.
            let _ = core::str::from_utf8(line.as_bytes()).unwrap();
        }
    }

    // -------- mandatory breaks --------

    #[test]
    fn mandatory_breaks_are_preserved() {
        let out = wrap_at_width("first line\nsecond line", 100);
        assert_eq!(out, vec!["first line".to_owned(), "second line".to_owned()]);
    }

    #[test]
    fn crlf_treated_as_single_mandatory_break() {
        let out = wrap_at_width("a\r\nb", 10);
        assert_eq!(out, vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn empty_line_between_mandatory_breaks_preserved() {
        let out = wrap_at_width("a\n\nb", 10);
        assert_eq!(out, vec!["a".to_owned(), String::new(), "b".to_owned()],);
    }

    // -------- fill --------

    #[test]
    fn fill_joins_with_newlines() {
        let out = fill("The quick brown fox.", 10);
        assert_eq!(out, "The quick\nbrown fox.");
    }

    #[test]
    fn fill_preserves_mandatory_breaks_as_newlines() {
        let out = fill("a\nb", 100);
        assert_eq!(out, "a\nb");
    }

    // -------- reflow --------

    #[test]
    fn reflow_collapses_single_newlines_to_space() {
        let src = "Line one\nline two";
        // Wide width — everything fits on one line after reflow.
        let out = reflow(src, 100);
        assert_eq!(out, "Line one line two");
    }

    #[test]
    fn reflow_preserves_paragraph_boundaries() {
        let src = "para one line one\npara one line two\n\npara two here";
        let out = reflow(src, 100);
        assert_eq!(out, "para one line one para one line two\n\npara two here");
    }

    #[test]
    fn reflow_collapses_triple_newlines_to_single_paragraph_boundary() {
        // Three consecutive newlines → still one paragraph boundary.
        let src = "one\n\n\ntwo";
        assert_eq!(reflow(src, 100), "one\n\ntwo");
    }

    #[test]
    fn reflow_rewraps_at_target_width() {
        let src = "one two three\nfour five six seven eight";
        let out = reflow(src, 12);
        // After reflow all words are joined by spaces; then wrapped
        // greedy first-fit at 12 cols:
        //   "one two"     (7)  — "one two three" (13) exceeds budget.
        //   "three four"  (10) — "three four five" (15) exceeds budget.
        //   "five six"    (8)  — "five six seven" (14) exceeds budget.
        //   "seven eight" (11) — the whole remaining input fits.
        assert_eq!(out, "one two\nthree four\nfive six\nseven eight");
    }

    #[test]
    fn reflow_empty_input() {
        assert_eq!(reflow("", 10), "");
    }

    #[test]
    fn reflow_only_whitespace_input() {
        assert_eq!(reflow("   \n\n   ", 10), "");
    }

    #[test]
    fn reflow_crlf_paragraph_boundary() {
        let src = "one\r\n\r\ntwo";
        assert_eq!(reflow(src, 100), "one\n\ntwo");
    }

    // -------- multibyte UTF-8 --------

    #[test]
    fn multibyte_never_split_mid_codepoint() {
        // "café world hôtel" — the é and ô are 2 bytes each.
        let text = "café world hôtel";
        let out = wrap_at_width_borrowed(text, 7);
        // Every returned slice must be valid UTF-8 (i.e., not split
        // mid-codepoint).
        for line in &out {
            core::str::from_utf8(line.as_bytes()).unwrap();
        }
        // The reconstructable words are still intact.
        let joined: String = out.join(" ");
        assert!(joined.contains("café"));
        assert!(joined.contains("hôtel"));
    }

    #[test]
    fn multibyte_column_width_accurate() {
        // "café" is 4 columns (é is width 1) but 5 UTF-8 bytes.
        // At width 4 it fits on one line.
        let out = wrap_at_width("café hôtel", 4);
        assert_eq!(out, vec!["café".to_owned(), "hôtel".to_owned()]);
    }

    #[test]
    fn cjk_double_width_respected() {
        // Two CJK ideographs each count as 2 columns. At width 4, two
        // fit per line; at width 3, only one fits per line.
        let text = "漢字 漢字 漢字";
        let out4 = wrap_at_width(text, 4);
        // Wait — with 3 pairs of "漢字" separated by spaces this is
        // "漢字 漢字 漢字" — greedy first-fit at width 4:
        //   "漢字" (4 cols) fits; adding " 漢字" makes 9 cols → wrap.
        //   next: "漢字" fits; adding " 漢字" makes 9 → wrap.
        //   last: "漢字".
        assert_eq!(
            out4,
            vec!["漢字".to_owned(), "漢字".to_owned(), "漢字".to_owned()],
        );
    }

    // -------- indents --------

    #[test]
    fn initial_indent_applied_only_to_first_line() {
        let opts = WrapOptions::new(20).initial_indent(">> ");
        let out = opts.wrap("one two three four five six");
        // "one two three four" = 18 cols; first line budget = 20 - 3 = 17
        // so "one two three" (13) fits, "one two three four" (18) doesn't.
        assert_eq!(out[0], ">> one two three");
        // Continuation lines have no indent.
        for (i, line) in out.iter().enumerate().skip(1) {
            assert!(
                !line.starts_with(">> "),
                "line {i} unexpectedly starts with >>: {line:?}",
            );
        }
    }

    #[test]
    fn subsequent_indent_applied_from_second_line_on() {
        let opts = WrapOptions::new(20).subsequent_indent("  ");
        let out = opts.wrap("one two three four five six seven");
        assert!(!out[0].starts_with(' '));
        for line in out.iter().skip(1) {
            assert!(line.starts_with("  "), "expected indent on: {line:?}");
        }
    }

    #[test]
    fn both_indents_together() {
        let opts = WrapOptions::new(15)
            .initial_indent("- ")
            .subsequent_indent("  ");
        let out = opts.fill("hello world how are you today");
        assert_eq!(out, "- hello world\n  how are you\n  today");
    }

    #[test]
    fn wrap_options_default_is_width_80() {
        let opts = WrapOptions::default();
        assert_eq!(opts.width, 80);
        assert!(!opts.break_words);
        assert_eq!(opts.initial_indent, "");
        assert_eq!(opts.subsequent_indent, "");
    }

    // -------- trailing whitespace stripping --------

    #[test]
    fn trailing_whitespace_stripped_from_soft_wrapped_lines() {
        let out = wrap_at_width("aaa bbb ccc", 4);
        for line in &out {
            assert!(!line.ends_with(' '), "trailing space on: {line:?}");
        }
    }

    #[test]
    fn trailing_terminator_not_in_output() {
        let out = wrap_at_width("hello world\n", 100);
        assert_eq!(out, vec!["hello world".to_owned()]);
    }

    // -------- corner: zero-width chars --------

    #[test]
    fn combining_marks_count_zero_width() {
        // "e" + combining acute (U+0301) is 3 bytes, 2 scalars, but 1
        // column (the base 'e' is width 1, the combining mark is 0).
        // So "e\u{301}llo" is 4 columns even though it's 6 UTF-8 bytes
        // and 5 scalars. At width 4 it fits on a single line.
        let src = "e\u{301}llo";
        assert_eq!(col_width(src), 4);
        assert_eq!(src.chars().count(), 5); // scalar count differs
        let out = wrap_at_width(src, 4);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], src);
    }

    // -------- edge: width 0 --------

    #[test]
    fn width_zero_places_every_break_on_own_line() {
        let out = wrap_at_width("a b c", 0);
        // With zero budget, every non-empty segment overflows and lands
        // on its own line.
        assert_eq!(out, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn width_zero_break_words_splits_every_grapheme() {
        let opts = WrapOptions::new(0).break_words(true);
        let out = opts.wrap("abc");
        assert_eq!(out, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    }

    // -------- WrapOptions immutability --------

    #[test]
    fn wrap_options_reusable_across_calls() {
        let opts = WrapOptions::new(10);
        let a = opts.wrap("hello world foo");
        let b = opts.wrap("bar baz qux quux");
        // Both calls succeed with independent output.
        assert!(!a.is_empty());
        assert!(!b.is_empty());
    }
}
