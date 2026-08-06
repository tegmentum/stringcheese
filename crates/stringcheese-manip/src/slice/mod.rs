//! Boundary-safe substring extraction.
//!
//! Every function in this module extracts a sub-region of the input at a
//! specific, named boundary — no silent choices, no "just call
//! `[a..b]` and hope". Each function names the boundary it works at:
//!
//! - **bytes** — raw UTF-8 byte offsets. [`slice_bytes`] returns
//!   `Option<&str>`; the result is `None` if either endpoint lands in the
//!   middle of a multi-byte scalar. [`take_bytes`] and [`drop_bytes`]
//!   panic on non-boundary offsets (matching [`str::split_at`]).
//! - **Unicode Scalar Values (USVs)** — `char`s; what [`str::chars`]
//!   yields. [`slice_scalars`], [`take_scalars`], [`drop_scalars`] index
//!   by scalar position.
//! - **extended grapheme clusters** — user-perceived characters, which
//!   may span multiple scalars. [`slice_graphemes`], [`take_graphemes`],
//!   [`drop_graphemes`] index by grapheme position and delegate to
//!   [`stringcheese_unicode::graphemes()`].
//!
//! # Allocation profile
//!
//! - [`slice_bytes`], [`take_bytes`], [`drop_bytes`], [`take_scalars`],
//!   [`drop_scalars`] return borrowed `&str`s and never allocate.
//! - [`slice_scalars`], [`slice_graphemes`], [`take_graphemes`],
//!   [`drop_graphemes`] return owned `String`s because the requested
//!   sub-region is not in general a contiguous slice of the input's UTF-8
//!   bytes indexed by the caller's boundary.
//!
//! # Range types
//!
//! The `slice_*` functions accept any type that implements
//! [`core::ops::RangeBounds<usize>`] — `a..b`, `a..`, `..b`, `..`, and
//! `..=b` all work. For [`slice_bytes`] the range types are those that
//! [`str::get`] accepts (i.e. anything that implements
//! [`core::slice::SliceIndex<str>`] with `str` output).
//!
//! # `no_std`
//!
//! Zero-alloc functions ([`slice_bytes`], `take_bytes`, `drop_bytes`,
//! `take_scalars`, `drop_scalars`) are available with no features enabled.
//! Owned-`String`-returning helpers ([`slice_scalars`],
//! [`slice_graphemes`], [`take_graphemes`], [`drop_graphemes`]) are gated
//! on `feature = "alloc"`.

#[cfg(test)]
mod tests;

#[cfg(feature = "alloc")]
use core::ops::{Bound, RangeBounds};

// ---------------------------------------------------------------------
// Byte-boundary slicing.
// ---------------------------------------------------------------------

/// Extracts the sub-slice of `s` at the given **UTF-8 byte** range.
///
/// Returns `None` if either endpoint of `range` is out of bounds or lands
/// in the middle of a multi-byte scalar. This delegates to [`str::get`],
/// so it accepts any range type — `a..b`, `a..`, `..b`, `..`, and `..=b`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// let s = "café";
/// assert_eq!(slice::slice_bytes(s, 0..3), Some("caf"));
/// assert_eq!(slice::slice_bytes(s, 3..), Some("é"));
/// assert_eq!(slice::slice_bytes(s, ..3), Some("caf"));
/// assert_eq!(slice::slice_bytes(s, ..), Some(s));
///
/// // Splitting the two-byte "é" mid-scalar is refused.
/// assert_eq!(slice::slice_bytes(s, 0..4), None);
/// // Out-of-bounds is refused too.
/// assert_eq!(slice::slice_bytes(s, 0..100), None);
/// ```
#[must_use]
#[inline]
pub fn slice_bytes<R>(s: &str, range: R) -> Option<&str>
where
    R: core::slice::SliceIndex<str, Output = str>,
{
    s.get(range)
}

/// Returns the first `n` **bytes** of `s`.
///
/// `n` is clamped to `s.len()`, so `take_bytes(s, s.len() + 100)` returns
/// the whole string rather than panicking on the out-of-range end.
///
/// # Panics
///
/// Panics if `n` lands in the middle of a multi-byte scalar. Callers who
/// want the fallible form should use [`slice_bytes`] with `..n`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// assert_eq!(slice::take_bytes("hello", 3), "hel");
/// assert_eq!(slice::take_bytes("hello", 100), "hello");
/// assert_eq!(slice::take_bytes("café", 3), "caf");
/// ```
#[must_use]
#[inline]
pub fn take_bytes(s: &str, n: usize) -> &str {
    let end = n.min(s.len());
    &s[..end]
}

/// Returns the sub-slice of `s` starting **after** its first `n` bytes.
///
/// `n` is clamped to `s.len()`, so `drop_bytes(s, s.len() + 100)` returns
/// the empty sub-slice at the end of `s` rather than panicking.
///
/// # Panics
///
/// Panics if `n` lands in the middle of a multi-byte scalar. Callers who
/// want the fallible form should use [`slice_bytes`] with `n..`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// assert_eq!(slice::drop_bytes("hello", 3), "lo");
/// assert_eq!(slice::drop_bytes("hello", 100), "");
/// assert_eq!(slice::drop_bytes("café", 3), "é");
/// ```
#[must_use]
#[inline]
pub fn drop_bytes(s: &str, n: usize) -> &str {
    let start = n.min(s.len());
    &s[start..]
}

// ---------------------------------------------------------------------
// Scalar-boundary slicing.
// ---------------------------------------------------------------------

/// Extracts the substring of `s` at the given **Unicode Scalar Value**
/// range.
///
/// The range is applied to the sequence of `char`s in the input (what
/// [`str::chars`] yields). The result is always a valid `String` even
/// when the range extends past the end of the input — extra scalars are
/// silently truncated, matching the standard-library iterator semantics.
///
/// Allocates because a scalar-indexed sub-region is not in general a
/// contiguous byte-slice starting at a known offset without a walk. The
/// walk here is `O(n)`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// assert_eq!(slice::slice_scalars("café", 0..3), "caf");
/// assert_eq!(slice::slice_scalars("café", 3..), "é");
/// assert_eq!(slice::slice_scalars("café", ..3), "caf");
/// assert_eq!(slice::slice_scalars("café", ..), "café");
///
/// // Ranges that overshoot are silently truncated.
/// assert_eq!(slice::slice_scalars("café", 0..100), "café");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn slice_scalars<R: RangeBounds<usize>>(s: &str, range: R) -> alloc::string::String {
    let (start, end) = resolve_bounds(&range, usize::MAX);
    let take = end.saturating_sub(start);
    s.chars().skip(start).take(take).collect()
}

/// Returns the first `n` **Unicode Scalar Values** of `s` as a borrowed
/// sub-slice.
///
/// If `n` exceeds the scalar count of `s`, the whole input is returned
/// (matching the behavior of iterator `take`).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// assert_eq!(slice::take_scalars("hello", 3), "hel");
/// assert_eq!(slice::take_scalars("café", 3), "caf");
/// assert_eq!(slice::take_scalars("café", 4), "café");
/// assert_eq!(slice::take_scalars("hi", 100), "hi");
/// ```
#[must_use]
pub fn take_scalars(s: &str, n: usize) -> &str {
    let end = s.char_indices().nth(n).map_or(s.len(), |(i, _)| i);
    &s[..end]
}

/// Returns the sub-slice of `s` starting **after** its first `n` Unicode
/// Scalar Values.
///
/// If `n` exceeds the scalar count of `s`, the empty sub-slice at the
/// end of `s` is returned.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// assert_eq!(slice::drop_scalars("hello", 3), "lo");
/// assert_eq!(slice::drop_scalars("café", 3), "é");
/// assert_eq!(slice::drop_scalars("café", 4), "");
/// assert_eq!(slice::drop_scalars("hi", 100), "");
/// ```
#[must_use]
pub fn drop_scalars(s: &str, n: usize) -> &str {
    let start = s.char_indices().nth(n).map_or(s.len(), |(i, _)| i);
    &s[start..]
}

// ---------------------------------------------------------------------
// Grapheme-boundary slicing.
// ---------------------------------------------------------------------

/// Extracts the substring of `s` at the given **grapheme-cluster** range.
///
/// The range is applied to the sequence of extended grapheme clusters
/// (per [Unicode Standard Annex #29]) in the input — what
/// [`stringcheese_unicode::graphemes()`] yields. Ranges that overshoot are
/// silently truncated.
///
/// Allocates for the output; the underlying grapheme iterator is
/// borrowing but the reassembly requires collecting into a `String`.
///
/// [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// // Decomposed "é" (e + combining acute) is one grapheme.
/// let s = "cafe\u{0301}";
/// assert_eq!(slice::slice_graphemes(s, 0..3), "caf");
/// assert_eq!(slice::slice_graphemes(s, 3..), "e\u{0301}");
/// // The whole flag emoji is one grapheme; slicing 0..1 keeps it intact.
/// assert_eq!(slice::slice_graphemes("\u{1F1EC}\u{1F1E7}", 0..1), "\u{1F1EC}\u{1F1E7}");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn slice_graphemes<R: RangeBounds<usize>>(s: &str, range: R) -> alloc::string::String {
    let (start, end) = resolve_bounds(&range, usize::MAX);
    let take = end.saturating_sub(start);
    stringcheese_unicode::graphemes(s)
        .skip(start)
        .take(take)
        .collect()
}

/// Returns the first `n` **grapheme clusters** of `s` as an owned
/// `String`.
///
/// If `n` exceeds the grapheme count of `s`, the whole input is returned.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// assert_eq!(slice::take_graphemes("hello", 3), "hel");
/// // The decomposed "é" is one grapheme even though it is two scalars.
/// assert_eq!(slice::take_graphemes("cafe\u{0301}", 4), "cafe\u{0301}");
/// assert_eq!(slice::take_graphemes("hi", 100), "hi");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn take_graphemes(s: &str, n: usize) -> alloc::string::String {
    stringcheese_unicode::graphemes(s).take(n).collect()
}

/// Returns the sub-string of `s` starting **after** its first `n`
/// grapheme clusters.
///
/// If `n` exceeds the grapheme count of `s`, an empty `String` is
/// returned.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::slice;
///
/// assert_eq!(slice::drop_graphemes("hello", 3), "lo");
/// // Dropping the four base-letter graphemes leaves nothing after the "é".
/// assert_eq!(slice::drop_graphemes("cafe\u{0301}", 4), "");
/// assert_eq!(slice::drop_graphemes("cafe\u{0301}!", 4), "!");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn drop_graphemes(s: &str, n: usize) -> alloc::string::String {
    stringcheese_unicode::graphemes(s).skip(n).collect()
}

// ---------------------------------------------------------------------
// Range-bounds helper.
// ---------------------------------------------------------------------

/// Resolves a `RangeBounds<usize>` into a `(start, end)` pair.
///
/// `end` is capped at `cap`; `start` is left uncapped because the
/// downstream `skip()`/`take()` combinators handle out-of-range starts
/// gracefully by returning empty iterators.
#[cfg(feature = "alloc")]
fn resolve_bounds<R: RangeBounds<usize>>(r: &R, cap: usize) -> (usize, usize) {
    let start = match r.start_bound() {
        Bound::Included(&n) => n,
        Bound::Excluded(&n) => n.saturating_add(1),
        Bound::Unbounded => 0,
    };
    let end = match r.end_bound() {
        Bound::Included(&n) => n.saturating_add(1).min(cap),
        Bound::Excluded(&n) => n.min(cap),
        Bound::Unbounded => cap,
    };
    (start, end)
}
