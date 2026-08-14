# Oracle gap summary

Cross-crate roll-up of the `--features oracle-benches` measurements
taken on `stringcheese-compare` and `stringcheese-align`. Answers the
question the perf audit posed:

> Are stringcheese's baselines already competitive, or is there real
> headroom vs best-in-class Rust libraries?

For per-kernel raw numbers see:

* `docs/perf/compare-oracle-gaps.md` (Levenshtein, OSA, Damerau,
  Hamming, Jaro, Jaro-Winkler)
* `docs/perf/align-oracle-gaps.md` (NW / SW × linear / affine ×
  score / align)

## Oracles used

| crate                    | oracle          | version | why                                                  | dep footprint            |
| :----------------------- | :-------------- | :------ | :--------------------------------------------------- | :----------------------- |
| `stringcheese-compare`   | `strsim`        | 0.11    | scalar pure-Rust reference for every distance metric | 0 transitive deps        |
| `stringcheese-compare`   | `triple_accel`  | 0.4     | SIMD-accelerated Levenshtein + Hamming (AVX2/NEON)   | 0 transitive deps        |
| `stringcheese-align`     | `bio`           | 2.3     | canonical Rust exact NW/SW (Gotoh three-matrix DP)   | **~117 packages**        |

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

## Verdict per crate × kernel

Verdict thresholds (task-defined):

* **competitive** — < 2× gap either way; no perf work needed
* **medium gap** — 2-5×, optimisation worth considering
* **large gap** — > 5×, clear perf lever
* **stringcheese ahead** — stringcheese is faster than the oracle

### `stringcheese-compare`

| kernel        | size | oracle       | mult   | verdict                       |
| :------------ | ---: | :----------- | -----: | :---------------------------- |
| levenshtein   |   32 | strsim       |  0.67× | competitive (stringcheese wins) |
| levenshtein   |  256 | strsim       |  2.82× | **medium gap** (strsim ahead) |
| levenshtein   | 2048 | strsim       |  3.31× | **medium gap** (strsim ahead) |
| levenshtein   |   32 | triple_accel |  0.49× | competitive (stringcheese wins) |
| levenshtein   |  256 | triple_accel |  0.83× | competitive                   |
| levenshtein   | 2048 | triple_accel |  0.88× | competitive                   |
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

### 1. `compare/levenshtein` at n ≥ 256 — vs `strsim` (3× gap)

* **Gap**: 2.8× at n = 256, 3.3× at n = 2048 vs pure-scalar `strsim`.
* **What's likely wrong**: stringcheese's trait-abstracted DP is not
  autovectorising. `strsim`'s tight `chars()`-iterator inner loop
  hits LLVM's autovectoriser cleanly. This is scalar-loop work, not
  SIMD-intrinsics work.
* **Estimated cost**: low-medium. The DP body is small; the fix is
  likely a `#[inline(always)]` on the cell update + eliminating a
  trait boundary in the hot loop.
* **Follow-on**: once the scalar loop closes the strsim gap, the next
  jump is a Myers 1999 bit-parallel Levenshtein (the crate has a
  `simd` feature scaffold; a dedicated Myers kernel is already the
  documented target). That is where the 5-10× SIMD headroom lives —
  but `triple_accel`'s `levenshtein_exp` shows that SIMD alone is not
  enough: the exponential-doubling wrapper can wipe out the SIMD win,
  and the bit-parallel Myers kernel needs to be dispatched
  unconditionally at unbounded edit distance to actually win.

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

### 3. `compare/{osa, damerau}` — vs `strsim` (~1.8-2× gap)

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
* **`compare/{search, minhash, ngram, lcs, learned}`**: no comparable
  scalar Rust oracle in the ecosystem worth wiring up right now.

## Actionable takeaway

The **one clear perf lever** surfaced by this measurement round is the
`compare/levenshtein` scalar DP at n ≥ 256. Everything else is either
already best-in-class (align, jaro, jaro_winkler, hamming vs strsim) or
sub-2× (osa, damerau) or SIMD-territory (hamming vs triple_accel).
