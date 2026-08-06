# Changelog

All notable changes to Comparand are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
Until the first `0.1.0` release, expect breaking changes on any minor
bump; `0.x` versions are pre-stability.

## [Unreleased]

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.1.0-alpha] — unreleased

Retroactive entry documenting everything on `main` up to the ship-rehearsal
wave. No published release has been cut yet. Section grouped by capability
rather than by commit; consult `git log` for the per-commit narrative.

### Added

#### Substrate

- Initial workspace and type-system substrate: result types, metric
  traits, mathematical-property descriptors, algorithm-variant registry,
  workspace and sequence traits, and the golden-case validation schema
  (`comparand-core`, `comparand-corpus`, `comparand`, and the
  placeholder crates `comparand-unicode`, `comparand-phonetic`,
  `comparand-search`, `comparand-cdc`, `comparand-index`,
  `comparand-bench`).
- `comparand-corpus` oracle framework, exhaustive generators, and
  differential vocabulary.

#### Algorithm crates

- **Edit distance and similarity.** `comparand-levenshtein` (full-matrix
  oracle, rolling-rows production kernel, Ukkonen-style banded cutoff),
  `comparand-hamming` (metric distance for equal-length sequences),
  `comparand-jaro` (Jaro and Jaro-Winkler similarity family),
  `comparand-damerau` (Optimal String Alignment and full
  Damerau-Levenshtein), and `comparand-lcs` (Longest Common Subsequence
  and LCS distance).
- **N-gram and set similarity.** `comparand-ngram` (character, byte,
  and token n-gram representations) and `comparand-set-similarity`
  (Dice, Jaccard, Overlap, Cosine over n-gram representations).
- **Alignment.** `comparand-align`: Needleman-Wunsch global alignment
  and Smith-Waterman local alignment with linear and affine gap
  penalties.
- **Phonetic.** `comparand-phonetic`: Soundex, NYSIIS, and the
  single-key Double Metaphone encoder, followed by the full two-key
  Double Metaphone variant.
- **Unicode preprocessing.** `comparand-unicode`: NFC/NFD/NFKC/NFKD
  normalization, Unicode case folding (via `icu_casemap`, including
  multi-character expansions), grapheme-cluster segmentation, and
  diacritic stripping.
- **Substring search.** `comparand-search`: Rabin-Karp, KMP, Boyer-Moore
  (bad-character), Aho-Corasick, followed by Horspool, Two-way, the
  full Boyer-Moore with good-suffix rule, and streaming wrappers over
  the single-pattern algorithms.
- **Content-defined chunking.** `comparand-cdc`: rolling-hash
  fingerprints (Rabin, polynomial, Gear) and FastCDC chunking.
- **Indexes.** `comparand-index`: BK-tree, VP-tree (with bulk build and
  sorted-neighborhood blocking added in a follow-up), and a q-gram
  inverted index for large-scale nearest-neighbor and blocking queries.
- **MinHash / LSH.** `comparand-minhash`: MinHash sketches and LSH
  banding for approximate Jaccard-similarity search at scale.

#### WebAssembly Component Model

- `component/`: WIT interface definition and a Rust host demonstrating
  Comparand consumed as a Component-Model component, plus a reference
  guest and matching integration tests.

#### Benchmarks, fuzzing, and cross-comparison

- `comparand-bench`: criterion benchmark suite over the algorithm
  surface (Levenshtein, Hamming, Jaro, Damerau, n-gram, batch).
- Allocation-counting harness in `comparand-bench` backed by `dhat-rs`,
  gated behind the opt-in `alloc-tracking` feature so a default
  `cargo bench` never inherits the profiler's global allocator.
- `bench-adapters/`: Rust head-to-head adapters against `strsim` and
  `rapidfuzz` so criterion runs report Comparand alongside the
  established crates.
- `fuzz/`: `cargo-fuzz` targets covering both differential comparisons
  against known-good implementations and metric-axiom checks
  (non-negativity, identity, symmetry, triangle inequality).
- Scheduled nightly fuzz workflow (`.github/workflows/fuzz-nightly.yml`)
  driving the `cargo-fuzz` corpus and reporting regressions.

#### Documentation

- `docs/DESIGN.md`: full project vision, algorithm coverage, validation
  strategy, CI requirements, and release gates.
- Five subordinate design documents under `docs/design/` covering the
  detailed design of the substrate and first-wave algorithms.
- `docs/wasm-build-recipes.md`: definitive per-crate matrix of the
  `wasm32-unknown-unknown` and `wasm32-wasip1` feature combinations
  Comparand supports.
- `docs/references.md`: consolidated bibliography citing the primary
  papers behind every algorithm shipped in the workspace.
- Per-crate paper references added to the edit-distance / similarity,
  phonetic / unicode / align, and search / cdc / index / minhash
  families, cross-linked into `docs/references.md`.

### Changed

- Hardened the wasm CI matrix: `continue-on-error` removed from the
  `wasm` job, so `wasm32-unknown-unknown` and `wasm32-wasip1` failures
  now fail the workflow instead of being merely reported.

### Removed

- The record-linkage scope (Fellegi-Sunter probabilistic record
  linkage), briefly landed in-tree, has been extracted into a separate
  sibling library. Comparand stays focused on sequence comparison; the
  record-linkage crate depends on Comparand rather than the other way
  around. See the sibling repository for the extracted code and its
  own history.

### Fixed

- `comparand-damerau`: dropped intra-doc links that pointed at the
  test-only `property_tests` module, which produced rustdoc warnings on
  a non-test build.

[Unreleased]: https://github.com/tegmentum/comparand/compare/HEAD...HEAD
[0.1.0-alpha]: https://github.com/tegmentum/comparand/commits/main
