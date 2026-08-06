//! Locate patterns inside a string.
//!
//! This module exposes ergonomic wrappers over the substring-search
//! kernels in [`stringcheese_compare::search`]. Every function returns
//! **byte offsets** — the same coordinate space [`str::find`] and
//! [`str::rfind`] use — and every operation is Unicode-agnostic (search
//! runs at the byte level, but because UTF-8 is prefix-free, a
//! byte-level match of a valid UTF-8 pattern in valid UTF-8 haystack is
//! always a scalar-aligned match).
//!
//! # Coordinate space
//!
//! Byte offsets, always. Callers who need scalar or grapheme offsets
//! should convert via [`str::char_indices`] or
//! [`stringcheese_unicode::graphemes()`] and are responsible for any
//! resulting cost.
//!
//! # Overlap semantics
//!
//! - [`find_all`], [`find_iter`], and [`count_matches`] report
//!   **non-overlapping** matches — the same semantics as
//!   [`str::matches`]. Searching for `"aa"` in `"aaaa"` yields matches
//!   at positions 0 and 2 (not 0, 1, 2).
//! - [`find_any`] is single-shot — it reports the leftmost match across
//!   any pattern in the input set, so overlap does not arise.
//!
//! # Algorithm delegation
//!
//! Single-pattern queries route through [`stringcheese_compare::Kmp`]
//! (Knuth-Morris-Pratt); the multi-pattern [`find_any`] uses
//! [`stringcheese_compare::AhoCorasick`]. This module never
//! reimplements substring search — every algorithm lives in
//! `stringcheese-compare::search`.
//!
//! # `no_std`
//!
//! Every item in this module is gated on `feature = "alloc"`:
//! [`stringcheese_compare`]'s search kernels themselves allocate their
//! preprocessing tables, so the delegation targets are only available
//! under `alloc`.

#![cfg(feature = "alloc")]

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use stringcheese_compare::search::{AhoCorasick, Kmp, SearchAlgorithm, SinglePatternSearch};

// ---------------------------------------------------------------------
// Single-pattern queries.
// ---------------------------------------------------------------------

/// Returns the byte offset of the first occurrence of `pat` in `haystack`,
/// or `None` if there is none.
///
/// Empty patterns match at position 0.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// assert_eq!(find::find("hello world", "world"), Some(6));
/// assert_eq!(find::find("hello", "z"), None);
/// // Empty pattern matches at 0.
/// assert_eq!(find::find("hello", ""), Some(0));
/// // Every string finds itself at 0.
/// assert_eq!(find::find("hello", "hello"), Some(0));
/// ```
#[must_use]
pub fn find(haystack: &str, pat: &str) -> Option<usize> {
    let prepared = Kmp::prepare(pat.as_bytes());
    Kmp::find(&prepared, haystack.as_bytes()).map(|m| m.position)
}

/// Returns the byte offset of the **last** occurrence of `pat` in
/// `haystack`, or `None` if there is none.
///
/// Delegates to [`str::rfind`]. Empty patterns are reported at
/// `haystack.len()` — the same as [`str::rfind`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// assert_eq!(find::rfind("hello world hello", "hello"), Some(12));
/// assert_eq!(find::rfind("hello", "z"), None);
/// ```
#[must_use]
#[inline]
pub fn rfind(haystack: &str, pat: &str) -> Option<usize> {
    haystack.rfind(pat)
}

/// Returns `true` if `pat` occurs anywhere in `haystack`.
///
/// Thin wrapper over [`find`] — equivalent to `find(...).is_some()`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// assert!(find::contains("hello world", "world"));
/// assert!(!find::contains("hello", "z"));
/// // Empty pattern always contained.
/// assert!(find::contains("hello", ""));
/// ```
#[must_use]
#[inline]
pub fn contains(haystack: &str, pat: &str) -> bool {
    find(haystack, pat).is_some()
}

/// Returns `true` if `haystack` begins with `pat`.
///
/// Delegates to [`str::starts_with`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// assert!(find::starts_with("hello world", "hello"));
/// assert!(!find::starts_with("hello world", "world"));
/// ```
#[must_use]
#[inline]
pub fn starts_with(haystack: &str, pat: &str) -> bool {
    haystack.starts_with(pat)
}

/// Returns `true` if `haystack` ends with `pat`.
///
/// Delegates to [`str::ends_with`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// assert!(find::ends_with("hello world", "world"));
/// assert!(!find::ends_with("hello world", "hello"));
/// ```
#[must_use]
#[inline]
pub fn ends_with(haystack: &str, pat: &str) -> bool {
    haystack.ends_with(pat)
}

// ---------------------------------------------------------------------
// Enumerating matches.
// ---------------------------------------------------------------------

/// Returns the byte offset of every **non-overlapping** occurrence of
/// `pat` in `haystack`, in ascending order.
///
/// Searching for `"aa"` in `"aaaa"` returns `[0, 2]` — after a match, the
/// scan advances past the match rather than by one byte. This matches the
/// semantics of [`str::matches`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// assert_eq!(find::find_all("banana", "an"), vec![1, 3]);
/// // Non-overlapping: "aa" in "aaaa" is [0, 2], not [0, 1, 2].
/// assert_eq!(find::find_all("aaaa", "aa"), vec![0, 2]);
/// // Empty pattern matches once at position 0.
/// assert_eq!(find::find_all("abc", ""), vec![0]);
/// ```
#[must_use]
pub fn find_all(haystack: &str, pat: &str) -> Vec<usize> {
    find_iter(haystack, pat).collect()
}

/// Returns the number of **non-overlapping** occurrences of `pat` in
/// `haystack`.
///
/// This is [`find_all(haystack, pat).len()`](find_all) without
/// materializing the intermediate vector.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// assert_eq!(find::count_matches("banana", "an"), 2);
/// assert_eq!(find::count_matches("aaaa", "aa"), 2);
/// assert_eq!(find::count_matches("abc", ""), 1);
/// ```
#[must_use]
pub fn count_matches(haystack: &str, pat: &str) -> usize {
    find_iter(haystack, pat).count()
}

/// Streams the byte offsets of every **non-overlapping** occurrence of
/// `pat` in `haystack`, in ascending order.
///
/// This is the streaming form of [`find_all`]. The returned iterator
/// prepares its search kernel once (as a KMP failure function) and
/// advances the haystack cursor lazily.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// let positions: Vec<usize> = find::find_iter("banana", "an").collect();
/// assert_eq!(positions, vec![1, 3]);
/// ```
pub fn find_iter<'h, 'p>(haystack: &'h str, pat: &'p str) -> FindIter<'h, 'p> {
    FindIter {
        haystack,
        pat,
        prepared: Kmp::prepare(pat.as_bytes()),
        offset: 0,
        exhausted: false,
    }
}

/// Streaming iterator returned by [`find_iter`].
///
/// Yields byte offsets of every non-overlapping match, in ascending
/// order. See [`find_iter`] for construction.
#[derive(Clone)]
pub struct FindIter<'h, 'p> {
    haystack: &'h str,
    pat: &'p str,
    prepared: <Kmp as SearchAlgorithm>::Prepared,
    offset: usize,
    exhausted: bool,
}

impl core::fmt::Debug for FindIter<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FindIter")
            .field("pat", &self.pat)
            .field("offset", &self.offset)
            .field("exhausted", &self.exhausted)
            .finish()
    }
}

impl Iterator for FindIter<'_, '_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.exhausted {
            return None;
        }
        // Empty-pattern policy: one match at position 0, then stop.
        if self.pat.is_empty() {
            self.exhausted = true;
            return Some(0);
        }
        if self.offset > self.haystack.len() {
            return None;
        }
        let slice = &self.haystack.as_bytes()[self.offset..];
        if let Some(m) = Kmp::find(&self.prepared, slice) {
            let pos = self.offset + m.position;
            // Non-overlapping: advance by pattern length.
            self.offset = pos + self.pat.len();
            Some(pos)
        } else {
            self.exhausted = true;
            None
        }
    }
}

// ---------------------------------------------------------------------
// Multi-pattern query.
// ---------------------------------------------------------------------

/// Returns the leftmost occurrence of any needle in `needles`, along with
/// the index of the needle that matched.
///
/// If two needles match at the same leftmost position, the one with the
/// lower `needle_index` is reported (deterministic tie-break).
///
/// Uses [`stringcheese_compare::AhoCorasick`] — the multi-pattern
/// automaton — to walk `haystack` once, regardless of `needles.len()`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::find;
///
/// let needles = &["dog", "cat", "at"];
/// // "cat" wins because it starts earliest.
/// assert_eq!(find::find_any("the cat sat", needles), Some((4, 1)));
///
/// // No match yields None.
/// assert_eq!(find::find_any("xyz", &["a", "b"]), None);
///
/// // Empty needle set yields None.
/// let empty: &[&str] = &[];
/// assert_eq!(find::find_any("hello", empty), None);
/// ```
#[must_use]
pub fn find_any(haystack: &str, needles: &[&str]) -> Option<(usize, usize)> {
    if needles.is_empty() {
        return None;
    }
    let byte_needles: Vec<&[u8]> = needles.iter().map(|n| n.as_bytes()).collect();
    let ac = AhoCorasick::build(&byte_needles);
    // find_all is sorted by ascending (position, pattern_index) — the
    // first entry is the deterministic leftmost match.
    let matches = ac.find_all(haystack.as_bytes());
    matches
        .into_iter()
        .next()
        .map(|m| (m.position, m.pattern_index))
}
