# Changelog

All notable changes to StringCheese are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
Until the first `0.1.0` release, expect breaking changes on any minor
bump; `0.x` versions are pre-stability.

## [Unreleased]

### Added

- **`stringcheese-manip`: `pipeline` module ships — all 15 modules real.**
  `TextPipeline` stages `Operation` trait objects into an ordered
  transformation IR that applies each in one pass over a ping-pong
  buffer pair (two heap allocations regardless of stage count;
  `apply_into` writes the final stage direct into the caller's
  buffer with no post-hoc copy). Concrete operations wrap the shipping
  modules — `Trim`, `Normalize`, `CaseFold`, `CollapseWhitespace`,
  `Remove`, `Replace`, `Escape`, `Truncate`. Operations expose
  `name()` for introspection; budget-limited ops (`Truncate`)
  short-circuit. `Truncate` is byte-budget with scalar-aligned cut
  (never splits a UTF-8 scalar). Adds 59 unit + 8 property + 15
  doctests.

- **`stringcheese-de` — German language pack.** New workspace crate.
  ~245 stopwords (drawn from Snowball's German stoplist), Snowball
  German stemmer (full 6-step spec: R1/R2 regions, u/i-between-two-vowels
  isolation, standard and rare-suffix cascades, undouble, un-accent),
  Kölner Phonetik encoder (Postel 1969) with Wikipedia's H-in-next-letter
  interpretation for the C rule. Public `GERMAN` constant. Snowball
  cross-verified against 46 hand-traced reference pairs; Kölner Phonetik
  against 15 well-known German surnames. `Freiheit → freiheit` (stem
  unchanged because R2 is past `-heit`) is explicitly tested — matches
  the spec, not a bug. Compound-noun splitting deferred (needs a
  dictionary).

- **`stringcheese-fr` — French language pack.** New workspace crate.
  246 stopwords (both apostrophe-suffixed and stripped clitic forms),
  Snowball French stemmer (full 6-step spec: R1/R2/RV with the
  `par`/`col`/`tap` exception, 15 rule groups with cascading
  precede-by rules, verb suffix passes 2a/2b with ment-family
  dispatch, script/residual cleanup, undouble, un-accent), PHONEX
  phonetic encoder (Soundex-shaped 4-character key with French-tuned
  preprocessing — accent folding, `PH → F`, `GN → N`, `CH → X`,
  `QU → K`, `Y → I`, `W → V`, `Ç → S`), elision-aware tokenizer
  (splits `l'`, `d'`, `qu'`, `jusqu'`, `lorsqu'`, `puisqu'`,
  `quoiqu'` case-insensitively; preserves `aujourd'hui` as one token;
  handles ASCII `'` and typographic `\u{2019}`). Public `FRENCH`
  constant. Snowball cross-verified against 46 pairs; PHONEX against
  22 pairs. Snowball French intentionally not universally idempotent
  (per spec, `dangereux → danger → dang`); property tests verify
  convergence in ≤5 iterations. Full-corpus verification, Métaphone
  Français alternative, soft-C/soft-G detection deferred.

- **`stringcheese-en`: Porter2 (Snowball 2001) stemmer companion.**
  Adds the revised Porter2 stemmer as `PORTER2_STEMMER` alongside the
  existing `PORTER_STEMMER` (Porter 1980). Both remain available.
  Public `ENGLISH_PORTER2` constant; new `English::with_porter()` and
  `English::with_porter2()` const constructors. The `ENGLISH`
  constant continues to use Porter 1980 for backwards compatibility.
  Full 5+ step spec with exception table (`skies → ski`, `sky → sky`,
  `dying → die`, `lying → lie`, `tying → tie`, `vying → vye`,
  `idly`, `gently`, `ugly`, `early`, `only`, `singly`, `news`,
  `atlas`, `cosmos`, `bias`, `andes`, `inning`, `outing`, `canning`,
  `herring`, `earring`, `evening`, `proceed`, `exceed`, `succeed`).
  R1/R2 markers, special prefix handling (`gener-`, `commun-`,
  `arsen-`), Y-vowel prelude, short-syllable predicate, double
  predicate. Cross-verified against **498 Snowball reference pairs**
  from the canonical `voc.txt` / `output.txt`.

- **`stringcheese-compare`: SIMD dispatch for Jaro and OSA.** Extends
  the SIMD dispatch pattern established for Levenshtein to
  `stringcheese_compare::jaro::simd` and
  `stringcheese_compare::damerau::osa::simd`. Both provide
  `similarity_bytes_with_workspace` / `distance_bytes_with_workspace`
  entry points that check the `simd` feature + byte-amenability and
  dispatch to AVX2 / SSE2 / NEON / scalar fallback. Arch-specific
  backends currently delegate to the scalar SIMD-shape for
  correctness scaffolding; wide-block true vector intrinsics are
  documented follow-up work. Full unrestricted Damerau-Levenshtein
  stays scalar (its HashMap-backed algorithm doesn't fit the Myers
  pattern). Hoisted `is_byte_amenable` into a shared
  `simd_dispatch` module used by all three SIMD sub-trees.
  Differential and property tests confirm bit-for-bit agreement with
  the existing scalar kernels; a proptest-caught bug in the Jaro
  SIMD scan's slice bounds (start > len_b when i > len_b + window)
  was fixed and pinned as a regression.

- **Python bench adapter.** `bench-adapters/python/` — pytest-benchmark
  head-to-head comparing StringCheese (loaded via `wasmtime-py` as the
  component-model `.wasm`) against `python-Levenshtein`, `jellyfish`,
  `rapidfuzz`. Establishes the non-Rust adapter pattern for future
  language adapters. wasmtime-py 41.0.0's `wasmtime.component` surface
  handles the nested exports and `result<T, E>` returns cleanly.
  Verified end-to-end with an actual benchmark run — StringCheese-via-
  wasm shows the expected FFI overhead vs native C-extension libraries
  for short strings. `damerau_distance` skipped because the full
  Damerau kernel isn't yet exposed at the WIT boundary (needs a
  wasm-portable hash story).

- **`docs/design/tokenizers.md` — tokenizer subsystem design.** New
  ~5,800-word design doc covering the tokenizer subsystem:
  `Tokenizer` vs `Segmenter` trait taxonomy with GAT-based signatures,
  three-tier crate layout (`stringcheese-tokenizer` for abstractions +
  built-ins; `-bpe` / `-wordpiece` / `-sentencepiece` for algorithms;
  `-tiktoken` / `-huggingface` for pre-configured models), SCUD
  extension for BPE data packs, ~65-line WIT interface with
  offset-preserving `encoding` record and configurable
  `special-policy`, integration sketches for `compare` / `cdc` /
  `manip` / `lang` / `index`, seven-phase implementation plan.
  Nine open questions flagged (trait residency, concurrent-tokenizer
  negotiation, SCUD compression measurement, loader sharing with
  wit-i18n, default special-token policy, borrowed vs owned segmenter
  output, ...). Design only — no implementation.

### Changed

- **`stringcheese-unicode`: wasm baseline shrunk from 213 KB to 190 KB
  (11 %).** New `case-fold` feature (default on) and
  `compiled-case-data` feature (default on) let callers opt out of
  icu_casemap entirely. New `case_fold_with_mapper` /
  `simple_case_fold_with_mapper` / `case_fold_turkic_with_mapper`
  entry points accept a caller-supplied `CaseMapper` when the tables
  aren't baked in; re-exports `icu_casemap::CaseMapper` for that.
  The 40-60 % target wasn't achievable through feature-gating alone —
  LTO was already stripping most of icu_casemap when the calling
  code doesn't reach `case_fold`; the residual ~145 KB is in
  unicode-normalization's NFC / NFD / NFKC / NFKD tables (needs an
  upstream `unicode-normalization` patch to feature-gate the compat
  tables). `.wasm-size-limits.toml` re-baselined at 189567 B;
  `docs/wasm-binary-size.md` updated with the new number + twiggy
  breakdown. No downstream crate Cargo.toml changes required
  (nothing in the workspace uses `case_fold`).

- **Project renamed from Comparand to StringCheese.** Every crate is renamed
  Second wave fills in `split`, `join`, `replace`, `normalize`, `slice`,
  `find`, `pad`, `lines`, `escape`, `quote`, and `template`. Only
  `pipeline` (the transformation IR) remains a scaffold. 14 of 15
  modules now ship. Every module names the boundary each function
  works at (bytes / USVs / graphemes / display width), delegates to
  `stringcheese-unicode` for Unicode-aware work and to
  `stringcheese-compare::search` for substring-search. See the crate's
  module map for the full surface.

- **`stringcheese-lang` — language pack infrastructure.** New crate
  defining the `Language` trait (`code` / `name` / `stopwords` /
  `is_stopword` / `stem` / `tokenize` / `phonetic_encoder` /
  `collator`), plus companion traits (`Stemmer`, `Collator`,
  `LanguageProvider`) and shared helper types (`Stopwords`,
  `SimpleTokenizer`, `LanguagePhoneticEncoder`,
  `Soundex`/`Nysiis`/`DoubleMetaphone` adapters). `no_std + alloc`
  compatible. Enables opt-in per-language packs.

- **`stringcheese-en` — English language pack.** First reference
  language pack. Ships the full 5-step Porter (1980) stemmer,
  ~150 stopwords, whitespace-and-punctuation tokenizer, Soundex as
  phonetic encoder. Public `ENGLISH` constant so callers write
  `stringcheese_en::ENGLISH.stem("caresses")` without construction
  ceremony. Porter cross-verified against 65 reference pairs from
  the original paper.

- **`stringcheese-compare`: Ristad-Yianilos (1998) learned string-edit
  distance.** Memoryless stochastic transducer over source/target
  alphabets, trained from labeled pairs via EM in log space (log-sum-exp
  throughout for numerical stability). `LearnedEditModel` +
  `LearnedEdit` (implements `DistanceMetric`) + `RistadYianilosEstimator`
  (builder-pattern EM with configurable iterations and convergence
  threshold). New `AlgorithmFamily::RistadYianilos` variant in
  `stringcheese-core`. Semimetric class — the model is symmetric only
  if the trained insert/delete/substitute costs are symmetric.
  `no_std + alloc` compatible for the query surface; training is
  std-gated (needs `f64::ln`/`exp`). 33 tests total (unit + property +
  golden).

- **`stringcheese-compare::levenshtein::simd` — SIMD-dispatched Myers
  kernel.** Opt-in `simd` feature swaps the Levenshtein kernel for
  Myers (1999) bit-parallel bit-vector formulation on hosts where it
  wins. Runtime dispatch picks AVX2 → SSE2 → NEON → scalar Myers →
  falls back to rolling-rows on non-viable input (too short, or
  unicode-heavy). Scalar Myers alone delivers ~1.9-12× speedup for
  m ≤ 64; wide-block true-SIMD kernels behind the same dispatch is
  documented follow-up work. `#![forbid(unsafe_code)]` softened to
  `#![deny]` in `stringcheese-compare` with an inline exception
  comment pointing at the SIMD sub-tree (every `unsafe fn` / block
  in the sub-tree carries a `SAFETY:` comment naming its CPU-feature
  precondition). 20 SIMD-specific tests (differential vs oracle +
  arch wrapper agreement).

- **`docs/design/wit-i18n.md` — SCUD + WIT i18n design doc.** ~5,300
  word design for the umbrella's ICU-alternative direction. Covers
  the six capability WIT interfaces (case / collation / plural /
  number / datetime / break) with a ~80-line illustrative WIT for
  `stringcheese-icu-case`; the SCUD compressed data-pack binary
  format (seven compression primitives — RangeDelta, AdaptivePages,
  PackedIntegers, SequencePool, StringPool, LoudsTrie,
  FiniteStateTable — plus outer Brotli/Zstd; loader API sketch);
  runtime discovery / fallback / composition / versioning; language
  pack integration; CLDR licensing threat model; six-phase
  implementation plan. Design only — no implementation touched.

- **Wasm binary-size CI gate.** New `wasm-size` GitHub Actions job
  runs on every PR: builds each crate's minimal-surface release
  wasm through a shared `wasm-size-probes` cdylib wrapper, runs
  `wasm-opt -Oz`, compares against per-crate thresholds in
  `.wasm-size-limits.toml` (default ±5 %, ±20 % for
  `stringcheese-core` whose 724 B baseline sits inside wasm-opt
  noise). `scripts/measure-wasm-size.sh` is the contributor-facing
  local reproducer. Baseline documented in
  `docs/wasm-binary-size.md` with per-crate size + twiggy top-N
  breakdowns. Not addressed by this gate: `stringcheese-unicode`
  weighs 213 KB (icu_casemap + unicode-normalization data);
  documented as instrumentation-only and a future size-shrink
  opportunity.

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

- **`stringcheese-cdc`: Buzhash rolling hash.** Uzgalis (1983)
  cyclic-polynomial rolling hash with a 256-entry byte-substitution
  table (generated at compile time from a fixed `SplitMix64` seed for
  cross-target reproducibility; variant slug `splitmix64-seed-buzz`
  pins the table). Implements the `RollingHash` trait alongside the
  existing Rabin, polynomial, and Gear implementations. New public
  types: `stringcheese_cdc::Buzhash` and
  `stringcheese_cdc::fingerprint::buzhash::{Buzhash, BUZ_TABLE}`.
  Windows larger than 64 bytes are supported by folding the eviction
  rotate through `window mod 64` (unlike Gear's 64-byte natural
  horizon). Adds 4 golden cases + 12 unit tests + 3 property tests
  (17 new tests).

- **`stringcheese-manip`: `inspect`, `trim`, and `case` modules.**
  First three scaffold-status modules become real implementations
  (11 of the 15 modules still ship as scaffold-only stubs — the
  charter is unchanged; those land in follow-on waves).
  * `inspect` — `is_empty`, `byte_len`, `scalar_count`,
    `grapheme_count`, `first_char` / `last_char`, `first_grapheme` /
    `last_grapheme`. Every function names its boundary in its doc
    comment (byte vs USV vs grapheme); all zero-allocation.
    Grapheme-inspect functions gated on `feature = "alloc"`
    (delegating to `stringcheese-unicode` which is `alloc`-gated).
  * `trim` — `trim` / `trim_start` / `trim_end` (whitespace),
    `trim_matches` (predicate), `trim_chars` (char-set) plus
    start-only / end-only variants; and the reusable `Trim`
    configured-operation type. Zero-allocation for all trim
    functions; `Trim` value type is `alloc`-gated (holds
    `Box<dyn Fn>`).
  * `case` — `to_lowercase` / `to_uppercase` / `to_title_case` /
    `capitalize`, both owned (`-> String`) and buffer-appending
    (`*_into(&str, &mut String)`) variants, plus ASCII fast paths.
    Delegates to `stringcheese-unicode` for Unicode-aware case
    folding. Title-case buffers each word's tail into a `String`
    and calls `str::to_lowercase()` on it so Greek final-sigma is
    honored. Word boundary is "grapheme whose first scalar is
    `char::is_alphabetic()`, preceded by a non-alphabetic
    grapheme"; full UAX #29 word segmentation deferred.
  * Adds 76 unit tests + 24 property tests + 29 doctests (133 new
    tests). Extension-trait API (`s.stringcheese_trim()` style)
    deferred to a follow-up wave.

- **`stringcheese-phonetic`: Double Metaphone rule sets completed.**
  All four remaining rule families from Philips (1999) now land in
  the encoder:
  * **Slavo-Germanic modifications** — the paper's standard heuristic
    (detected by presence of `W`, `K`, `CZ`, or `WITZ` in the
    normalized name) enables three conditional rules: initial `S`
    before `L`/`M`/`N`/`W` diverges the alternate to `X`
    (*Sniepis*, *Slavik*); `CZ` anywhere emits `S` in primary, `X`
    in alternate (*Czajka*); word-final `-WITZ` has the alternate
    emit `F` and consume the whole cluster (*Rabinowitz*,
    *Horowitz*).
  * **SC-before-IEY** — `SC` (not `SCH`) followed by `I`/`E`/`Y`
    collapses to `S` in both branches (*Scientific*, *Scenic*,
    *Ascension*); `SCH` followed by `ER`/`EN`/`OO`/`UY`/`ED`/`EM`
    emits `SK` (the German consonantal reading — *Schenker*,
    *Schooner*, *Schuyler*).
  * **French silent-terminal endings** — word-final `-GN` skips the
    `G` (*Reign*, *Coign*); word-final `-MB` emits `M` and skips
    the silent `B` (*Lamb*, *Thumb*, *Coulomb*); the `-MPT-`
    cluster silences the `P` (*Compton*, *Hampton*, *Empty*,
    *Symptom*), with `-MPS` (Thompson) intentionally preserved.
  * **Surname exceptions** — chemistry `CH` at word start followed
    by `IA` / `YS` / `EM` emits `K` (*Chianti*, *Chemistry*),
    overriding the default `X`. Scots/Irish `MC` and `MAC`
    patronymic prefixes force hard `K` on their `C` even when the
    following letter would otherwise soften it (*McIver*,
    *MacBride*).

  Primary-key stability is preserved: the module's contract that the
  primary-only variant's primary key equals the full variant's
  primary key byte-for-byte for every input is honored, verified
  case-by-case against all pre-existing goldens (including surnames
  that now count as Slavo-Germanic under the new heuristic). Adds
  24 inline unit tests + 40 primary-only goldens + 10 full-variant
  goldens + 5 property tests.

- **`stringcheese-compare::minhash`: three new sibling sketches.**
  * **SimHash** (Charikar 2002) — signed random projections for
    cosine LSH. `SimHashSketch` exposes `from_iter`, `signature()`,
    `hamming_distance()`, and `estimated_cosine_similarity()`
    (std-gated). Fixed 64-bit signature; empty accumulators
    tie-break to non-negative producing `u64::MAX` so two empty
    sketches have Hamming 0 and cosine 1.0. Multiset (signed-sum)
    semantics rather than the set-invariance of regular MinHash —
    callers who want set semantics dedupe upstream.
  * **One-permutation MinHash** (Li-Owen-Zhang 2012 with
    Shrivastava-Li 2014 rotation densification) — single-permutation
    approximation with cleaner densification for empty bins.
    Bucket assignment uses Lemire top-bits multiplication
    (`((h as u128 * k as u128) >> 64) as usize`) not `hash % k` —
    top-bits preserves item-hash order, which is the condition
    Li-Owen-Zhang's unbiasedness proof requires; the low-bits
    variant fails a golden case empirically. Densification stores
    `splitmix64(source_value XOR splitmix64(hop_distance))` rather
    than a raw copy so long runs of empty bins with a shared
    source don't collapse to a single value on both sketches
    (which would inflate the estimator).
  * **p-stable LSH** (Datar-Immorlica-Indyk-Mirrokni 2004) — LSH
    families for L_p distances. `PStableLshSketch` +
    `PStableFamily::{L1, L2}`. Adds `AlgorithmFamily::PStableLsh`
    to `stringcheese-core`. Std-gated (needs `sqrt`/`ln` for
    inversion sampling — Box-Muller for Gaussian; direct inverse
    for Cauchy). Exposes `bucket() -> i64` and
    `collide_with(other) -> bool`; callers compose multiple
    sketches for LSH amplification.

  Adds 8 golden cases + 13 property tests (57 new tests total
  spanning the three sketches).

### Changed

- **`proptest` gated off wasm; wasm-runtime CI matrix expanded to the
  full workspace.** Every crate that used `proptest` in
  `[dev-dependencies]` (`stringcheese-compare`, `-align`, `-cdc`,
  `-index`, `-manip`, `-phonetic`, `-unicode`) now declares it under
  `[target.'cfg(not(target_family = "wasm"))'.dev-dependencies]`, and
  every property-test module (`mod property_tests;` or inline
  `mod properties { ... }`) is gated on the matching
  `#[cfg(not(target_family = "wasm"))]` predicate. Reason: `proptest`
  transitively depends on `wait-timeout`, which is `#[cfg(unix)]` /
  `#[cfg(windows)]` only with no wasm branch; leaving it unconditional
  broke the wasm-runtime CI job's `cargo test --target wasm32-wasip1`
  at LINK time. Host `cargo test` runs are unchanged — proptest is
  still picked up for every non-wasm target and the property tests
  continue to run.

  With the gate in place, the wasm-runtime CI job now runs
  `cargo test --workspace --exclude stringcheese-bench
  --target wasm32-wasip1` — 10 crates, was 3. Locally, 942 tests
  pass under wasmtime on `wasm32-wasip1` (host `cargo test` remains
  1,233 including the property tests). `stringcheese-bench` stays
  excluded because criterion depends on host-only timing/IO.

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
