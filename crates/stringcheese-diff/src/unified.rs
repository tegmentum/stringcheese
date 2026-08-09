//! Unified-diff format reader and writer.
//!
//! Turns edit scripts (at [`DiffUnit::Lines`] granularity) into
//! the standard git / `patch(1)` unified format and back:
//!
//! - [`fn@write`] / [`write_default`] / [`write_named`] — script → text.
//! - [`diff_lines_unified`] — one-shot `(old, new) → text`.
//! - [`parse`] — text → [`ParsedPatch`] carrying filenames + hunks.
//!
//! Once parsed, a patch applies to an old-side string via
//! [`crate::patch::apply`] to reconstruct the new-side string. That
//! round-trips against this writer's output on every well-formed
//! diff.
//!
//! [`DiffUnit::Lines`]: crate::DiffUnit::Lines
//!
//! ## Format cheat-sheet
//!
//! ```text
//! --- a/{old_name}
//! +++ b/{new_name}
//! @@ -{old_start},{old_len} +{new_start},{new_len} @@
//!  context line
//! -removed line
//! +added line
//! ```
//!
//! Positions are **1-based** and lengths are counts. Lines within
//! a hunk are prefixed with a space (Keep), minus (Delete), or
//! plus (Insert).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write as _;

use crate::Edit;
use crate::hunks::{Hunk, hunks};

/// Options for the unified-diff writer.
///
/// `context` matches git's `-U<n>`; the two filename fields become
/// the `---` / `+++` header lines.
#[derive(Debug, Clone)]
pub struct UnifiedOptions {
    /// Number of surrounding Keep lines per hunk. Git's default is 3.
    pub context: usize,
    /// Name written as `--- a/{old_name}`. Common values: a path,
    /// `"a"`, or an empty string (some tooling omits the prefix).
    pub old_name: String,
    /// Name written as `+++ b/{new_name}`.
    pub new_name: String,
}

impl Default for UnifiedOptions {
    fn default() -> Self {
        Self {
            context: 3,
            old_name: String::from("a"),
            new_name: String::from("b"),
        }
    }
}

/// Serialize `script` to the standard unified-diff textual format.
///
/// The script must be at line granularity — each element rendered
/// via `AsRef<str>`. For a byte / char / grapheme diff, keep the
/// [`Vec<Edit<T>>`] and format it yourself.
///
/// Returns an empty string when the script contains no changes.
#[must_use]
pub fn write<T>(script: &[Edit<T>], opts: &UnifiedOptions) -> String
where
    T: AsRef<str> + Clone,
{
    let hs = hunks(script, opts.context);
    if hs.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    writeln!(out, "--- a/{}", opts.old_name).ok();
    writeln!(out, "+++ b/{}", opts.new_name).ok();
    for h in &hs {
        write_hunk(&mut out, h);
    }
    out
}

fn write_hunk<T: AsRef<str>>(out: &mut String, h: &Hunk<T>) {
    // Positions are 1-based in unified format. An empty side
    // (old_len == 0 or new_len == 0) renders as `0` in the header
    // per POSIX, not `1`.
    let old_disp = if h.old_len == 0 { 0 } else { h.old_start + 1 };
    let new_disp = if h.new_len == 0 { 0 } else { h.new_start + 1 };
    writeln!(
        out,
        "@@ -{},{} +{},{} @@",
        old_disp, h.old_len, new_disp, h.new_len
    )
    .ok();
    for e in &h.edits {
        match e {
            Edit::Keep(t) => {
                out.push(' ');
                out.push_str(t.as_ref());
                out.push('\n');
            }
            Edit::Delete(t) => {
                out.push('-');
                out.push_str(t.as_ref());
                out.push('\n');
            }
            Edit::Insert(t) => {
                out.push('+');
                out.push_str(t.as_ref());
                out.push('\n');
            }
        }
    }
}

/// Serialize `script` to unified format using the default options
/// (3 lines of context, filenames `"a"` and `"b"`).
#[must_use]
pub fn write_default<T>(script: &[Edit<T>]) -> String
where
    T: AsRef<str> + Clone,
{
    write(script, &UnifiedOptions::default())
}

/// Serialize `script` to unified format with a specific filename
/// pair and the default context width.
#[must_use]
pub fn write_named<T>(
    script: &[Edit<T>],
    old_name: impl Into<String>,
    new_name: impl Into<String>,
) -> String
where
    T: AsRef<str> + Clone,
{
    write(
        script,
        &UnifiedOptions {
            context: 3,
            old_name: old_name.into(),
            new_name: new_name.into(),
        },
    )
}

/// Convenience: diff two `&str`s at line granularity via [`Myers`]
/// and return the unified-format output as a single `String`.
///
/// [`Myers`]: crate::Myers
#[must_use]
pub fn diff_lines_unified(old: &str, new: &str, opts: &UnifiedOptions) -> String {
    let script = crate::diff_at(old, new, crate::DiffUnit::Lines, crate::Myers);
    // `script` is `Vec<Edit<&str>>` — `&str` implements `AsRef<str>`
    // and `Clone`, so the writer accepts it directly.
    write(&script, opts)
}

// ---------------------------------------------------------------------
// Parser — text → ParsedPatch
// ---------------------------------------------------------------------

/// A parsed unified-diff patch — filenames plus the sequence of
/// hunks. Element type is `String` because the parser owns the
/// line data extracted from the diff text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPatch {
    /// Old-side filename (from the `--- a/…` header); empty when
    /// the input didn't include a header.
    pub old_name: String,
    /// New-side filename (from the `+++ b/…` header); empty when
    /// the input didn't include a header.
    pub new_name: String,
    /// Hunks in the order they appeared, ready to feed to
    /// [`crate::patch::apply`].
    pub hunks: Vec<Hunk<String>>,
}

/// Errors returned by [`parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// A `@@ … @@` hunk header didn't parse — malformed positions
    /// or missing separators.
    BadHunkHeader {
        /// 1-based line number where the malformed header appeared.
        line: usize,
        /// The offending header text.
        text: String,
    },
    /// A content line inside a hunk didn't start with ` ` / `-` / `+`.
    BadContentLine {
        /// 1-based line number where the bad line appeared.
        line: usize,
        /// The offending content.
        text: String,
    },
    /// Hunk body ended before consuming the announced `old_len` /
    /// `new_len` counts.
    TruncatedHunk {
        /// 1-based line number where the hunk started.
        line: usize,
    },
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadHunkHeader { line, text } => {
                write!(f, "malformed hunk header at line {line}: {text:?}")
            }
            Self::BadContentLine { line, text } => {
                write!(f, "bad content line at line {line}: {text:?}")
            }
            Self::TruncatedHunk { line } => {
                write!(f, "hunk starting at line {line} ended prematurely")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

/// Parse a unified-diff text into a [`ParsedPatch`].
///
/// Accepts input with or without the `--- a/…` / `+++ b/…` header
/// pair — the tolerant case handles patches embedded in commit
/// bodies or mail threads where the header is stripped. Any lines
/// before the first `@@` are treated as commentary and skipped;
/// header lines within that region are recognised and their
/// filenames extracted.
///
/// # Errors
///
/// See [`ParseError`].
pub fn parse(text: &str) -> Result<ParsedPatch, ParseError> {
    let mut old_name = String::new();
    let mut new_name = String::new();
    let mut hunks_out: Vec<Hunk<String>> = Vec::new();

    let mut lines = text.lines().enumerate().peekable();
    // Optional header lines before the first `@@`.
    while let Some(&(_, line)) = lines.peek() {
        if line.starts_with("@@") {
            break;
        }
        if let Some(rest) = line.strip_prefix("--- ") {
            old_name = strip_ab_prefix(rest.trim()).to_string();
        } else if let Some(rest) = line.strip_prefix("+++ ") {
            new_name = strip_ab_prefix(rest.trim()).to_string();
        }
        lines.next();
    }

    // Hunks.
    while let Some((idx, header)) = lines.next() {
        let line_no = idx + 1;
        if !header.starts_with("@@") {
            // Anything after all hunks are consumed is trailing
            // content — either a `--` signature line, an empty
            // trailer, or garbage. Stop cleanly on the first
            // non-hunk-header we hit after the first hunk.
            if hunks_out.is_empty() {
                continue;
            }
            break;
        }
        let (old_start_1based, old_len, new_start_1based, new_len) = parse_hunk_header(header)
            .ok_or_else(|| ParseError::BadHunkHeader {
                line: line_no,
                text: header.to_string(),
            })?;
        // Convert 1-based positions to the 0-based positions the
        // Hunk struct records. An empty side (len == 0) has
        // 1-based position `0`, which the format writer emits and
        // the parser here maps back to `0` on both sides — the
        // exact semantic detail is that `start` isn't meaningful
        // when `len == 0`, so any value works.
        let old_start = if old_len == 0 {
            0
        } else {
            old_start_1based.saturating_sub(1)
        };
        let new_start = if new_len == 0 {
            0
        } else {
            new_start_1based.saturating_sub(1)
        };

        // Consume `old_len + new_len - keeps` content lines (each
        // Keep counts toward both sides, Delete only old, Insert
        // only new). We drive off old_seen + new_seen counters.
        let mut edits: Vec<Edit<String>> = Vec::new();
        let mut old_seen = 0usize;
        let mut new_seen = 0usize;
        while old_seen < old_len || new_seen < new_len {
            let Some((body_idx, body_line)) = lines.next() else {
                return Err(ParseError::TruncatedHunk { line: line_no });
            };
            let body_line_no = body_idx + 1;
            let (kind, content) = match body_line.as_bytes().first().copied() {
                Some(b' ') => ('=', &body_line[1..]),
                Some(b'-') => ('-', &body_line[1..]),
                Some(b'+') => ('+', &body_line[1..]),
                Some(b'\\') => {
                    // `\ No newline at end of file` — informational,
                    // doesn't advance counters.
                    continue;
                }
                _ if body_line.is_empty() => (' ', ""), // empty line = keep of empty line
                _ => {
                    return Err(ParseError::BadContentLine {
                        line: body_line_no,
                        text: body_line.to_string(),
                    });
                }
            };
            match kind {
                '=' | ' ' => {
                    edits.push(Edit::Keep(content.to_string()));
                    old_seen += 1;
                    new_seen += 1;
                }
                '-' => {
                    edits.push(Edit::Delete(content.to_string()));
                    old_seen += 1;
                }
                '+' => {
                    edits.push(Edit::Insert(content.to_string()));
                    new_seen += 1;
                }
                _ => unreachable!(),
            }
        }

        hunks_out.push(Hunk {
            old_start,
            old_len,
            new_start,
            new_len,
            edits,
        });
    }

    Ok(ParsedPatch {
        old_name,
        new_name,
        hunks: hunks_out,
    })
}

/// Strip a leading `a/` or `b/` path prefix, returning the rest.
/// git prepends these to filenames in its diff headers; the parser
/// unwraps them so `ParsedPatch::old_name` carries the bare path.
fn strip_ab_prefix(s: &str) -> &str {
    s.strip_prefix("a/")
        .or_else(|| s.strip_prefix("b/"))
        .unwrap_or(s)
}

/// Parse `@@ -O,L +N,L @@ …trailing…` — returns `(old_start, old_len,
/// new_start, new_len)` on success, `None` on malformed input.
fn parse_hunk_header(header: &str) -> Option<(usize, usize, usize, usize)> {
    // Split at the ` @@` that closes the range block, but be
    // tolerant of an omitted trailing `@@` (some tools drop it).
    let rest = header.strip_prefix("@@")?.trim_start();
    // The header may or may not have a section-name trailer after
    // the closing `@@`; split on `@@` to isolate the range block.
    let (range, _tail) = match rest.split_once("@@") {
        Some((r, t)) => (r.trim(), t),
        None => (rest.trim(), ""),
    };
    // range is like `-1,3 +1,3` or `-1 +2` (when len == 1).
    let mut parts = range.split_whitespace();
    let old_range = parts.next()?;
    let new_range = parts.next()?;
    let (old_start, old_len) = parse_range(old_range.strip_prefix('-')?)?;
    let (new_start, new_len) = parse_range(new_range.strip_prefix('+')?)?;
    Some((old_start, old_len, new_start, new_len))
}

/// Parse `N` or `N,L` — returns `(start, len)`. Missing `,L`
/// defaults to length 1 (the same convention `git diff` uses).
fn parse_range(s: &str) -> Option<(usize, usize)> {
    if let Some((s_start, s_len)) = s.split_once(',') {
        Some((s_start.parse().ok()?, s_len.parse().ok()?))
    } else {
        Some((s.parse().ok()?, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::{DiffAlgorithm, Myers};

    #[test]
    fn empty_script_produces_empty_output() {
        let script: Vec<Edit<&str>> = Vec::new();
        assert_eq!(write_default(&script), "");
    }

    #[test]
    fn identical_input_produces_empty_output() {
        let x: Vec<&str> = vec!["one", "two", "three"];
        let script = Myers.diff(&x, &x);
        assert_eq!(write_default(&script), "");
    }

    #[test]
    fn single_line_change_produces_one_hunk() {
        let old: Vec<&str> = vec!["one", "two", "three"];
        let new: Vec<&str> = vec!["one", "TWO", "three"];
        let script = Myers.diff(&old, &new);
        let out = write_default(&script);
        // Header lines present.
        assert!(out.starts_with("--- a/a\n+++ b/b\n"));
        // Hunk header with 1-based positions and correct counts:
        // old spans lines 1..3 (all three), new spans lines 1..3.
        assert!(out.contains("@@ -1,3 +1,3 @@\n"));
        // Both change lines and the context lines are present.
        assert!(out.contains(" one\n"));
        assert!(out.contains("-two\n"));
        assert!(out.contains("+TWO\n"));
        assert!(out.contains(" three\n"));
    }

    #[test]
    fn pure_delete_zero_new_len() {
        let old: Vec<&str> = vec!["x"];
        let new: Vec<&str> = vec![];
        let script = Myers.diff(&old, &new);
        let out = write_default(&script);
        // Deletion-only hunk has +0,0 on the new side per POSIX.
        assert!(out.contains("@@ -1,1 +0,0 @@\n"), "{out}");
        assert!(out.contains("-x\n"));
    }

    #[test]
    fn pure_insert_zero_old_len() {
        let old: Vec<&str> = vec![];
        let new: Vec<&str> = vec!["x"];
        let script = Myers.diff(&old, &new);
        let out = write_default(&script);
        assert!(out.contains("@@ -0,0 +1,1 @@\n"), "{out}");
        assert!(out.contains("+x\n"));
    }

    // --- Parser tests --------------------------------------------------

    #[test]
    fn parse_round_trips_writer_output() {
        let old: Vec<&str> = vec!["one", "two", "three", "four", "five"];
        let new: Vec<&str> = vec!["one", "TWO", "three", "four", "five"];
        let script = Myers.diff(&old, &new);
        let text = write_default(&script);

        let parsed = parse(&text).expect("writer output parses");
        assert_eq!(parsed.old_name, "a");
        assert_eq!(parsed.new_name, "b");
        assert_eq!(parsed.hunks.len(), 1);
        let h = &parsed.hunks[0];
        // Same hunk shape the writer emitted.
        assert_eq!(h.old_start, 0);
        assert_eq!(h.new_start, 0);
        // Body carries the exact context + change lines.
        let contents: Vec<_> = h
            .edits
            .iter()
            .map(|e| match e {
                Edit::Keep(s) => alloc::format!(" {s}"),
                Edit::Delete(s) => alloc::format!("-{s}"),
                Edit::Insert(s) => alloc::format!("+{s}"),
            })
            .collect();
        assert!(contents.iter().any(|s| s == "-two"));
        assert!(contents.iter().any(|s| s == "+TWO"));
    }

    #[test]
    fn parse_tolerates_missing_headers() {
        let text = "@@ -1,2 +1,2 @@\n a\n-b\n+B\n";
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.old_name, "");
        assert_eq!(parsed.new_name, "");
        assert_eq!(parsed.hunks.len(), 1);
    }

    #[test]
    fn parse_pure_insert() {
        let text = "--- a/x\n+++ b/x\n@@ -0,0 +1,1 @@\n+hello\n";
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.hunks.len(), 1);
        assert_eq!(parsed.hunks[0].old_len, 0);
        assert_eq!(parsed.hunks[0].new_len, 1);
    }

    #[test]
    fn parse_pure_delete() {
        let text = "--- a/x\n+++ b/x\n@@ -1,1 +0,0 @@\n-hello\n";
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.hunks[0].old_len, 1);
        assert_eq!(parsed.hunks[0].new_len, 0);
    }

    #[test]
    fn parse_multiple_hunks() {
        let text = "--- a/f\n+++ b/f\n\
                    @@ -1,1 +1,1 @@\n-a\n+A\n\
                    @@ -5,1 +5,1 @@\n-e\n+E\n";
        let parsed = parse(text).unwrap();
        assert_eq!(parsed.hunks.len(), 2);
    }

    #[test]
    fn parse_error_on_bad_header() {
        let text = "@@ garbage @@\n";
        let err = parse(text).unwrap_err();
        assert!(matches!(err, ParseError::BadHunkHeader { .. }));
    }
}
