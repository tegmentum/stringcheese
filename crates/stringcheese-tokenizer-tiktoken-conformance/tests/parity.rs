//! End-to-end parity test: run the harness against the shipped
//! corpus for every variant and print a report.
//!
//! Only executes with `--features parity-real-vocab`. In the default
//! configuration this file compiles to an empty test binary.
//!
//! The test **does not fail** on divergences by default — it prints
//! them and marks the test as "informational" via a captured
//! summary. This matches the Phase 3 posture in the design doc: the
//! harness's job is to *surface* the divergences so a follow-up
//! phase can drive them to zero. Set the env var
//! `TIKTOKEN_PARITY_STRICT=1` to promote any divergence to a hard
//! failure — CI does this on the dedicated parity job so a
//! regression against a previously-clean variant surfaces loudly.

#![cfg(feature = "parity-real-vocab")]

use std::env;

use stringcheese_tokenizer_tiktoken_conformance::{corpus, parity, variant};

#[test]
fn cl100k_base_parity_over_shipped_corpus() {
    run_variant(&variant::CL100K_BASE);
}

#[test]
fn o200k_base_parity_over_shipped_corpus() {
    run_variant(&variant::O200K_BASE);
}

fn run_variant(v: &variant::Variant) {
    let report = match parity::run_parity(v, corpus::CORPUS) {
        Ok(r) => r,
        Err(e) => {
            // A load/fetch failure is always a hard error — there is
            // no meaningful "informational" outcome when the
            // harness could not even construct the tokenizers.
            panic!("parity harness failed to run for {}: {e}", v.name);
        }
    };

    println!(
        "=== parity report for {} ===\n  passed:               {}/{}\n  divergences:          {}\n  truncated after cap:  {}",
        report.variant_name,
        report.passed,
        report.total,
        report.divergences.len(),
        report.truncated_divergences,
    );
    let by_cat = report.divergences_by_category();
    if !by_cat.is_empty() {
        println!("  divergences by category:");
        for (cat, n) in &by_cat {
            println!("    {cat}: {n}");
        }
    }
    for d in report.divergences.iter().take(5) {
        println!(
            "  first-5 divergence (idx={}, category={}):",
            d.input_index, d.category
        );
        println!("    input:    {:?}", d.input);
        println!("    expected: {:?}", d.expected_ids);
        println!("    actual:   {:?}", d.actual_ids);
        println!("    first diff at index: {:?}", d.first_diff_at);
    }

    let strict = env::var_os("TIKTOKEN_PARITY_STRICT").is_some();
    assert!(
        !strict || report.is_perfect(),
        "TIKTOKEN_PARITY_STRICT=1 and {} produced {} divergence(s) — see stdout",
        v.name,
        report.divergences.len() + report.truncated_divergences,
    );
}
