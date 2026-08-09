//! ASCII case-insensitive collator — the fast path for known-ASCII
//! keys (identifiers, machine tokens, log lines).
//!
//! Byte-by-byte compare with `to_ascii_lowercase` on each byte;
//! doesn't allocate. For pure-ASCII inputs the ordering matches
//! what UCA would produce at the primary weight, orders of
//! magnitude cheaper.

use core::cmp::Ordering;

use crate::Collator;

/// ASCII case-insensitive collator.
#[derive(Copy, Clone, Debug, Default)]
pub struct AsciiCiCollator;

impl AsciiCiCollator {
    /// Construct.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Collator for AsciiCiCollator {
    fn compare(&self, a: &str, b: &str) -> Ordering {
        let ab = a.as_bytes();
        let bb = b.as_bytes();
        let n = ab.len().min(bb.len());
        for i in 0..n {
            let lhs = ab[i].to_ascii_lowercase();
            let rhs = bb[i].to_ascii_lowercase();
            let cmp = lhs.cmp(&rhs);
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        ab.len().cmp(&bb.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn case_insensitive_ordering() {
        let c = AsciiCiCollator::new();
        assert_eq!(c.compare("Apple", "banana"), Ordering::Less);
        assert_eq!(c.compare("APPLE", "apple"), Ordering::Equal);
    }

    #[test]
    fn shorter_prefix_is_less() {
        let c = AsciiCiCollator::new();
        assert_eq!(c.compare("app", "apple"), Ordering::Less);
    }

    #[test]
    fn sort_by_produces_case_insensitive_order() {
        let c = AsciiCiCollator::new();
        let mut xs: Vec<&str> = vec!["Zulu", "apple", "Bravo", "berry"];
        xs.sort_by(|a, b| c.compare(a, b));
        // Byte-lowercase compare — "berry" and "Bravo" both start
        // with 'b'; second-byte compare gives 'e' < 'r', so
        // "berry" sorts before "Bravo".
        assert_eq!(xs, vec!["apple", "berry", "Bravo", "Zulu"]);
    }
}
