//! Allocation-report binary for the full (unrestricted) Damerau-Levenshtein
//! distance.
//!
//! Reports the production `HashMap`-backed kernel — the full-matrix
//! Damerau *oracle* is O(m² · n) time and would take tens of seconds per
//! iteration at n ≥ 512, so we cap its variant at n ≤ 256 and let the
//! production kernel run at the full sweep.
//!
//! The production kernel maintains a `HashMap` of "last position for
//! symbol". Its per-call allocation cost therefore depends on how many
//! *distinct symbols* appear in the shorter input, not just on the input's
//! length in bytes — a fact the reused-workspace path amortizes across
//! calls when the workspace's map already has entries from a previous
//! comparison. This binary reports the cold-workspace and hot-workspace
//! variants so that dependency is visible in the data.
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
use stringcheese_damerau::{
    Damerau, DamerauWorkspace,
    damerau::{
        full_matrix::distance_full_matrix as damerau_full_matrix,
        production::distance_production_with_workspace as damerau_production,
    },
};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const LENGTHS: &[usize] = &[8, 32, 128, 512];
/// The full-matrix Damerau oracle is O(m² · n) in time (and O(m · n) in
/// space); cap it at inputs where a single call stays under a second on a
/// modern desktop.
const LENGTHS_ORACLE: &[usize] = &[8, 32, 128];

fn build_pair(len: usize, regime: &str) -> (Vec<u8>, Vec<u8>) {
    match regime {
        "random" => (random_ascii(len, 0x21), random_ascii(len, 0x22)),
        "similar" => similar_pair(len, 0.05, 0x23),
        "identical" => identical_pair(len, 0x24),
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
    let _profiler = dhat::Profiler::builder().testing().build();

    println!("algorithm\tvariant\tlen\tregime\tblocks\tbytes\tmax_blocks\tmax_bytes");

    // Full-Damerau production kernel (`HashMap`-backed) at the full sweep.
    for &len in LENGTHS {
        for regime in ["random", "similar", "identical"] {
            let (a, b) = build_pair(len, regime);

            let (_, m) = measure(|| Damerau.distance(&a, &b));
            print_row("damerau", "trait_call", len, regime, m);

            let mut ws_cold: DamerauWorkspace<u8> = DamerauWorkspace::new();
            let (_, m) = measure(|| Damerau.distance_with_workspace(&a, &b, &mut ws_cold));
            print_row("damerau", "reused_workspace_first", len, regime, m);

            let mut ws_hot: DamerauWorkspace<u8> = DamerauWorkspace::new();
            let _ = Damerau.distance_with_workspace(&a, &b, &mut ws_hot);
            let (_, m) = measure(|| Damerau.distance_with_workspace(&a, &b, &mut ws_hot));
            print_row("damerau", "reused_workspace_hot", len, regime, m);

            let mut ws_prod: DamerauWorkspace<u8> = DamerauWorkspace::new();
            let _ = damerau_production(&a, &b, &mut ws_prod);
            let (_, m) = measure(|| damerau_production(&a, &b, &mut ws_prod));
            print_row("damerau", "production_hot", len, regime, m);
        }
    }

    // Full-Damerau *oracle* (Lowrance-Wagner full matrix) capped at n <= 128.
    for &len in LENGTHS_ORACLE {
        for regime in ["random", "similar", "identical"] {
            let (a, b) = build_pair(len, regime);
            let (_, m) = measure(|| damerau_full_matrix(a.as_slice(), b.as_slice()));
            print_row("damerau", "full_matrix", len, regime, m);
        }
    }
}
