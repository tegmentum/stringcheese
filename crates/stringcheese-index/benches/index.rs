//! Baseline throughput benches for `stringcheese-index`.
//!
//! Three families of candidate-generation index share one criterion
//! binary because they share their input corpora (short ASCII pseudo-
//! random words):
//!
//! * **`index/bk_tree/find_within`** — Burkhard-Keller tree range
//!   query over a corpus wrapped in [`Levenshtein`]. Measures the
//!   pruning-effectiveness of the tree by reporting `records/sec`
//!   *queried* plus the number of candidate pairs the query surfaced
//!   (see below).
//! * **`index/vp_tree/*`** — Vantage-Point tree range + k-NN queries
//!   over the same corpus. `from_corpus` (balanced bulk build) is
//!   the constructor benched here; the incremental `insert` path is
//!   deliberately not benched separately because its worst-case
//!   depth is `O(n)` on adversarial inputs and the balanced build is
//!   the shape practical callers reach for.
//! * **`index/qgram/*`** — `QgramIndex` bigram inverted-index
//!   candidate generator. Two shapes:
//!   `overlap_candidates` (postings-driven) and
//!   `length_filter_candidates` (length-only), matching the two
//!   modes the crate's public API exposes.
//!
//! # Corpus shape
//!
//! `1000` and `10000` short synthetic words, produced by seeding a
//! deterministic `SplitMix64` PRNG once and drawing 3-8 lower-case
//! ASCII characters per record. Fixed seed → byte-identical corpus
//! across every run.
//!
//! The corpus is *not* drawn from a real word list because the
//! interesting metric here is index-structure throughput per record,
//! not per-word correctness — every index in this crate is generic
//! over its symbol / gram type, so realistic English words would not
//! measure a materially different code path than random ASCII.
//!
//! # Reported metrics
//!
//! Every bench uses [`Throughput::Elements`], so criterion reports
//! `records/sec` (queries/sec for BK-tree / VP-tree, insertions/sec
//! for the index-build benches, index-lookups/sec for QgramIndex).
//! Each bench also prints the total number of candidate pairs it
//! surfaced during a single sample (via `eprintln!`) so a reviewer
//! reading the raw output alongside the timing can sanity-check that
//! the query fanout matches the corpus shape — a value of "0"
//! reliably points at a degenerate corpus that pruned everything and
//! makes the timing meaningless. Run with `-- --nocapture` to see
//! the counts.
//!
//! # Baseline numbers (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative
//! `--quick` criterion run (three-second sample budget per bench).
//! Wall-clock samples vary ±5-15 % on a laptop under load; treat the
//! ratios as informative, the absolutes as ±30 % ballpark.
//! Throughput reported per-record (queries/sec for query benches,
//! insertions/sec for build benches). Higher is better.
//!
//! ```text
//! group                                          1 000           10 000
//! -----------------------------------------------------------------------
//! index/bk_tree/build                            1.68 M rec/s    1.16 M rec/s
//! index/bk_tree/find_within r=1                   48.9 K qps      6.8 K qps
//! index/vp_tree/from_corpus                      1.25 M rec/s    907 K rec/s
//! index/vp_tree/find_within r=1                   24.9 K qps     2.6 K qps
//! index/vp_tree/find_k_nearest k=5                 9.5 K qps      785 elem/s
//! index/qgram/build                              1.21 M rec/s    1.38 M rec/s
//! index/qgram/overlap_candidates o=2              588 K qps       51 K qps
//! index/qgram/length_filter_candidates θ=0.6     1.07 M qps       40 K qps
//! ```
//!
//! Candidate fanout across 100 queries (deterministic across runs):
//!
//! ```text
//! group                                        1 000       10 000
//! -----------------------------------------------------------------
//! bk_tree/find_within r=1                          6          91
//! vp_tree/find_within r=1                          6          91
//! vp_tree/find_k_nearest k=5                     500         500
//! qgram/overlap_candidates o=2                   193       1 883
//! qgram/length_filter_candidates θ=0.6        54 990     549 227
//! ```
//!
//! Read:
//!
//! * **`bk_tree` vs `vp_tree` find_within** — BK-tree is ~2× faster
//!   than VP-tree at radius 1 on this ASCII-word corpus: Levenshtein
//!   over short strings has a narrow output range (integers 0-8),
//!   which means BK-tree children are keyed by a small handful of
//!   distinct labels and the range prune `[d-r, d+r]` cuts almost
//!   every subtree. VP-tree's binary partition on the same low-range
//!   distance is coarser. A larger radius (r=2, r=3) would narrow the
//!   gap.
//! * **`qgram/overlap_candidates` vs `qgram/length_filter_candidates`** —
//!   at 1 000 records `overlap` and `length_filter` are close in
//!   throughput because the postings for 2-byte grams over a 26-
//!   letter alphabet are short (postings-walk cost is comparable to
//!   the linear `item_lens` scan). At 10 000 records `overlap` grows
//!   sub-linearly with corpus size while `length_filter` stays
//!   `O(n)`, so both drop to a similar level; the length filter
//!   surfaces ~285× as many candidates though, so downstream
//!   rescoring cost dwarfs the pre-filter cost when Jaccard-θ is
//!   this loose.
//! * **`vp_tree/find_k_nearest`** — runs ~2.5× slower than the range
//!   query at radius 1 because a k=5 heap cutoff has to visit more
//!   nodes before the top-k fills and the pruning radius tightens.
//!   Both are the same pruning skeleton though; a 5-8× gap on the
//!   same corpus would flag a heap-management regression.
//! * **`qgram/build` vs `bk_tree/build`** — `qgram` build is ~0.7×
//!   BK-tree build here because inserting bigrams touches multiple
//!   `BTreeMap` postings per record while BK-tree does one Levenshtein
//!   descent per record. A HashMap-backed variant would probably
//!   ~2× q-gram insertion throughput at the cost of losing the
//!   deterministic iteration order the crate commits to.
//!
//! Update this table whenever a perf change lands so future readers
//! don't have to re-run the bench to see whether the delta improved
//! or regressed the baseline.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-index --bench index
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-index --bench index -- vp_tree
//! ```
//!
//! Show per-bench candidate-fanout counts:
//!
//! ```text
//! cargo bench -p stringcheese-index --bench index -- --nocapture
//! ```
//!
//! [`Levenshtein`]: stringcheese_compare::levenshtein::Levenshtein

#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench binary is not part of the crate's public API"
)]
// Doctring calls out API names, some of which criterion / clippy's
// `doc_markdown` flags on every occurrence; leave the prose readable.
#![allow(clippy::doc_markdown)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_compare::levenshtein::Levenshtein;
use stringcheese_index::{BkTree, QgramIndex, VpTree};

// -------------------------------------------------------------------
// Corpus construction — deterministic across every run.
// -------------------------------------------------------------------

const CORPUS_SIZES: &[(usize, &str)] = &[(1_000, "1000"), (10_000, "10000")];
const RADIUS: u32 = 1;
const K_NEAREST: usize = 5;
const OVERLAP: u32 = 2;
const JACCARD_THRESHOLD: f64 = 0.6;

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// One synthetic 3-8-byte lowercase-ASCII "word".
#[allow(
    clippy::cast_possible_truncation,
    reason = "the input is a hashed u64 whose low bits are the deliberate signal; the % 26 that follows keeps the output in range"
)]
fn synthetic_word(state: &mut u64) -> Vec<u8> {
    let seed = splitmix64(state);
    let len = 3 + ((seed >> 32) & 0x7) as usize;
    let mut out = Vec::with_capacity(len);
    let mut s = seed;
    for _ in 0..len {
        s = splitmix64(&mut s);
        // 26 letters, well distributed by taking the high byte.
        out.push(b'a' + ((s >> 40) as u8) % 26);
    }
    out
}

/// `n` distinct-seed synthetic words. Deterministic across runs.
fn build_corpus(n: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut state = seed;
    (0..n).map(|_| synthetic_word(&mut state)).collect()
}

/// A separate small set of *queries* (100 items) drawn with a
/// different seed so the queries are not in the corpus but come from
/// the same character distribution.
fn build_queries(n: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut state = seed;
    (0..n).map(|_| synthetic_word(&mut state)).collect()
}

const QUERY_COUNT: usize = 100;

/// Character bigrams for the q-gram benches. `Vec<u8>` so the gram
/// type is `Ord + Clone` — matches the crate's golden-test shape.
fn char_bigrams(input: &[u8]) -> Vec<Vec<u8>> {
    input.windows(2).map(<[u8]>::to_vec).collect()
}

/// One-time report of candidate fanout for a given bench group. Prints
/// to stderr so `-- --nocapture` surfaces the number, and does nothing
/// when the bench harness swallows stderr (which is the default). The
/// same counter is not part of the timed sample — the closure only
/// runs once, up front, so the timing loop stays pure.
fn report_fanout(group: &str, corpus_size: usize, total_candidates: usize) {
    eprintln!(
        "FANOUT index/{group} corpus={corpus_size}: candidates surfaced (single-pass) = {total_candidates}",
    );
}

// -------------------------------------------------------------------
// BK-tree — build and range-query throughput.
// -------------------------------------------------------------------

fn bench_bk_tree(c: &mut Criterion) {
    let queries = build_queries(QUERY_COUNT, 0xB77_5EED);
    for &(size, _label) in CORPUS_SIZES {
        let corpus = build_corpus(size, 0xC077_5EED);

        // Build throughput: one insert per item, reported per-record.
        let mut group = c.benchmark_group("index/bk_tree/build");
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &corpus, |b, corpus| {
            b.iter(|| {
                let mut tree = BkTree::new(Levenshtein);
                for item in corpus {
                    tree.insert(item.clone());
                }
                black_box(tree.len());
            });
        });
        group.finish();

        // Query throughput. Report per-query (Elements = QUERY_COUNT).
        // Also report total candidates surfaced across the query set
        // as a one-time fanout number so the pruning-effectiveness of
        // the tree is visible from the raw output.
        let mut tree = BkTree::new(Levenshtein);
        for item in &corpus {
            tree.insert(item.clone());
        }
        let fanout: usize = queries
            .iter()
            .map(|q| tree.find_within(q, RADIUS).len())
            .sum();
        report_fanout("bk_tree/find_within", size, fanout);

        let mut group = c.benchmark_group("index/bk_tree/find_within");
        group.throughput(Throughput::Elements(queries.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &queries, |b, queries| {
            b.iter(|| {
                let mut total = 0usize;
                for q in queries {
                    total += tree.find_within(black_box(q), RADIUS).len();
                }
                black_box(total);
            });
        });
        group.finish();
    }
}

// -------------------------------------------------------------------
// VP-tree — bulk build + range + k-NN throughput.
// -------------------------------------------------------------------

fn bench_vp_tree(c: &mut Criterion) {
    let queries = build_queries(QUERY_COUNT, 0xB77_5EED);
    for &(size, _label) in CORPUS_SIZES {
        let corpus = build_corpus(size, 0xC077_5EED);

        // Bulk build throughput. `from_corpus` is what practical
        // callers reach for; the incremental `insert` path is not
        // benched separately because its worst-case depth is `O(n)`
        // on adversarial insertion orders.
        let mut group = c.benchmark_group("index/vp_tree/from_corpus");
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &corpus, |b, corpus| {
            b.iter(|| {
                let tree = VpTree::from_corpus(Levenshtein, corpus.clone());
                black_box(tree.len());
            });
        });
        group.finish();

        // Range query.
        let tree = VpTree::from_corpus(Levenshtein, corpus.clone());
        let fanout_r: usize = queries
            .iter()
            .map(|q| tree.find_within(q, RADIUS).len())
            .sum();
        report_fanout("vp_tree/find_within", size, fanout_r);

        let mut group = c.benchmark_group("index/vp_tree/find_within");
        group.throughput(Throughput::Elements(queries.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &queries, |b, queries| {
            b.iter(|| {
                let mut total = 0usize;
                for q in queries {
                    total += tree.find_within(black_box(q), RADIUS).len();
                }
                black_box(total);
            });
        });
        group.finish();

        // k-nearest query.
        let fanout_k: usize = queries
            .iter()
            .map(|q| tree.find_k_nearest(q, K_NEAREST).len())
            .sum();
        report_fanout("vp_tree/find_k_nearest", size, fanout_k);

        let mut group = c.benchmark_group("index/vp_tree/find_k_nearest");
        group.throughput(Throughput::Elements(queries.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &queries, |b, queries| {
            b.iter(|| {
                let mut total = 0usize;
                for q in queries {
                    total += tree.find_k_nearest(black_box(q), K_NEAREST).len();
                }
                black_box(total);
            });
        });
        group.finish();
    }
}

// -------------------------------------------------------------------
// QgramIndex — build + overlap + length-filter throughput.
// -------------------------------------------------------------------

fn bench_qgram(c: &mut Criterion) {
    let queries: Vec<Vec<Vec<u8>>> = build_queries(QUERY_COUNT, 0xB77_5EED)
        .iter()
        .map(|q| char_bigrams(q))
        .collect();

    for &(size, _label) in CORPUS_SIZES {
        let corpus = build_corpus(size, 0xC077_5EED);
        let corpus_grams: Vec<Vec<Vec<u8>>> = corpus.iter().map(|w| char_bigrams(w)).collect();

        // Build throughput — insertion of the pre-computed gram list.
        let mut group = c.benchmark_group("index/qgram/build");
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &corpus_grams, |b, cg| {
            b.iter(|| {
                let mut idx: QgramIndex<Vec<u8>> = QgramIndex::new();
                for grams in cg {
                    idx.insert(grams.clone());
                }
                black_box(idx.len());
            });
        });
        group.finish();

        // Build the index once for the query benches.
        let mut idx: QgramIndex<Vec<u8>> = QgramIndex::new();
        for grams in &corpus_grams {
            idx.insert(grams.clone());
        }

        // Overlap candidates (postings-driven).
        let fanout_o: usize = queries
            .iter()
            .map(|qg| idx.overlap_candidates(qg, OVERLAP).len())
            .sum();
        report_fanout("qgram/overlap_candidates", size, fanout_o);

        let mut group = c.benchmark_group("index/qgram/overlap_candidates");
        group.throughput(Throughput::Elements(queries.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &queries, |b, queries| {
            b.iter(|| {
                let mut total = 0usize;
                for qg in queries {
                    total += idx.overlap_candidates(black_box(qg), OVERLAP).len();
                }
                black_box(total);
            });
        });
        group.finish();

        // Length filter (length-only pruning).
        let fanout_l: usize = queries
            .iter()
            .map(|qg| {
                // The length filter takes the query's gram count as a
                // u32; use the pre-built gram vec's length.
                #[allow(clippy::cast_possible_truncation)]
                let qlen = qg.len() as u32;
                idx.length_filter_candidates(qlen, JACCARD_THRESHOLD).len()
            })
            .sum();
        report_fanout("qgram/length_filter_candidates", size, fanout_l);

        let mut group = c.benchmark_group("index/qgram/length_filter_candidates");
        group.throughput(Throughput::Elements(queries.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &queries, |b, queries| {
            b.iter(|| {
                let mut total = 0usize;
                for qg in queries {
                    #[allow(clippy::cast_possible_truncation)]
                    let qlen = qg.len() as u32;
                    total += idx
                        .length_filter_candidates(black_box(qlen), JACCARD_THRESHOLD)
                        .len();
                }
                black_box(total);
            });
        });
        group.finish();
    }
}

criterion_group!(benches, bench_bk_tree, bench_vp_tree, bench_qgram);
criterion_main!(benches);
