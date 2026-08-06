# Changelog

All notable changes to StringCheese are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
Until the first `0.1.0` release, expect breaking changes on any minor
bump; `0.x` versions are pre-stability.

## [Unreleased]

### Added

- **`stringcheese-compare` crate.** Consolidates the nine sibling
  comparison crates (`stringcheese-levenshtein`, `stringcheese-hamming`,
  `stringcheese-jaro`, `stringcheese-damerau`, `stringcheese-lcs`,
  `stringcheese-ngram`, `stringcheese-search`,
  `stringcheese-set-similarity`, and `stringcheese-minhash`) into one
  crate with a top-level module per family (`levenshtein`, `hamming`,
  `jaro`, `damerau`, `lcs`, `ngram`, `search`, `set_similarity`,
  `minhash`). Every load-bearing type each source crate re-exported
  at its own root is also re-exported at `stringcheese-compare`'s root,
  so `use stringcheese_compare::Levenshtein` and
  `use stringcheese_compare::levenshtein::Levenshtein` both resolve.
  Public API is preserved.

- **`stringcheese-manip` crate.** New sub-project — the manipulation
  half of the StringCheese charter. Scaffold only in v0.1: every
  module (`inspect`, `trim`, `case`, `split`, `join`, `replace`,
  `normalize`, `pad`, `slice`, `find`, `escape`, `quote`, `lines`,
  `template`, `pipeline`) is declared with module-level docs
  describing its scope, but no items ship yet. Depending on
  `stringcheese-manip` today is safe — items will only be added at
  this pre-1.0 stage, never removed. Also re-exported from the
  facade as `stringcheese::manip`.

- **`docs/DESIGN.md` Charter/Scope/Architecture rewrite.** The Vision,
  Scope, and Architecture sections now reflect the umbrella charter
  (string processing, not just comparison) — sub-project map, the
  planned language packs (`stringcheese-<language>`), and the planned
  WIT-based i18n direction (SCUD data packs) are documented. Record
  linkage stays a sibling library (planned rename to
  `stringcheese-linkage`); the substantive scope split is unchanged.

### Changed

- **Import paths.** `use stringcheese_<family>::X` becomes
  `use stringcheese_compare::<family>::X` (with `set-similarity`
  spelled `set_similarity` on the Rust side). The umbrella `stringcheese`
  facade re-exports the same nine module names as before, so
  `use stringcheese::levenshtein::Levenshtein` keeps working unchanged.

- **Project renamed from Comparand to StringCheese.** Every crate is renamed
  from `comparand-*` to `stringcheese-*`; the umbrella facade is `stringcheese`
  (was `comparand`); the WIT package is `stringcheese:core` (was `comparand:core`)
  and the interface file is `component/wit/stringcheese.wit`; the produced
  component binary is `stringcheese_component_host.wasm`. The `Comparand` name
  is retired — a comparison sub-project stays under the StringCheese umbrella
  as `stringcheese-compare` in a follow-up wave. Repository is
  `https://github.com/tegmentum/stringcheese`. Maintainer email is
  `zachary.whitley@tegmentum.ai`.
- The `DifferenceClassification::ComparandDefect` variant is now
  `DifferenceClassification::StringCheeseDefect`. Callers that named the
  variant by path in match arms must update.

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
  (`stringcheese-core`, `stringcheese-corpus`, `stringcheese`, and the
  placeholder crates `stringcheese-unicode`, `stringcheese-phonetic`,
  `stringcheese-search`, `stringcheese-cdc`, `stringcheese-index`,
  `stringcheese-bench`).
- `stringcheese-corpus` oracle framework, exhaustive generators, and
  differential vocabulary.

#### Algorithm crates

- **Edit distance and similarity.** `stringcheese-levenshtein` (full-matrix
  oracle, rolling-rows production kernel, Ukkonen-style banded cutoff),
  `stringcheese-hamming` (metric distance for equal-length sequences),
  `stringcheese-jaro` (Jaro and Jaro-Winkler similarity family),
  `stringcheese-damerau` (Optimal String Alignment and full
  Damerau-Levenshtein), and `stringcheese-lcs` (Longest Common Subsequence
  and LCS distance).
- **N-gram and set similarity.** `stringcheese-ngram` (character, byte,
  and token n-gram representations) and `stringcheese-set-similarity`
  (Dice, Jaccard, Overlap, Cosine over n-gram representations).
- **Alignment.** `stringcheese-align`: Needleman-Wunsch global alignment
  and Smith-Waterman local alignment with linear and affine gap
  penalties.
- **Phonetic.** `stringcheese-phonetic`: Soundex, NYSIIS, and the
  single-key Double Metaphone encoder, followed by the full two-key
  Double Metaphone variant.
- **Unicode preprocessing.** `stringcheese-unicode`: NFC/NFD/NFKC/NFKD
  normalization, Unicode case folding (via `icu_casemap`, including
  multi-character expansions), grapheme-cluster segmentation, and
  diacritic stripping.
- **Substring search.** `stringcheese-search`: Rabin-Karp, KMP, Boyer-Moore
  (bad-character), Aho-Corasick, followed by Horspool, Two-way, the
  full Boyer-Moore with good-suffix rule, and streaming wrappers over
  the single-pattern algorithms.
- **Content-defined chunking.** `stringcheese-cdc`: rolling-hash
  fingerprints (Rabin, polynomial, Gear) and FastCDC chunking.
- **Indexes.** `stringcheese-index`: BK-tree, VP-tree (with bulk build and
  sorted-neighborhood blocking added in a follow-up), and a q-gram
  inverted index for large-scale nearest-neighbor and blocking queries.
- **MinHash / LSH.** `stringcheese-minhash`: MinHash sketches and LSH
  banding for approximate Jaccard-similarity search at scale.

#### WebAssembly Component Model

- `component/`: WIT interface definition and a Rust host demonstrating
  StringCheese consumed as a Component-Model component, plus a reference
  guest and matching integration tests.

#### Benchmarks, fuzzing, and cross-comparison

- `stringcheese-bench`: criterion benchmark suite over the algorithm
  surface (Levenshtein, Hamming, Jaro, Damerau, n-gram, batch).
- Allocation-counting harness in `stringcheese-bench` backed by `dhat-rs`,
  gated behind the opt-in `alloc-tracking` feature so a default
  `cargo bench` never inherits the profiler's global allocator.
- `bench-adapters/`: Rust head-to-head adapters against `strsim` and
  `rapidfuzz` so criterion runs report StringCheese alongside the
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
  StringCheese supports.
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
  sibling library. StringCheese stays focused on sequence comparison; the
  record-linkage crate depends on StringCheese rather than the other way
  around. See the sibling repository for the extracted code and its
  own history.

### Fixed

- `stringcheese-damerau`: dropped intra-doc links that pointed at the
  test-only `property_tests` module, which produced rustdoc warnings on
  a non-test build.

[Unreleased]: https://github.com/tegmentum/stringcheese/compare/HEAD...HEAD
[0.1.0-alpha]: https://github.com/tegmentum/stringcheese/commits/main
