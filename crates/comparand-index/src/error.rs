//! Error type returned by fallible index constructors that require a metric.
//!
//! Both [`BkTree::try_new`] and [`VpTree::try_new`] reject algorithms whose
//! [`MetricProperties`] do not satisfy the definition of a true metric.
//! Callers that hit this case usually want to select a different algorithm
//! or restructure their pipeline, not panic — hence the separate fallible
//! entry points.
//!
//! [`BkTree::try_new`]: crate::bk_tree::BkTree::try_new
//! [`VpTree::try_new`]: crate::vp_tree::VpTree::try_new

use comparand_core::MetricProperties;
use core::fmt;

/// Returned by [`BkTree::try_new`] and [`VpTree::try_new`] when the supplied
/// algorithm does not satisfy the definition of a true metric.
///
/// Carries the observed [`MetricProperties`] so downstream diagnostic output
/// can name *which* axiom is missing rather than just reporting a boolean
/// failure. The most common reason in practice is a violated triangle
/// inequality (semimetrics such as OSA), which is exactly the axiom the
/// tree's pruning depends on.
///
/// [`BkTree::try_new`]: crate::bk_tree::BkTree::try_new
/// [`VpTree::try_new`]: crate::vp_tree::VpTree::try_new
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotAMetricError {
    /// The properties the rejected algorithm reported.
    pub observed: MetricProperties,
}

impl NotAMetricError {
    /// Constructs the error from the observed properties.
    #[inline]
    #[must_use]
    pub const fn new(observed: MetricProperties) -> Self {
        Self { observed }
    }
}

impl fmt::Display for NotAMetricError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Enumerate the axioms in the same order they appear on
        // `MetricProperties`. Callers get a concrete list of missing axioms
        // rather than a generic "not a metric" string.
        f.write_str("supplied algorithm is not a true metric: missing")?;
        let mut missing: [&str; 4] = ["", "", "", ""];
        let mut len = 0;
        if !self.observed.symmetric {
            missing[len] = "symmetric";
            len += 1;
        }
        if !self.observed.identity_of_indiscernibles {
            missing[len] = "identity-of-indiscernibles";
            len += 1;
        }
        if !self.observed.triangle_inequality {
            missing[len] = "triangle-inequality";
            len += 1;
        }
        if !self.observed.non_negative {
            missing[len] = "non-negative";
            len += 1;
        }
        if len == 0 {
            f.write_str(" (no axiom flagged — inconsistent report)")?;
        } else {
            f.write_str(" ")?;
            for (i, axiom) in missing[..len].iter().enumerate() {
                if i > 0 {
                    f.write_str(", ")?;
                }
                f.write_str(axiom)?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NotAMetricError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semimetric_reports_missing_triangle() {
        let err = NotAMetricError::new(MetricProperties::SEMIMETRIC);
        let text = alloc::format!("{err}");
        assert!(text.contains("triangle-inequality"), "was: {text}");
    }

    #[test]
    fn quasimetric_reports_missing_symmetry() {
        let err = NotAMetricError::new(MetricProperties::QUASIMETRIC);
        let text = alloc::format!("{err}");
        assert!(text.contains("symmetric"), "was: {text}");
    }
}
