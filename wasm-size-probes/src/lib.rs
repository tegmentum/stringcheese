//! Wasm size probes — a cdylib wrapper whose sole purpose is to force
//! LTO to retain the measured StringCheese crate's public API surface so
//! the resulting `.wasm` is a meaningful size measurement rather than
//! a near-empty stub.
//!
//! Structure. Each `probe-*` feature turns on the corresponding
//! optional dependency (see `Cargo.toml`) and gates a matching
//! `#[unsafe(no_mangle)] pub extern "C" fn probe_<crate>()` below. Any
//! such exported symbol is a retention root: LTO retains everything
//! reachable from it. The probe body calls a representative slice of
//! the crate's public API and pipes the results through
//! `core::hint::black_box`, so the compiler cannot fold the reference
//! away. Anything the probe does not touch can (and will) be stripped,
//! so the measured number is an upper bound on "what a browser or edge
//! bundle pays if it imports these entry points from the measured
//! crate".
//!
//! The features are additive (per Cargo's model) — enabling several at
//! once builds a wasm whose size is the union of their reachable
//! surfaces. In practice CI and `scripts/measure-wasm-size.sh` build
//! each probe with exactly one feature at a time so per-crate numbers
//! stay isolated; the additive shape only matters for
//! `cargo check --all-features` on this crate, which must still
//! succeed.
//!
//! Adding a new API to a measured crate does not automatically grow the
//! probe. If a wasm-size regression traces back to a symbol not touched
//! by the probe, add a call to the corresponding `probe_<crate>()` —
//! the probe is documentation as much as instrumentation.
//!
//! Feature-set choice: each measured crate is enabled with `std` (see
//! `Cargo.toml`), not `alloc` only. That gives us a panic handler and a
//! global allocator on `wasm32-unknown-unknown` (both required to link
//! any cdylib for that target) and picks up std-only kernels like
//! Damerau's HashMap-backed production kernel that a real deployment
//! would ship.

#![forbid(unsafe_op_in_unsafe_fn)]
// Probe bodies exercise the measured crate's public API on purpose —
// they deliberately reach for the same constructors and byte slices a
// real caller would, so clippy's "prefer the more elegant form" hints
// would only obscure the intent. Kept scoped to this crate to keep the
// main workspace under its stricter pedantic set.
#![allow(
    clippy::default_constructed_unit_structs,
    clippy::byte_char_slices,
    clippy::unnecessary_wraps,
    clippy::iter_on_single_items
)]

use core::hint::black_box;

// -----------------------------------------------------------------
// stringcheese (facade). Re-exports the workspace surface; the probe
// walks a handful of top-level items from each sub-crate so LTO keeps
// the compiled slice representative of a typical downstream import.
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese() -> usize {
    use stringcheese::compare::{Hamming, Jaro, JaroWinkler, Levenshtein, Osa, hamming_distance};
    use stringcheese::{DistanceMetric, NormalizedDistance, Similarity, SimilarityMetric};

    let a = black_box("kitten".as_bytes());
    let b = black_box("sitting".as_bytes());
    let mut acc: usize = 0;
    acc ^= Levenshtein.distance(a, b).into_inner() as usize;
    acc ^= Osa.distance(a, b).into_inner() as usize;
    acc ^= Jaro.similarity(a, b).into_inner().to_bits() as usize;
    acc ^= JaroWinkler::classic()
        .similarity(a, b)
        .into_inner()
        .to_bits() as usize;
    acc ^= hamming_distance(a, &b[..a.len().min(b.len())]).into_inner() as usize;
    let _ = Hamming;
    let _ = Similarity::<f64>::new(0.5);
    let _ = NormalizedDistance::new(0.5);
    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-core (types and traits only — no algorithms).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-core")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_core() -> usize {
    use stringcheese_core::descriptor::{AlgorithmFamily, DescriptorVersion, VariantId};
    use stringcheese_core::normalization::NormalizationPolicy;
    use stringcheese_core::result::{
        BoundedDistance, Distance, NormalizedDistance, NormalizedSimilarity, Score, Similarity,
    };

    let mut acc: usize = 0;
    acc ^= Distance::<u32>::new(black_box(1)).into_inner() as usize;
    acc ^= Distance::<u64>::new(black_box(1)).into_inner() as usize;
    acc ^= Similarity::<f64>::new(black_box(0.5))
        .into_inner()
        .to_bits() as usize;
    acc ^= Score::<f64>::new(black_box(0.5)).into_inner().to_bits() as usize;
    acc ^= NormalizedDistance::new(black_box(0.5))
        .map(|d| d.into_inner().to_bits() as usize)
        .unwrap_or(0);
    acc ^= NormalizedSimilarity::new(black_box(0.5))
        .map(|s| s.into_inner().to_bits() as usize)
        .unwrap_or(0);
    let bd = BoundedDistance::<u32>::Within(black_box(Distance::new(3)));
    acc ^= bd.within().map(|d| d.into_inner() as usize).unwrap_or(0);
    let _ = black_box(AlgorithmFamily::Levenshtein);
    let _ = black_box(DescriptorVersion::new(0, 1, 0));
    let _ = black_box(VariantId("probe"));
    let _ = black_box(NormalizationPolicy::ByMaxLength);
    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-corpus (golden-case schema; no shipping algorithms).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-corpus")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_corpus() -> usize {
    use stringcheese_corpus::{
        DifferenceClassification, count_sequences, exhaustive_over_alphabet, exhaustive_pairs,
    };

    let mut acc: usize = 0;
    acc ^= count_sequences(black_box(3), black_box(2)) as usize;
    let alpha = black_box(&[b'a', b'b'][..]);
    acc ^= exhaustive_over_alphabet(alpha, black_box(2)).count();
    acc ^= exhaustive_pairs(alpha, black_box(2)).count();
    let _ = black_box(DifferenceClassification::Agreement);
    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-compare (edit distances, similarity, search, minhash).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-compare")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_compare() -> usize {
    use stringcheese_compare::{Hamming, Jaro, JaroWinkler, Levenshtein, Osa, hamming_distance};
    use stringcheese_core::{DistanceMetric, SimilarityMetric};

    let a = black_box("kitten".as_bytes());
    let b = black_box("sitting".as_bytes());
    let mut acc: usize = 0;
    acc ^= Levenshtein.distance(a, b).into_inner() as usize;
    acc ^= Osa.distance(a, b).into_inner() as usize;
    acc ^= Jaro.similarity(a, b).into_inner().to_bits() as usize;
    acc ^= JaroWinkler::classic()
        .similarity(a, b)
        .into_inner()
        .to_bits() as usize;
    acc ^= hamming_distance(a, &b[..a.len().min(b.len())]).into_inner() as usize;
    let _ = Hamming;
    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-unicode (normalization, graphemes, diacritics).
//
// Note: `case_fold` (behind `case-fold` + `compiled-case-data`) and
// `words` / `sentences` (behind `word-segmentation` /
// `sentence-segmentation`) are all deliberately excluded from this
// probe's baseline. Baking the ICU case-mapping tables into the
// binary costs roughly 110 KB, and adding the UAX #29
// word- / sentence-break tables costs roughly 60 KB, in a
// `wasm-opt -Oz` build. Only wasm callers who opt in should pay
// those. The size-limit baseline this probe drives therefore reflects
// the minimum useful surface (normalization + graphemes + diacritic
// stripping) — see `docs/wasm-binary-size.md`.
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-unicode")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_unicode() -> usize {
    use stringcheese_unicode::{graphemes, nfc, nfd, nfkc, nfkd, strip_diacritics};

    let s = black_box("Café");
    let mut acc: usize = 0;
    acc ^= nfc(s).len();
    acc ^= nfd(s).len();
    acc ^= nfkc(s).len();
    acc ^= nfkd(s).len();
    acc ^= strip_diacritics(s).len();
    acc ^= graphemes(s).count();
    // Optional: enable the `unicode-with-compiled-case-data` probe
    // feature (see `Cargo.toml`) to additionally reach `case_fold` and
    // measure the fuller-default configuration. Not part of the gated
    // baseline — see `docs/wasm-binary-size.md` for the rationale.
    #[cfg(feature = "unicode-with-compiled-case-data")]
    {
        use stringcheese_unicode::case_fold;
        acc ^= case_fold(s).len();
    }
    // Optional: enable the `unicode-with-segmentation` probe feature
    // (see `Cargo.toml`) to additionally reach `words` and
    // `sentences`, forcing LTO to retain the `Word_Break` and
    // `Sentence_Break` tables. Not part of the gated baseline for the
    // same reason as `unicode-with-compiled-case-data` — the tracked
    // number reflects the minimum useful surface, and both features
    // are individually toggleable by size-conscious wasm callers.
    #[cfg(feature = "unicode-with-segmentation")]
    {
        use stringcheese_unicode::{sentences, words};
        acc ^= words(s).count();
        acc ^= sentences(s).count();
    }
    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-phonetic (Soundex, NYSIIS, Double Metaphone).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-phonetic")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_phonetic() -> usize {
    use stringcheese_phonetic::{DoubleMetaphone, Nysiis, PhoneticEncoder, Soundex};

    let s = black_box("Robert");
    let mut acc: usize = 0;
    acc ^= Soundex::default().encode(s).len();
    acc ^= Nysiis::default().encode(s).len();
    let dm = DoubleMetaphone::default().encode(s);
    acc ^= dm.primary.len();
    acc ^= dm.alternate.as_ref().map(|a| a.len()).unwrap_or(0);
    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-cdc (FastCDC + rolling-hash fingerprints).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-cdc")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_cdc() -> usize {
    use stringcheese_cdc::{
        Buzhash, FastCdc, FastCdcConfig, GearHash, PolynomialHash, RabinFingerprint, RollingHash,
    };

    let bytes: &[u8] = black_box(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    let mut acc: usize = 0;

    let cfg = FastCdcConfig::default_8k();
    let cdc = FastCdc::new(cfg);
    acc ^= cdc.chunk_boundaries(bytes).count();

    let mut rabin: RabinFingerprint = RollingHash::new(4);
    for &b in bytes {
        rabin.roll(b);
    }
    acc ^= rabin.digest() as usize;

    let mut poly: PolynomialHash = RollingHash::new(4);
    for &b in bytes {
        poly.roll(b);
    }
    acc ^= poly.digest() as usize;

    let mut gear: GearHash = RollingHash::new(4);
    for &b in bytes {
        gear.roll(b);
    }
    acc ^= gear.digest() as usize;

    let mut buz: Buzhash = RollingHash::new(4);
    for &b in bytes {
        buz.roll(b);
    }
    acc ^= buz.digest() as usize;

    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-index (BK-tree, VP-tree, q-gram inverted index).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-index")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_index() -> usize {
    use stringcheese_core::{Distance, DistanceMetric, MetricClass, MetricProperties};
    use stringcheese_index::{BkTree, QgramIndex, VpTree, length_filter};

    // BK-tree / VP-tree both refuse construction unless the wrapped
    // metric returns `is_metric() == true`. A minimal probe-local
    // metric — trivially a true metric, `|a - b|`-style on `u8` slices
    // — satisfies the check without pulling `stringcheese-compare` in
    // as an extra dep and inflating the measurement.
    #[derive(Debug, Default)]
    struct ProbeMetric;
    impl DistanceMetric<[u8]> for ProbeMetric {
        type Output = u32;
        fn distance(&self, a: &[u8], b: &[u8]) -> Distance<u32> {
            Distance::new(a.len().abs_diff(b.len()) as u32)
        }
        fn properties(&self) -> MetricProperties {
            MetricProperties::METRIC
        }
        fn class(&self) -> MetricClass {
            MetricClass::Metric
        }
    }

    let mut acc: usize = 0;

    let mut bk: BkTree<u8, ProbeMetric> = BkTree::new(ProbeMetric);
    bk.insert(black_box(b"apple".to_vec()));
    bk.insert(black_box(b"apricot".to_vec()));
    acc ^= bk.len();
    acc ^= bk.find_within(black_box(b"apple"), 1).len();

    let vp: VpTree<u8, ProbeMetric> = VpTree::from_corpus(
        ProbeMetric,
        black_box(alloc::vec![b"apple".to_vec(), b"banana".to_vec()]),
    );
    acc ^= vp.len();

    let mut q: QgramIndex<u8> = QgramIndex::new();
    acc ^= q.insert(black_box([b'a', b'b', b'c']));
    acc ^= q.len();
    acc ^= q
        .length_filter_candidates(black_box(5), black_box(0.5))
        .len();

    let range = length_filter(black_box(5), black_box(0.5));
    acc ^= (*range.start() + *range.end()) as usize;
    black_box(acc)
}

extern crate alloc;

// -----------------------------------------------------------------
// stringcheese-align (Needleman-Wunsch, Smith-Waterman).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-align")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_align() -> usize {
    use stringcheese_align::{AffineGap, LinearGap, NeedlemanWunsch, SmithWaterman};

    let a = black_box("GATTACA".as_bytes());
    let b = black_box("GCATGCU".as_bytes());
    let linear = LinearGap::simple();
    let affine = AffineGap::default_affine();
    let mut acc: usize = 0;
    acc ^= NeedlemanWunsch::new(linear).score(a, b).into_inner() as isize as usize;
    acc ^= SmithWaterman::new(linear).score(a, b).into_inner() as isize as usize;
    acc ^= NeedlemanWunsch::new(affine).score(a, b).into_inner() as isize as usize;
    black_box(acc)
}

// -----------------------------------------------------------------
// stringcheese-manip (inspect, trim, case; other modules stubbed).
// -----------------------------------------------------------------
#[cfg(feature = "probe-stringcheese-manip")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_stringcheese_manip() -> usize {
    use stringcheese_manip::{case, inspect, trim};

    let s = black_box("  Hello, World!  ");
    let mut acc: usize = 0;
    acc ^= trim::trim(s).len();
    acc ^= trim::trim_start(s).len();
    acc ^= trim::trim_end(s).len();
    acc ^= inspect::byte_len(s);
    acc ^= inspect::scalar_count(s);
    acc ^= inspect::grapheme_count(s);
    acc ^= case::to_lowercase(s).len();
    acc ^= case::to_uppercase(s).len();
    acc ^= case::to_title_case(s).len();
    acc ^= case::capitalize(s).len();
    black_box(acc)
}

// A single-crate default target when no probe feature is selected. This
// exists so `cargo check` (with no features) still succeeds, which keeps
// the `no_std / alloc` and workspace-wide feature-matrix jobs happy.
// The panic handler and allocator only get pulled in when a probe
// feature enables one of the measured crates (all of which enable
// `std`), so a featureless build is trivially linkable as an rlib but
// intentionally cannot produce a working cdylib.
#[cfg(not(any(
    feature = "probe-stringcheese",
    feature = "probe-stringcheese-core",
    feature = "probe-stringcheese-corpus",
    feature = "probe-stringcheese-compare",
    feature = "probe-stringcheese-unicode",
    feature = "probe-stringcheese-phonetic",
    feature = "probe-stringcheese-cdc",
    feature = "probe-stringcheese-index",
    feature = "probe-stringcheese-align",
    feature = "probe-stringcheese-manip",
)))]
#[unsafe(no_mangle)]
pub extern "C" fn probe_none() -> usize {
    0
}
