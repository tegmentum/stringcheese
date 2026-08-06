//! Combine string pieces back into a single owned `String`.
//!
//! Every function in this module returns (or writes into) an owned
//! [`String`]. Callers who need only borrowed sub-slices should stay in
//! the [`crate::split`] world; `join`'s job is the composition side of
//! the trip.
//!
//! # Allocation profile
//!
//! - [`join`], [`concat()`], [`intercalate`], [`join_with`] each perform
//!   **exactly one** heap allocation: the item list is walked once to
//!   compute the total byte-length of the output, then the destination
//!   `String` is built with `String::with_capacity(total)` so no
//!   reallocation happens as the pieces are pushed.
//! - [`join_into`] appends into a caller-owned buffer. If the buffer
//!   already has capacity for the output, this function performs **zero**
//!   allocations at all; otherwise the buffer grows exactly once (via
//!   `String::reserve(deficit)` — not the amortized-doubling of naive
//!   `push_str`).
//!
//! The one-allocation guarantee costs one preliminary pass over the
//! items. For iterators whose items are cheap to walk (`Vec<&str>`,
//! slices, arrays) this is a wash; for expensive iterators the caller
//! can materialize once and pass a slice to avoid double work.
//!
//! # Boundaries
//!
//! Every function joins at the **byte** boundary — the input pieces are
//! concatenated with the separator's bytes between them. Since every
//! piece is a valid `&str` and the separator is a `&str`, the resulting
//! `String` is guaranteed valid UTF-8. No normalization, no Unicode
//! sanitization; if you need those, run [`crate::normalize`] afterwards.
//!
//! # `no_std`
//!
//! Every item in this module is gated on `feature = "alloc"`: an owned
//! `String` output requires the heap.

#![cfg(feature = "alloc")]

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(test)]
mod tests;

/// Joins `items` with `sep` between each pair, returning a fresh
/// `String`.
///
/// The item list is walked once to compute the total output length, then
/// the destination `String` is built with pre-reserved capacity. This
/// costs one preliminary pass but guarantees exactly one heap allocation
/// for the output — no incremental reallocations as pieces are appended.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::join;
///
/// assert_eq!(join::join(["a", "b", "c"], ","), "a,b,c");
/// // Any `S: AsRef<str>` works — strings, string slices, `Cow<str>`.
/// let parts: Vec<String> = vec!["hello".into(), "world".into()];
/// assert_eq!(join::join(parts, " "), "hello world");
///
/// // Empty input yields an empty string.
/// assert_eq!(join::join(Vec::<&str>::new(), ","), "");
/// // Single item is returned verbatim (no separator to insert).
/// assert_eq!(join::join(["solo"], ","), "solo");
/// ```
#[must_use]
pub fn join<I, S>(items: I, sep: &str) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let items: Vec<S> = items.into_iter().collect();
    if items.is_empty() {
        return String::new();
    }
    let total: usize = items.iter().map(|s| s.as_ref().len()).sum::<usize>()
        + sep.len() * items.len().saturating_sub(1);
    let mut out = String::with_capacity(total);
    let mut first = true;
    for item in &items {
        if !first {
            out.push_str(sep);
        }
        first = false;
        out.push_str(item.as_ref());
    }
    out
}

/// Appends the join of `items` (with `sep` between each pair) into
/// `out`.
///
/// If `out` already has capacity for the appended data, this function
/// performs **zero** heap allocations. Otherwise the buffer is grown
/// exactly once to fit the deficit (never the amortized-doubling that
/// a naive [`push_str`](String::push_str) loop would trigger).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::join;
///
/// let mut buf = String::from("prefix:");
/// join::join_into(["a", "b", "c"], ",", &mut buf);
/// assert_eq!(buf, "prefix:a,b,c");
///
/// // Repeated calls append without touching what's already there.
/// join::join_into([":d", ":e"], "", &mut buf);
/// assert_eq!(buf, "prefix:a,b,c:d:e");
/// ```
pub fn join_into<I, S>(items: I, sep: &str, out: &mut String)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let items: Vec<S> = items.into_iter().collect();
    if items.is_empty() {
        return;
    }
    let total: usize = items.iter().map(|s| s.as_ref().len()).sum::<usize>()
        + sep.len() * items.len().saturating_sub(1);
    out.reserve(total);
    let mut first = true;
    for item in &items {
        if !first {
            out.push_str(sep);
        }
        first = false;
        out.push_str(item.as_ref());
    }
}

/// Joins `items` after applying `format` to each item, with `sep`
/// between each pair.
///
/// Useful when the items are not themselves strings — a formatting
/// closure converts each `T` into an `S: AsRef<str>` on the fly.
///
/// This function walks the input twice — once to format-and-buffer, and
/// once to compute total capacity — because the formatter may allocate
/// per call and there is no way to size the output without observing
/// every formatted piece.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::join;
///
/// let items = [1, 2, 3];
/// let out = join::join_with(items, ", ", |n| n.to_string());
/// assert_eq!(out, "1, 2, 3");
/// ```
#[must_use]
pub fn join_with<I, T, S, F>(items: I, sep: &str, format: F) -> String
where
    I: IntoIterator<Item = T>,
    S: AsRef<str>,
    F: Fn(&T) -> S,
{
    let items: Vec<T> = items.into_iter().collect();
    if items.is_empty() {
        return String::new();
    }
    let formatted: Vec<S> = items.iter().map(&format).collect();
    let total: usize = formatted.iter().map(|s| s.as_ref().len()).sum::<usize>()
        + sep.len() * formatted.len().saturating_sub(1);
    let mut out = String::with_capacity(total);
    let mut first = true;
    for s in &formatted {
        if !first {
            out.push_str(sep);
        }
        first = false;
        out.push_str(s.as_ref());
    }
    out
}

/// Concatenates `items` with no separator between them.
///
/// Exactly equivalent to `join(items, "")`; single-allocation.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::join;
///
/// assert_eq!(join::concat(["hello", " ", "world"]), "hello world");
/// assert_eq!(join::concat(Vec::<&str>::new()), "");
/// ```
#[must_use]
pub fn concat<I, S>(items: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    join(items, "")
}

/// Synonym for [`join`], for callers who prefer the Haskell-flavored
/// name.
///
/// The behavior is bit-for-bit identical to [`join`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::join;
///
/// assert_eq!(join::intercalate([", "; 0], ""), "");
/// assert_eq!(join::intercalate(["a", "b", "c"], "-"), "a-b-c");
/// ```
#[must_use]
pub fn intercalate<I, S>(items: I, sep: &str) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    join(items, sep)
}
