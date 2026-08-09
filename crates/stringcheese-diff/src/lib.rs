//! # StringCheese diff and sequence-alignment
//!
//! Algorithm-agnostic diff over any `T: Eq` sequence. Ships two
//! algorithms today ([`Myers`] O(ND), the workhorse; [`Patience`],
//! better for source code), a [`unified`] writer that emits the
//! git-style unified-diff format from an edit script, and a
//! [`DiffUnit`]-driven convenience layer that hands `&str` inputs
//! to the algorithms at the caller-chosen granularity (bytes / code
//! points / graphemes / words / sentences / lines).
//!
//! ## Design
//!
//! - **Sequence-abstract.** [`diff`] takes `&[T] where T: Eq`. Byte
//!   diff, code-point diff, grapheme diff, line diff, token diff —
//!   all one algorithm, one edit-script shape.
//! - **Unicode semantic units are explicit.** [`DiffUnit`] names the
//!   segmentation the caller wants; the top-level convenience
//!   functions ([`diff_at`], [`line_diff`], [`word_diff`]) split
//!   the input accordingly.
//! - **Two output shapes, both first-class.** [`Vec<Edit<T>>`] for
//!   programmatic consumers (LSP tooling, structured UIs); the
//!   [`unified`] module for anything that needs to interop with
//!   `patch(1)` or Git's textual diff format.
//! - **Post-processing is a pipeline of edit-script → edit-script
//!   transforms.** The algorithm produces a raw script; optional
//!   cleanup passes (semantic-alignment, small-equality collapse)
//!   run afterwards. This is the accommodation for a future
//!   diff-match-patch-style semantic-cleanup pipeline — the
//!   algorithm trait doesn't need to know about cleanups.
//!
//! ## Non-goals
//!
//! - **No structured diff** (JSON / HTML / AST). Different problem.
//! - **No 3-way merge.** Different problem, needs conflict-resolution
//!   semantics.
//! - **No `diff-match-patch` semantic cleanups in this cut.** The
//!   pipeline shape accommodates them; the transforms themselves
//!   land in a follow-up.
//!
//! ## Example
//!
//! ```
//! use stringcheese_diff::{diff, Edit, algo::Myers};
//!
//! let old = ["a", "b", "c", "d"];
//! let new = ["a", "c", "d", "e"];
//! let edits = diff(&old, &new, Myers);
//!
//! use Edit::*;
//! assert_eq!(
//!     edits,
//!     vec![Keep(&"a"), Delete(&"b"), Keep(&"c"), Keep(&"d"), Insert(&"e")]
//! );
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

pub mod algo;
#[cfg(feature = "alloc")]
pub mod hunks;
#[cfg(feature = "alloc")]
pub mod patch;
#[cfg(feature = "alloc")]
pub mod segment;
#[cfg(feature = "alloc")]
pub mod unified;

pub use algo::{DiffAlgorithm, Myers, Patience};
pub use segment::DiffUnit;

// ---------------------------------------------------------------------
// Edit — the atom every algorithm produces and every output shape
// consumes.
// ---------------------------------------------------------------------

/// One step of an edit script.
///
/// A `Vec<Edit<T>>` describes how to transform `old` into `new`:
/// walk the vec left-to-right and, for each variant, take the
/// carried element into the output. Every element in `old` and
/// every element in `new` appears exactly once across the combined
/// [`Keep`](Self::Keep) / [`Delete`](Self::Delete) /
/// [`Insert`](Self::Insert) sequence.
///
/// `T` is the carried type. Idiomatic callers use `T = &U` for
/// borrowed edits (`Vec<Edit<&Token>>`) or `T = U` for owned edits
/// (`Vec<Edit<Token>>`). The [`DiffAlgorithm`] trait's `diff`
/// method returns `Vec<Edit<&'a T>>` — one level of borrow into the
/// source slices — and the top-level [`diff_at`] returns
/// `Vec<Edit<&'a str>>` because `str` is unsized and slices of it
/// are already the borrow.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Edit<T> {
    /// Element present in both `old` and `new`. Kept from the `old`
    /// occurrence — the two are `==`-equal by construction, so any
    /// reader is indifferent.
    Keep(T),
    /// Element present in `new` but not in the corresponding
    /// position of `old`. Carries the element from `new`.
    Insert(T),
    /// Element present in `old` but not in the corresponding
    /// position of `new`. Carries the element from `old`.
    Delete(T),
}

impl<T> Edit<T> {
    /// True when this edit is a [`Keep`](Self::Keep) — the identity
    /// portion of the script.
    #[must_use]
    pub const fn is_keep(&self) -> bool {
        matches!(self, Self::Keep(_))
    }

    /// True when this edit is a [`Delete`](Self::Delete) or
    /// [`Insert`](Self::Insert) — the diverging portions.
    #[must_use]
    pub const fn is_change(&self) -> bool {
        !self.is_keep()
    }

    /// Access the carried element regardless of variant.
    #[must_use]
    pub const fn value(&self) -> &T {
        match self {
            Self::Keep(t) | Self::Insert(t) | Self::Delete(t) => t,
        }
    }

    /// Map the carried element through `f`, preserving the variant.
    #[must_use]
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Edit<U> {
        match self {
            Self::Keep(t) => Edit::Keep(f(t)),
            Self::Insert(t) => Edit::Insert(f(t)),
            Self::Delete(t) => Edit::Delete(f(t)),
        }
    }
}

// ---------------------------------------------------------------------
// Top-level convenience API
// ---------------------------------------------------------------------

/// Diff two sequences with `algo`, returning the full edit script.
///
/// Elements are borrowed from `old` and `new` — the returned edit
/// script's `T` is `&'a T`. Algorithm choice is a
/// [`DiffAlgorithm`] impl the caller passes in — [`Myers`] is the
/// safe default; [`Patience`] wins on source-code inputs where
/// unique-line anchoring matters.
#[cfg(feature = "alloc")]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Myers/Patience are ZSTs; by-value keeps the call-site idiomatic.
pub fn diff<'a, T, A>(old: &'a [T], new: &'a [T], algo: A) -> Vec<Edit<&'a T>>
where
    T: Eq,
    A: DiffAlgorithm,
{
    algo.diff(old, new)
}

/// Diff two `&str`s at the specified [`DiffUnit`] boundary.
///
/// Splits `old` and `new` according to `unit`, runs `algo`, and
/// returns an edit script whose elements are `&'a str` slices of
/// the original inputs. Since `str` is unsized, the script's
/// carried type is `&'a str` directly (rather than `&&str`).
#[cfg(feature = "alloc")]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Myers/Patience are ZSTs; by-value keeps the call-site idiomatic.
pub fn diff_at<'a, A: DiffAlgorithm>(
    old: &'a str,
    new: &'a str,
    unit: DiffUnit,
    algo: A,
) -> Vec<Edit<&'a str>> {
    let old_parts: Vec<&'a str> = segment::split(old, unit).collect();
    let new_parts: Vec<&'a str> = segment::split(new, unit).collect();
    // The algorithm hands back `Edit<&&str>` referencing INTO the
    // local Vecs; deref the outer & to get `Edit<&'a str>` that
    // outlives both Vecs (the inner `&str` is Copy).
    algo.diff(&old_parts, &new_parts)
        .into_iter()
        .map(|e| e.map(|s| *s))
        .collect()
}

/// Convenience: line-level diff of two `&str`s using [`Myers`].
///
/// Splits at `\n` (drops the newline itself from each slice — the
/// unified-diff writer re-adds it). For UAX #14 line breaking,
/// enable the `segment-lines-uax14` feature and pass
/// [`DiffUnit::LinesUax14`] to [`diff_at`].
#[cfg(feature = "alloc")]
#[must_use]
pub fn line_diff<'a>(old: &'a str, new: &'a str) -> Vec<Edit<&'a str>> {
    diff_at(old, new, DiffUnit::Lines, Myers)
}

/// Convenience: word-level diff using [`Myers`]. Requires the
/// `segment-icu` feature for real UAX #29 word boundaries; without
/// it, splits at ASCII whitespace runs.
#[cfg(feature = "alloc")]
#[must_use]
pub fn word_diff<'a>(old: &'a str, new: &'a str) -> Vec<Edit<&'a str>> {
    diff_at(old, new, DiffUnit::Words, Myers)
}

// ---------------------------------------------------------------------
// Cleanup pipeline — makes room for future diff-match-patch semantics
// ---------------------------------------------------------------------

/// A post-processing pass on an edit script.
///
/// Cleanups are `Vec<Edit> → Vec<Edit>` transforms — no dependence
/// on the underlying algorithm. Callers chain them:
///
/// ```
/// use stringcheese_diff::{diff, algo::Myers, cleanup::collapse_adjacent};
///
/// let script = diff(&["a", "b", "c"], &["a", "x", "c"], Myers);
/// let cleaned = collapse_adjacent(script);
/// ```
///
/// The trait exists so future cleanups (diff-match-patch's semantic
/// alignment, efficiency-cleanup, word-boundary alignment) plug in
/// through the same shape without changing the algorithm surface.
#[cfg(feature = "alloc")]
pub mod cleanup {
    use super::Edit;
    use alloc::vec::Vec;

    /// Coalesce consecutive [`Delete`](Edit::Delete) → [`Insert`](Edit::Insert)
    /// runs into a single change block per position, preserving the
    /// original order (all deletes first, all inserts second).
    ///
    /// The raw output of Myers / Patience often interleaves single-
    /// element deletes and inserts in the same region; grouping them
    /// makes downstream rendering (unified format, side-by-side
    /// viewers) simpler without changing the edit-script's meaning.
    #[must_use]
    pub fn collapse_adjacent<'a, T>(script: Vec<Edit<&'a T>>) -> Vec<Edit<&'a T>> {
        let mut out: Vec<Edit<&'a T>> = Vec::with_capacity(script.len());
        let mut buf_del: Vec<&'a T> = Vec::new();
        let mut buf_ins: Vec<&'a T> = Vec::new();
        let flush =
            |out: &mut Vec<Edit<&'a T>>, buf_del: &mut Vec<&'a T>, buf_ins: &mut Vec<&'a T>| {
                for t in buf_del.drain(..) {
                    out.push(Edit::Delete(t));
                }
                for t in buf_ins.drain(..) {
                    out.push(Edit::Insert(t));
                }
            };
        for edit in script {
            match edit {
                Edit::Keep(t) => {
                    flush(&mut out, &mut buf_del, &mut buf_ins);
                    out.push(Edit::Keep(t));
                }
                Edit::Delete(t) => buf_del.push(t),
                Edit::Insert(t) => buf_ins.push(t),
            }
        }
        flush(&mut out, &mut buf_del, &mut buf_ins);
        out
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn edit_map_transforms_carried_value() {
        // The old `to_owned` API is superseded by `Edit::map` — a
        // caller with `Edit<&u32>` reaches an owned `Edit<u32>` by
        // dereffing through map.
        let x = 42u32;
        let borrowed = Edit::Keep(&x);
        let owned = borrowed.map(|r| *r);
        assert_eq!(owned, Edit::Keep(42));
    }

    #[test]
    fn edit_is_keep_and_is_change() {
        let x = 0u8;
        assert!(Edit::Keep(&x).is_keep());
        assert!(!Edit::Keep(&x).is_change());
        assert!(Edit::Insert(&x).is_change());
        assert!(Edit::Delete(&x).is_change());
    }

    #[test]
    fn cleanup_collapses_interleaved_edits() {
        use cleanup::collapse_adjacent;
        let a = 1u32;
        let b = 2u32;
        let c = 3u32;
        let d = 4u32;
        // Interleaved: Del(a) Ins(b) Del(c) Ins(d)
        let script = vec![
            Edit::Delete(&a),
            Edit::Insert(&b),
            Edit::Delete(&c),
            Edit::Insert(&d),
        ];
        let cleaned = collapse_adjacent(script);
        // Expected: Del(a) Del(c) Ins(b) Ins(d) — deletes first.
        assert_eq!(
            cleaned,
            vec![
                Edit::Delete(&a),
                Edit::Delete(&c),
                Edit::Insert(&b),
                Edit::Insert(&d),
            ]
        );
    }
}
