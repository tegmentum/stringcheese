//! Allocation-report binary for n-gram representation construction.
//!
//! The three concrete representations — [`GramSet`], [`GramMultiSet`], and
//! [`GramVector`] — have distinct allocation profiles even at the same
//! `(n, len)` because they back onto different `alloc::collections`
//! containers (`BTreeSet<G>`, `BTreeMap<G, usize>`, `BTreeMap<G, f64>`
//! respectively). This binary makes those differences visible in bytes.
//!
//! For comparison the [`CharacterGrams`] owned-window iterator (one
//! `Vec<u8>` per emitted gram) and the [`CharacterGramSlices`] zero-alloc
//! iterator (borrowed `&[u8]` windows) are reported alongside: the delta
//! between the two is exactly the per-gram `Vec` allocation cost that the
//! zero-alloc fast path was built to eliminate.
//!
//! # Skipped
//!
//! `Jaro` and `JaroWinkler` are not covered here. Both currently allocate
//! two `Vec<bool>` bitmasks per call and have no workspace to hoist the
//! allocation into, so a report would just say "two `Vec`s, `len` bytes
//! each" at every input size — a comment in this doc-block records that
//! result rather than spending a binary to re-derive it. When those
//! algorithms grow a workspace-aware variant (v0.2), a report binary
//! following this file's template should be added.
//!
//! `Hamming` is also skipped: it allocates nothing at all.
#![allow(
    missing_docs,
    reason = "binary entry points do not need item-level docs beyond the file-level module doc above"
)]

use comparand_bench::alloc_harness::{AllocMeasurement, measure};
use comparand_bench::inputs::random_ascii;
use comparand_ngram::{
    CharacterGramSlices, CharacterGrams, GramMultiSet, GramSet, GramVector, NGramGenerator,
    PaddingPolicy,
};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const LENGTHS: &[usize] = &[32, 128, 512];
const NS: &[usize] = &[2, 3, 5];

fn print_row(algorithm: &str, variant: &str, len: usize, n: usize, m: AllocMeasurement) {
    // `regime` is fixed at "random" for the ngram bin (there is no
    // similar/identical axis for a *representation* built from a single
    // input); n replaces `regime` in the second column so the row shape
    // stays consistent with the other binaries.
    println!(
        "{algorithm}\t{variant}\t{len}\tn{n}\t{blocks}\t{bytes}\t{max_blocks}\t{max_bytes}",
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
        let input = random_ascii(len, 0x31);
        for &n in NS {
            let gen_none = CharacterGrams::new(n, PaddingPolicy::<u8>::None);
            let gen_boundary = CharacterGrams::new(
                n,
                PaddingPolicy::Boundary {
                    start: b'^',
                    end: b'$',
                },
            );
            let gen_slices = CharacterGramSlices::new(n);

            // Owned-window iterator, no padding — one Vec<u8> per gram.
            let (_, m) = measure(|| {
                let mut sink: usize = 0;
                for g in gen_none.grams(input.as_slice()) {
                    sink = sink.wrapping_add(g.len());
                }
                sink
            });
            print_row("ngram", "character_grams_none", len, n, m);

            // Owned-window iterator, boundary padding — pads the input then
            // yields per-gram Vec<u8>s.
            let (_, m) = measure(|| {
                let mut sink: usize = 0;
                for g in gen_boundary.grams(input.as_slice()) {
                    sink = sink.wrapping_add(g.len());
                }
                sink
            });
            print_row("ngram", "character_grams_boundary", len, n, m);

            // Zero-allocation borrowed-slice iterator — the fast path.
            let (_, m) = measure(|| {
                let mut sink: usize = 0;
                for g in gen_slices.grams(input.as_slice()) {
                    sink = sink.wrapping_add(g.len());
                }
                sink
            });
            print_row("ngram", "character_gram_slices", len, n, m);

            // GramSet (BTreeSet<Vec<u8>>) construction.
            let (_, m) = measure(|| {
                let set: GramSet<Vec<u8>> = GramSet::from_generator(&gen_none, input.as_slice());
                set
            });
            print_row("ngram", "gram_set", len, n, m);

            // GramMultiSet (BTreeMap<Vec<u8>, usize>) construction — the
            // multiplicity-preserving alternative to GramSet.
            let (_, m) = measure(|| {
                let ms: GramMultiSet<Vec<u8>> =
                    GramMultiSet::from_generator(&gen_none, input.as_slice());
                ms
            });
            print_row("ngram", "gram_multiset", len, n, m);

            // GramVector (BTreeMap<Vec<u8>, f64>) construction.
            let (_, m) = measure(|| {
                let v: GramVector<Vec<u8>> =
                    GramVector::from_generator_counts(&gen_none, input.as_slice());
                v
            });
            print_row("ngram", "gram_vector", len, n, m);
        }
    }
}
