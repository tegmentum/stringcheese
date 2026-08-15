//! Baseline throughput benchmarks for the load-bearing
//! `stringcheese-phonetic` encoders.
//!
//! Mirrors the shape of the reference bench crate
//! `stringcheese-tokenizer-hf-bench` (and its sibling
//! `benches/compare.rs`, `benches/manip.rs` files): one bench file,
//! criterion-owned `main` via `harness = false`, groups named
//! `<crate>/<kernel>/<flavor>/<len>`. Purpose is a regression
//! trip-wire and a last-measured baseline table for the biggest
//! crate in the phonetic subsystem, not an intra-crate perf
//! comparison (the encoders here compute distinct outputs and
//! aren't cross-comparable in throughput terms).
//!
//! # Encoders
//!
//! Five encoder groups, all exported from the crate's public
//! surface and all reached from sibling language packs
//! (`stringcheese-en`, `stringcheese-uk`, `stringcheese-sr`, …) —
//! so measurable regressions here surface immediately in the
//! downstream lang crates too:
//!
//! * `soundex` — the 1918 NARA Soundex, the entity-resolution
//!   workhorse for a century of US census indexing. Zero-sized
//!   encoder, called as `Soundex::encode(input)`.
//! * `nysiis` — Robert L. Taft's 1970 NYSIIS encoder, still
//!   widely deployed in record-linkage tooling. Also zero-sized
//!   and called as `Nysiis::encode(input)`.
//! * `double_metaphone_primary` — the primary-only variant of
//!   Lawrence Philips' 1999 Double Metaphone, constructed once
//!   as `DoubleMetaphone::primary_only()` and reused per-iter.
//!   Returns a [`DoubleMetaphoneKey`] whose `alternate` is
//!   always `None` on this path.
//! * `double_metaphone_full` — the full two-key variant
//!   (`DoubleMetaphone::full()`). Same public entry point,
//!   additionally computes the alternate key at every
//!   documented divergence point; the primary key remains
//!   byte-identical to the primary-only variant.
//! * `slavic_metaphone` — the crate's Slavic-language pack, a
//!   variable-length Metaphone-shaped encoder covering ru / uk /
//!   be / bg / mk / sr / hr / bs / sl / pl / cs / sk across both
//!   Cyrillic and Latin scripts. Constructed once as
//!   `SlavicMetaphone::new()` and reused per-iter.
//!
//! # Input sizes
//!
//! Three character counts per (encoder, flavor): 32, 256, 2048.
//! Real phonetic-encoding workloads bill against surnames and
//! short phrases, so 32 is the most representative regime; the
//! larger sizes exercise the inner-loop steady state and any
//! per-call fixed overhead amortizes out.
//!
//! # Flavors
//!
//! Two per size:
//!
//! * `ascii` — deterministic random `[a-z]` bytes. Exercises
//!   the ASCII-letter fast path every encoder here takes as its
//!   first step. This is what a real English record-linkage
//!   pipeline pays.
//! * `utf8` — deterministic random `char`s drawn from Cyrillic
//!   plus a diverse Unicode pool (Latin-with-diacritics,
//!   Cyrillic, CJK ideographs). For [`Soundex`], [`Nysiis`],
//!   and [`DoubleMetaphone`] this input is largely *stripped*
//!   in the first pass — the algorithms are ASCII-only — so the
//!   utf8 row measures the strip-and-scan cost, which is what
//!   the crate actually charges callers who feed it non-English
//!   input. For [`SlavicMetaphone`] the Cyrillic-heavy pool is
//!   the intended input, so the utf8 row on the slavic group is
//!   the *hot* row, not a fallback.
//!
//! # Running
//!
//! ```text
//! cargo bench -p stringcheese-phonetic
//! ```
//!
//! Filter to one group:
//!
//! ```text
//! cargo bench -p stringcheese-phonetic -- soundex
//! ```
//!
//! Smoke check (compile-only, no measurement — used by CI):
//!
//! ```text
//! cargo bench -p stringcheese-phonetic --no-run
//! ```
//!
//! Baseline numbers table lives in the crate-level `//!` docs of
//! `src/lib.rs`.

#![allow(
    missing_docs,
    reason = "criterion_group! / criterion_main! macros emit undocumented public fns; the bench binary is publish = false and not user-facing"
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use stringcheese_phonetic::{DoubleMetaphone, Nysiis, SlavicMetaphone, Soundex};

// ---------------------------------------------------------------------------
// Deterministic RNG + input generation.
//
// The bench crate must be self-contained (no rand dev-dep). A tiny
// SplitMix64-flavoured PRNG is sufficient: bench-hot-path only, seed
// derivation is deterministic so re-runs on the same host produce the
// same corpora.
// ---------------------------------------------------------------------------

/// Length grid swept by every group. Chosen to span
/// {surname, sentence, paragraph} regimes. 32 is the representative
/// working point (real phonetic inputs are short); 2048 exercises the
/// inner-loop steady state so per-call fixed overhead amortizes out.
const LENGTHS: &[usize] = &[32, 256, 2048];

/// The two flavors every group cycles over. `ascii` exercises the
/// ASCII-letter fast path; `utf8` exercises the strip-and-scan (for the
/// English-only encoders) or the Cyrillic hot path (for
/// [`SlavicMetaphone`]).
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

    fn next_diverse_char(&mut self) -> char {
        // Cycle across a handful of Unicode blocks that together
        // stress the strip-and-scan (for the English-only encoders)
        // and the Cyrillic hot path (for `SlavicMetaphone`): 4-byte
        // CJK ideographs, 2-byte Cyrillic and Latin-with-diacritics,
        // 1-byte ASCII. Weight the choice toward Cyrillic so
        // `SlavicMetaphone` sees mostly hot-path input.
        let r = self.next_u64();
        let block = r % 5;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "top-half draw, truncation is the operational intent"
        )]
        let offset = (r >> 8) as u32;
        let cp = match block {
            // Cyrillic Ru-lowercase range (SlavicMetaphone hot path)
            0..=2 => 0x0430 + (offset % 0x20),
            // ASCII lowercase (English encoders' hot path)
            3 => 0x0061 + (offset % 26),
            // CJK unified ideographs (stripped by every encoder here)
            _ => 0x4E00 + (offset % 0x1000),
        };
        char::from_u32(cp).unwrap_or('a')
    }
}

/// A `len`-byte ASCII corpus, deterministic per length.
fn ascii_input(len: usize) -> String {
    let mut rng = Rng::new(seed_for(len, 0x11));
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        bytes.push(rng.next_ascii_lower());
    }
    // Every byte is [a-z], so the resulting slice is valid UTF-8.
    String::from_utf8(bytes).expect("ascii bytes are valid utf-8")
}

/// A `len`-code-point Unicode-diverse corpus (Cyrillic-weighted for
/// the [`SlavicMetaphone`] hot path).
fn utf8_input(len: usize) -> String {
    let mut rng = Rng::new(seed_for(len, 0x21));
    let mut s = String::with_capacity(len * 3);
    for _ in 0..len {
        s.push(rng.next_diverse_char());
    }
    s
}

// ---------------------------------------------------------------------------
// Encoder groups.
//
// Throughput is reported in *input bytes*, not code points. That
// matches the reporting shape used by `benches/compare.rs` and
// `benches/manip.rs` — the utf8 flavor's per-scalar cost inflates
// against the ascii flavor at the same character count because a
// utf8 char pool weighs 2-4 bytes per scalar.
// ---------------------------------------------------------------------------

fn bench_soundex(c: &mut Criterion) {
    let mut group = c.benchmark_group("phonetic/soundex");
    for &flavor in FLAVORS {
        for &len in LENGTHS {
            let input = if flavor == "ascii" {
                ascii_input(len)
            } else {
                utf8_input(len)
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, len), &input, |bencher, input| {
                bencher.iter(|| black_box(Soundex::encode(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_nysiis(c: &mut Criterion) {
    let mut group = c.benchmark_group("phonetic/nysiis");
    for &flavor in FLAVORS {
        for &len in LENGTHS {
            let input = if flavor == "ascii" {
                ascii_input(len)
            } else {
                utf8_input(len)
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, len), &input, |bencher, input| {
                bencher.iter(|| black_box(Nysiis::encode(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_double_metaphone_primary(c: &mut Criterion) {
    let mut group = c.benchmark_group("phonetic/double_metaphone_primary");
    let encoder = DoubleMetaphone::primary_only();
    for &flavor in FLAVORS {
        for &len in LENGTHS {
            let input = if flavor == "ascii" {
                ascii_input(len)
            } else {
                utf8_input(len)
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, len), &input, |bencher, input| {
                bencher.iter(|| black_box(encoder.encode(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_double_metaphone_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("phonetic/double_metaphone_full");
    let encoder = DoubleMetaphone::full();
    for &flavor in FLAVORS {
        for &len in LENGTHS {
            let input = if flavor == "ascii" {
                ascii_input(len)
            } else {
                utf8_input(len)
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, len), &input, |bencher, input| {
                bencher.iter(|| black_box(encoder.encode(black_box(input))));
            });
        }
    }
    group.finish();
}

fn bench_slavic_metaphone(c: &mut Criterion) {
    // The Cyrillic-weighted `utf8` pool is the intended hot path for
    // this encoder; the `ascii` row measures the Latin-script arm.
    // Both are supported inputs — the encoder's applicability
    // declares both `Cyrl` and `Latn` scripts.
    let mut group = c.benchmark_group("phonetic/slavic_metaphone");
    let encoder = SlavicMetaphone::new();
    for &flavor in FLAVORS {
        for &len in LENGTHS {
            let input = if flavor == "ascii" {
                ascii_input(len)
            } else {
                utf8_input(len)
            };
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(BenchmarkId::new(flavor, len), &input, |bencher, input| {
                bencher.iter(|| black_box(encoder.encode(black_box(input))));
            });
        }
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Oracle-comparison groups (feature-gated on `oracle-benches`).
//
// Goal: side-by-side throughput of stringcheese vs a best-in-class Rust
// phonetic-encoder crate on identical inputs. Not a correctness oracle
// — the two implementations of each algorithm are not obliged to agree
// on output byte-for-byte (Soundex has census / genealogy variants; the
// Double Metaphone paper leaves several rule details underspecified;
// NYSIIS has a strict / non-strict split with different length caps) —
// but a *perf* oracle: it answers "is stringcheese competitive, or is
// there real headroom vs the ecosystem?"
//
// Oracle chosen (`optional = true` in `Cargo.toml`, gated on the
// `oracle-benches` feature):
//
// * `rphonetic` (3.0) — Rust port of the Apache commons-codec phonetic
//   family. The only Rust crate that covers all four stringcheese
//   English encoders (Soundex, primary Double Metaphone, full Double
//   Metaphone, NYSIIS) in one place. See the crate's `Cargo.toml`
//   header for the alternative oracles considered and rejected, and
//   the dep-footprint notes.
//
// Every oracle group cycles the ASCII flavor of the primary groups'
// length grid (`LENGTHS = 32/256/2048`). The utf8 flavor is *not*
// included in the oracle rows: every English-only encoder here (both
// stringcheese and rphonetic) strips non-ASCII input in its first
// pass, so the utf8 numbers would measure the strip-and-scan cost of
// each implementation rather than any algorithmic difference. Slavic
// Metaphone has no oracle counterpart (stringcheese-specific) so it
// stays out of the oracle rows.
// ---------------------------------------------------------------------------

#[cfg(feature = "oracle-benches")]
fn bench_oracle_soundex(c: &mut Criterion) {
    use rphonetic::Encoder as _;
    let mut group = c.benchmark_group("phonetic/oracle/soundex");
    let rphonetic_soundex = rphonetic::Soundex::default();
    for &len in LENGTHS {
        let input = ascii_input(len);
        group.throughput(Throughput::Bytes(input.len() as u64));

        // stringcheese lane — mirrors `bench_soundex`'s ASCII row.
        group.bench_with_input(
            BenchmarkId::new("stringcheese/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(Soundex::encode(black_box(input))));
            },
        );

        // rphonetic lane — default US NARA Soundex mapping.
        group.bench_with_input(
            BenchmarkId::new("rphonetic/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(rphonetic_soundex.encode(black_box(input))));
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
fn bench_oracle_nysiis(c: &mut Criterion) {
    use rphonetic::Encoder as _;
    let mut group = c.benchmark_group("phonetic/oracle/nysiis");
    // `Nysiis::default()` is strict-mode (6-char cap), which matches the
    // canonical Taft-1970 NYSIIS. stringcheese's `Nysiis::encode` also
    // produces the standard 6-char output.
    let rphonetic_nysiis = rphonetic::Nysiis::default();
    for &len in LENGTHS {
        let input = ascii_input(len);
        group.throughput(Throughput::Bytes(input.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("stringcheese/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(Nysiis::encode(black_box(input))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rphonetic/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(rphonetic_nysiis.encode(black_box(input))));
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
fn bench_oracle_double_metaphone_primary(c: &mut Criterion) {
    use rphonetic::Encoder as _;
    let mut group = c.benchmark_group("phonetic/oracle/double_metaphone_primary");
    let sc_encoder = DoubleMetaphone::primary_only();
    // Two rphonetic configurations are benched side by side because the
    // rphonetic default (`max_code_length = Some(4)`) *early-exits* as
    // soon as the primary key has 4 characters — stringcheese's
    // `DoubleMetaphone::primary_only()` emits an uncapped key. At
    // n = 32 the two configurations agree; at n = 256 / 2048 the capped
    // configuration walks a tiny fraction of the input before returning,
    // which trivialises the throughput comparison for those sizes. The
    // uncapped (`None`) row is the fair apples-to-apples comparison; the
    // capped (`Some(4)`) row documents the "off-the-shelf default"
    // gap that a caller who used `DoubleMetaphone::default()` would see.
    let rphonetic_dm_capped = rphonetic::DoubleMetaphone::default();
    let rphonetic_dm_uncapped = rphonetic::DoubleMetaphone::new(None);
    for &len in LENGTHS {
        let input = ascii_input(len);
        group.throughput(Throughput::Bytes(input.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("stringcheese/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(sc_encoder.encode(black_box(input))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rphonetic-uncapped/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(rphonetic_dm_uncapped.encode(black_box(input))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rphonetic-cap4/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(rphonetic_dm_capped.encode(black_box(input))));
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
fn bench_oracle_double_metaphone_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("phonetic/oracle/double_metaphone_full");
    let sc_encoder = DoubleMetaphone::full();
    // Same capped/uncapped split as `bench_oracle_double_metaphone_primary`.
    // `DoubleMetaphone::double_metaphone` returns a
    // `DoubleMetaphoneResult { primary, alternate }` — the two-key
    // shape stringcheese's `full()` variant produces.
    let rphonetic_dm_capped = rphonetic::DoubleMetaphone::default();
    let rphonetic_dm_uncapped = rphonetic::DoubleMetaphone::new(None);
    for &len in LENGTHS {
        let input = ascii_input(len);
        group.throughput(Throughput::Bytes(input.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("stringcheese/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(sc_encoder.encode(black_box(input))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rphonetic-uncapped/ascii", len),
            &input,
            |bencher, input| {
                bencher
                    .iter(|| black_box(rphonetic_dm_uncapped.double_metaphone(black_box(input))));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("rphonetic-cap4/ascii", len),
            &input,
            |bencher, input| {
                bencher.iter(|| black_box(rphonetic_dm_capped.double_metaphone(black_box(input))));
            },
        );
    }
    group.finish();
}

#[cfg(feature = "oracle-benches")]
criterion_group!(
    oracle_benches,
    bench_oracle_soundex,
    bench_oracle_nysiis,
    bench_oracle_double_metaphone_primary,
    bench_oracle_double_metaphone_full,
);

criterion_group!(
    phonetic,
    bench_soundex,
    bench_nysiis,
    bench_double_metaphone_primary,
    bench_double_metaphone_full,
    bench_slavic_metaphone,
);

#[cfg(feature = "oracle-benches")]
criterion_main!(phonetic, oracle_benches);
#[cfg(not(feature = "oracle-benches"))]
criterion_main!(phonetic);
