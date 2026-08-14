//! Baseline throughput benchmarks for the load-bearing
//! `stringcheese-compare` kernels.
//!
//! Mirrors the shape of the reference bench crate
//! `stringcheese-tokenizer-hf-bench`: one bench file, criterion-owned
//! `main` via `harness = false`, groups named
//! `<kernel>/<flavor>/<len>`. The purpose is not to compare
//! implementations against each other (the intra-crate
//! `stringcheese-bench` suite already does that at higher resolution)
//! but to establish a regression trip-wire and last-measured baseline
//! table for the biggest compute-heavy crate in the workspace.
//!
//! # Kernels
//!
//! Eight groups spanning every distance / similarity family the crate
//! ships:
//!
//! * `levenshtein` — the primary Levenshtein rolling-rows kernel via
//!   `Levenshtein::distance_with_workspace`. Byte inputs.
//! * `damerau_osa` — Optimal String Alignment (restricted Damerau)
//!   rolling-rows via `Osa::distance_with_workspace`.
//! * `damerau_full` — full (unrestricted) Damerau-Levenshtein via
//!   `Damerau::distance_with_workspace`. `HashMap`-backed production
//!   kernel; costs more per pair than OSA at every scale.
//! * `hamming` — equal-length byte Hamming via
//!   `Hamming::distance_bytes`.
//! * `jaro` — Jaro similarity via `Jaro::similarity_with_workspace`.
//! * `jaro_winkler` — Winkler-boosted Jaro via
//!   `JaroWinkler::classic().similarity_with_workspace`.
//! * `lcs` — Longest Common Subsequence distance via
//!   `lcs_distance_rolling_rows_with_workspace`.
//! * `jaccard_set` — Jaccard over character bigram `GramSet`s. Includes
//!   the `GramSet` construction cost on both operands each iter — the
//!   set-similarity kernels themselves are trivially cheap, so measuring
//!   them in isolation would report near-zero.
//!
//! # Input sizes
//!
//! Three lengths per group: 32, 256, and 2048 characters. 2048 is
//! ~4M cell updates for the O(n²) DP kernels — still comfortably
//! sub-second on modern hardware; anything larger becomes noisy on a
//! shared runner.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `ascii` — deterministic random `[a-z]` bytes.
//! * `utf8` — deterministic random `char`s drawn from a wide
//!   Unicode-scalar range (Latin, Cyrillic, Greek, CJK ideographs). This
//!   is the path that exercises the generic-`T` kernels rather than the
//!   byte-slice fast paths, and it inflates per-element cost by ~3-4× on
//!   the DP kernels because `char` is 4 bytes and window-comparison
//!   happens at scalar granularity.
//!
//! For the Hamming group the two operands are the same length by
//! construction (Hamming requires equal-length inputs); for every other
//! group the two operands are the same length by convention, so the
//! DP-cell count matches the reported throughput bytes.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-compare
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-compare -- levenshtein
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-compare --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;

use stringcheese_compare::damerau::{Damerau, DamerauWorkspace, Osa, OsaWorkspace};
use stringcheese_compare::hamming::Hamming;
use stringcheese_compare::jaro::{Jaro, JaroWinkler, JaroWorkspace};
use stringcheese_compare::lcs::{LcsWorkspace, lcs_distance_rolling_rows_with_workspace};
use stringcheese_compare::levenshtein::{Levenshtein, LevenshteinWorkspace};
use stringcheese_compare::ngram::{CharacterGrams, GramSet, PaddingPolicy};
use stringcheese_compare::set_similarity::JaccardOverSet;

// ---------------------------------------------------------------------------
// Deterministic RNG + input generation.
//
// The bench crate must be self-contained (no dev-dep on
// `stringcheese-bench`, which would create a workspace cycle since that
// crate depends on `stringcheese-compare`). A tiny SplitMix64-flavoured
// PRNG is sufficient: bench-hot-path only, seed derivation is
// deterministic so re-runs on the same host produce the same corpora.
// ---------------------------------------------------------------------------

/// Length grid swept by every group. Chosen to span
/// {short-word, sentence, paragraph} regimes without pushing the O(n²)
/// DP kernels into multi-second territory.
const LENGTHS: &[usize] = &[32, 256, 2048];

/// The two flavors every group cycles over. `ascii` exercises byte
/// slices (`&[u8]`); `utf8` exercises `&[char]` and, transitively, the
/// generic-`T` kernel paths.
const FLAVORS: &[&str] = &["ascii", "utf8"];

/// Deterministic per-(len, salt) seed. Uses the length as an entropy
/// mixer so extending `LENGTHS` does not reshuffle unrelated rows.
#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

/// Minimal `SplitMix64` PRNG. Fine for corpus generation; not
/// cryptographic, not statistically robust, but reproducible and fast.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        // Zero-seed guard: SplitMix64 is well-defined at 0 but the
        // constant offset here makes even the zero-seed corpus non-trivial.
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_ascii_lower(&mut self) -> u8 {
        // Modulo-26 of a u64 always fits in a u8; the truncation is
        // intentional and the wrapping add to b'a' stays within ASCII.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "value is bounded by % 26, always fits in u8"
        )]
        let byte = (self.next_u64() % 26) as u8;
        b'a' + byte
    }

    /// Uniform-ish `usize` in `[0, bound)`. Bench-only, so the modulo
    /// bias is acceptable — every draw is used to perturb one input
    /// position, and the pattern is deterministic per seed anyway.
    fn next_bounded(&mut self, bound: usize) -> usize {
        // On 32-bit targets `u64 as usize` truncates the top 32 bits;
        // that only biases the low bits, which is fine for corpus
        // perturbation. On 64-bit targets the cast is lossless.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bench-only bounded draw; low-bit bias acceptable"
        )]
        let r = self.next_u64() as usize;
        r % bound.max(1)
    }

    fn next_diverse_char(&mut self) -> char {
        // Cycle across a handful of Unicode blocks that together stress
        // the generic-`T` kernels: 4-byte UTF-8 CJK ideographs, 3-byte
        // Georgian, 2-byte Cyrillic and Greek, and 1-byte ASCII. The
        // block choice is deterministic per draw so the resulting `char`
        // sequence has broad byte-width diversity without exhausting any
        // one block's codepoint space.
        let r = self.next_u64();
        let block = r % 5;
        // Take the top-half bits as the intra-block offset. `as u32`
        // discards the high 32 bits deliberately — we want a uniform-
        // ish per-block position modulo the block size, and the
        // subsequent `% block_size` (with block_size ≤ 0x1000) means
        // the truncated bits carry no signal.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "top-half draw, truncation is the operational intent"
        )]
        let offset = (r >> 8) as u32;
        let cp = match block {
            0 => 0x0061 + (offset % 26),     // ASCII lowercase
            1 => 0x0400 + (offset % 0x40),   // Cyrillic
            2 => 0x0370 + (offset % 0x40),   // Greek and Coptic
            3 => 0x10A0 + (offset % 0x30),   // Georgian
            _ => 0x4E00 + (offset % 0x1000), // CJK unified ideographs
        };
        char::from_u32(cp).unwrap_or('a')
    }
}

/// A pair of `len`-byte ASCII corpora with a small (≈5%) edit rate
/// between them — realistic for near-duplicate matching workloads and a
/// middle ground between "identical" (which some kernels short-circuit)
/// and "fully random" (which is the DP worst case). The two seeds
/// diverge by a fixed salt so the same length always produces the same
/// pair.
fn ascii_pair(len: usize) -> (Vec<u8>, Vec<u8>) {
    let mut a = Vec::with_capacity(len);
    let mut rng = Rng::new(seed_for(len, 0x11));
    for _ in 0..len {
        a.push(rng.next_ascii_lower());
    }
    let mut b = a.clone();
    // Perturb ~5% of positions with independent random bytes. Fixed seed
    // per length so the perturbation pattern is stable across runs.
    let mut mut_rng = Rng::new(seed_for(len, 0x12));
    let edits = len.div_ceil(20);
    for _ in 0..edits {
        let idx = mut_rng.next_bounded(len);
        b[idx] = mut_rng.next_ascii_lower();
    }
    (a, b)
}

/// A pair of `len`-code-point Unicode-diverse corpora, edited on the
/// same ~5% budget as `ascii_pair`.
fn utf8_pair(len: usize) -> (Vec<char>, Vec<char>) {
    let mut a = Vec::with_capacity(len);
    let mut rng = Rng::new(seed_for(len, 0x21));
    for _ in 0..len {
        a.push(rng.next_diverse_char());
    }
    let mut b = a.clone();
    let mut mut_rng = Rng::new(seed_for(len, 0x22));
    let edits = len.div_ceil(20);
    for _ in 0..edits {
        let idx = mut_rng.next_bounded(len);
        b[idx] = mut_rng.next_diverse_char();
    }
    (a, b)
}

// ---------------------------------------------------------------------------
// DP-distance groups
// ---------------------------------------------------------------------------

fn bench_levenshtein(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare/levenshtein");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = LevenshteinWorkspace::new();
                        bencher.iter(|| {
                            Levenshtein.distance_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                "utf8" => {
                    let (a, b) = utf8_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = LevenshteinWorkspace::new();
                        bencher.iter(|| {
                            Levenshtein.distance_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

fn bench_damerau_osa(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare/damerau_osa");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = OsaWorkspace::new();
                        bencher.iter(|| {
                            Osa.distance_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                "utf8" => {
                    let (a, b) = utf8_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = OsaWorkspace::new();
                        bencher.iter(|| {
                            Osa.distance_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

fn bench_damerau_full(c: &mut Criterion) {
    // Full Damerau's production kernel is `HashMap`-backed; the DP is
    // still O(m·n) but the per-cell constant is meaningfully higher than
    // OSA. Benched at the same three sizes so a regression in either
    // kernel is directly comparable against the OSA row.
    let mut group = c.benchmark_group("compare/damerau_full");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws: DamerauWorkspace<u8> = DamerauWorkspace::new();
                        bencher.iter(|| {
                            Damerau.distance_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                "utf8" => {
                    let (a, b) = utf8_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws: DamerauWorkspace<char> = DamerauWorkspace::new();
                        bencher.iter(|| {
                            Damerau.distance_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

fn bench_hamming(c: &mut Criterion) {
    // Hamming requires equal-length inputs; `ascii_pair` and
    // `utf8_pair` both preserve length so this is trivially the case.
    let mut group = c.benchmark_group("compare/hamming");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        bencher.iter(|| {
                            Hamming.distance_bytes(black_box(a.as_slice()), black_box(b.as_slice()))
                        });
                    });
                }
                "utf8" => {
                    // `distance_bytes` is byte-oriented; the char-slice
                    // path goes through the generic `hamming_distance`
                    // helper. Cheap either way (linear scan), but this
                    // preserves the two-flavor pattern used by the DP
                    // groups above.
                    let (a, b) = utf8_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        bencher.iter(|| {
                            stringcheese_compare::hamming::hamming_distance(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                            )
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

fn bench_jaro(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare/jaro");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = JaroWorkspace::new();
                        bencher.iter(|| {
                            Jaro.similarity_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                "utf8" => {
                    let (a, b) = utf8_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = JaroWorkspace::new();
                        bencher.iter(|| {
                            Jaro.similarity_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

fn bench_jaro_winkler(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare/jaro_winkler");
    let jw = JaroWinkler::classic();
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = JaroWorkspace::new();
                        bencher.iter(|| {
                            jw.similarity_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                "utf8" => {
                    let (a, b) = utf8_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = JaroWorkspace::new();
                        bencher.iter(|| {
                            jw.similarity_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

fn bench_lcs(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare/lcs");
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = LcsWorkspace::new();
                        bencher.iter(|| {
                            lcs_distance_rolling_rows_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                "utf8" => {
                    let (a, b) = utf8_pair(len);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        let mut ws = LcsWorkspace::new();
                        bencher.iter(|| {
                            lcs_distance_rolling_rows_with_workspace(
                                black_box(a.as_slice()),
                                black_box(b.as_slice()),
                                &mut ws,
                            )
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

fn bench_jaccard_set(c: &mut Criterion) {
    // Set-similarity itself is a linear pass over sorted keys; measuring
    // it in isolation reports near-zero. Bench the realistic caller
    // shape: build both bigram `GramSet`s and take the Jaccard. This is
    // the pipeline the crate's public docs recommend for dedupe /
    // near-neighbour workloads.
    let mut group = c.benchmark_group("compare/jaccard_set_bigrams");
    let jaccard = JaccardOverSet;
    for &len in LENGTHS {
        group.throughput(Throughput::Bytes(len as u64));
        for &flavor in FLAVORS {
            let id = BenchmarkId::new(flavor, len);
            match flavor {
                "ascii" => {
                    let (a, b) = ascii_pair(len);
                    let gen_a: CharacterGrams<u8> = CharacterGrams::new(2, PaddingPolicy::None);
                    let gen_b: CharacterGrams<u8> = CharacterGrams::new(2, PaddingPolicy::None);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        bencher.iter(|| {
                            let set_a: GramSet<Vec<u8>> =
                                GramSet::from_generator(&gen_a, black_box(a.as_slice()));
                            let set_b: GramSet<Vec<u8>> =
                                GramSet::from_generator(&gen_b, black_box(b.as_slice()));
                            jaccard.similarity_normalized(&set_a, &set_b)
                        });
                    });
                }
                "utf8" => {
                    let (a, b) = utf8_pair(len);
                    let gen_a: CharacterGrams<char> = CharacterGrams::new(2, PaddingPolicy::None);
                    let gen_b: CharacterGrams<char> = CharacterGrams::new(2, PaddingPolicy::None);
                    group.bench_with_input(id, &(a, b), |bencher, (a, b)| {
                        bencher.iter(|| {
                            let set_a: GramSet<Vec<char>> =
                                GramSet::from_generator(&gen_a, black_box(a.as_slice()));
                            let set_b: GramSet<Vec<char>> =
                                GramSet::from_generator(&gen_b, black_box(b.as_slice()));
                            jaccard.similarity_normalized(&set_a, &set_b)
                        });
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_levenshtein,
    bench_damerau_osa,
    bench_damerau_full,
    bench_hamming,
    bench_jaro,
    bench_jaro_winkler,
    bench_lcs,
    bench_jaccard_set,
);
criterion_main!(benches);
