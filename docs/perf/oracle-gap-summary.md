# Oracle gap summary

Cross-crate roll-up of the `--features oracle-benches` measurements
taken on `stringcheese-compare`, `stringcheese-align`,
`stringcheese-cdc`, and `stringcheese-phonetic`. Answers the question
the perf audit posed:

> Are stringcheese's baselines already competitive, or is there real
> headroom vs best-in-class Rust libraries?

For per-kernel raw numbers see:

* `docs/perf/compare-oracle-gaps.md` (Levenshtein, OSA, Damerau,
  Hamming, Jaro, Jaro-Winkler)
* `docs/perf/align-oracle-gaps.md` (NW / SW × linear / affine ×
  score / align)
* `docs/perf/cdc-oracle-gaps.md` (FastCDC 8k/16k, GEAR roll,
  GEAR SIMD digest)
* `docs/perf/phonetic-oracle-gaps.md` (Soundex, NYSIIS, primary
  Double Metaphone, full Double Metaphone)

## Oracles used

| crate                    | oracle          | version | why                                                  | dep footprint            |
| :----------------------- | :-------------- | :------ | :--------------------------------------------------- | :----------------------- |
| `stringcheese-compare`   | `strsim`        | 0.11    | scalar pure-Rust reference for every distance metric | 0 transitive deps        |
| `stringcheese-compare`   | `triple_accel`  | 0.4     | SIMD-accelerated Levenshtein + Hamming (AVX2/NEON)   | 0 transitive deps        |
| `stringcheese-align`     | `bio`           | 2.3     | canonical Rust exact NW/SW (Gotoh three-matrix DP)   | **~117 packages**        |
| `stringcheese-cdc`       | `fastcdc`       | 4.0     | canonical Rust FastCDC v2020 (Fiedler)               | 1 transitive dep (cfg-if) |
| `stringcheese-cdc`       | `gearhash`      | 0.1     | dedicated GEAR rolling hash (scalar + AVX2/SSE4.2)   | 1 transitive dep (cfg-if) |
| `stringcheese-phonetic`  | `rphonetic`     | 3.0     | Rust port of Apache commons-codec phonetic family    | 6 new lock entries       |

The `bio` footprint is the one dep-footprint red flag in this round.
The `oracle-benches` feature is off by default and only compile-
checked (not run) in CI, so a plain `cargo test --workspace` never
pulls it in. Any CI job that enables `--all-features` will pay a
`~15-30 s` `bio` compile hit; documented in
`crates/stringcheese-align/src/lib.rs` and `Cargo.toml`.

Alternative oracles considered and rejected for align:

* `parasail-rs` — FFI + libclang / bindgen at build time. Would
  contaminate CI with a C-compiler system dep.
* `block-aligner` — pure Rust with SIMD, but banded / adaptive; does
  not compute the same score as exact NW/SW unless the band spans the
  whole DP. Asymmetric correctness surface makes the throughput
  comparison suspect.

Alternative oracles considered and rejected for cdc:

* `fastcdc-alt` — API-alternative fork of `fastcdc`; no algorithmic
  coverage over the upstream crate. Duplicate.
* `chunkrs` / `mothcdc` / `cavs-chunker` — newer / less-established
  CDC crates. Rejected in favour of the widely-adopted `fastcdc-rs`
  reference to keep the "canonical Rust ecosystem" bar.
* `rabin-fingerprint-rs` / other pure-Rust Rabin fingerprint crates —
  none maintained at reasonable-maturity on crates.io. `Buzhash`
  similarly has no maintained pure-Rust standalone crate. Those three
  fingerprint families therefore have no oracle row; their baselines
  continue to live under the standalone baseline table in
  `crates/stringcheese-cdc/benches/cdc.rs`'s module doc.

Alternative oracles considered and rejected for phonetic:

* `metaphone` (0.1) — Metaphone-only crate. Would add ~2 additional
  `Cargo.lock` entries (`aho-corasick`, `itertools`) on top of
  `rphonetic` for zero incremental algorithm coverage. Rejected as
  duplicate.
* `soundex` / `soundex-rs` — Soundex-only. Same rejection as
  `metaphone`; `rphonetic` covers Soundex already.
* `goldenphonetic-core`, `phonetics`, `amt-phonetic` — newer /
  less-established. Rejected in favour of the widely-adopted
  Apache-commons-codec-derived `rphonetic` to hold the "canonical
  Rust ecosystem" bar.
* Slavic Metaphone has no Rust oracle at all — the algorithm is
  stringcheese-specific across ru / uk / be / bg / mk / sr / hr /
  bs / sl / pl / cs / sk on Cyrillic and Latin. Baseline continues
  to live under `crates/stringcheese-phonetic/src/lib.rs`'s `//!`
  docs table; regression tripwire is preserved.

## Verdict per crate × kernel

Verdict thresholds (task-defined):

* **competitive** — < 2× gap either way; no perf work needed
* **medium gap** — 2-5×, optimisation worth considering
* **large gap** — > 5×, clear perf lever
* **stringcheese ahead** — stringcheese is faster than the oracle

### `stringcheese-compare`

| kernel        | size | oracle       | mult   | verdict                       |
| :------------ | ---: | :----------- | -----: | :---------------------------- |
| levenshtein   |   32 | strsim       |  0.74× | stringcheese 1.3× ahead       |
| levenshtein   |  256 | strsim       |  1.20× | competitive (strsim ahead)    |
| levenshtein   | 2048 | strsim       |  1.24× | competitive (strsim ahead)    |
| levenshtein   |   32 | triple_accel |  0.26× | stringcheese 3.9× ahead       |
| levenshtein   |  256 | triple_accel |  0.35× | stringcheese 2.9× ahead       |
| levenshtein   | 2048 | triple_accel |  0.36× | stringcheese 2.8× ahead       |
| osa           |   32 | strsim       |  1.58× | competitive                   |
| osa           |  256 | strsim       |  1.81× | competitive                   |
| osa           | 2048 | strsim       |  1.82× | competitive                   |
| damerau       |   32 | strsim       |  1.96× | competitive                   |
| damerau       |  256 | strsim       |  1.83× | competitive                   |
| damerau       | 2048 | strsim       |  1.92× | competitive                   |
| hamming       |   32 | strsim       |  0.30× | competitive (stringcheese 3× ahead) |
| hamming       |  256 | strsim       |  0.17× | stringcheese 5.7× ahead       |
| hamming       | 2048 | strsim       |  0.17× | stringcheese 5.8× ahead       |
| hamming       |   32 | triple_accel |  2.58× | **medium gap** (triple_accel ahead) |
| hamming       |  256 | triple_accel |  1.93× | competitive                   |
| hamming       | 2048 | triple_accel |  2.08× | **medium gap** (triple_accel ahead) |
| jaro          |   32 | strsim       |  0.39× | stringcheese 2.6× ahead       |
| jaro          |  256 | strsim       |  0.45× | stringcheese 2.2× ahead       |
| jaro          | 2048 | strsim       |  0.36× | stringcheese 2.8× ahead       |
| jaro_winkler  |   32 | strsim       |  0.53× | stringcheese 1.9× ahead       |
| jaro_winkler  |  256 | strsim       |  0.52× | stringcheese 1.9× ahead       |
| jaro_winkler  | 2048 | strsim       |  0.39× | stringcheese 2.6× ahead       |

### `stringcheese-cdc`

Post wave-15 batch rewrite; the pre-rewrite table is preserved under
`docs/perf/cdc-oracle-gaps.md` for regression comparison.

| kernel                       | size  | flavor | oracle       | mult   | verdict                             |
| :--------------------------- | ----: | :----- | :----------- | -----: | :---------------------------------- |
| fastcdc/default_8k           |  1MiB | random | fastcdc      |  0.89× | stringcheese 1.13× ahead            |
| fastcdc/default_8k           |  1MiB | prose  | fastcdc      |  0.89× | stringcheese 1.13× ahead            |
| fastcdc/default_8k           | 10MiB | random | fastcdc      |  0.87× | stringcheese 1.14× ahead            |
| fastcdc/default_8k           | 10MiB | prose  | fastcdc      |  0.89× | stringcheese 1.13× ahead            |
| fastcdc/default_16k          |  1MiB | random | fastcdc      |  0.88× | stringcheese 1.13× ahead            |
| fastcdc/default_16k          |  1MiB | prose  | fastcdc      |  0.89× | stringcheese 1.13× ahead            |
| fastcdc/default_16k          | 10MiB | random | fastcdc      |  0.88× | stringcheese 1.14× ahead            |
| fastcdc/default_16k          | 10MiB | prose  | fastcdc      |  0.88× | stringcheese 1.14× ahead            |
| gear/roll_per_byte           |  1MiB | random | gearhash     |  1.04× | competitive (tied)                  |
| gear/roll_per_byte           | 10MiB | random | gearhash     |  0.85× | competitive (stringcheese 1.2× ahead) |
| gear/roll_per_byte           | 10MiB | prose  | gearhash     |  1.01× | competitive (tied)                  |
| gear/next_match (SIMD lane)  |  1MiB | random | gearhash     |  0.58× | stringcheese 1.7× ahead             |
| gear/next_match (SIMD lane)  |  1MiB | prose  | gearhash     |  0.49× | stringcheese 2.1× ahead             |
| gear/next_match (SIMD lane)  | 10MiB | random | gearhash     |  0.46× | stringcheese 2.2× ahead             |
| gear/next_match (SIMD lane)  | 10MiB | prose  | gearhash     |  0.91× | competitive (stringcheese slightly ahead) |

Notes:

* The GEAR SIMD lane on the M-series host compares stringcheese's
  NEON `digest_of_slice` against gearhash's aarch64 scalar
  `next_match` — upstream `gearhash` has no NEON backend, so this is
  vectorised bytes vs a scalar shift-add loop. Re-run on x86_64 for a
  true SIMD-vs-SIMD head-to-head.
* `Buzhash`, `PolynomialHash`, `RabinFingerprint` have no oracle rows
  (no maintained pure-Rust crate at the maturity bar).

### `stringcheese-phonetic`

Fair-comparison (same-work) rows only. rphonetic's Soundex runs an
upfront full-input `soundex_clean` pass and its Double Metaphone
default caps output at 4 chars while stringcheese's does not; see
`docs/perf/phonetic-oracle-gaps.md` for the methodology note and the
`rphonetic-cap4` early-exit rows.

| kernel                         | size | oracle          | mult    | verdict                              |
| :----------------------------- | ---: | :-------------- | ------: | :----------------------------------- |
| soundex                        |   32 | rphonetic       |  0.022× | stringcheese 45× ahead               |
| soundex                        |  256 | rphonetic       |  0.006× | stringcheese 175× ahead              |
| soundex                        | 2048 | rphonetic       |  0.0003× | stringcheese 3000× ahead            |
| nysiis                         |   32 | rphonetic       |  0.104× | stringcheese 9.6× ahead              |
| nysiis                         |  256 | rphonetic       |  0.097× | stringcheese 10.3× ahead             |
| nysiis                         | 2048 | rphonetic       |  0.078× | stringcheese 12.8× ahead             |
| double_metaphone_primary       |   32 | rphonetic-uncapped |  0.113× | stringcheese 8.8× ahead           |
| double_metaphone_primary       |  256 | rphonetic-uncapped |  0.045× | stringcheese 22× ahead            |
| double_metaphone_primary       | 2048 | rphonetic-uncapped |  0.032× | stringcheese 31× ahead            |
| double_metaphone_full          |   32 | rphonetic-uncapped |  0.204× | stringcheese 4.9× ahead           |
| double_metaphone_full          |  256 | rphonetic-uncapped |  0.082× | stringcheese 12× ahead            |
| double_metaphone_full          | 2048 | rphonetic-uncapped |  0.042× | stringcheese 24× ahead            |
| double_metaphone_full          |   32 | rphonetic-cap4  |  2.54×  | **feature-parity gap** (cap4 wins on 4-char early exit) |

Read: stringcheese is 5-3000× ahead on every fair (same-work)
oracle row. The one row where rphonetic leads (`double_metaphone_full`
at n = 32 with `rphonetic-cap4`) is a feature-parity gap — rphonetic
early-exits after emitting 4 characters. Adding an optional
`max_code_length` to `DoubleMetaphoneVariant` would close it
mechanically; see the perf-target section below.

### `stringcheese-align`

Bio is 2-5× slower everywhere. Sample rows (divergent, worst-case DP
fill; matching flavor is within noise of divergent for all rows —
see `docs/perf/align-oracle-gaps.md` for the full table):

| kernel            | size | mult (bio÷sc) | verdict                       |
| :---------------- | ---: | ------------: | :---------------------------- |
| nw_score_linear   |   32 |         0.22× | stringcheese 4.5× ahead       |
| nw_score_linear   |  128 |         0.28× | stringcheese 3.6× ahead       |
| nw_score_linear   |  512 |         0.27× | stringcheese 3.6× ahead       |
| nw_score_affine   |   32 |         0.35× | stringcheese 2.9× ahead       |
| nw_score_affine   |  128 |         0.51× | stringcheese 2.0× ahead       |
| nw_score_affine   |  512 |         0.31× | stringcheese 3.3× ahead       |
| nw_align_linear   |   32 |         0.29× | stringcheese 3.5× ahead       |
| nw_align_linear   |  512 |         0.25× | stringcheese 4.0× ahead       |
| sw_score_linear   |  512 |         0.45× | stringcheese 2.2× ahead       |
| sw_score_affine   |  512 |         0.36× | stringcheese 2.8× ahead       |
| sw_align_linear   |  512 |         0.42× | stringcheese 2.4× ahead       |

## Recommended next perf targets

Ranked by `(gap size × input-size-that-matters × implementation-cost)`.
Only rows where an oracle is *ahead* of stringcheese are candidates.

### 1. `compare/levenshtein` at n ≥ 256 — vs `strsim` (closed to ~1.2×)

* **Status**: closed. The rolling-rows kernel was rewritten to hoist
  the outer symbol and the "carry" cell `d[i][j-1]` into scalar locals
  and to walk `row[1..=n]` and the inner sequence in lockstep via a
  `zip` iterator; that unblocked LLVM's autovectoriser and moved
  throughput from 2.8× / 3.3× behind `strsim` at n = 256 / 2048 to
  within ~1.2× at both sizes (below the 2× "medium gap" threshold).
  Bench code and inputs are unchanged; the win is entirely in the
  inner-loop shape.
* **Follow-on**: the residual ~1.2× is the last bit of scalar
  headroom. The next jump is a Myers 1999 bit-parallel Levenshtein
  (the crate has a `simd` feature scaffold; a dedicated Myers kernel
  is already the documented target). That is where the 5-10× SIMD
  headroom lives — but `triple_accel`'s `levenshtein_exp` shows that
  SIMD alone is not enough: the exponential-doubling wrapper can wipe
  out the SIMD win, and the bit-parallel Myers kernel needs to be
  dispatched unconditionally at unbounded edit distance to actually
  win.

### 2. `compare/hamming` — vs `triple_accel` (2× gap, memory-bandwidth-bound)

* **Gap**: 2× at n = 32, ~2× at n ≥ 256 (memory-bandwidth-bound
  territory, 5.8 GiB/s stringcheese vs 12 GiB/s `triple_accel`).
* **What's likely wrong**: stringcheese's block-compare uses `u64`
  chunks; `triple_accel` uses NEON 128-bit lanes.
* **Estimated cost**: medium. Would need a per-arch SIMD dispatcher
  along the same lines as the crate's optional `simd` feature for
  Levenshtein. The absolute wins are already at multi-GiB/s so a real
  workload rarely bottlenecks on Hamming.
* **Priority**: lower than Levenshtein — the absolute throughput is
  already high enough that this is unlikely to be a real workload
  hotspot.

### 3. `cdc/fastcdc/{default_8k, default_16k}` — vs `fastcdc-rs` (closed)

* **Status**: **closed** in wave 15. The batch
  `FastCdc::chunk_boundaries` iterator was rewritten to drive a
  two-byte-per-iteration `next_cut` inner loop backed by a pre-
  shifted gear table (`GEAR_TABLE_LS1` on
  `crate::fingerprint::gear`). The two-byte body folds each pair of
  gear-hash updates into `(hash << 2) + GEAR_LS1[a] + GEAR_TABLE[b]`
  — one shift + two adds per byte pair vs the old two shifts + two
  adds — which shortens the loop-carried dependency chain and lifts
  the FastCDC end-to-end throughput from ~1.1-1.4 GiB/s to
  ~2.0-2.5 GiB/s across every measured cell (1.8-2.1× local
  speed-up). Post-rewrite stringcheese sits at 1.13-1.14× *ahead*
  of `fastcdc-rs` on every one of the eight measurement cells; the
  medium-gap row (1 MiB / random / default_16k, previously 2.15×
  behind) closes to 0.88× (stringcheese ahead).
* **Correctness**: preserved via three layers of pinning — the
  existing streaming-vs-contiguous property tests, a new in-crate
  differential test (`batch_matches_byte_at_a_time_over_pseudorandom_input`)
  that runs 1 MiB of pseudorandom bytes through both the new batch
  path and the unchanged `FastCdcStream::feed` state machine and
  asserts byte-identical boundary lists at both paper configs, and
  a fallback branch in `next_cut` that dispatches to a scalar
  reference implementation for the pathological `mask & (1 << 63)
  != 0` configs the two-byte identity would not cover.
* **Follow-on**: SIMD would push absolute throughput past ~3 GiB/s
  but is deliberately deferred — the scalar rewrite already puts
  stringcheese ahead of the best pure-Rust oracle, so the
  cost/benefit shifts to Levenshtein-style SIMD work.

### 4. `compare/{osa, damerau}` — vs `strsim` (~1.8-2× gap)

* **Gap**: below the 2× "medium gap" threshold — competitive.
* **Priority**: not worth a targeted round. Both would benefit
  incidentally if the Levenshtein DP-loop cleanup (item 1) generalises
  to the OSA / Damerau kernels (same rolling-rows shape).

### 5. `phonetic/double_metaphone_full` at n = 32 — vs `rphonetic-cap4` (2.54×)

* **Gap**: `rphonetic-cap4` is 2.54× ahead at n = 32 only (130 MiB/s
  vs 51 MiB/s). Under the fair (uncapped) comparison stringcheese
  is 4.9× *ahead* at the same size, so the number is entirely the
  "capped rphonetic terminates early on short input" effect — not
  an algorithmic win.
* **What's likely wrong**: nothing on the perf side. This is a
  *feature-parity* gap: stringcheese's `DoubleMetaphoneVariant`
  emits uncapped keys, rphonetic's default emits a 4-char-capped
  key. Adding an optional `max_code_length` to
  `stringcheese-phonetic::DoubleMetaphoneVariant` would close it
  mechanically.
* **Estimated cost**: small. Convenience-only; callers who need a
  4-char cap today can post-truncate the returned key.
* **Priority**: low. Included only because it is the single row in
  the entire phonetic oracle round where any rphonetic column ever
  leads stringcheese.

### Not on the list

* **All `stringcheese-align` kernels**: stringcheese is 2-5× *ahead*
  of `bio`. Any further headroom is SIMD-ceiling / parasail-C
  territory — not scalar-DP work.
* **`compare/jaro`, `compare/jaro_winkler`**: stringcheese is 2-3×
  *ahead* of `strsim`. No action needed.
* **`cdc/fingerprint/gear` (per-byte roll + SIMD digest)**: tied with
  `gearhash` on the scalar row, 1.7-2.2× *ahead* on the SIMD row
  (aarch64 dispatch quirk favours stringcheese — see the caveat in
  `docs/perf/cdc-oracle-gaps.md`). No action needed.
* **`cdc/fingerprint/{buzhash, polynomial, rabin}`**: no maintained
  pure-Rust oracle at the maturity bar. Baseline table under the
  bench file's module doc remains the regression tripwire.
* **`compare/{search, minhash, ngram, lcs, learned}`**: no comparable
  scalar Rust oracle in the ecosystem worth wiring up right now.
* **All `stringcheese-phonetic` fair (same-work) rows**: stringcheese
  is 5-3000× *ahead* of `rphonetic` on every one — Soundex, NYSIIS,
  primary Double Metaphone (uncapped), and full Double Metaphone
  (uncapped). No perf lever.
* **`phonetic/slavic_metaphone`**: no comparable Rust oracle
  (stringcheese-specific multilingual arm). Baseline table under
  the crate's `src/lib.rs` `//!` docs remains the regression
  tripwire.

## Actionable takeaway

The one clear scalar perf lever surfaced by the initial measurement
round — `compare/levenshtein` at n ≥ 256 — has been **closed** by the
rolling-rows autovectorisation pass; stringcheese is now within ~1.2×
of `strsim` at every size (competitive by the < 2× threshold), and
1.3× *ahead* of `strsim` at n = 32.

The CDC oracle round's scalar perf lever —
**`cdc/fastcdc` at both presets** — has also been **closed** in
wave 15 by the two-byte-per-iteration + pre-shifted-gear-table
rewrite of `FastCdc::chunk_boundaries`: post-rewrite stringcheese
is 1.13-1.14× *ahead* of `fastcdc-rs` at every one of the eight
measured cells, and the previously 2.15× medium-gap row (1 MiB /
random / default_16k) closes to 0.88×.

Everything else is either already best-in-class (align, jaro,
jaro_winkler, hamming vs strsim, gear roll vs gearhash, gear SIMD
digest vs gearhash on aarch64, FastCDC vs fastcdc-rs, and every
`stringcheese-phonetic` fair-comparison row vs rphonetic — 5-3000×
ahead), sub-2× (osa, damerau), or SIMD-territory (hamming vs
triple_accel; Levenshtein bit-parallel Myers).

The phonetic oracle round surfaced one feature-parity item — an
optional output-length cap on `DoubleMetaphoneVariant` — that would
close a stylistic "off-the-shelf default" gap against
`rphonetic::DoubleMetaphone::default()` at n = 32. Convenience-only;
not a perf lever.
