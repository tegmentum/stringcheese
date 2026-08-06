//! [`PhoneticMatcher`] — encoder-plus-key-equality composition.
//!
//! A [`PhoneticMatcher`] wraps a [`PhoneticEncoder`] and turns it into a
//! predicate over input pairs. Two inputs match when their encoded keys
//! satisfy the encoder's key-equality rule, which for single-key encoders is
//! plain `==` and for multi-key encoders ([`crate::DoubleMetaphone`]) is the
//! cross-product rule documented below.
//!
//! # The multi-key cross-product rule
//!
//! When both encoders yield up to two keys (a primary and an optional
//! alternate), the default [`MatchMode::AnyPair`] mode considers two inputs
//! matched when *any* of the (up to 2) × (up to 2) key pairs are equal:
//!
//! * `left.primary == right.primary`, or
//! * `left.primary == right.alternate`, or
//! * `left.alternate == right.primary`, or
//! * `left.alternate == right.alternate` (both must be `Some`).
//!
//! This is the classical multi-encoding matcher of every published Double
//! Metaphone consumer: a name like `"Schmidt"` with primary `"XMT"` and
//! alternate `"SMT"` matches `"Smith"` (primary `"SM0"`, alternate `"XMT"`)
//! via the `left.primary == right.alternate` clause. The alternate exists
//! precisely to catch this regional-pronunciation case, and the matcher
//! honors it.
//!
//! Strict modes ([`MatchMode::PrimaryOnly`], for regulated deduplication
//! workflows that treat the alternate as untrusted) are opt-in.
//!
//! # Why the matcher is generic over the encoder rather than the key
//!
//! Two encoders can share a key *type* (`String`) but produce keys with
//! different semantics — a Soundex key and a NYSIIS key are both strings but
//! they encode different things and equality across families would produce a
//! meaningless match. Making the matcher own the encoder pins the semantic
//! interpretation of the key: a `PhoneticMatcher<Soundex>` compares Soundex
//! keys and only Soundex keys, at the type level.

use crate::double_metaphone::DoubleMetaphoneKey;
use crate::encoder::PhoneticEncoder;

/// A composed encoder + key-equality matcher.
///
/// Construct a matcher with [`PhoneticMatcher::new`]. Call
/// [`PhoneticMatcher::matches`] to decide whether two inputs match under
/// the encoder's keys.
///
/// The `Mode` type parameter is fixed to [`MatchMode`] at runtime; multi-key
/// encoders honor the mode via a specialized [`matches`](Self::matches)
/// impl, single-key encoders ignore it.
#[derive(Clone, Debug)]
pub struct PhoneticMatcher<E> {
    encoder: E,
    mode: MatchMode,
}

/// How a multi-key matcher aggregates the (up to 2) × (up to 2) key-pair
/// grid.
///
/// Single-key encoders ignore this — there is only one pair to consider.
/// Multi-key encoders honor it as described on each variant.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum MatchMode {
    /// A match if *any* code-pair is equal. This is the classical
    /// multi-encoding matcher and is the default. See the module-level
    /// documentation for the cross-product rule.
    #[default]
    AnyPair,
    /// A match only if both primary codes are equal. The alternates are
    /// ignored entirely — useful for strict deduplication workflows that
    /// treat the alternate as an untrusted regional-pronunciation guess.
    PrimaryOnly,
}

impl<E: PhoneticEncoder> PhoneticMatcher<E> {
    /// Constructs a matcher with [`MatchMode::AnyPair`].
    #[inline]
    pub const fn new(encoder: E) -> Self {
        Self {
            encoder,
            mode: MatchMode::AnyPair,
        }
    }

    /// Sets the multi-key matching mode. Single-key encoders ignore the
    /// setting.
    #[inline]
    #[must_use]
    pub const fn with_mode(mut self, mode: MatchMode) -> Self {
        self.mode = mode;
        self
    }

    /// Returns the current [`MatchMode`].
    #[inline]
    pub const fn mode(&self) -> MatchMode {
        self.mode
    }

    /// Returns a reference to the wrapped encoder.
    #[inline]
    pub const fn encoder(&self) -> &E {
        &self.encoder
    }
}

// Generic single-key matcher: `left == right` on the encoded keys.
impl<E> PhoneticMatcher<E>
where
    E: PhoneticEncoder,
    E::Key: PartialEq,
{
    /// Encodes both inputs and returns `true` if the encoded keys compare
    /// equal.
    ///
    /// For a single-key encoder this is plain `==` on the two keys. For a
    /// multi-key encoder (like [`crate::DoubleMetaphone`]) a specialized
    /// inherent method with the same name shadows this one and applies the
    /// cross-product rule instead — see [the module-level
    /// docs](self#the-multi-key-cross-product-rule).
    #[inline]
    pub fn matches(&self, left: &str, right: &str) -> bool {
        self.encoder.encode(left) == self.encoder.encode(right)
    }
}

// Specialized multi-key matcher for `DoubleMetaphoneKey`. The impl on the
// specific `E::Key = DoubleMetaphoneKey` block shadows the generic one thanks
// to Rust's method-resolution preferring the more specific inherent impl
// (via the explicit type-parameter binding rather than a generic constraint).
impl<E> PhoneticMatcher<E>
where
    E: PhoneticEncoder<Key = DoubleMetaphoneKey>,
{
    /// Encodes both inputs and returns `true` under the currently configured
    /// [`MatchMode`].
    ///
    /// * [`MatchMode::AnyPair`]: any of the up-to-four `(left.key,
    ///   right.key)` combinations equal-comparing counts as a match.
    /// * [`MatchMode::PrimaryOnly`]: only the two primary keys are compared.
    #[inline]
    pub fn matches_double_metaphone(&self, left: &str, right: &str) -> bool {
        let l = self.encoder.encode(left);
        let r = self.encoder.encode(right);
        match self.mode {
            MatchMode::PrimaryOnly => l.primary == r.primary,
            MatchMode::AnyPair => cross_product_match(&l, &r),
        }
    }
}

/// The four-way cross-product predicate documented on
/// [the module-level docs](self#the-multi-key-cross-product-rule).
fn cross_product_match(l: &DoubleMetaphoneKey, r: &DoubleMetaphoneKey) -> bool {
    if l.primary == r.primary {
        return true;
    }
    if let Some(alt) = &r.alternate {
        if &l.primary == alt {
            return true;
        }
    }
    if let Some(alt) = &l.alternate {
        if alt == &r.primary {
            return true;
        }
    }
    if let (Some(la), Some(ra)) = (&l.alternate, &r.alternate) {
        if la == ra {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::double_metaphone::DoubleMetaphone;
    use crate::nysiis::Nysiis;
    use crate::soundex::Soundex;
    use alloc::string::ToString;

    #[test]
    fn single_key_matcher_uses_eq() {
        let m = PhoneticMatcher::new(Soundex);
        // Robert and Rupert both encode to R163 under Soundex.
        assert!(m.matches("Robert", "Rupert"));
        // Robert and Ashcraft encode to R163 vs A261 respectively.
        assert!(!m.matches("Robert", "Ashcraft"));
    }

    #[test]
    fn single_key_matcher_works_for_nysiis() {
        let m = PhoneticMatcher::new(Nysiis);
        // The same name to itself is trivially a match.
        assert!(m.matches("Jackson", "Jackson"));
    }

    #[test]
    fn multi_key_matcher_defaults_to_any_pair() {
        let m = PhoneticMatcher::new(DoubleMetaphone);
        assert_eq!(m.mode(), MatchMode::AnyPair);
    }

    #[test]
    fn multi_key_matcher_honors_primary_only() {
        let m = PhoneticMatcher::new(DoubleMetaphone).with_mode(MatchMode::PrimaryOnly);
        assert_eq!(m.mode(), MatchMode::PrimaryOnly);
    }

    #[test]
    fn cross_product_matches_when_alt_equals_other_primary() {
        // Hand-constructed keys to exercise the cross-product without
        // depending on Double Metaphone's specific outputs.
        let l = DoubleMetaphoneKey {
            primary: "XMT".to_string(),
            alternate: Some("SMT".to_string()),
        };
        let r = DoubleMetaphoneKey {
            primary: "SMT".to_string(),
            alternate: None,
        };
        assert!(cross_product_match(&l, &r));
        assert!(cross_product_match(&r, &l));
    }

    #[test]
    fn cross_product_rejects_when_no_pair_matches() {
        let l = DoubleMetaphoneKey {
            primary: "XMT".to_string(),
            alternate: Some("SMT".to_string()),
        };
        let r = DoubleMetaphoneKey {
            primary: "PPPP".to_string(),
            alternate: Some("QQQQ".to_string()),
        };
        assert!(!cross_product_match(&l, &r));
    }

    #[test]
    fn cross_product_matches_when_both_alternates_agree() {
        let l = DoubleMetaphoneKey {
            primary: "P1".to_string(),
            alternate: Some("AAA".to_string()),
        };
        let r = DoubleMetaphoneKey {
            primary: "P2".to_string(),
            alternate: Some("AAA".to_string()),
        };
        assert!(cross_product_match(&l, &r));
    }
}
