//! Remove characters from the edges of a string.
//!
//! Every function in this module returns a **borrowed sub-slice** of the
//! input — no allocation, no copying, no mutation. All trimming happens
//! at the *scalar* boundary: the predicate is asked about `char`s and
//! only whole characters are stripped, so a returned `&str` is always
//! valid UTF-8.
//!
//! # Free functions vs. configured operation
//!
//! Two API shapes cover different use cases:
//!
//! - **Free functions** ([`trim`], [`trim_matches`], [`trim_chars`], and
//!   the `_start` / `_end` variants) are the pleasant default when a
//!   trim happens once at a call site.
//! - **The [`Trim`] configured operation** captures the trim policy —
//!   which edges, which characters or predicate — as a reusable value,
//!   allocating any character-set or boxed-predicate storage once so
//!   [`Trim::apply`] can be called any number of times without repeating
//!   the setup cost.
//!
//! # Whitespace
//!
//! The free `whitespace` variants ([`trim`], [`trim_start`], [`trim_end`])
//! delegate directly to the standard library, which strips characters
//! whose Unicode `White_Space` property is `Yes`. This is exactly what
//! [`str::trim`] does; the wrapper is here so callers using `inspect::*`
//! and `trim::*` idiomatically do not have to switch back to inherent
//! methods.
//!
//! # `no_std`
//!
//! The free functions require no features. [`Trim`] and its supporting
//! types are gated on `feature = "alloc"` because they own a `Vec<char>`
//! (for character sets) or a `Box<dyn Fn>` (for predicates).

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------
// Free functions.
// ---------------------------------------------------------------------

/// Removes leading and trailing Unicode whitespace.
///
/// Delegates to [`str::trim`]; whitespace is defined by the Unicode
/// `White_Space` property. Zero allocation.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim("  hello  "), "hello");
/// assert_eq!(trim::trim("\thi\n"), "hi");
/// ```
#[must_use]
#[inline]
pub fn trim(s: &str) -> &str {
    s.trim()
}

/// Removes leading Unicode whitespace.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_start("  hello  "), "hello  ");
/// ```
#[must_use]
#[inline]
pub fn trim_start(s: &str) -> &str {
    s.trim_start()
}

/// Removes trailing Unicode whitespace.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_end("  hello  "), "  hello");
/// ```
#[must_use]
#[inline]
pub fn trim_end(s: &str) -> &str {
    s.trim_end()
}

/// Removes characters from both ends of `s` for as long as `predicate`
/// returns `true`.
///
/// The predicate is asked about `char`s. Zero allocation.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_matches("///path///", |c: char| c == '/'), "path");
/// assert_eq!(trim::trim_matches("aabbccbbaa", |c: char| c == 'a'), "bbccbb");
/// ```
#[must_use]
#[inline]
pub fn trim_matches<P>(s: &str, predicate: P) -> &str
where
    P: Fn(char) -> bool,
{
    s.trim_matches(predicate)
}

/// Removes characters from the start of `s` for as long as `predicate`
/// returns `true`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_start_matches("---note", |c: char| c == '-'), "note");
/// ```
#[must_use]
#[inline]
pub fn trim_start_matches<P>(s: &str, predicate: P) -> &str
where
    P: Fn(char) -> bool,
{
    s.trim_start_matches(predicate)
}

/// Removes characters from the end of `s` for as long as `predicate`
/// returns `true`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_end_matches("note---", |c: char| c == '-'), "note");
/// ```
#[must_use]
#[inline]
pub fn trim_end_matches<P>(s: &str, predicate: P) -> &str
where
    P: Fn(char) -> bool,
{
    s.trim_end_matches(predicate)
}

/// Removes any character in `chars` from both ends of `s`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_chars("//a/b//", &['/']), "a/b");
/// assert_eq!(trim::trim_chars(" \t hi \t ", &[' ', '\t']), "hi");
/// ```
#[must_use]
#[inline]
pub fn trim_chars<'s>(s: &'s str, chars: &[char]) -> &'s str {
    s.trim_matches(|c: char| chars.contains(&c))
}

/// Removes any character in `chars` from the start of `s`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_start_chars("//a/b//", &['/']), "a/b//");
/// ```
#[must_use]
#[inline]
pub fn trim_start_chars<'s>(s: &'s str, chars: &[char]) -> &'s str {
    s.trim_start_matches(|c: char| chars.contains(&c))
}

/// Removes any character in `chars` from the end of `s`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::trim;
///
/// assert_eq!(trim::trim_end_chars("//a/b//", &['/']), "//a/b");
/// ```
#[must_use]
#[inline]
pub fn trim_end_chars<'s>(s: &'s str, chars: &[char]) -> &'s str {
    s.trim_end_matches(|c: char| chars.contains(&c))
}

// ---------------------------------------------------------------------
// Configured operation.
// ---------------------------------------------------------------------

/// Which edges of the input to trim.
///
/// Consumed by [`Trim::edges`] and inspected by [`Trim::apply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TrimEdge {
    /// Trim from both the leading and trailing edge. This is the default
    /// for a freshly constructed [`Trim`].
    #[default]
    Both,
    /// Trim only from the leading edge.
    Start,
    /// Trim only from the trailing edge.
    End,
}

/// A reusable trim policy — captures the character set (or predicate) and
/// which edges to strip, so a single value can be applied many times
/// without repeating the setup cost.
///
/// This is the "configured operation" level of the crate's four-level
/// API. Where the free functions in this module are the pleasant default
/// for one-off calls, `Trim` is what you build once and reach for
/// repeatedly:
///
/// ```
/// use stringcheese_manip::trim::{Trim, TrimEdge};
///
/// // Strip surrounding slashes from every path fragment.
/// let strip_slashes = Trim::chars(&['/']);
/// assert_eq!(strip_slashes.apply("//a//"), "a");
/// assert_eq!(strip_slashes.apply("/b/"), "b");
///
/// // Only from the leading edge:
/// let lead = Trim::chars(&['-']).edges(TrimEdge::Start);
/// assert_eq!(lead.apply("---note---"), "note---");
/// ```
///
/// # Allocation profile
///
/// Character-set trims own a `Vec<char>` (allocated once at construction
/// and re-used across every [`apply`](Trim::apply) call).
/// Predicate trims own a `Box<dyn Fn>`. Whitespace and edge selection do
/// not allocate.
///
/// # `no_std`
///
/// Gated on `feature = "alloc"`.
#[cfg(feature = "alloc")]
pub struct Trim {
    strategy: TrimStrategy,
    edge: TrimEdge,
}

#[cfg(feature = "alloc")]
enum TrimStrategy {
    /// Trim Unicode whitespace.
    Whitespace,
    /// Trim any character in the owned set.
    Chars(alloc::vec::Vec<char>),
    /// Trim by user-supplied predicate.
    Predicate(alloc::boxed::Box<dyn Fn(char) -> bool + Send + Sync>),
}

#[cfg(feature = "alloc")]
impl core::fmt::Debug for Trim {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let strategy_name = match &self.strategy {
            TrimStrategy::Whitespace => "Whitespace",
            TrimStrategy::Chars(_) => "Chars",
            TrimStrategy::Predicate(_) => "Predicate",
        };
        f.debug_struct("Trim")
            .field("strategy", &strategy_name)
            .field("edge", &self.edge)
            .finish()
    }
}

#[cfg(feature = "alloc")]
impl Trim {
    /// A trim policy that strips Unicode whitespace from both ends of the
    /// input.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::trim::Trim;
    ///
    /// let ws = Trim::whitespace();
    /// assert_eq!(ws.apply("  hello  "), "hello");
    /// ```
    #[must_use]
    pub fn whitespace() -> Self {
        Self {
            strategy: TrimStrategy::Whitespace,
            edge: TrimEdge::Both,
        }
    }

    /// A trim policy that strips any character in `chars`. The character
    /// set is copied into an owned `Vec<char>` so the returned `Trim`
    /// outlives the passed slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::trim::Trim;
    ///
    /// let slashes = Trim::chars(&['/']);
    /// assert_eq!(slashes.apply("//a//"), "a");
    /// ```
    #[must_use]
    pub fn chars(chars: &[char]) -> Self {
        Self {
            strategy: TrimStrategy::Chars(chars.to_vec()),
            edge: TrimEdge::Both,
        }
    }

    /// A trim policy that strips characters for which `predicate` returns
    /// `true`. The predicate is boxed so `Trim` remains a concrete
    /// (non-generic) type suitable for storage in pipelines and
    /// collections.
    ///
    /// The predicate must be `Send + Sync + 'static` — good hygiene for
    /// values that may cross threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::trim::Trim;
    ///
    /// let digits = Trim::predicate(|c: char| c.is_ascii_digit());
    /// assert_eq!(digits.apply("42hello99"), "hello");
    /// ```
    #[must_use]
    pub fn predicate<F>(predicate: F) -> Self
    where
        F: Fn(char) -> bool + Send + Sync + 'static,
    {
        Self {
            strategy: TrimStrategy::Predicate(alloc::boxed::Box::new(predicate)),
            edge: TrimEdge::Both,
        }
    }

    /// Sets which edges [`apply`](Self::apply) will trim.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::trim::{Trim, TrimEdge};
    ///
    /// let lead = Trim::whitespace().edges(TrimEdge::Start);
    /// assert_eq!(lead.apply("  hi  "), "hi  ");
    /// ```
    #[must_use]
    pub fn edges(mut self, edge: TrimEdge) -> Self {
        self.edge = edge;
        self
    }

    /// Applies this policy to `s`, returning a borrowed sub-slice of the
    /// input.
    ///
    /// Zero allocation per call — the character set / predicate is
    /// captured once at construction.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::trim::Trim;
    ///
    /// let quoted = Trim::chars(&['"', '\'']);
    /// assert_eq!(quoted.apply("\"hi\""), "hi");
    /// assert_eq!(quoted.apply("'hi'"), "hi");
    /// ```
    #[must_use]
    pub fn apply<'s>(&self, s: &'s str) -> &'s str {
        match (&self.strategy, self.edge) {
            (TrimStrategy::Whitespace, TrimEdge::Both) => s.trim(),
            (TrimStrategy::Whitespace, TrimEdge::Start) => s.trim_start(),
            (TrimStrategy::Whitespace, TrimEdge::End) => s.trim_end(),
            (TrimStrategy::Chars(cs), TrimEdge::Both) => s.trim_matches(|c: char| cs.contains(&c)),
            (TrimStrategy::Chars(cs), TrimEdge::Start) => {
                s.trim_start_matches(|c: char| cs.contains(&c))
            }
            (TrimStrategy::Chars(cs), TrimEdge::End) => {
                s.trim_end_matches(|c: char| cs.contains(&c))
            }
            (TrimStrategy::Predicate(f), TrimEdge::Both) => s.trim_matches(|c: char| f(c)),
            (TrimStrategy::Predicate(f), TrimEdge::Start) => s.trim_start_matches(|c: char| f(c)),
            (TrimStrategy::Predicate(f), TrimEdge::End) => s.trim_end_matches(|c: char| f(c)),
        }
    }
}
