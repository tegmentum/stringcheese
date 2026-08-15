# Oracle gap summary

Cross-crate roll-up of the `--features oracle-benches` measurements
taken on `stringcheese-compare`, `stringcheese-align`, and
`stringcheese-cdc`. Answers the question the perf audit posed:

> Are stringcheese's baselines already competitive, or is there real
> headroom vs best-in-class Rust libraries?

For per-kernel raw numbers see:

* `docs/perf/compare-oracle-gaps.md` (Levenshtein, OSA, Damerau,
  Hamming, Jaro, Jaro-Winkler)
* `docs/perf/align-oracle-gaps.md` (NW / SW × linear / affine ×
  score / align)
* `docs/perf/cdc-oracle-gaps.md` (FastCDC 8k/16k, GEAR roll,
  GEAR SIMD digest)

## Oracles used

| crate                    | oracle          | version | why                                                  | dep footprint            |
| :----------------------- | :-------------- | :------ | :--------------------------------------------------- | :----------------------- |
| `stringcheese-compare`   | `strsim`        | 0.11    | scalar pure-Rust reference for every distance metric | 0 transitive deps        |
| `stringcheese-compare`   | `triple_accel`  | 0.4     | SIMD-accelerated Levenshtein + Hamming (AVX2/NEON)   | 0 transitive deps        |
| `stringcheese-align`     | `bio`           | 2.3     | canonical Rust exact NW/SW (Gotoh three-matrix DP)   | **~117 packages**        |
| `stringcheese-cdc`       | `fastcdc`       | 4.0     | canonical Rust FastCDC v2020 (Fiedler)               | 1 transitive dep (cfg-if) |
| `stringcheese-cdc`       | `gearhash`      | 0.1     | dedicated GEAR rolling hash (scalar + AVX2/SSE4.2)   | 1 transitive dep (cfg-if) |

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

| kernel                       | size  | flavor | oracle       | mult   | verdict                             |
| :--------------------------- | ----: | :----- | :----------- | -----: | :---------------------------------- |
| fastcdc/default_8k           |  1MiB | random | fastcdc      |  1.63× | competitive (fastcdc ahead)         |
| fastcdc/default_8k           |  1MiB | prose  | fastcdc      |  1.69× | competitive (fastcdc ahead)         |
| fastcdc/default_8k           | 10MiB | random | fastcdc      |  1.79× | competitive (fastcdc ahead)         |
| fastcdc/default_8k           | 10MiB | prose  | fastcdc      |  1.38× | competitive (fastcdc ahead)         |
| fastcdc/default_16k          |  1MiB | random | fastcdc      |  2.15× | **medium gap** (fastcdc-rs ahead)   |
| fastcdc/default_16k          |  1MiB | prose  | fastcdc      |  1.56× | competitive (fastcdc ahead)         |
| fastcdc/default_16k          | 10MiB | random | fastcdc      |  1.61× | competitive (fastcdc ahead)         |
| fastcdc/default_16k          | 10MiB | prose  | fastcdc      |  1.38× | competitive (fastcdc ahead)         |
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

### 3. `cdc/fastcdc/{default_8k, default_16k}` — vs `fastcdc-rs` (~1.4-2.2× gap)

* **Gap**: 1.4-1.8× on most rows; one row (1 MiB / random /
  default_16k) crosses the 2× "medium gap" threshold at 2.15×.
* **What's likely wrong**: `fastcdc-rs` implements the paper's
  "rolling two bytes each time" optimisation (per the v2020 module
  docs, "about a 20% improvement" on Apple M1 over v2016) and holds
  pre-shifted variants of the gear table (`gear` + `gear_ls`) so a
  two-byte step costs one memory read from each. stringcheese's
  `FastCdc` state machine is a plain one-byte-per-step loop.
* **Estimated cost**: medium. Adding a two-byte-stride inner loop
  plus a pre-shifted table doubles the code path (min-size skip, one-
  byte tail, two-byte body) but is well-scoped and would land under
  the existing `FastCdc` public surface — no API break.
* **Priority**: sits between the Hamming SIMD row (item 2) and the
  OSA / Damerau incidental-cleanup rows (item 4). At ~1.6× typical
  gap and ~2 GiB/s absolute, this is a real workload lever for CDC-
  heavy pipelines (dedupe / snapshot / rsync-style deltas) — those
  jobs are IO-adjacent and the CPU side of the chunker is often the
  bottleneck.

### 4. `compare/{osa, damerau}` — vs `strsim` (~1.8-2× gap)

* **Gap**: below the 2× "medium gap" threshold — competitive.
* **Priority**: not worth a targeted round. Both would benefit
  incidentally if the Levenshtein DP-loop cleanup (item 1) generalises
  to the OSA / Damerau kernels (same rolling-rows shape).

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

## Actionable takeaway

The one clear scalar perf lever surfaced by the initial measurement
round — `compare/levenshtein` at n ≥ 256 — has been **closed** by the
rolling-rows autovectorisation pass; stringcheese is now within ~1.2×
of `strsim` at every size (competitive by the < 2× threshold), and
1.3× *ahead* of `strsim` at n = 32.

The CDC oracle round adds one new scalar perf lever:
**`cdc/fastcdc` at both presets** trails `fastcdc-rs` by ~1.4-2.2×
(one row crosses the 2× medium-gap threshold). The identified lever
is the "rolling two bytes each time" + pre-shifted-gear-table
optimisation the upstream crate ships; medium-cost, well-scoped,
non-breaking.

Everything else is either already best-in-class (align, jaro,
jaro_winkler, hamming vs strsim, gear roll vs gearhash, gear SIMD
digest vs gearhash on aarch64), sub-2× (osa, damerau), or SIMD-
territory (hamming vs triple_accel; Levenshtein bit-parallel Myers).
