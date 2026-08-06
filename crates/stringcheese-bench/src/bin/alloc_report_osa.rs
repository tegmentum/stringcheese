//! Allocation-report binary for OSA (Optimal String Alignment).
//!
//! Same structure as the `alloc_report_levenshtein` sibling binary: one
//! execution per (variant, input-length, similarity-regime) triple, TSV
//! output to stdout, `Osa` in place of `Levenshtein`.
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
    Osa, OsaWorkspace,
    osa::{
        banded::distance_banded_with_workspace as osa_banded,
        full_matrix::distance_full_matrix as osa_full_matrix,
        rolling_rows::distance_rolling_rows_with_workspace as osa_rolling_rows,
    },
};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const LENGTHS: &[usize] = &[8, 32, 128, 512];

fn build_pair(len: usize, regime: &str) -> (Vec<u8>, Vec<u8>) {
    match regime {
        "random" => (random_ascii(len, 0x11), random_ascii(len, 0x12)),
        "similar" => similar_pair(len, 0.05, 0x13),
        "identical" => identical_pair(len, 0x14),
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

    for &len in LENGTHS {
        for regime in ["random", "similar", "identical"] {
            let (a, b) = build_pair(len, regime);

            let (_, m) = measure(|| Osa.distance(&a, &b));
            print_row("osa", "trait_call", len, regime, m);

            let mut ws_cold = OsaWorkspace::new();
            let (_, m) = measure(|| Osa.distance_with_workspace(&a, &b, &mut ws_cold));
            print_row("osa", "reused_workspace_first", len, regime, m);

            let mut ws_hot = OsaWorkspace::new();
            let _ = Osa.distance_with_workspace(&a, &b, &mut ws_hot);
            let (_, m) = measure(|| Osa.distance_with_workspace(&a, &b, &mut ws_hot));
            print_row("osa", "reused_workspace_hot", len, regime, m);

            let (_, m) = measure(|| osa_full_matrix(a.as_slice(), b.as_slice()));
            print_row("osa", "full_matrix", len, regime, m);

            let cutoff = u32::try_from(len).unwrap_or(u32::MAX);
            let mut ws_banded_p = OsaWorkspace::new();
            let _ = osa_banded(&a, &b, cutoff, &mut ws_banded_p);
            let (_, m) = measure(|| osa_banded(&a, &b, cutoff, &mut ws_banded_p));
            print_row("osa", "banded_permissive_hot", len, regime, m);

            let mut ws_banded_t = OsaWorkspace::new();
            let _ = osa_banded(&a, &b, 3u32, &mut ws_banded_t);
            let (_, m) = measure(|| osa_banded(&a, &b, 3u32, &mut ws_banded_t));
            print_row("osa", "banded_tight_k3_hot", len, regime, m);

            let mut ws_rr = OsaWorkspace::new();
            let _ = osa_rolling_rows(&a, &b, &mut ws_rr);
            let (_, m) = measure(|| osa_rolling_rows(&a, &b, &mut ws_rr));
            print_row("osa", "rolling_rows_hot", len, regime, m);
        }
    }
}
