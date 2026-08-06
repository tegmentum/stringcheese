//! Line-oriented operations on a string.
//!
//! Every function in this module treats the input as a sequence of
//! **lines** separated by `\n` or `\r\n`. Line terminators are recognized
//! by [`str::lines`]-style rules:
//!
//! - A `\n` terminates the preceding line.
//! - A `\r\n` pair terminates the preceding line (the `\r` is *not*
//!   yielded as part of the line content).
//! - A bare `\r` inside a line is treated as ordinary text — no line
//!   split.
//! - A **trailing** terminator does **not** produce a trailing empty
//!   line (`"a\n".lines()` yields exactly one item, `"a"`), matching
//!   [`str::lines`].
//!
//! # Iterator vs. owned-`String` outputs
//!
//! - **Zero-allocation iterators** ([`lines`], [`lines_with_terminators`],
//!   [`non_empty_lines`]) return borrowed sub-slices of the input.
//! - **Owned-`String` transformations** ([`trim_lines`], [`prefix_lines`],
//!   [`suffix_lines`], [`indent`], [`dedent`]) allocate exactly one
//!   output buffer sized for the result.
//!
//! # `no_std`
//!
//! The iterators require no features. The owned-`String` transformations
//! are gated on `feature = "alloc"`.

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------
// Iterators over the input.
// ---------------------------------------------------------------------

/// Iterates over `s` line-by-line, splitting at `\n` and `\r\n` and
/// **not** yielding the line terminators.
///
/// A trailing terminator does not produce a trailing empty line — this
/// matches [`str::lines`] semantics exactly (this function is a direct
/// wrapper).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// let all: Vec<&str> = lines::lines("a\nb\nc").collect();
/// assert_eq!(all, vec!["a", "b", "c"]);
///
/// // Trailing newline does not yield an extra empty line:
/// let all: Vec<&str> = lines::lines("a\n").collect();
/// assert_eq!(all, vec!["a"]);
///
/// // CRLF is recognized; \r is not part of the line content:
/// let all: Vec<&str> = lines::lines("a\r\nb").collect();
/// assert_eq!(all, vec!["a", "b"]);
/// ```
#[inline]
pub fn lines(s: &str) -> core::str::Lines<'_> {
    s.lines()
}

/// Iterates over `s` line-by-line, **including** each line's terminator
/// in the yielded slice.
///
/// A final line without a trailing terminator is yielded verbatim. A
/// `\r\n` pair is yielded as part of the preceding line, so callers can
/// reconstitute the input by concatenating the yielded slices.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// let all: Vec<&str> = lines::lines_with_terminators("a\nb\nc").collect();
/// assert_eq!(all, vec!["a\n", "b\n", "c"]);
///
/// // Trailing newline yields a "b\n" and stops.
/// let all: Vec<&str> = lines::lines_with_terminators("a\nb\n").collect();
/// assert_eq!(all, vec!["a\n", "b\n"]);
///
/// // CRLF is preserved intact.
/// let all: Vec<&str> = lines::lines_with_terminators("a\r\nb").collect();
/// assert_eq!(all, vec!["a\r\n", "b"]);
/// ```
#[must_use]
pub fn lines_with_terminators(s: &str) -> LinesWithTerminators<'_> {
    LinesWithTerminators { rest: s }
}

/// Iterator yielded by [`lines_with_terminators`].
///
/// See that function for construction; the type is opaque and only
/// interesting for its `Iterator<Item = &str>` impl.
#[derive(Debug, Clone)]
pub struct LinesWithTerminators<'a> {
    rest: &'a str,
}

impl<'a> Iterator for LinesWithTerminators<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            return None;
        }
        // Locate the next `\n`; that's the end of the current line.
        if let Some(idx) = self.rest.find('\n') {
            let (line, rest) = self.rest.split_at(idx + 1);
            self.rest = rest;
            Some(line)
        } else {
            let line = self.rest;
            self.rest = "";
            Some(line)
        }
    }
}

/// Iterates over `s` line-by-line, skipping any line that is empty.
///
/// "Empty" means literally zero-length after the terminator is stripped
/// — a line consisting only of whitespace is *not* skipped. Callers who
/// want to skip whitespace-only lines should filter the output of
/// [`lines`] with `.filter(|l| !l.trim().is_empty())`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// let all: Vec<&str> = lines::non_empty_lines("a\n\nb\n\nc").collect();
/// assert_eq!(all, vec!["a", "b", "c"]);
///
/// // A whitespace-only line is preserved (not zero-length).
/// let all: Vec<&str> = lines::non_empty_lines("a\n \nb").collect();
/// assert_eq!(all, vec!["a", " ", "b"]);
/// ```
pub fn non_empty_lines(s: &str) -> impl Iterator<Item = &str> {
    s.lines().filter(|line| !line.is_empty())
}

/// Returns the number of lines in `s` — the same count as
/// `lines(s).count()`.
///
/// A trailing terminator does not produce a trailing empty line, so
/// `count_lines("a\n") == 1`. An empty input has zero lines.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// assert_eq!(lines::count_lines(""), 0);
/// assert_eq!(lines::count_lines("a"), 1);
/// assert_eq!(lines::count_lines("a\n"), 1);
/// assert_eq!(lines::count_lines("a\nb\nc"), 3);
/// assert_eq!(lines::count_lines("a\n\nb"), 3);
/// ```
#[must_use]
pub fn count_lines(s: &str) -> usize {
    s.lines().count()
}

// ---------------------------------------------------------------------
// Owned-String transformations.
// ---------------------------------------------------------------------

#[cfg(feature = "alloc")]
use alloc::string::String;

/// Returns `s` with each line's leading and trailing whitespace stripped.
///
/// Terminators are preserved between lines; a trailing terminator on the
/// input yields a trailing terminator on the output. Whitespace inside a
/// line is left intact.
///
/// Trimming uses the Unicode `White_Space` property (same as
/// [`str::trim`]).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// assert_eq!(lines::trim_lines("  a  \n  b  \n"), "a\nb\n");
/// assert_eq!(lines::trim_lines("\ta\t\n\tb\t"), "a\nb");
/// // Whitespace-only lines collapse to empty lines.
/// assert_eq!(lines::trim_lines("a\n   \nb"), "a\n\nb");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn trim_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in lines_with_terminators(s) {
        let (content, term) = split_terminator(line);
        out.push_str(content.trim());
        out.push_str(term);
    }
    out
}

/// Returns `s` with `prefix` prepended to every line.
///
/// The prefix is added at the *start* of each line — before any content,
/// after (or at) the previous line's terminator. A trailing terminator on
/// the input does not cause a trailing prefixed empty line to be emitted.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// assert_eq!(lines::prefix_lines("a\nb\nc", "> "), "> a\n> b\n> c");
/// assert_eq!(lines::prefix_lines("a\nb\n", "> "), "> a\n> b\n");
/// assert_eq!(lines::prefix_lines("", "> "), "");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn prefix_lines(s: &str, prefix: &str) -> String {
    let line_count_estimate = s.matches('\n').count() + 1;
    let mut out = String::with_capacity(s.len() + prefix.len() * line_count_estimate);
    for line in lines_with_terminators(s) {
        out.push_str(prefix);
        out.push_str(line);
    }
    out
}

/// Returns `s` with `suffix` appended to every line, **before** the
/// line's terminator.
///
/// If a line has no terminator (the final line of a
/// no-trailing-newline input), the suffix is appended at end-of-string.
/// A trailing empty logical line (from a trailing terminator) does not
/// get a suffix — this matches the "no trailing empty line" convention.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// assert_eq!(lines::suffix_lines("a\nb\nc", ";"), "a;\nb;\nc;");
/// assert_eq!(lines::suffix_lines("a\nb\n", ";"), "a;\nb;\n");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn suffix_lines(s: &str, suffix: &str) -> String {
    let line_count_estimate = s.matches('\n').count() + 1;
    let mut out = String::with_capacity(s.len() + suffix.len() * line_count_estimate);
    for line in lines_with_terminators(s) {
        let (content, term) = split_terminator(line);
        out.push_str(content);
        out.push_str(suffix);
        out.push_str(term);
    }
    out
}

/// Adds `spaces` ASCII space characters at the start of every
/// **non-empty** line of `s`, returning the result.
///
/// Empty lines are left alone — mirroring Python's
/// [`textwrap.indent`](https://docs.python.org/3/library/textwrap.html#textwrap.indent)
/// with its default `predicate`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// assert_eq!(lines::indent("a\nb\nc", 2), "  a\n  b\n  c");
/// // Empty lines are not indented.
/// assert_eq!(lines::indent("a\n\nb", 4), "    a\n\n    b");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn indent(s: &str, spaces: usize) -> String {
    let mut prefix = String::with_capacity(spaces);
    for _ in 0..spaces {
        prefix.push(' ');
    }
    let mut out = String::with_capacity(s.len() + spaces * (s.matches('\n').count() + 1));
    for line in lines_with_terminators(s) {
        let (content, term) = split_terminator(line);
        if !content.is_empty() {
            out.push_str(&prefix);
        }
        out.push_str(content);
        out.push_str(term);
    }
    out
}

/// Removes the longest common leading-whitespace prefix from every
/// non-empty line of `s`, returning the result.
///
/// This is the [Python `textwrap.dedent`
/// algorithm](https://docs.python.org/3/library/textwrap.html#textwrap.dedent):
///
/// 1. Lines consisting entirely of whitespace are ignored when computing
///    the common prefix, but *are* trimmed to a lone newline in the
///    output.
/// 2. Tabs and spaces are treated as ordinary characters — a line
///    starting with `"\tfoo"` and another with `"    foo"` share no
///    common prefix.
///
/// If no lines have any leading whitespace (or if the input is empty),
/// the output equals the input.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::lines;
///
/// let src = "    hello\n    world\n";
/// assert_eq!(lines::dedent(src), "hello\nworld\n");
///
/// // Mixed depths → common prefix is the shorter one.
/// let src = "  a\n    b\n";
/// assert_eq!(lines::dedent(src), "a\n  b\n");
///
/// // Whitespace-only lines don't affect the common prefix.
/// let src = "    a\n\n    b\n";
/// assert_eq!(lines::dedent(src), "a\n\nb\n");
///
/// // No common prefix leaves everything unchanged.
/// let src = "a\n    b\n";
/// assert_eq!(lines::dedent(src), "a\n    b\n");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn dedent(s: &str) -> String {
    // Phase 1: find the longest common leading-whitespace prefix over
    // every non-whitespace-only line.
    let mut common: Option<&str> = None;
    for line in s.lines() {
        // Skip lines that are entirely whitespace — they don't
        // constrain the prefix.
        if line.chars().all(char::is_whitespace) {
            continue;
        }
        let leading = leading_whitespace(line);
        common = Some(match common {
            None => leading,
            Some(prev) => common_prefix(prev, leading),
        });
    }
    let prefix = common.unwrap_or("");

    // Phase 2: rewrite each line, stripping `prefix` from non-empty
    // lines and collapsing whitespace-only lines to a bare terminator.
    let mut out = String::with_capacity(s.len());
    for line in lines_with_terminators(s) {
        let (content, term) = split_terminator(line);
        if content.chars().all(char::is_whitespace) {
            // Whitespace-only line — collapse content to empty.
            out.push_str(term);
        } else if !prefix.is_empty() && content.starts_with(prefix) {
            out.push_str(&content[prefix.len()..]);
            out.push_str(term);
        } else {
            out.push_str(content);
            out.push_str(term);
        }
    }
    out
}

// ---------------------------------------------------------------------
// Internal helpers.
// ---------------------------------------------------------------------

/// Splits a line (as yielded by [`lines_with_terminators`]) into its
/// content and its terminator.
///
/// Returns `(content, terminator)` where the terminator is one of
/// `"\n"`, `"\r\n"`, or `""`.
#[cfg(feature = "alloc")]
fn split_terminator(line: &str) -> (&str, &str) {
    if let Some(without_lf) = line.strip_suffix('\n') {
        if let Some(without_crlf) = without_lf.strip_suffix('\r') {
            (without_crlf, &line[without_crlf.len()..])
        } else {
            (without_lf, &line[without_lf.len()..])
        }
    } else {
        (line, "")
    }
}

/// Returns the leading whitespace of `s` — the maximal prefix of `s`
/// consisting entirely of whitespace characters.
#[cfg(feature = "alloc")]
fn leading_whitespace(s: &str) -> &str {
    let end = s
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map_or(s.len(), |(i, _)| i);
    &s[..end]
}

/// Returns the longest string that is a prefix of both `a` and `b`.
#[cfg(feature = "alloc")]
fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a
        .char_indices()
        .zip(b.chars())
        .find(|((_, ca), cb)| ca != cb)
        .map_or_else(|| a.len().min(b.len()), |((i, _), _)| i);
    &a[..end]
}
