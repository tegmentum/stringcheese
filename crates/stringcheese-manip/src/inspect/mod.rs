//! Read-only interrogation of a string.
//!
//! Every function in this module answers a *question* about a `&str`
//! without touching its bytes and without allocating. Each function names
//! the boundary it works at:
//!
//! - **bytes** — the raw UTF-8 length of the string ([`byte_len`]).
//! - **Unicode Scalar Values (USVs)** — `char`s; what `str::chars` yields
//!   ([`scalar_count`], [`first_char`], [`last_char`]).
//! - **extended grapheme clusters** — what a user perceives as one
//!   "character"; may span multiple scalars ([`grapheme_count`],
//!   [`first_grapheme`], [`last_grapheme`]). Delegated to
//!   [`stringcheese_unicode`].
//!
//! Never guess. Callers pick the boundary that matches the question they
//! are actually asking:
//!
//! - Sizing an I/O buffer? [`byte_len`].
//! - Counting `char`s for a scalar-level algorithm? [`scalar_count`].
//! - Counting *user-perceived characters* — the answer to "how long is
//!   this word?" — [`grapheme_count`].
//!
//! # Allocation profile
//!
//! Every function here is zero-allocation. Grapheme-aware helpers
//! ([`grapheme_count`], [`first_grapheme`], [`last_grapheme`]) require
//! the `alloc` feature only because they route through
//! [`stringcheese_unicode`] — the operation itself walks the string
//! without any heap traffic.
//!
//! # `no_std`
//!
//! The byte / scalar / character helpers are available with no features
//! enabled. The grapheme-aware helpers are gated behind
//! `feature = "alloc"`; without that feature, the crate that provides
//! grapheme segmentation is compiled as an empty surface.

#[cfg(test)]
mod tests;

/// Returns `true` if the string contains no bytes.
///
/// This is exactly equivalent to [`str::is_empty`] and is included so
/// callers reading `inspect::*` code do not have to jump between the
/// module namespace and the primitive's inherent method.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert!(inspect::is_empty(""));
/// assert!(!inspect::is_empty("x"));
/// ```
#[must_use]
#[inline]
pub fn is_empty(s: &str) -> bool {
    s.is_empty()
}

/// Returns the length of the string in **UTF-8 bytes**.
///
/// This is what [`str::len`] returns. It is *not* a character count — a
/// single grapheme can span from one to many bytes, depending on the
/// scripts involved and whether the sequence includes combining marks.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert_eq!(inspect::byte_len(""), 0);
/// assert_eq!(inspect::byte_len("abc"), 3);
/// // A single non-ASCII scalar is often two or more bytes:
/// assert_eq!(inspect::byte_len("é"), 2);
/// ```
#[must_use]
#[inline]
pub fn byte_len(s: &str) -> usize {
    s.len()
}

/// Returns the number of **Unicode Scalar Values** (`char`s) in the
/// string.
///
/// A USV is what `str::chars` yields. `scalar_count` walks the string
/// once and counts the yielded values; it does not allocate.
///
/// This count agrees with [`byte_len`] only for pure-ASCII strings.
/// It may exceed [`grapheme_count`] because a single grapheme can be
/// composed of many scalars (`e` + combining acute is two scalars but
/// one grapheme).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert_eq!(inspect::scalar_count(""), 0);
/// assert_eq!(inspect::scalar_count("abc"), 3);
/// // "é" as e + combining acute is two scalars, one grapheme.
/// assert_eq!(inspect::scalar_count("e\u{0301}"), 2);
/// ```
#[must_use]
#[inline]
pub fn scalar_count(s: &str) -> usize {
    s.chars().count()
}

/// Returns the number of **extended grapheme clusters** in the string,
/// per [Unicode Standard Annex #29].
///
/// A grapheme cluster is the smallest unit of text a human would call
/// "one character". Flags, emoji sequences, and precomposed vs.
/// decomposed accented letters all count as *one* grapheme even though
/// their scalar count varies.
///
/// Zero allocation — the grapheme iterator walks the string without
/// buffering the segments.
///
/// [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert_eq!(inspect::grapheme_count(""), 0);
/// // Precomposed "é" is one grapheme.
/// assert_eq!(inspect::grapheme_count("caf\u{00E9}"), 4);
/// // Decomposed "é" (e + combining acute) is also one grapheme.
/// assert_eq!(inspect::grapheme_count("cafe\u{0301}"), 4);
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn grapheme_count(s: &str) -> usize {
    stringcheese_unicode::graphemes(s).count()
}

/// Returns the first Unicode scalar value, or `None` if the string is
/// empty.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert_eq!(inspect::first_char(""), None);
/// assert_eq!(inspect::first_char("abc"), Some('a'));
/// assert_eq!(inspect::first_char("é"), Some('é'));
/// ```
#[must_use]
#[inline]
pub fn first_char(s: &str) -> Option<char> {
    s.chars().next()
}

/// Returns the last Unicode scalar value, or `None` if the string is
/// empty.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert_eq!(inspect::last_char(""), None);
/// assert_eq!(inspect::last_char("abc"), Some('c'));
/// // Watch out: the last *scalar* of a decomposed accented letter is
/// // the combining mark, not the base letter — the last *grapheme*
/// // would be the whole cluster. See `last_grapheme`.
/// assert_eq!(inspect::last_char("e\u{0301}"), Some('\u{0301}'));
/// ```
#[must_use]
#[inline]
pub fn last_char(s: &str) -> Option<char> {
    s.chars().next_back()
}

/// Returns the first extended grapheme cluster as a borrowed sub-slice,
/// or `None` if the string is empty.
///
/// Zero allocation — the returned `&str` is a slice of the input.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert_eq!(inspect::first_grapheme(""), None);
/// assert_eq!(inspect::first_grapheme("abc"), Some("a"));
/// // The whole decomposed "é" is the first grapheme, not just "e".
/// assert_eq!(inspect::first_grapheme("e\u{0301}bc"), Some("e\u{0301}"));
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn first_grapheme(s: &str) -> Option<&str> {
    stringcheese_unicode::graphemes(s).next()
}

/// Returns the last extended grapheme cluster as a borrowed sub-slice,
/// or `None` if the string is empty.
///
/// Zero allocation. Runs in `O(n)` because [`stringcheese_unicode`]'s
/// grapheme iterator is exposed as a plain `impl Iterator`; the walk
/// starts from the beginning to find the last cluster. A future revision
/// may expose a `DoubleEndedIterator` for `O(1)` access.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::inspect;
///
/// assert_eq!(inspect::last_grapheme(""), None);
/// assert_eq!(inspect::last_grapheme("abc"), Some("c"));
/// // The decomposed "é" is the final grapheme, not just the acute.
/// assert_eq!(inspect::last_grapheme("abe\u{0301}"), Some("e\u{0301}"));
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn last_grapheme(s: &str) -> Option<&str> {
    stringcheese_unicode::graphemes(s).last()
}
