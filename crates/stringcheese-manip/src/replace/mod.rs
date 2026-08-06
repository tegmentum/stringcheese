//! Substitute matches in a string.
//!
//! Every function in this module returns (or requires) an owned
//! [`String`]. Callers who only want to *locate* matches without
//! materializing a new string should reach for [`crate::find`] instead.
//!
//! # Boundaries
//!
//! - **Substring replaces** ([`replace`], [`replace_n`], [`replace_first`],
//!   [`replace_last`], [`replace_with`], [`remove`], [`replace_bounded`],
//!   [`replace_many`]) match on a byte-for-byte substring. Because both
//!   the needle and the haystack are valid `&str`, no scalar is ever
//!   split mid-way; the returned `String` is guaranteed valid UTF-8.
//! - **Char-level replaces** ([`replace_matches`], [`translate`]) map at
//!   the Unicode Scalar Value (`char`) boundary. The predicate or lookup
//!   table is asked about `char`s.
//!
//! # Empty-needle policy
//!
//! Every function that takes a needle string treats an **empty needle**
//! as a **no-op**: it returns a clone of the input unchanged rather than
//! inserting the replacement between every scalar (which is
//! [`str::replace`]'s behavior). This choice was made deliberately —
//! silent expansion is a common source of bugs and length-explosion
//! surprises. If you truly want the "insert between every char"
//! semantics, call [`str::replace`] directly.
//!
//! # Allocation profile
//!
//! Every function performs at most a small constant number of
//! allocations (typically one for the output buffer, plus per-match
//! writes). [`replace_bounded`] additionally caps the output length so
//! adversarial inputs cannot force unbounded growth
//! (`replace("a".repeat(1M), "a", "aa")` doubles every pass — the
//! bounded variant guards against that shape of "replacement
//! amplification" attack).
//!
//! # `no_std`
//!
//! Every item in this module is gated on `feature = "alloc"`: an owned
//! `String` output requires the heap. [`replace_many`] additionally
//! relies on [`stringcheese_compare::search::AhoCorasick`], which itself
//! requires `alloc`.

#![cfg(feature = "alloc")]

use alloc::string::String;
use alloc::vec::Vec;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------
// Substring replacement.
// ---------------------------------------------------------------------

/// Replaces every non-overlapping occurrence of `from` in `s` with `to`,
/// returning a fresh `String`.
///
/// An empty `from` is a **no-op** (see the module documentation for the
/// rationale): the input is cloned unchanged.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// assert_eq!(replace::replace("banana", "a", "o"), "bonono");
/// assert_eq!(replace::replace("hello", "xyz", "!"), "hello");
/// // Empty needle is a no-op:
/// assert_eq!(replace::replace("abc", "", "!"), "abc");
/// ```
#[must_use]
pub fn replace(s: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return String::from(s);
    }
    s.replace(from, to)
}

/// Replaces the first `n` non-overlapping occurrences of `from` in `s`
/// with `to`, returning a fresh `String`.
///
/// Equivalent to [`str::replacen`] with the empty-needle-is-no-op policy
/// applied. If `n == 0`, the input is cloned unchanged.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// assert_eq!(replace::replace_n("banana", "a", "o", 2), "bonona");
/// // Cap at zero: no change.
/// assert_eq!(replace::replace_n("banana", "a", "o", 0), "banana");
/// // More than exists: replaces all of them.
/// assert_eq!(replace::replace_n("banana", "a", "o", 99), "bonono");
/// ```
#[must_use]
pub fn replace_n(s: &str, from: &str, to: &str, n: usize) -> String {
    if from.is_empty() || n == 0 {
        return String::from(s);
    }
    s.replacen(from, to, n)
}

/// Replaces the first occurrence of `from` in `s` with `to`, returning a
/// fresh `String`. Exactly equivalent to `replace_n(s, from, to, 1)`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// assert_eq!(replace::replace_first("banana", "a", "o"), "bonana");
/// assert_eq!(replace::replace_first("hello", "xyz", "!"), "hello");
/// ```
#[must_use]
pub fn replace_first(s: &str, from: &str, to: &str) -> String {
    replace_n(s, from, to, 1)
}

/// Replaces the *last* occurrence of `from` in `s` with `to`, returning
/// a fresh `String`.
///
/// Uses [`str::rfind`] to locate the terminal match. If `from` never
/// occurs, the input is cloned unchanged. An empty `from` is a no-op.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// assert_eq!(replace::replace_last("banana", "a", "o"), "banano");
/// assert_eq!(replace::replace_last("hello", "xyz", "!"), "hello");
/// ```
#[must_use]
pub fn replace_last(s: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return String::from(s);
    }
    match s.rfind(from) {
        Some(pos) => {
            let mut out = String::with_capacity(s.len() - from.len() + to.len());
            out.push_str(&s[..pos]);
            out.push_str(to);
            out.push_str(&s[pos + from.len()..]);
            out
        }
        None => String::from(s),
    }
}

/// Replaces every scalar for which `predicate` returns `true` with the
/// string `to`.
///
/// The predicate is called once per `char`. Because the replacement is a
/// full `&str`, the output can differ in length from the input.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// // Strip digits by replacing them with nothing.
/// let out = replace::replace_matches("h1e2l3l4o", |c: char| c.is_ascii_digit(), "");
/// assert_eq!(out, "hello");
///
/// // Expand each vowel to two copies:
/// let out = replace::replace_matches("hello", |c: char| "aeiou".contains(c), "*");
/// assert_eq!(out, "h*ll*");
/// ```
#[must_use]
pub fn replace_matches<P>(s: &str, mut predicate: P, to: &str) -> String
where
    P: FnMut(char) -> bool,
{
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if predicate(c) {
            out.push_str(to);
        } else {
            out.push(c);
        }
    }
    out
}

/// Replaces every non-overlapping occurrence of `from` in `s` with the
/// output of `replacer(matched)`, returning a fresh `String`.
///
/// The closure receives the exact byte substring that was matched (which,
/// for a literal-needle search, is always equal to `from`). This is the
/// building block for regex-like scenarios that only need a fixed needle
/// but a dynamic replacement (uppercasing a match, wrapping it, etc.).
///
/// An empty `from` is a no-op.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// let out = replace::replace_with("hello world", "world", |m| {
///     format!("[{}]", m.to_uppercase())
/// });
/// assert_eq!(out, "hello [WORLD]");
/// ```
#[must_use]
pub fn replace_with<F>(s: &str, from: &str, mut replacer: F) -> String
where
    F: FnMut(&str) -> String,
{
    if from.is_empty() {
        return String::from(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut last_end = 0;
    let mut i = 0;
    while i + from.len() <= s.len() {
        if s.is_char_boundary(i) && s[i..].starts_with(from) {
            out.push_str(&s[last_end..i]);
            out.push_str(&replacer(&s[i..i + from.len()]));
            i += from.len();
            last_end = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&s[last_end..]);
    out
}

/// Replaces each scalar in `s` according to the (from, to) pairs in
/// `mapping`, à la Python's `str.translate`.
///
/// The mapping is scanned linearly per input scalar. For large mapping
/// tables consider constructing a `HashMap` externally and iterating;
/// this function is optimized for small, ergonomic tables (a handful of
/// entries).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// let out = replace::translate("hello", &[('l', 'L'), ('o', '0')]);
/// assert_eq!(out, "heLL0");
/// // Unmapped characters pass through unchanged.
/// let out = replace::translate("abc", &[('x', 'y')]);
/// assert_eq!(out, "abc");
/// ```
#[must_use]
pub fn translate(s: &str, mapping: &[(char, char)]) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let replaced = mapping
            .iter()
            .find_map(|(from, to)| if *from == c { Some(*to) } else { None })
            .unwrap_or(c);
        out.push(replaced);
    }
    out
}

/// Removes every non-overlapping occurrence of `needle` from `s`,
/// returning a fresh `String`. Sugar for `replace(s, needle, "")`.
///
/// An empty `needle` is a no-op.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// assert_eq!(replace::remove("hello world", "l"), "heo word");
/// assert_eq!(replace::remove("abc123", "123"), "abc");
/// ```
#[must_use]
pub fn remove(s: &str, needle: &str) -> String {
    replace(s, needle, "")
}

/// Error returned by [`replace_bounded`] when a substitution would grow
/// the output past its length cap.
///
/// Callers can use this to safely reject replacement-amplification
/// inputs (`"a".repeat(N)` with `from="a", to="aa"` doubles every pass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplaceError {
    /// The length cap that was exceeded, in bytes.
    pub max_len: usize,
    /// The byte length the output would have reached had the operation
    /// not been aborted.
    pub attempted_len: usize,
}

impl core::fmt::Display for ReplaceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "replacement output would exceed the {}-byte cap (attempted {} bytes)",
            self.max_len, self.attempted_len
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ReplaceError {}

/// Replaces every non-overlapping occurrence of `from` in `s` with `to`,
/// aborting with a [`ReplaceError`] if the output would exceed
/// `max_len` bytes.
///
/// This is the version to reach for when the input is untrusted and the
/// replacement string could be larger than the needle — a shape known as
/// "replacement amplification". For example, replacing `"a"` with
/// `"a".repeat(100)` doubles output size per pass; the bounded variant
/// caps that early rather than crashing on an allocation.
///
/// An empty `from` is a no-op that returns the cloned input (never
/// exceeds `max_len` unless the input itself does — see the check
/// below).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// // Fits under the cap:
/// assert_eq!(replace::replace_bounded("hello", "l", "LL", 32).unwrap(), "heLLLLo");
///
/// // Exceeds the cap — returns Err instead of a huge string.
/// let e = replace::replace_bounded("aaaa", "a", "bbb", 8).unwrap_err();
/// assert_eq!(e.max_len, 8);
/// assert!(e.attempted_len > 8);
/// ```
pub fn replace_bounded(
    s: &str,
    from: &str,
    to: &str,
    max_len: usize,
) -> Result<String, ReplaceError> {
    if from.is_empty() {
        if s.len() > max_len {
            return Err(ReplaceError {
                max_len,
                attempted_len: s.len(),
            });
        }
        return Ok(String::from(s));
    }
    let mut out = String::new();
    let mut i = 0;
    let mut last_end = 0;
    while let Some(rel) = s[i..].find(from) {
        let pos = i + rel;
        let piece_len = pos - last_end;
        let attempted = out.len() + piece_len + to.len();
        if attempted > max_len {
            return Err(ReplaceError {
                max_len,
                attempted_len: attempted,
            });
        }
        out.push_str(&s[last_end..pos]);
        out.push_str(to);
        i = pos + from.len();
        last_end = i;
    }
    let attempted = out.len() + (s.len() - last_end);
    if attempted > max_len {
        return Err(ReplaceError {
            max_len,
            attempted_len: attempted,
        });
    }
    out.push_str(&s[last_end..]);
    Ok(out)
}

/// Applies many (needle, replacement) substitutions to `s` in a single
/// pass, using [`stringcheese_compare::search::AhoCorasick`] for the
/// underlying multi-pattern search.
///
/// Matches are consumed left-to-right; **overlapping matches take the
/// earliest** (and, among those tied on start, the leftmost in the
/// `pairs` slice — mirroring Aho-Corasick's "first output at this end
/// state" ordering). Non-matching regions are copied through unchanged.
///
/// This is dramatically faster than looping [`replace`] once per pair
/// when the pattern set is large: `O(|s| + Σ|patterns|)` vs.
/// `O(|s| * Σ|patterns|)`.
///
/// Empty needles in `pairs` are silently skipped (they would otherwise
/// insert their replacement between every byte, which is inconsistent
/// with the module's empty-needle policy).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::replace;
///
/// let out = replace::replace_many(
///     "the quick brown fox",
///     &[("quick", "slow"), ("brown", "red")],
/// );
/// assert_eq!(out, "the slow red fox");
///
/// // Overlapping patterns: leftmost start wins.
/// let out = replace::replace_many("aabbcc", &[("aabb", "X"), ("bbcc", "Y")]);
/// assert_eq!(out, "Xcc");
/// ```
#[must_use]
pub fn replace_many(s: &str, pairs: &[(&str, &str)]) -> String {
    use stringcheese_compare::search::AhoCorasick;

    // Filter out empty needles up front; index-remap the survivors so we
    // can look up each match's replacement by AC's `pattern_index`.
    let survivors: Vec<(&str, &str)> = pairs
        .iter()
        .copied()
        .filter(|(f, _)| !f.is_empty())
        .collect();
    if survivors.is_empty() {
        return String::from(s);
    }
    let patterns: Vec<&[u8]> = survivors.iter().map(|(f, _)| f.as_bytes()).collect();
    let ac = AhoCorasick::build(&patterns);
    let mut matches = ac.find_all(s.as_bytes());
    // Sort by (position, pattern_index) so the earliest start wins and
    // ties break deterministically (leftmost pair in the input list).
    matches.sort_by_key(|m| (m.position, m.pattern_index));

    let mut out = String::with_capacity(s.len());
    let mut cursor = 0;
    for m in matches {
        if m.position < cursor {
            // This match overlaps a previously-consumed region — skip it.
            continue;
        }
        // Snap to a char boundary — AhoCorasick works on raw bytes, so
        // a pattern could match in the middle of a multi-byte scalar
        // (extremely unlikely for the pattern-shaped inputs the API
        // encourages, but the check is cheap and load-bearing for
        // correctness).
        if !s.is_char_boundary(m.position) {
            continue;
        }
        let end = m.position + survivors[m.pattern_index].0.len();
        if !s.is_char_boundary(end) {
            continue;
        }
        out.push_str(&s[cursor..m.position]);
        out.push_str(survivors[m.pattern_index].1);
        cursor = end;
    }
    out.push_str(&s[cursor..]);
    out
}
