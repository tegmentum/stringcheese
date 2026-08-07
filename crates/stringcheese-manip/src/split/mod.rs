//! Divide a string into pieces.
//!
//! Every function in this module returns an iterator (or `Option`) of
//! **borrowed sub-slices** of the input — no allocation, no copying, no
//! mutation. All splitting happens at scalar boundaries: the underlying
//! `str::split` machinery never returns a `&str` whose bounds fall in the
//! middle of a multi-byte scalar, so the yielded items are always valid
//! UTF-8.
//!
//! # Boundaries
//!
//! Each function names the boundary it splits at:
//!
//! - **Separator strings** ([`split`], [`splitn`], [`rsplit`],
//!   [`split_terminator`], [`split_once`], [`rsplit_once`]) match on a
//!   byte-for-byte substring. The separator is compared as bytes; because
//!   it must itself be a valid `&str`, no scalar is ever split mid-way.
//! - **Predicates on scalars** ([`split_matches`]) call the predicate
//!   once per `char`.
//! - **Whitespace** ([`split_whitespace`]) is exactly [`str::split_whitespace`]:
//!   any Unicode `White_Space` scalar delimits, and runs of whitespace
//!   collapse — no empty fragments are yielded.
//! - **Lines** ([`split_lines`]) is exactly [`str::lines`], hoisted into
//!   this namespace for API completeness. The forthcoming [`crate::lines`]
//!   module will offer richer line-oriented operations; use this thin
//!   wrapper when you only need the delimiter-agnostic line split.
//! - **Extended grapheme clusters** ([`split_graphemes`]) delegates to
//!   [`stringcheese_unicode::graphemes()`] and returns one item per grapheme.
//! - **UAX #29 words** ([`split_words`]) delegates to
//!   [`stringcheese_unicode::words()`] — the same UAX #29 word-boundary
//!   rules that treat `"don't"` as one word and drop punctuation.
//! - **UAX #29 sentences** ([`split_sentences`]) delegates to
//!   [`stringcheese_unicode::sentences()`] — the sentence-boundary
//!   rules that split on `.`, `!`, `?` with numeric-decimal /
//!   abbreviation carve-outs.
//!
//! # Allocation profile
//!
//! Every function here is zero-allocation on its own. The grapheme
//! iterator requires the `alloc` feature only because it routes through
//! [`stringcheese_unicode`]; the walk itself is heap-free.
//!
//! # `no_std`
//!
//! Every separator-, predicate-, whitespace-, and line-based split is
//! available with no features enabled. [`split_graphemes`],
//! [`split_words`], and [`split_sentences`] are gated on
//! `feature = "alloc"` — plus, respectively, the underlying
//! `stringcheese-unicode/word-segmentation` and
//! `stringcheese-unicode/sentence-segmentation` features (both on by
//! default in this crate's `default` feature set).

#[cfg(test)]
mod tests;

/// Splits `s` at every occurrence of `sep`, yielding the pieces between
/// separators as borrowed sub-slices.
///
/// Exactly equivalent to [`str::split`] over a `&str` needle. Zero
/// allocation.
///
/// When `sep` occurs at either end of the input, an empty fragment is
/// yielded for that end. If `sep` never occurs, the whole input is
/// yielded as a single item. If `sep` is empty, the iterator yields an
/// empty item at every scalar boundary — same as `str::split("")`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let parts: Vec<&str> = split::split("a,b,c", ",").collect();
/// assert_eq!(parts, vec!["a", "b", "c"]);
///
/// // Trailing separator produces a trailing empty item:
/// let parts: Vec<&str> = split::split("a,", ",").collect();
/// assert_eq!(parts, vec!["a", ""]);
///
/// // No occurrence: one item, the whole input.
/// let parts: Vec<&str> = split::split("abc", ",").collect();
/// assert_eq!(parts, vec!["abc"]);
/// ```
#[must_use]
#[inline]
pub fn split<'s, 'p>(s: &'s str, sep: &'p str) -> core::str::Split<'s, &'p str> {
    s.split(sep)
}

/// Splits `s` at every run of Unicode whitespace, yielding the
/// non-whitespace fragments as borrowed sub-slices.
///
/// Exactly equivalent to [`str::split_whitespace`]. Runs of consecutive
/// whitespace collapse into a single split point and no empty fragments
/// are ever yielded — leading and trailing whitespace disappear.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let parts: Vec<&str> = split::split_whitespace("  hello   world  ").collect();
/// assert_eq!(parts, vec!["hello", "world"]);
///
/// // Tabs, newlines, and Unicode whitespace all delimit.
/// let parts: Vec<&str> = split::split_whitespace("a\tb\nc\u{00A0}d").collect();
/// assert_eq!(parts, vec!["a", "b", "c", "d"]);
/// ```
#[must_use]
#[inline]
pub fn split_whitespace(s: &str) -> core::str::SplitWhitespace<'_> {
    s.split_whitespace()
}

/// Splits `s` at every scalar for which `predicate` returns `true`.
///
/// The predicate is called once per `char`. Equivalent to [`str::split`]
/// with a closure needle; zero allocation.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let parts: Vec<&str> = split::split_matches("a1b2c3", |c: char| c.is_ascii_digit()).collect();
/// assert_eq!(parts, vec!["a", "b", "c", ""]);
///
/// // Predicate that never matches: one item, the whole input.
/// let parts: Vec<&str> = split::split_matches("hello", |c: char| c == '/').collect();
/// assert_eq!(parts, vec!["hello"]);
/// ```
#[inline]
pub fn split_matches<P>(s: &str, predicate: P) -> impl Iterator<Item = &str>
where
    P: FnMut(char) -> bool,
{
    s.split(predicate)
}

/// Splits `s` at every occurrence of `sep`, but *suppresses* the trailing
/// empty item that would arise from a separator at the very end of the
/// input.
///
/// Exactly equivalent to [`str::split_terminator`]. Useful when the
/// separator is a *terminator* rather than an infix delimiter (line
/// endings, record separators, and the like).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// // Trailing separator no longer yields an empty final piece.
/// let parts: Vec<&str> = split::split_terminator("a,b,c,", ",").collect();
/// assert_eq!(parts, vec!["a", "b", "c"]);
///
/// // Without a trailing separator the result matches `split`.
/// let parts: Vec<&str> = split::split_terminator("a,b,c", ",").collect();
/// assert_eq!(parts, vec!["a", "b", "c"]);
/// ```
#[must_use]
#[inline]
pub fn split_terminator<'s, 'p>(
    s: &'s str,
    sep: &'p str,
) -> core::str::SplitTerminator<'s, &'p str> {
    s.split_terminator(sep)
}

/// Splits `s` at every occurrence of `sep`, but stops after `n - 1`
/// splits — the last item contains the unsplit remainder.
///
/// Exactly equivalent to [`str::splitn`]. If `n == 0`, the returned
/// iterator yields nothing. If `n == 1`, the whole input is returned as
/// a single item.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// // Cap at 2: one split, the rest stays whole.
/// let parts: Vec<&str> = split::splitn("a,b,c,d", 2, ",").collect();
/// assert_eq!(parts, vec!["a", "b,c,d"]);
///
/// // `n == 1` yields the whole input.
/// let parts: Vec<&str> = split::splitn("a,b,c", 1, ",").collect();
/// assert_eq!(parts, vec!["a,b,c"]);
/// ```
#[must_use]
#[inline]
pub fn splitn<'s, 'p>(s: &'s str, n: usize, sep: &'p str) -> core::str::SplitN<'s, &'p str> {
    s.splitn(n, sep)
}

/// Splits `s` at every occurrence of `sep`, iterating from the right end
/// of the input.
///
/// Exactly equivalent to [`str::rsplit`]. The items are yielded in
/// reverse order — the piece after the *last* separator comes out first.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let parts: Vec<&str> = split::rsplit("a,b,c", ",").collect();
/// assert_eq!(parts, vec!["c", "b", "a"]);
/// ```
#[must_use]
#[inline]
pub fn rsplit<'s, 'p>(s: &'s str, sep: &'p str) -> core::str::RSplit<'s, &'p str> {
    s.rsplit(sep)
}

/// Splits `s` at the first occurrence of `sep`, returning the piece
/// before and the piece after as a two-tuple. Returns `None` if `sep`
/// does not occur.
///
/// Exactly equivalent to [`str::split_once`]. The two returned slices
/// together with `sep` reconstruct the input:
/// `s == format!("{}{}{}", head, sep, tail)`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// assert_eq!(split::split_once("key=value", "="), Some(("key", "value")));
/// // A separator with no tail still yields the empty tail:
/// assert_eq!(split::split_once("key=", "="), Some(("key", "")));
/// // No occurrence:
/// assert_eq!(split::split_once("keyvalue", "="), None);
/// ```
#[must_use]
#[inline]
pub fn split_once<'s>(s: &'s str, sep: &str) -> Option<(&'s str, &'s str)> {
    s.split_once(sep)
}

/// Splits `s` at the *last* occurrence of `sep`, returning the piece
/// before and the piece after as a two-tuple. Returns `None` if `sep`
/// does not occur.
///
/// Exactly equivalent to [`str::rsplit_once`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// assert_eq!(split::rsplit_once("a.b.c", "."), Some(("a.b", "c")));
/// assert_eq!(split::rsplit_once("no-dot", "."), None);
/// ```
#[must_use]
#[inline]
pub fn rsplit_once<'s>(s: &'s str, sep: &str) -> Option<(&'s str, &'s str)> {
    s.rsplit_once(sep)
}

/// Splits `s` into lines, matching [`str::lines`] exactly.
///
/// A line boundary is any of `\n`, `\r\n`, or (as a legacy Mac-style
/// terminator handled by std) a bare `\r` in some code paths. Line
/// endings are stripped from the yielded items. See the standard-library
/// documentation for the precise contract.
///
/// This is a thin re-export for API completeness — the forthcoming
/// [`crate::lines`] module will offer richer line-oriented operations
/// (non-empty-only iteration, per-line trimming, prefix/suffix). Use
/// this when you only need the plain delimiter-agnostic line split.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let parts: Vec<&str> = split::split_lines("a\nb\r\nc").collect();
/// assert_eq!(parts, vec!["a", "b", "c"]);
///
/// // Trailing newline does not yield an empty final line.
/// let parts: Vec<&str> = split::split_lines("a\nb\n").collect();
/// assert_eq!(parts, vec!["a", "b"]);
/// ```
#[inline]
pub fn split_lines(s: &str) -> core::str::Lines<'_> {
    s.lines()
}

/// Splits `s` into extended grapheme clusters, one per yielded item, per
/// [Unicode Standard Annex #29].
///
/// Delegates to [`stringcheese_unicode::graphemes()`]. A grapheme cluster
/// is the smallest unit of text a human would call "one character";
/// precomposed vs. decomposed accented letters, emoji flags, and ZWJ
/// sequences all count as *one* grapheme even though their scalar count
/// varies.
///
/// Zero allocation — the yielded `&str`s are sub-slices of the input.
///
/// [Unicode Standard Annex #29]: https://www.unicode.org/reports/tr29/
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let parts: Vec<&str> = split::split_graphemes("naïve").collect();
/// assert_eq!(parts, vec!["n", "a", "ï", "v", "e"]);
///
/// // Decomposed é is one grapheme, not two:
/// let parts: Vec<&str> = split::split_graphemes("e\u{0301}b").collect();
/// assert_eq!(parts, vec!["e\u{0301}", "b"]);
/// ```
#[cfg(feature = "alloc")]
pub fn split_graphemes(s: &str) -> impl Iterator<Item = &str> {
    stringcheese_unicode::graphemes(s)
}

/// Splits `s` into UAX #29 words, dropping whitespace and
/// punctuation.
///
/// Delegates to [`stringcheese_unicode::words()`]. Under UAX #29,
/// `"don't"` is a single word (the apostrophe is a joiner), `"3.14"`
/// is a single word (numeric decimal), and stray punctuation such as
/// `","` or `"!"` is *not* yielded — see
/// [`stringcheese_unicode::word_bounds`] if you want the
/// input-preserving view.
///
/// Zero allocation — the yielded `&str`s are sub-slices of the input.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let ws: Vec<&str> = split::split_words("don't stop!").collect();
/// assert_eq!(ws, vec!["don't", "stop"]);
///
/// let ws: Vec<&str> = split::split_words("hello, world").collect();
/// assert_eq!(ws, vec!["hello", "world"]);
/// ```
#[cfg(feature = "alloc")]
pub fn split_words(s: &str) -> impl Iterator<Item = &str> {
    stringcheese_unicode::words(s)
}

/// Splits `s` into UAX #29 sentences.
///
/// Delegates to [`stringcheese_unicode::sentences()`]. Boundaries are
/// inferred from the Unicode `Sentence_Break` property — mostly `.`,
/// `!`, and `?` followed by whitespace, with carve-outs for numeric
/// decimals (`"3.14"` does not break). Trailing whitespace between
/// sentences belongs to the *earlier* sentence's yielded slice, so
/// the concatenation of the yielded sentences reconstructs the input.
///
/// Zero allocation — the yielded `&str`s are sub-slices of the input.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::split;
///
/// let ss: Vec<&str> = split::split_sentences("Hello. World.").collect();
/// assert_eq!(ss.len(), 2);
///
/// // Numeric decimal is not a sentence break.
/// let ss: Vec<&str> = split::split_sentences("Pi is 3.14 today.").collect();
/// assert_eq!(ss.len(), 1);
/// ```
#[cfg(feature = "alloc")]
pub fn split_sentences(s: &str) -> impl Iterator<Item = &str> {
    stringcheese_unicode::sentences(s)
}
