//! Apply a parsed unified-diff patch to reconstruct new-side text.
//!
//! Companion to [`crate::unified::parse`] — feeds the parser's
//! [`ParsedPatch`] hunks through [`apply`] against the original
//! old-side string to produce the new-side string, verifying that
//! every context and delete line matches the input at the recorded
//! position.
//!
//! Mismatches short-circuit with a [`PatchError`] that names the
//! hunk, line, and expected vs found content — enough for a
//! caller to decide whether to reject, fuzz-align, or bail.
//!
//! ## What this crate does NOT try to be
//!
//! `patch(1)`'s fuzz alignment (accepts non-adjacent context if
//! the exact-line context fails) is deliberately not implemented.
//! Fuzz alignment is a productivity feature for the `patch` binary
//! — it's actively unhelpful when a caller programmatically
//! generated the patch from a known old-side string and expects
//! deterministic application. Callers who want fuzz semantics
//! reach for the `patch(1)` binary or a library that models its
//! behaviour explicitly.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::Edit;
use crate::hunks::Hunk;
use crate::unified::ParsedPatch;

/// Reasons application can fail. Every variant carries the source
/// line number and enough content to diagnose without cross-
/// referencing the original patch text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    /// A hunk's context or delete line didn't match the old-side
    /// content at the recorded position.
    ContextMismatch {
        /// 0-based hunk index within the patch.
        hunk: usize,
        /// 0-based line index within the old-side string.
        old_line: usize,
        /// Line the hunk expected to see.
        expected: String,
        /// Line actually present in the old-side string.
        found: String,
    },
    /// A hunk's `old_start` position is beyond the end of the
    /// old-side string.
    HunkOutOfBounds {
        /// 0-based hunk index within the patch.
        hunk: usize,
        /// The recorded start (0-based).
        old_start: usize,
        /// Length of the old-side string in lines.
        old_lines: usize,
    },
    /// A hunk's `old_len` runs past the end of the old-side string.
    HunkExtendsPastEnd {
        /// 0-based hunk index within the patch.
        hunk: usize,
        /// The recorded start (0-based).
        old_start: usize,
        /// The recorded length.
        old_len: usize,
        /// Length of the old-side string in lines.
        old_lines: usize,
    },
    /// Two hunks overlap in the old-side coordinates.
    OverlappingHunks {
        /// 0-based hunk index of the second (offending) hunk.
        hunk: usize,
    },
}

impl core::fmt::Display for PatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ContextMismatch {
                hunk,
                old_line,
                expected,
                found,
            } => write!(
                f,
                "hunk #{hunk} context mismatch at old line {old_line}: \
                 expected {expected:?}, found {found:?}",
            ),
            Self::HunkOutOfBounds {
                hunk,
                old_start,
                old_lines,
            } => write!(
                f,
                "hunk #{hunk} start {old_start} is past end of {old_lines}-line old file",
            ),
            Self::HunkExtendsPastEnd {
                hunk,
                old_start,
                old_len,
                old_lines,
            } => write!(
                f,
                "hunk #{hunk} at {old_start} spans {old_len} lines — past end of {old_lines}-line old file",
            ),
            Self::OverlappingHunks { hunk } => {
                write!(f, "hunk #{hunk} overlaps a previous hunk in the same patch")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PatchError {}

/// Apply `patch` to `old`, returning the reconstructed new-side
/// string.
///
/// `old` is split at `\n` — same convention as
/// [`crate::DiffUnit::Lines`] — and the returned string joins with
/// `\n`. A trailing `\n` in `old` is preserved in the output when
/// the patch's final hunk doesn't touch it.
///
/// # Errors
///
/// See [`PatchError`].
pub fn apply(old: &str, patch: &ParsedPatch) -> Result<String, PatchError> {
    apply_hunks(old, &patch.hunks)
}

/// Same as [`apply`] but takes a hunk slice directly — useful when
/// hunks were built programmatically (from a diff run) rather than
/// parsed from unified-format text.
///
/// # Errors
///
/// See [`PatchError`].
pub fn apply_hunks(old: &str, hunks: &[Hunk<String>]) -> Result<String, PatchError> {
    let old_lines: Vec<&str> = old.split('\n').collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(old_lines.len());
    let mut cursor = 0usize; // 0-based position in old_lines

    for (idx, hunk) in hunks.iter().enumerate() {
        // Bounds checks.
        if hunk.old_len == 0 {
            // Pure-insertion hunk — insert at `old_start` position.
            // Emit all old lines up to hunk.old_start unchanged,
            // then the inserts, then continue.
            let target = hunk.old_start;
            if target < cursor {
                return Err(PatchError::OverlappingHunks { hunk: idx });
            }
            if target > old_lines.len() {
                return Err(PatchError::HunkOutOfBounds {
                    hunk: idx,
                    old_start: target,
                    old_lines: old_lines.len(),
                });
            }
            for &line in &old_lines[cursor..target] {
                out_lines.push(line.to_string());
            }
            for edit in &hunk.edits {
                if let Edit::Insert(s) = edit {
                    out_lines.push(s.clone());
                }
            }
            cursor = target;
            continue;
        }

        // General case — hunk spans old_start..old_start+old_len.
        if hunk.old_start < cursor {
            return Err(PatchError::OverlappingHunks { hunk: idx });
        }
        if hunk.old_start > old_lines.len() {
            return Err(PatchError::HunkOutOfBounds {
                hunk: idx,
                old_start: hunk.old_start,
                old_lines: old_lines.len(),
            });
        }
        if hunk.old_start + hunk.old_len > old_lines.len() {
            return Err(PatchError::HunkExtendsPastEnd {
                hunk: idx,
                old_start: hunk.old_start,
                old_len: hunk.old_len,
                old_lines: old_lines.len(),
            });
        }

        // Emit everything before the hunk verbatim.
        for &line in &old_lines[cursor..hunk.old_start] {
            out_lines.push(line.to_string());
        }

        // Walk the hunk's edits, consuming old lines for
        // Keep/Delete, emitting old-side content for Keep and
        // new-side content for Insert.
        let mut old_cursor = hunk.old_start;
        for edit in &hunk.edits {
            match edit {
                Edit::Keep(expected) => {
                    let found = old_lines[old_cursor];
                    if expected != found {
                        return Err(PatchError::ContextMismatch {
                            hunk: idx,
                            old_line: old_cursor,
                            expected: expected.clone(),
                            found: found.to_string(),
                        });
                    }
                    out_lines.push(found.to_string());
                    old_cursor += 1;
                }
                Edit::Delete(expected) => {
                    let found = old_lines[old_cursor];
                    if expected != found {
                        return Err(PatchError::ContextMismatch {
                            hunk: idx,
                            old_line: old_cursor,
                            expected: expected.clone(),
                            found: found.to_string(),
                        });
                    }
                    // Delete — do not emit; just advance old cursor.
                    old_cursor += 1;
                }
                Edit::Insert(s) => {
                    out_lines.push(s.clone());
                }
            }
        }
        cursor = old_cursor;
    }

    // Emit any remaining old-side tail.
    for &line in &old_lines[cursor..] {
        out_lines.push(line.to_string());
    }

    Ok(out_lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::{DiffAlgorithm, Myers};
    use crate::unified::{parse, write_default};

    // Round-trip: diff two strings, serialise, parse, apply, get
    // back the new string. This is the whole point of the module.
    fn round_trip(old: &str, new: &str) -> String {
        let old_lines: Vec<&str> = old.split('\n').collect();
        let new_lines: Vec<&str> = new.split('\n').collect();
        let script = Myers.diff(&old_lines, &new_lines);
        let text = write_default(&script);
        if text.is_empty() {
            // No changes — apply is a no-op.
            return old.to_string();
        }
        let parsed = parse(&text).expect("writer output parses");
        apply(old, &parsed).expect("well-formed patch applies")
    }

    #[test]
    fn apply_no_change_returns_original() {
        let s = "one\ntwo\nthree";
        assert_eq!(round_trip(s, s), s);
    }

    #[test]
    fn apply_single_line_change() {
        let old = "one\ntwo\nthree";
        let new = "one\nTWO\nthree";
        assert_eq!(round_trip(old, new), new);
    }

    #[test]
    fn apply_pure_insert() {
        let old = "one\ntwo";
        let new = "zero\none\ntwo";
        assert_eq!(round_trip(old, new), new);
    }

    #[test]
    fn apply_pure_delete() {
        let old = "one\ntwo\nthree";
        let new = "one\nthree";
        assert_eq!(round_trip(old, new), new);
    }

    #[test]
    fn apply_multiple_hunks() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let new = "a\nB\nc\nd\ne\nf\ng\nh\nI\nj";
        assert_eq!(round_trip(old, new), new);
    }

    #[test]
    fn apply_all_deletes() {
        let old = "gone\nalso gone\nfine";
        let new = "fine";
        assert_eq!(round_trip(old, new), new);
    }

    #[test]
    fn apply_all_inserts_into_empty() {
        // Empty-string input has one "empty" line under split('\n');
        // an insert-only patch prepends its content and preserves
        // that one trailing empty.
        let old = "";
        let new = "first\nsecond";
        assert_eq!(round_trip(old, new), new);
    }

    #[test]
    fn apply_context_mismatch_errors() {
        // Craft a patch that expects `two` but hand it an old-side
        // string with `TWO` — apply should refuse.
        let text = "@@ -1,3 +1,3 @@\n one\n-two\n+2\n three\n";
        let parsed = parse(text).unwrap();
        let err = apply("one\nTWO\nthree", &parsed).unwrap_err();
        assert!(matches!(err, PatchError::ContextMismatch { .. }));
    }

    #[test]
    fn apply_hunk_out_of_bounds_errors() {
        // Hunk claims to start at line 100 of a 3-line file.
        let text = "@@ -100,1 +100,1 @@\n-x\n+X\n";
        let parsed = parse(text).unwrap();
        let err = apply("a\nb\nc", &parsed).unwrap_err();
        assert!(matches!(
            err,
            PatchError::HunkOutOfBounds { .. } | PatchError::HunkExtendsPastEnd { .. }
        ));
    }
}
