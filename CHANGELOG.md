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

Retroactive entry documenting the substrate that has landed on `main`
since project inception. No published release has been cut yet.

### Added

- Initial workspace and type-system substrate: result types, metric
  traits, mathematical-property descriptors, algorithm-variant registry,
  workspace and sequence traits, and the golden-case validation schema
  (`comparand-core`, `comparand-corpus`, `comparand`, and the
  placeholder crates `comparand-unicode`, `comparand-phonetic`,
  `comparand-search`, `comparand-cdc`, `comparand-index`,
  `comparand-bench`).
- `comparand-corpus` oracle framework, exhaustive generators, and
  differential vocabulary.
- `comparand-levenshtein`: unit-cost Levenshtein edit distance, first
  algorithm end-to-end — full-matrix oracle, rolling-rows production
  kernel, and Ukkonen-style banded cutoff variant.
- `comparand-hamming`: metric distance for equal-length sequences.
- `comparand-jaro`: Jaro and Jaro-Winkler similarity family.

### Added (docs)

- `docs/DESIGN.md`: full project vision, algorithm coverage, validation
  strategy, CI requirements, and release gates.
- Five subordinate design documents under `docs/design/` covering the
  detailed design of the substrate and first-wave algorithms.

[Unreleased]: https://github.com/zacharywhitley/comparand/compare/HEAD...HEAD
[0.1.0-alpha]: https://github.com/zacharywhitley/comparand/commits/main
