//! Natural-order collation — numeric runs compare by their
//! numeric value, not lexicographically.
//!
//! `NaturalCollator` wraps any inner [`Collator`] and interleaves
//! numeric-run comparison. `"file2"` sorts before `"file10"` (not
//! after as plain lexicographic ordering would say), while
//! non-numeric spans defer to the inner collator's rules.

use core::cmp::Ordering;

use crate::Collator;

/// Numeric-run-aware wrapper around any [`Collator`].
///
/// The natural-sort predicate: partition each string into a
/// sequence of maximal runs where each run is either "all ASCII
/// digits" or "no ASCII digits", zip the runs, and compare each
/// pair — digit-runs compare by leading-zero-stripped numeric
/// value, non-digit-runs by the inner collator.
pub struct NaturalCollator<C: Collator> {
    inner: C,
}

impl<C: Collator> NaturalCollator<C> {
    /// Construct with an inner collator (e.g.
    /// [`crate::UcaCollator`] for Unicode-aware non-numeric runs,
    /// or [`crate::AsciiCiCollator`] for the pure-ASCII fast path).
    #[must_use]
    pub fn new(inner: C) -> Self {
        Self { inner }
    }

    /// Access the wrapped collator.
    #[must_use]
    pub fn inner(&self) -> &C {
        &self.inner
    }
}

impl<C: Collator> Collator for NaturalCollator<C> {
    fn compare(&self, a: &str, b: &str) -> Ordering {
        // Bench-driven cleanup (2026-08-09): the earlier
        // implementation created two unused `peekable()`
        // iterators every call to sidestep an unused-var lint on
        // a stale variable. Dropping that + classifying each run
        // once (rather than re-deriving digit-ness after the
        // scan) reclaims measurable per-compare time on hot
        // sort_by loops.
        let ab = a.as_bytes();
        let bb = b.as_bytes();
        let mut a_pos = 0usize;
        let mut b_pos = 0usize;
        loop {
            match (a_pos == ab.len(), b_pos == bb.len()) {
                (true, true) => return Ordering::Equal,
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                (false, false) => {}
            }
            let a_is_digit = ab[a_pos].is_ascii_digit();
            let b_is_digit = bb[b_pos].is_ascii_digit();
            let a_end = a_pos + run_len(ab, a_pos, a_is_digit);
            let b_end = b_pos + run_len(bb, b_pos, b_is_digit);
            let a_run = &a[a_pos..a_end];
            let b_run = &b[b_pos..b_end];
            let cmp = if a_is_digit && b_is_digit {
                compare_numeric(a_run, b_run)
            } else {
                // Non-digit runs, and mixed-kind runs (one digit +
                // one non-digit at the same position), both defer
                // to the inner collator's rules — UCA puts digits
                // before letters, which is what a natural sort
                // caller expects at a mixed-run boundary.
                self.inner.compare(a_run, b_run)
            };
            if cmp != Ordering::Equal {
                return cmp;
            }
            a_pos = a_end;
            b_pos = b_end;
        }
    }
}

/// Length of the maximal run starting at `from` whose bytes all
/// share the same digit-vs-non-digit polarity as the pre-classified
/// `is_digit`. Non-digit runs may contain arbitrary UTF-8
/// continuation bytes; `is_ascii_digit` returns false for every
/// continuation byte too, so the run stays contiguous.
fn run_len(bytes: &[u8], from: usize, is_digit: bool) -> usize {
    let mut i = from;
    while i < bytes.len() && bytes[i].is_ascii_digit() == is_digit {
        i += 1;
    }
    i - from
}

fn compare_numeric(a: &str, b: &str) -> Ordering {
    // Strip leading zeros for the value comparison; length after
    // stripping gives the magnitude tie-breaker for
    // arbitrarily-large numbers (no u64 overflow risk).
    let a_stripped = a.trim_start_matches('0');
    let b_stripped = b.trim_start_matches('0');
    // Empty means the whole run was zeros; treat as "0".
    let (a_stripped, b_stripped) = match (a_stripped, b_stripped) {
        ("", "") => return a.len().cmp(&b.len()),
        ("", _) => return Ordering::Less,
        (_, "") => return Ordering::Greater,
        pair => pair,
    };
    match a_stripped.len().cmp(&b_stripped.len()) {
        Ordering::Equal => {
            // Same-length — lexicographic ASCII compare gives the
            // numeric ordering because both are digit strings.
            match a_stripped.cmp(b_stripped) {
                Ordering::Equal => {
                    // Same magnitude, tie-break on leading-zero count
                    // (shorter representation first) so `007` sorts
                    // before `7` — stable rendering-preserving order.
                    a.len().cmp(&b.len())
                }
                other => other,
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UcaCollator;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn file10_sorts_after_file2() {
        let c = NaturalCollator::new(UcaCollator::new());
        assert_eq!(c.compare("file2", "file10"), Ordering::Less);
        assert_eq!(c.compare("file10", "file2"), Ordering::Greater);
    }

    #[test]
    fn plain_lexicographic_would_say_the_opposite() {
        // Sanity: without natural-order, "file10" < "file2".
        assert!("file10" < "file2");
    }

    #[test]
    fn sort_by_produces_natural_order() {
        let c = NaturalCollator::new(UcaCollator::new());
        let mut xs = vec!["file10", "file2", "file1", "file20", "file3"];
        xs.sort_by(|a, b| c.compare(a, b));
        assert_eq!(xs, vec!["file1", "file2", "file3", "file10", "file20"]);
    }

    #[test]
    fn embedded_numbers_at_multiple_positions() {
        let c = NaturalCollator::new(UcaCollator::new());
        let mut xs: Vec<&str> = vec!["ch1v10", "ch1v2", "ch10v1", "ch2v1"];
        xs.sort_by(|a, b| c.compare(a, b));
        assert_eq!(xs, vec!["ch1v2", "ch1v10", "ch2v1", "ch10v1"]);
    }

    #[test]
    fn plain_strings_defer_to_inner_collator() {
        let c = NaturalCollator::new(UcaCollator::new());
        // No digits — same as bare UCA ordering.
        assert_eq!(c.compare("apple", "banana"), Ordering::Less);
    }

    #[test]
    fn leading_zeros_do_not_change_value_order() {
        let c = NaturalCollator::new(UcaCollator::new());
        // 7 vs 007 — same numeric value, so should compare Equal
        // at the value level and only differ on representation
        // length (007 sorts second here by convention).
        assert_eq!(c.compare("v7", "v007"), Ordering::Less);
    }

    #[test]
    fn all_zeros_run_treated_as_zero() {
        let c = NaturalCollator::new(UcaCollator::new());
        assert_eq!(c.compare("v0", "v1"), Ordering::Less);
        assert_eq!(c.compare("v00", "v0"), Ordering::Greater);
    }
}
