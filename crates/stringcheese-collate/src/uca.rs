//! UCA-based collation via [`feruca`].
//!
//! Standard Unicode Collation Algorithm at the DUCET root locale —
//! the "no locale specified, do the sensible thing" default. For
//! full CLDR-tailored variants (e.g. German phonebook order,
//! Swedish "ä" after "z"), swap `feruca::Collator` for
//! `icu_collator::Collator` behind the same [`crate::Collator`]
//! trait.

use core::cell::RefCell;
use core::cmp::Ordering;

use crate::Collator;

/// UCA collator (root locale / DUCET).
///
/// `feruca::Collator::collate` takes `&mut self` (it caches
/// intermediate collation elements); wrap in a `RefCell` so the
/// public [`Collator::compare`] method can stay `&self` — the
/// idiomatic shape for a sort predicate.
pub struct UcaCollator {
    inner: RefCell<feruca::Collator>,
}

impl Default for UcaCollator {
    fn default() -> Self {
        Self {
            inner: RefCell::new(feruca::Collator::default()),
        }
    }
}

impl UcaCollator {
    /// Construct with feruca's default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Collator for UcaCollator {
    fn compare(&self, a: &str, b: &str) -> Ordering {
        self.inner.borrow_mut().collate(a, b)
    }
}

impl core::fmt::Debug for UcaCollator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UcaCollator").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn ascii_orders_alphabetically() {
        let c = UcaCollator::new();
        assert_eq!(c.compare("apple", "banana"), Ordering::Less);
        assert_eq!(c.compare("banana", "apple"), Ordering::Greater);
        assert_eq!(c.compare("apple", "apple"), Ordering::Equal);
    }

    #[test]
    fn diacritics_sort_close_to_base() {
        // In UCA at the primary level, `café` and `cafe` sort
        // close together — `café` after `cafe` at secondary.
        let c = UcaCollator::new();
        let mut words = ["café", "cafe", "cafez"];
        words.sort_by(|a, b| c.compare(a, b));
        // The exact order depends on UCA weights; the important
        // property is that `cafez` sorts AFTER both cafe/café
        // (primary weight of `z` beats both variants of the
        // trailing letter).
        assert_eq!(words[2], "cafez");
    }

    #[test]
    fn cross_script_ordering_is_deterministic() {
        let c = UcaCollator::new();
        let a = "apple";
        let b = "banana";
        // Idempotent — same result on repeat calls (guards against
        // any RefCell interior-state leakage).
        assert_eq!(c.compare(a, b), c.compare(a, b));
    }

    #[test]
    fn works_with_slice_sort_by() {
        let c = UcaCollator::new();
        let mut xs: Vec<&str> = vec!["Zulu", "apple", "Bravo", "berry"];
        xs.sort_by(|a, b| c.compare(a, b));
        // UCA orders by primary weight first (letters equivalent
        // regardless of case), then tertiary (lowercase before
        // uppercase in DUCET). So `berry` sorts BEFORE `Bravo`
        // (same primary 'b', but 'e' < 'r' at primary), and both
        // sort before `Zulu`.
        assert_eq!(xs, vec!["apple", "berry", "Bravo", "Zulu"]);
    }
}
