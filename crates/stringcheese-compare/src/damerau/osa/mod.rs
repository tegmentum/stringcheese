//! Optimal String Alignment ("restricted Damerau-Levenshtein") kernels.
//!
//! OSA extends unit-cost Levenshtein with a single new operation — the
//! transposition of two adjacent symbols, cost `1` — subject to the
//! restriction that no substring may be edited more than once. Under that
//! restriction the DP recurrence gains a single new branch that reaches two
//! rows back and two columns back:
//!
//! ```text
//!   d[i][j] = min(
//!       d[i-1][j]   + 1,             // deletion
//!       d[i][j-1]   + 1,             // insertion
//!       d[i-1][j-1] + cost,          // substitution
//!       d[i-2][j-2] + 1              // transposition (only if
//!                                    //   a[i-1] == b[j-2] and
//!                                    //   a[i-2] == b[j-1])
//!   )
//! ```
//!
//! Because the transposition branch reaches to `d[i-2][j-2]`, the
//! rolling-row implementation needs three rows in scratch, not two —
//! otherwise the value the transposition branch would read has already been
//! overwritten. That is the only structural difference between OSA and the
//! ordinary Levenshtein rolling-rows kernel.
//!
//! # Not a metric
//!
//! OSA is *symmetric*, *non-negative*, and satisfies *identity of
//! indiscernibles* under `T: Eq`. It does **not** satisfy the triangle
//! inequality — the "no substring edited twice" restriction admits
//! configurations where the direct route between two strings is longer than
//! the sum of two intermediate routes. The classical distinguishing example
//! is `"ca"` vs `"abc"`, which OSA scores as `3` while the unrestricted
//! Damerau algorithm scores as `2`. See the crate's `property_tests`
//! module (test-only) for the triangle-violation test.

pub mod banded;
pub mod full_matrix;
pub mod rolling_rows;

/// Optional SIMD-accelerated byte-slice OSA backend. Compiled only under
/// `--features simd`; see the [`simd`] module docs for the dispatch
/// architecture and the documented unsafe-code exception the module
/// carries. Full unrestricted Damerau-Levenshtein (`crate::damerau::damerau`)
/// stays scalar — its HashMap-backed algorithm does not vectorize under
/// the Myers bit-parallel pattern.
#[cfg(feature = "simd")]
pub mod simd;

pub use banded::distance_banded_with_workspace;
pub use full_matrix::distance_full_matrix;
pub use rolling_rows::distance_rolling_rows_with_workspace;
