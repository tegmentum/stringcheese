//! Padding policy for [`NGramGenerator`] boundaries.
//!
//! # Why padding matters
//!
//! Without padding, an input shorter than `n` produces *no* grams at all,
//! and short inputs produce dramatically fewer grams than long inputs of the
//! same information content. A trigram set over the string `"kit"` under the
//! `PaddingPolicy::None` policy is `{"kit"}` — a single gram — while the
//! same trigram set over `"kitten"` is `{"kit", "itt", "tte", "ten"}`. Set
//! similarities computed against those two representations weight one gram
//! against four; that is almost never what a caller wants when comparing
//! short strings.
//!
//! Boundary markers preserve prefix and suffix signal. Under the
//! [`PaddingPolicy::Boundary`] policy with markers `'^'` and `'$'`, `"kit"`
//! becomes the effective sequence `['^', '^', 'k', 'i', 't', '$', '$']`
//! before window enumeration (for `n = 3`); the trigram set is
//! `{"^^k", "^ki", "kit", "it$", "t$$"}`. That set is comparable in size to
//! the trigram set of longer strings, and it retains the information that
//! `"k"` began the string and `"t"` ended it — a signal that unpadded
//! bigrams over `"kit"` and `"skit"` completely erase.
//!
//! # Marker semantics
//!
//! [`PaddingPolicy::Boundary`] places `n - 1` copies of the start marker
//! immediately before the input and `n - 1` copies of the end marker
//! immediately after it. This is the classical padding scheme; every gram
//! window that touches the input boundary now starts (or ends) with at
//! least one marker.
//!
//! Distinct start and end markers matter. Using the same marker on both
//! sides creates artificial palindromic collisions — `'^cat^'` and
//! `'^tac^'` share more grams under a single-marker scheme than under a
//! two-marker scheme. StringCheese keeps them separate by default.
//!
//! # Custom padding
//!
//! [`PaddingPolicy::Custom`] takes an explicit prefix and suffix as owned
//! [`alloc::vec::Vec`]s. The prefix is prepended once (not `n - 1` times);
//! the suffix is appended once. This is the general-purpose form — a
//! caller who wants three sentinel symbols on the left and one on the
//! right can express that directly.
//!
//! The variant is behind the `alloc` feature because it stores owned
//! [`Vec`]s; on the pure-`core` build, [`PaddingPolicy`] is unavailable
//! entirely (the whole crate's public surface is `alloc`-gated).
//!
//! [`NGramGenerator`]: crate::NGramGenerator
//! [`Vec`]: alloc::vec::Vec

use alloc::vec::Vec;
use core::fmt;

/// How a generator handles sequence boundaries.
///
/// Parameterized over the same symbol type the generator emits inside each
/// gram. `T: Clone` is required because padding markers are copied into
/// every gram that crosses a boundary.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PaddingPolicy<T: Clone> {
    /// No padding. Only positions where a full `n`-window fits produce a
    /// gram; inputs shorter than `n` produce zero grams.
    None,
    /// Prepend `n - 1` copies of `start` and append `n - 1` copies of `end`.
    ///
    /// This is the classical padding scheme. Every gram that touches an
    /// original boundary starts (or ends) with at least one marker.
    Boundary {
        /// The marker prepended `n - 1` times before the input.
        start: T,
        /// The marker appended `n - 1` times after the input.
        end: T,
    },
    /// Prepend `prefix` once and append `suffix` once — the general-purpose
    /// form when neither the sentinel count nor the sentinel symbol at each
    /// end is uniform.
    Custom {
        /// Symbols prepended, in order, once.
        prefix: Vec<T>,
        /// Symbols appended, in order, once.
        suffix: Vec<T>,
    },
}

impl<T: Clone> PaddingPolicy<T> {
    /// Returns the number of padding symbols this policy adds to the left
    /// of the input, given a generator arity `n`.
    #[must_use]
    pub fn left_len(&self, n: usize) -> usize {
        match self {
            Self::None => 0,
            Self::Boundary { .. } => n.saturating_sub(1),
            Self::Custom { prefix, .. } => prefix.len(),
        }
    }

    /// Returns the number of padding symbols this policy adds to the right
    /// of the input, given a generator arity `n`.
    #[must_use]
    pub fn right_len(&self, n: usize) -> usize {
        match self {
            Self::None => 0,
            Self::Boundary { .. } => n.saturating_sub(1),
            Self::Custom { suffix, .. } => suffix.len(),
        }
    }

    /// Materializes `input` with the policy's padding applied, into a fresh
    /// [`Vec`].
    ///
    /// This is the reference implementation of the padding rules;
    /// generators use it (or an equivalent) to build the effective sequence
    /// they window over.
    ///
    /// [`Vec`]: alloc::vec::Vec
    #[must_use]
    pub fn apply(&self, input: &[T], n: usize) -> Vec<T> {
        let mut out = Vec::with_capacity(input.len() + self.left_len(n) + self.right_len(n));
        match self {
            Self::None => {}
            Self::Boundary { start, .. } => {
                for _ in 0..n.saturating_sub(1) {
                    out.push(start.clone());
                }
            }
            Self::Custom { prefix, .. } => out.extend(prefix.iter().cloned()),
        }
        out.extend(input.iter().cloned());
        match self {
            Self::None => {}
            Self::Boundary { end, .. } => {
                for _ in 0..n.saturating_sub(1) {
                    out.push(end.clone());
                }
            }
            Self::Custom { suffix, .. } => out.extend(suffix.iter().cloned()),
        }
        out
    }
}

/// Constructor precondition violation: `n` must be nonzero.
///
/// A generator with `n == 0` has no defined semantics — there is no
/// zero-length window that meaningfully represents a sequence — and
/// silently producing zero grams (or panicking deep inside an iterator)
/// would hide the mistake. Every constructor in this crate that takes `n`
/// either panics with a clear message or exposes a fallible companion that
/// returns [`InvalidN`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct InvalidN;

impl fmt::Display for InvalidN {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("n-gram arity `n` must be at least 1")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InvalidN {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn none_adds_no_padding() {
        let p: PaddingPolicy<char> = PaddingPolicy::None;
        assert_eq!(p.left_len(3), 0);
        assert_eq!(p.right_len(3), 0);
        let padded = p.apply(&['c', 'a', 't'], 3);
        assert_eq!(padded, vec!['c', 'a', 't']);
    }

    #[test]
    fn boundary_adds_n_minus_one_each_side() {
        let p = PaddingPolicy::Boundary {
            start: '^',
            end: '$',
        };
        assert_eq!(p.left_len(3), 2);
        assert_eq!(p.right_len(3), 2);
        let padded = p.apply(&['c', 'a', 't'], 3);
        assert_eq!(padded, vec!['^', '^', 'c', 'a', 't', '$', '$']);
    }

    #[test]
    fn boundary_zero_pad_at_arity_one() {
        // `n = 1` needs zero markers on each side — a unigram never crosses
        // a boundary in a way that a marker could reveal.
        let p = PaddingPolicy::Boundary {
            start: '^',
            end: '$',
        };
        let padded = p.apply(&['c', 'a', 't'], 1);
        assert_eq!(padded, vec!['c', 'a', 't']);
    }

    #[test]
    fn custom_uses_prefix_and_suffix_once() {
        let p = PaddingPolicy::Custom {
            prefix: vec!['<', '<'],
            suffix: vec!['>'],
        };
        // Note: `n` is irrelevant for `Custom`'s padding count — the caller
        // committed to a specific length when they built the prefix/suffix.
        assert_eq!(p.left_len(3), 2);
        assert_eq!(p.right_len(3), 1);
        let padded = p.apply(&['c', 'a', 't'], 3);
        assert_eq!(padded, vec!['<', '<', 'c', 'a', 't', '>']);
    }

    #[test]
    fn invalid_n_displays_message() {
        let e = InvalidN;
        assert_eq!(alloc::format!("{e}"), "n-gram arity `n` must be at least 1");
    }
}
