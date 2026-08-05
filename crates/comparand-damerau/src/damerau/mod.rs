//! Full (unrestricted) Damerau-Levenshtein kernels.
//!
//! The full Damerau-Levenshtein distance is the algorithm from Damerau's
//! 1964 paper: the same four operations as OSA (insertion, deletion,
//! substitution, adjacent transposition) but *without* the "no substring
//! edited twice" restriction. A character may be transposed and then later
//! be involved in another edit, which OSA disallows. The characteristic
//! example that distinguishes the two: `"ca"` vs `"abc"` scores `2` under
//! full Damerau (transpose "ca" → "ac", then insert "b" to obtain "abc")
//! and `3` under OSA (which cannot cross-edit the transposed pair).
//!
//! Lifting the OSA restriction requires the DP to look further back than
//! two rows: for each `(i, j)`, the transposition branch may reach a cell
//! at `(k-1, l-1)` where `k` is the row of the most recent occurrence of
//! `b[j-1]` in `a`, and `l` is the column of the most recent match of
//! `a[i-1]` in `b` within the current row. The Lowrance-Wagner 1975
//! formulation adds this in as one extra DP branch and preserves the
//! `O(m · n)` time bound *provided* the "last position of symbol" lookup
//! is `O(1)` — for which a hash table is the standard tool.
//!
//! # Kernels
//!
//! * [`full_matrix`] — the deliberately-simple oracle: linear scan through
//!   the previous rows of `a` to find the last occurrence of a symbol from
//!   `b`. `O(m² · n)` time. Requires only `T: Eq`. The linear scan is a
//!   genuine structural difference from the production kernel — the two
//!   compute the same recurrence but locate the transposition source
//!   through independent code paths, which is what makes the differential
//!   test between them meaningful.
//! * [`production`] — the production kernel: `HashMap<&T, usize>` for the
//!   auxiliary lookup, giving `O(m · n)` expected time. Requires
//!   `T: Eq + Hash`.
//!
//! # A true metric
//!
//! Full Damerau-Levenshtein under unit costs is a *metric* on
//! sequences: symmetric, non-negative, satisfies identity of indiscernibles
//! under `T: Eq`, and (unlike OSA) satisfies the triangle inequality. The
//! crate's [`property_tests`](crate::property_tests) exercise all four
//! axioms on generated inputs.

pub mod full_matrix;
#[cfg(feature = "std")]
pub mod production;

pub use full_matrix::distance_full_matrix;
#[cfg(feature = "std")]
pub use production::distance_production_with_workspace;
