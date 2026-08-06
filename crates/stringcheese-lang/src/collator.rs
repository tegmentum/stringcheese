//! [`Collator`] — the plugin trait for locale-aware sort orders.
//!
//! A collator answers "does string A sort before, equal to, or after
//! string B under language L's rules?" Different languages fold and
//! order strings differently: Swedish sorts `å ä ö` after `z`, German
//! sorts `ß` as `ss`, Turkish distinguishes dotted and dotless `i`,
//! and CJK stroke-order collation is another thing entirely. The
//! trait's signature is deliberately minimal so any of these fits.
//!
//! Language packs that don't need a specialized collator return
//! `None` from
//! [`Language::collator`](crate::Language::collator); callers then
//! fall back to Unicode code-point ordering ([`str::cmp`] under the
//! hood).
//!
//! # Contract
//!
//! Implementations should be:
//!
//! - **Total.** Every `(a, b)` pair returns exactly one of
//!   [`Less`](core::cmp::Ordering::Less),
//!   [`Equal`](core::cmp::Ordering::Equal), or
//!   [`Greater`](core::cmp::Ordering::Greater).
//! - **Reflexive.** `compare(x, x)` returns
//!   [`Equal`](core::cmp::Ordering::Equal).
//! - **Antisymmetric.** `compare(a, b)` and `compare(b, a)` are
//!   reverses of each other.
//! - **Transitive.** If `compare(a, b) == Less` and
//!   `compare(b, c) == Less` then `compare(a, c) == Less`.
//!
//! These are the standard total-order axioms — the same axioms
//! [`core::cmp::Ord`] requires — restated because a language-specific
//! collator is more likely to have a subtle bug than the default
//! code-point order.

use core::cmp::Ordering;

/// A locale-aware string comparator.
///
/// See the [module-level docs](self) for the contract. Language packs
/// that don't ship a specialized collator return `None` from
/// [`Language::collator`](crate::Language::collator).
pub trait Collator: Send + Sync {
    /// Compares `a` against `b` under the collator's rules.
    fn compare(&self, a: &str, b: &str) -> Ordering;
}
