//! Global (Needleman-Wunsch) and local (Smith-Waterman) sequence alignment.
//!
//! This crate is the v0.2 StringCheese alignment subsystem. Unlike an edit-
//! distance kernel, an alignment algorithm produces a *score* plus an
//! *edit script* — a step-by-step account of how each symbol of the two
//! inputs was paired (matched, substituted, or aligned to a gap). A
//! downstream consumer can use the script to visualize the alignment,
//! render a diff, or reconstruct one sequence from the other.
//!
//! # Algorithms
//!
//! * [`NeedlemanWunsch`] — global alignment. Every symbol of both inputs
//!   appears in the resulting alignment, aligned to a symbol or to a gap.
//! * [`SmithWaterman`] — local alignment. Returns the highest-scoring pair
//!   of substrings and the alignment between them; low-scoring regions of
//!   both inputs are discarded (their scores are floored to zero during the
//!   DP).
//!
//! Both algorithms share the same scoring vocabulary and can be
//! parameterized by any [`ScoringScheme`]. Two concrete schemes ship with
//! the crate:
//!
//! * [`LinearGap`] — every gap symbol costs the same amount. Total
//!   `k`-symbol gap cost is `k * gap_penalty`.
//! * [`AffineGap`] — opening a gap costs `gap_open`; each additional symbol
//!   in the same gap costs `gap_extend`. Total `k`-symbol gap cost is
//!   `gap_open + (k - 1) * gap_extend`. When the scheme is affine
//!   (`gap_open != gap_extend`) both aligners select a three-matrix
//!   Gotoh 1982 DP; otherwise they use a simpler single-matrix DP.
//!
//! # Reconstructed alignments
//!
//! [`Alignment`] holds the score, the ordered [`EditOp`] script, and — for
//! local alignment — the start indices of the aligned substrings in the
//! two inputs. Utility methods on [`Alignment`] rebuild the aligned
//! substrings ([`Alignment::extract_a`], [`Alignment::extract_b`]) or apply
//! the script to a witness ([`Alignment::apply_to`]).
//!
//! # Sequence type
//!
//! All alignment operations are generic over `&[T]` where `T: Eq` (and
//! `T: Clone` for edit-script reconstruction). Representation is chosen at
//! the call site — bytes, `char`s, `u32` code points, tokens, or any
//! `Eq + Clone` type.
//!
//! # Score type
//!
//! Alignment scores are reported as [`Score<i32>`](stringcheese_core::Score).
//! `i32` was chosen instead of a generic numeric type because every scoring
//! scheme currently expresses match, mismatch and gap costs as small
//! integers, and `i32` gives ample headroom (a `10 000 x 10 000` alignment
//! with `match_score = i16::MAX` still fits). See the "Design choices"
//! section of the crate's README for the trade-off.
//!
//! # `no_std`
//!
//! `no_std`-compatible; the `alloc` feature gates the DP scratch buffers,
//! the edit-script [`Vec`], and both alignment algorithms
//! themselves (which allocate their DP matrices).
//!
//! # Metric class
//!
//! Alignment scores are not distances: they can be positive or negative and
//! do not satisfy the triangle inequality without further transformation.
//! Both algorithms therefore report
//! [`MetricClass::Score`](stringcheese_core::MetricClass::Score) and deliberately
//! implement *neither* [`DistanceMetric`](stringcheese_core::DistanceMetric)
//! *nor* [`SimilarityMetric`](stringcheese_core::SimilarityMetric) — the
//! entry points are inherent methods only.
//!
//! # What's not here
//!
//! * SIMD backends — out of scope for v0.2.
//! * Multiple sequence alignment (MSA) — v0.3+.
//! * Profile-based alignment (PSSMs, HMMs) — v0.3+.
//! * Banded alignment for constrained-similarity inputs — future work.
//!
//! # Benchmarks
//!
//! A criterion bench harness lives at `benches/align.rs`; run it with
//!
//! ```text
//! cargo bench -p stringcheese-align
//! ```
//!
//! Six groups pair every (algorithm, gap-model, output) combination:
//! `nw_score_linear`, `nw_score_affine`, `nw_align_linear`,
//! `sw_score_linear`, `sw_score_affine`, `sw_align_linear`. Each group
//! runs three sizes (32×32, 128×128, 512×512 byte sequences) crossed
//! with two flavors — `matching` (mostly-shared prefix with a ~5 % edit
//! rate; the shape Smith-Waterman's local-alignment optimisation is
//! designed for) and `divergent` (independent random `[a-z]` on both
//! operands; DP worst case).
//!
//! # Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 3 --sample-size 15`). Wall-clock
//! samples vary ±10–15 % on a laptop under load; treat the ratios as
//! informative and the absolutes as illustrative. Throughput reported
//! over one operand's byte length — the true DP work is O(m·n), so a
//! "3 MiB/s" number at n = 128 corresponds to roughly `128² / 40µs` cell
//! updates. Higher is better.
//!
//! ```text
//! group                                32×32           128×128         512×512
//! --------------------------------------------------------------------------------
//! nw_score_linear     / matching     17.6 MiB/s      3.46 MiB/s        783 KiB/s
//! nw_score_linear     / divergent    17.5 MiB/s      3.45 MiB/s        830 KiB/s
//! nw_score_affine     / matching     12.2 MiB/s      2.60 MiB/s        749 KiB/s
//! nw_score_affine     / divergent    12.3 MiB/s      2.56 MiB/s        733 KiB/s
//! nw_align_linear     / matching     10.7 MiB/s      3.16 MiB/s        772 KiB/s
//! nw_align_linear     / divergent    11.0 MiB/s      3.11 MiB/s        795 KiB/s
//! sw_score_linear     / matching     13.0 MiB/s      2.65 MiB/s        592 KiB/s
//! sw_score_linear     / divergent    13.0 MiB/s      2.65 MiB/s        628 KiB/s
//! sw_score_affine     / matching      9.2 MiB/s      2.34 MiB/s        248 KiB/s
//! sw_score_affine     / divergent    10.4 MiB/s      1.83 MiB/s        230 KiB/s
//! sw_align_linear     / matching     11.2 MiB/s      2.54 MiB/s        636 KiB/s
//! sw_align_linear     / divergent    11.9 MiB/s      2.53 MiB/s        641 KiB/s
//! ```
//!
//! Read:
//!
//! * **Affine vs linear (score-only)**: the Gotoh three-matrix DP costs
//!   ~30–40 % more per cell than the single-matrix linear-gap DP at every
//!   size. The ratio widens on Smith-Waterman at 512² because the affine
//!   kernel does not benefit from the running-max optimisation as
//!   uniformly.
//! * **Align vs score (linear-gap)**: adding the edit-script backtrace
//!   costs the full DP matrix retention plus an O(m + n) walk from
//!   `(m, n)` to `(0, 0)`. On small inputs the fixed cost dominates
//!   (32² is ~1.6× the score-only kernel); at 512² the extra cost is
//!   amortised into the matrix fill and the two rows converge to within
//!   ~5 %.
//! * **NW vs SW at the same size** is broadly a wash — both aligners do
//!   O(m·n) work, and the running-max step Smith-Waterman adds per cell
//!   costs about what the boundary-fill Needleman-Wunsch pays outside
//!   the inner loop.
//! * **`matching` vs `divergent`** shows almost no signal on this
//!   corpus — 5 % edit-rate is not enough for the running-max short-
//!   circuit to skip meaningful work, and both aligners fill the whole
//!   grid regardless. A future divergence-heavy corpus (e.g. Zipfian
//!   mixed-symbol runs) is the natural next axis if the SW fast-path
//!   becomes a hotspot.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15 %. A number outside that
//!   band on a subsequent run is either a genuine regression or a
//!   measurement environment change (thermal throttling, background
//!   load); rerun with `--sample-size 30` and a quiet host before
//!   filing a fix.
//!
//! # Oracle gap vs best-in-class Rust
//!
//! With `--features oracle-benches` the bench harness adds a
//! side-by-side lane for each kernel that pits stringcheese against
//! `bio` (rust-bio v2), the canonical Rust bioinformatics crate. Bio
//! uses Gotoh three-matrix affine-gap DP always, and its
//! `Aligner::global` / `Aligner::local` returns a full `Alignment`
//! (score + traceback) on every call — no score-only fast path — so the
//! "score" comparison also captures bio's per-call trace allocation
//! cost. That is the real-world cost of picking bio for a score-only
//! pipeline. Multiplier is bio-thrpt ÷ stringcheese-thrpt at the same
//! size and flavor. Full table in `docs/perf/align-oracle-gaps.md`.
//!
//! ```text
//! kernel            / size × flavor     stringcheese   bio           mult    verdict
//! -----------------------------------------------------------------------------------
//! nw_score_linear   / 32   × div         17.5 MiB/s     3.9 MiB/s   0.22×   stringcheese 4.5× ahead
//! nw_score_linear   / 128  × div          3.3 MiB/s   941  KiB/s    0.28×   stringcheese 3.6× ahead
//! nw_score_linear   / 512  × div        644   KiB/s   177  KiB/s    0.27×   stringcheese 3.6× ahead
//! nw_score_affine   / 32   × div         11.0 MiB/s     3.8 MiB/s   0.35×   stringcheese 2.9× ahead
//! nw_score_affine   / 128  × div          1.8 MiB/s   913  KiB/s    0.51×   stringcheese 2.0× ahead
//! nw_score_affine   / 512  × div        680   KiB/s   208  KiB/s    0.31×   stringcheese 3.3× ahead
//! nw_align_linear   / 32   × div         13.8 MiB/s     4.0 MiB/s   0.29×   stringcheese 3.5× ahead
//! nw_align_linear   / 512  × div        811   KiB/s   206  KiB/s    0.25×   stringcheese 3.9× ahead
//! sw_score_linear   / 512  × div        593   KiB/s   270  KiB/s    0.45×   stringcheese 2.2× ahead
//! sw_score_affine   / 512  × div        744   KiB/s   270  KiB/s    0.36×   stringcheese 2.8× ahead
//! sw_align_linear   / 512  × div        641   KiB/s   267  KiB/s    0.42×   stringcheese 2.4× ahead
//! ```
//!
//! Read:
//!
//! * **Every combination**: stringcheese is 2-5× faster than bio at
//!   every size. The gap widens for score-only kernels (bio always
//!   allocates the trace) and stays stable for align kernels.
//! * **Linear-gap advantage**: stringcheese's specialised single-matrix
//!   linear-gap DP shows the widest gap (~3.6-4.5× on divergent 32-512)
//!   because bio uses three-matrix Gotoh even for degenerate linear
//!   `gap_open == gap_extend` input. That is a design choice on bio's
//!   side, not an inefficiency in stringcheese, but it is exactly the
//!   real-world cost of picking bio for a linear-gap pipeline.
//! * **Verdict**: `stringcheese-align` is already best-in-class among
//!   pure-Rust exact NW/SW libraries. Any further headroom is a SIMD-
//!   ceiling story (parasail C library territory), not scalar-DP
//!   headroom. No scalar perf work needed.
//! * **Dep-footprint red flag**: enabling `oracle-benches` pulls
//!   `bio` v2 (`~117 transitive packages`: ndarray, statrs, petgraph,
//!   csv, regex, rand, ...) into the workspace `Cargo.lock`. This is
//!   the entire reason the feature is off by default. `--all-features`
//!   workspace jobs pay a `~15-30 s` `bio` compile hit.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates, reason = "consumed by submodules")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod edit_script;
#[cfg(feature = "alloc")]
pub mod needleman_wunsch;
pub mod scoring;
#[cfg(feature = "alloc")]
pub mod smith_waterman;
#[cfg(feature = "alloc")]
pub mod workspace;

#[cfg(all(test, feature = "alloc"))]
mod golden;
#[cfg(all(test, feature = "alloc"))]
#[cfg(not(target_family = "wasm"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use edit_script::{Alignment, EditOp};
#[cfg(feature = "alloc")]
pub use needleman_wunsch::NeedlemanWunsch;
pub use scoring::{AffineGap, LinearGap, ScoringScheme};
#[cfg(feature = "alloc")]
pub use smith_waterman::SmithWaterman;
#[cfg(feature = "alloc")]
pub use workspace::AlignmentWorkspace;
