//! Shared globset+regex plumbing used by [`crate::wildcard`] and
//! [`crate::glob`].
//!
//! Both wildcard and glob compile through the same pipeline:
//!
//! 1. Parse the pattern with [`globset::GlobBuilder`], with
//!    `literal_separator(false)` (single `*` matches path
//!    separators — StringCheese isn't path-scoped) and
//!    `backslash_escape(true)` (matches our existing escape rules).
//! 2. Extract the resulting regex string via [`globset::Glob::regex`].
//! 3. Transform it based on [`MatchUnit`]:
//!    - `Bytes` — keep globset's output verbatim (`(?-u)` byte
//!      mode; `.` matches one byte).
//!    - `CodePoints` — replace every unescaped `.` with a UTF-8-
//!      scalar matcher so `?` consumes one Unicode scalar.
//!      Character-class content stays byte-oriented (ASCII
//!      classes work identically in either mode).
//!    - `Graphemes` — construction panics upstream.
//! 4. Anchoring:
//!    - Anchored — regex keeps its `^…$` — [`Regex::is_match`] gives
//!      whole-string matching.
//!    - Anywhere — strip anchors so [`Regex::find_iter`] scans.

use alloc::boxed::Box;
use alloc::string::{String, ToString};

use regex::bytes::Regex;

use crate::{Match, MatchUnit};

/// One reason a pattern rejected at compile time. Callers surface it
/// as a panic in the current API — patterns are treated as programmer
/// input; the panic message identifies what globset objected to.
#[derive(Debug)]
pub(crate) enum CompileError {
    Globset(globset::Error),
    Regex(regex::Error),
}

impl core::fmt::Display for CompileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Globset(e) => write!(f, "glob syntax error: {e}"),
            Self::Regex(e) => write!(f, "regex compile error: {e}"),
        }
    }
}

/// Compile a globset-syntax pattern to a [`Regex`], respecting the
/// caller's [`MatchUnit`] and anchoring choice.
pub(crate) fn compile(
    pattern: &str,
    unit: MatchUnit,
    anchored: bool,
) -> Result<Regex, CompileError> {
    let glob = globset::GlobBuilder::new(pattern)
        .literal_separator(false)
        .backslash_escape(true)
        .empty_alternates(false)
        .build()
        .map_err(CompileError::Globset)?;

    let mut body = glob.regex().to_string();

    // Globset always emits its regex in `(?-u)` byte mode with any
    // multi-byte literals expanded to `\xNN\xNN\xNN` byte sequences
    // (see the shape below). We stay in byte mode — `regex::bytes::
    // Regex` — for both `MatchUnit`s and change only how `?` and
    // `*` expand:
    //
    //   Bytes       — `.` matches one byte (globset's default; keep
    //                  the pattern verbatim).
    //   CodePoints  — replace unescaped `.` with a UTF-8-scalar
    //                  matcher so `?` consumes one code point,
    //                  regardless of scalar width.
    //
    // Literal `.` from the input arrives escaped as `\.`; class
    // interiors like `[a-z]` don't contain bare `.`. So a targeted
    // replace of standalone `.` is safe against everything globset
    // emits today.
    if matches!(unit, MatchUnit::CodePoints) {
        body = replace_dot_with_utf8_scalar(&body);
    }

    if !anchored {
        body = strip_anchors(&body);
    }

    Regex::new(&body).map_err(CompileError::Regex)
}

/// Regex fragment matching exactly one UTF-8-encoded Unicode
/// scalar (1–4 bytes). Used to replace bare `.` in the globset
/// output for [`MatchUnit::CodePoints`] mode.
const UTF8_SCALAR: &str = "(?:[\\x00-\\x7F]|[\\xC2-\\xDF][\\x80-\\xBF]|[\\xE0-\\xEF][\\x80-\\xBF]{2}|[\\xF0-\\xF4][\\x80-\\xBF]{3})";

fn replace_dot_with_utf8_scalar(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;
    // Track whether we're inside a `[...]` character class — bare
    // `.` there is a literal dot, not the any-atom metachar.
    let mut in_class = false;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && i + 1 < bytes.len() {
            // Pass any escape through as-is (`\.`, `\xNN`, `\\`, …).
            out.push('\\');
            out.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        if b == b'[' && !in_class {
            in_class = true;
            out.push('[');
            i += 1;
            continue;
        }
        if b == b']' && in_class {
            in_class = false;
            out.push(']');
            i += 1;
            continue;
        }
        if b == b'.' && !in_class {
            out.push_str(UTF8_SCALAR);
            i += 1;
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out
}

fn strip_anchors(body: &str) -> String {
    // Body looks like one of:
    //   "(?-u)^…$"   (Bytes mode)
    //   "^…$"        (CodePoints mode)
    // Find the `^` after any leading `(?…)` flag block, and the
    // trailing `$`. Skip both.
    let bytes = body.as_bytes();
    let mut start = 0usize;
    if body.starts_with("(?")
        && let Some(end) = body.find(')')
    {
        start = end + 1;
    }
    let anchor_start = if bytes.get(start) == Some(&b'^') {
        start + 1
    } else {
        start
    };
    let mut end = bytes.len();
    if end > anchor_start && bytes[end - 1] == b'$' {
        end -= 1;
    }
    let prefix = &body[..start];
    let inner = &body[anchor_start..end];
    let mut out = String::with_capacity(prefix.len() + inner.len());
    out.push_str(prefix);
    out.push_str(inner);
    out
}

/// True when the compiled regex matches `haystack`. For anchored
/// regexes this is whole-string match; for anywhere regexes it's
/// find-first.
pub(crate) fn is_match(regex: &Regex, haystack: &str) -> bool {
    regex.is_match(haystack.as_bytes())
}

/// Yield [`Match`]es for an already-compiled regex. Anchored regexes
/// yield at most one whole-string match; anywhere regexes yield
/// non-overlapping left-to-right matches.
///
/// Matches that land mid-scalar (only possible for `Bytes`-mode
/// patterns on non-ASCII haystacks) are skipped rather than
/// returned — [`Match::matched`] must be a valid `&str`, and the
/// public API's implicit contract is that Bytes mode is used with
/// byte-clean (ASCII / already-tokenised) input.
pub(crate) fn find_iter<'h>(
    regex: &Regex,
    haystack: &'h str,
    anchored: bool,
) -> Box<dyn Iterator<Item = Match<'h>> + 'h> {
    if anchored {
        if regex.is_match(haystack.as_bytes()) {
            return Box::new(core::iter::once(Match {
                start: 0,
                end: haystack.len(),
                matched: haystack,
            }));
        }
        return Box::new(core::iter::empty());
    }
    let spans: alloc::vec::Vec<(usize, usize)> = regex
        .find_iter(haystack.as_bytes())
        .map(|m| (m.start(), m.end()))
        .filter(|&(s, e)| haystack.is_char_boundary(s) && haystack.is_char_boundary(e))
        .collect();
    Box::new(spans.into_iter().map(move |(start, end)| Match {
        start,
        end,
        matched: &haystack[start..end],
    }))
}
