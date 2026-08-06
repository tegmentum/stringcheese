//! Allocation-report binary for Levenshtein.
//!
//! Prints a TSV table of heap-allocation counts and bytes for every
//! (variant, input-length, similarity-regime) combination. Each cell is a
//! single execution of the algorithm — no repeats — so the reported numbers
//! are the exact allocation cost of one call.
//!
//! # Variants
//!
//! * `trait_call` — `Levenshtein.distance(a, b)`. Fresh workspace per call,
//!   the "user just wants an answer" entry point. This is the baseline the
//!   design doc's workspace-reuse story is measured against.
//! * `reused_workspace_first` — `Levenshtein.distance_with_workspace(a, b,
//!   &mut ws)` on a workspace that has *not* seen this length before. Every
//!   call grows the workspace's `Vec<u32>` at least once.
//! * `reused_workspace_hot` — same, but on a workspace previously grown to
//!   fit this length. This should report zero allocations for lengths at or
//!   below the workspace's high-water mark — that's the whole point of
//!   caller-owned scratch space.
//! * `full_matrix` — `distance_full_matrix(a, b)`. Allocates an m×n
//!   `Vec<u32>` on every call; reported so the O(m·n) space cost is
//!   quantified in bytes as well as in asymptotic notation.
//! * `banded_permissive` and `banded_tight_k3` — the banded kernel at two
//!   cutoffs, both workspace-reused-hot. Their `total_bytes` should match
//!   `reused_workspace_hot` at 0 once the workspace is warm.
#![allow(
    missing_docs,
    reason = "binary entry points do not need item-level docs beyond the file-level module doc above"
)]
#![allow(
    clippy::cast_possible_truncation,
    reason = "input lengths and cutoffs are small compile-time constants that fit in every integer width this binary uses"
)]

use stringcheese_bench::alloc_harness::{AllocMeasurement, measure};
use stringcheese_bench::inputs::{identical_pair, random_ascii, similar_pair};
use stringcheese_core::DistanceMetric;
use stringcheese_levenshtein::{
    Levenshtein, LevenshteinWorkspace, distance_banded_with_workspace, distance_full_matrix,
    distance_rolling_rows_with_workspace,
};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const LENGTHS: &[usize] = &[8, 32, 128, 512];

fn build_pair(len: usize, regime: &str) -> (Vec<u8>, Vec<u8>) {
    match regime {
        "random" => (random_ascii(len, 0x01), random_ascii(len, 0x02)),
        "similar" => similar_pair(len, 0.05, 0x03),
        "identical" => identical_pair(len, 0x04),
        _ => unreachable!("unknown similarity regime"),
    }
}

fn print_row(algorithm: &str, variant: &str, len: usize, regime: &str, m: AllocMeasurement) {
    println!(
        "{algorithm}\t{variant}\t{len}\t{regime}\t{blocks}\t{bytes}\t{max_blocks}\t{max_bytes}",
        blocks = m.total_blocks,
        bytes = m.total_bytes,
        max_blocks = m.max_blocks,
        max_bytes = m.max_bytes,
    );
}

fn main() {
    // The profiler guard must outlive every `HeapStats::get()` call — bind
    // it to `_profiler` (not `_`) so its `Drop` runs at the end of `main`
    // and not immediately.
    let _profiler = dhat::Profiler::builder().testing().build();

    println!("algorithm\tvariant\tlen\tregime\tblocks\tbytes\tmax_blocks\tmax_bytes");

    for &len in LENGTHS {
        for regime in ["random", "similar", "identical"] {
            let (a, b) = build_pair(len, regime);

            // Trait entry point: fresh workspace per call.
            let (_, m) = measure(|| Levenshtein.distance(&a, &b));
            print_row("levenshtein", "trait_call", len, regime, m);

            // Cold workspace: allocated but not sized for this input yet.
            let mut ws_cold = LevenshteinWorkspace::new();
            let (_, m) = measure(|| Levenshtein.distance_with_workspace(&a, &b, &mut ws_cold));
            print_row("levenshtein", "reused_workspace_first", len, regime, m);

            // Hot workspace: previously grown to fit this input.
            let mut ws_hot = LevenshteinWorkspace::new();
            // Warm-up call outside `measure` so its allocations don't count.
            let _ = Levenshtein.distance_with_workspace(&a, &b, &mut ws_hot);
            let (_, m) = measure(|| Levenshtein.distance_with_workspace(&a, &b, &mut ws_hot));
            print_row("levenshtein", "reused_workspace_hot", len, regime, m);

            // Full-matrix oracle: allocates m*n `u32` per call, unconditional.
            let (_, m) = measure(|| distance_full_matrix(a.as_slice(), b.as_slice()));
            print_row("levenshtein", "full_matrix", len, regime, m);

            // Banded, permissive cutoff, hot workspace.
            let cutoff = u32::try_from(len).unwrap_or(u32::MAX);
            let mut ws_banded_p = LevenshteinWorkspace::new();
            let _ = distance_banded_with_workspace(&a, &b, cutoff, &mut ws_banded_p);
            let (_, m) =
                measure(|| distance_banded_with_workspace(&a, &b, cutoff, &mut ws_banded_p));
            print_row("levenshtein", "banded_permissive_hot", len, regime, m);

            // Banded, tight cutoff (k=3), hot workspace.
            let mut ws_banded_t = LevenshteinWorkspace::new();
            let _ = distance_banded_with_workspace(&a, &b, 3u32, &mut ws_banded_t);
            let (_, m) = measure(|| distance_banded_with_workspace(&a, &b, 3u32, &mut ws_banded_t));
            print_row("levenshtein", "banded_tight_k3_hot", len, regime, m);

            // Rolling-rows kernel called directly, hot workspace — the same
            // code path `trait_call` runs, minus the per-call workspace
            // allocation. The delta between this row and `trait_call` is
            // exactly the cost of `LevenshteinWorkspace::new()` plus the
            // first `resize`.
            let mut ws_rr = LevenshteinWorkspace::new();
            let _ = distance_rolling_rows_with_workspace(&a, &b, &mut ws_rr);
            let (_, m) = measure(|| distance_rolling_rows_with_workspace(&a, &b, &mut ws_rr));
            print_row("levenshtein", "rolling_rows_hot", len, regime, m);
        }
    }
}
