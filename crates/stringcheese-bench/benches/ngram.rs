//! N-gram generation and representation benchmarks.
//!
//! The ngram crate ships representation-layer primitives, not similarity
//! functions of its own, so this bench covers construction only:
//!
//! * [`CharacterGrams::grams`] iterated to completion — the general-purpose
//!   owned-window generator. Benched at n = 2, 3, 5 over input lengths 32,
//!   128, 512, with `PaddingPolicy::None` and `PaddingPolicy::Boundary`.
//! * [`CharacterGramSlices::grams`] — the zero-allocation fast path over
//!   an already-padded input, for comparison against the owned path.
//! * [`GramSet::from_generator`] — deduplicated construction.
//! * [`GramMultiSet::from_generator`] — count-preserving construction.
//! * [`count_grams`] — the closed-form preallocation helper. Trivial in
//!   theory, but worth benching so a regression here would be caught.
#![allow(
    missing_docs,
    reason = "criterion_group! macro emits undocumented public fns; the bench crate is publish = false and not user-facing"
)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use stringcheese_bench::inputs::random_ascii;
use stringcheese_ngram::{
    CharacterGramSlices, CharacterGrams, GramMultiSet, GramSet, NGramGenerator, PaddingPolicy,
    count_grams,
};

const LENGTHS: &[usize] = &[32, 128, 512];
const NS: &[usize] = &[2, 3, 5];

#[inline]
fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

fn bench_character_grams_none(c: &mut Criterion) {
    // Owned-window generator, no padding. Measures the per-gram Vec
    // allocation cost plus the padded-buffer clone (which for
    // PaddingPolicy::None is a single copy of the input).
    let mut group = c.benchmark_group("ngram/character_grams_none");
    for &len in LENGTHS {
        let input = random_ascii(len, seed_for(len, 0x41));
        group.throughput(Throughput::Bytes(len as u64));
        for &n in NS {
            let gnr = CharacterGrams::new(n, PaddingPolicy::<u8>::None);
            group.bench_with_input(
                BenchmarkId::new(format!("n{n}"), len),
                &(gnr, &input),
                |bencher, (gnr, input)| {
                    bencher.iter(|| {
                        let mut sink: usize = 0;
                        for g in gnr.grams(black_box(input.as_slice())) {
                            // Fold each yielded gram into a small side effect
                            // so the loop body cannot be optimized away.
                            sink = sink.wrapping_add(g.len());
                        }
                        black_box(sink);
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_character_grams_boundary(c: &mut Criterion) {
    let mut group = c.benchmark_group("ngram/character_grams_boundary");
    for &len in LENGTHS {
        let input = random_ascii(len, seed_for(len, 0x42));
        group.throughput(Throughput::Bytes(len as u64));
        for &n in NS {
            let gnr = CharacterGrams::new(
                n,
                PaddingPolicy::Boundary {
                    start: b'^',
                    end: b'$',
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("n{n}"), len),
                &(gnr, &input),
                |bencher, (gnr, input)| {
                    bencher.iter(|| {
                        let mut sink: usize = 0;
                        for g in gnr.grams(black_box(input.as_slice())) {
                            sink = sink.wrapping_add(g.len());
                        }
                        black_box(sink);
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_character_gram_slices(c: &mut Criterion) {
    // Zero-alloc fast path: iterates borrowed `&[u8]` windows. The
    // delta between this group and `character_grams_none` is exactly
    // the per-gram Vec allocation cost, which is the whole reason the
    // fast path exists.
    let mut group = c.benchmark_group("ngram/character_gram_slices");
    for &len in LENGTHS {
        let input = random_ascii(len, seed_for(len, 0x43));
        group.throughput(Throughput::Bytes(len as u64));
        for &n in NS {
            let gnr = CharacterGramSlices::new(n);
            group.bench_with_input(
                BenchmarkId::new(format!("n{n}"), len),
                &(gnr, &input),
                |bencher, (gnr, input)| {
                    bencher.iter(|| {
                        let mut sink: usize = 0;
                        for g in gnr.grams(black_box(input.as_slice())) {
                            sink = sink.wrapping_add(g.len());
                        }
                        black_box(sink);
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_gram_set_from_generator(c: &mut Criterion) {
    let mut group = c.benchmark_group("ngram/gram_set_from_generator");
    for &len in LENGTHS {
        let input = random_ascii(len, seed_for(len, 0x44));
        group.throughput(Throughput::Bytes(len as u64));
        for &n in NS {
            let gnr = CharacterGrams::new(n, PaddingPolicy::<u8>::None);
            group.bench_with_input(
                BenchmarkId::new(format!("n{n}"), len),
                &(gnr, &input),
                |bencher, (gnr, input)| {
                    bencher.iter(|| {
                        let set: GramSet<Vec<u8>> =
                            GramSet::from_generator(black_box(gnr), black_box(input.as_slice()));
                        black_box(set);
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_gram_multiset_from_generator(c: &mut Criterion) {
    // Should be measurably distinct from the set version: the multiset
    // does a BTreeMap increment per gram instead of BTreeSet insertion,
    // and on inputs where many grams repeat (short n, long input) the
    // two structures diverge in cost.
    let mut group = c.benchmark_group("ngram/gram_multiset_from_generator");
    for &len in LENGTHS {
        let input = random_ascii(len, seed_for(len, 0x45));
        group.throughput(Throughput::Bytes(len as u64));
        for &n in NS {
            let gnr = CharacterGrams::new(n, PaddingPolicy::<u8>::None);
            group.bench_with_input(
                BenchmarkId::new(format!("n{n}"), len),
                &(gnr, &input),
                |bencher, (gnr, input)| {
                    bencher.iter(|| {
                        let ms: GramMultiSet<Vec<u8>> = GramMultiSet::from_generator(
                            black_box(gnr),
                            black_box(input.as_slice()),
                        );
                        black_box(ms);
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_count_grams(c: &mut Criterion) {
    // A closed-form arithmetic helper; the bench exists to catch any
    // future regression that turns the constant-time formula into
    // something linear by accident.
    let mut group = c.benchmark_group("ngram/count_grams");
    let padding_none: PaddingPolicy<u8> = PaddingPolicy::None;
    let padding_boundary: PaddingPolicy<u8> = PaddingPolicy::Boundary {
        start: b'^',
        end: b'$',
    };
    for &len in LENGTHS {
        for &n in NS {
            group.bench_with_input(
                BenchmarkId::new(format!("none_n{n}"), len),
                &(len, n),
                |bencher, (len, n)| {
                    bencher
                        .iter(|| count_grams::<u8>(black_box(*len), black_box(*n), &padding_none));
                },
            );
            group.bench_with_input(
                BenchmarkId::new(format!("boundary_n{n}"), len),
                &(len, n),
                |bencher, (len, n)| {
                    bencher.iter(|| {
                        count_grams::<u8>(black_box(*len), black_box(*n), &padding_boundary)
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_character_grams_none,
    bench_character_grams_boundary,
    bench_character_gram_slices,
    bench_gram_set_from_generator,
    bench_gram_multiset_from_generator,
    bench_count_grams,
);
criterion_main!(benches);
