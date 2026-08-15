# `stringcheese-compare` oracle gap table

Side-by-side throughput of `stringcheese-compare` vs the best-in-class
Rust distance-kernel ecosystem. Oracles chosen:

* **`strsim`** (0.11) — pure-Rust scalar Levenshtein / Damerau /
  OSA / Hamming / Jaro / Jaro-Winkler. Zero transitive deps. Standard
  scalar `chars()`-driven inner loops.
* **`triple_accel`** (0.4) — SIMD-accelerated (AVX2 / SSE4 / NEON)
  Levenshtein and Hamming. Zero transitive deps. Uses exponential-
  doubling `levenshtein_exp` under the hood — SIMD-fastest for tightly
  bounded edit distances, but pays a fixed multi-iteration overhead for
  unbounded searches.

## How to reproduce

```bash
cargo bench -p stringcheese-compare --bench compare \
    --features oracle-benches --locked -- \
    --warm-up-time 1 --measurement-time 3 --sample-size 15 \
    "compare/oracle/"
```

The `oracle-benches` feature is off by default. It is bench-only — no
library code looks at either oracle crate, and the ordinary `cargo bench
-p stringcheese-compare` invocation runs the standalone baseline groups
without pulling either extra dep in.

## Measurement environment

* aarch64 Apple M-series, macOS 15 / darwin 24.5
* rustc 1.97.1, release + LTO ("thin", codegen-units = 1)
* criterion 0.5, `--sample-size 15 --measurement-time 3`
* Wall-clock samples vary ±10-20 % on a laptop under load; treat
  ratios as informative, absolutes as illustrative.

## Gap table

Throughput reported as median of one representative run. `mult` is
`oracle_thrpt ÷ stringcheese_thrpt` at the same size — `> 1` means the
oracle is faster than stringcheese, `< 1` means stringcheese wins.
Verdict categories: **competitive** (< 2× gap either way), **medium
gap** (2-5×, optimization worth considering), **large gap** (> 5×,
clear perf lever).

### Levenshtein

| size | stringcheese  | strsim        | triple_accel  | strsim mult | triple_accel mult | verdict                          |
| ---: | :------------ | :------------ | :------------ | ----------: | ----------------: | :------------------------------- |
|   32 |  35   MiB/s   |  26   MiB/s   |   9.0 MiB/s   |       0.74× |             0.26× | stringcheese 1.34× ahead         |
|  256 |   3.25 MiB/s  |   3.90 MiB/s  |   1.13 MiB/s  |       1.20× |             0.35× | competitive (strsim 1.20× ahead) |
| 2048 | 402   KiB/s   | 500   KiB/s   | 145   KiB/s   |       1.24× |             0.36× | competitive (strsim 1.24× ahead) |

Reads:

* The rolling-rows kernel now walks the row buffer and the inner
  sequence in lockstep via a `zip` iterator and keeps the outer symbol
  and the "carry" cell `d[i][j-1]` in scalar locals. That change
  unblocked LLVM's autovectoriser on the tight scalar inner loop and
  moved throughput from ~2.8-3.3× behind `strsim` at n ≥ 256 into the
  same band (within ~1.2× — below the 2× "medium gap" threshold, so
  the row is now "competitive"). At n = 32 the constant-fold advantage
  from workspace reuse pushes stringcheese ~1.3× *ahead* of `strsim`.
* `triple_accel` on aarch64 is now 3-4× *behind* stringcheese, not
  because its NEON kernel is slow but because it calls
  `levenshtein_exp`, which runs the SIMD DP with successively doubling
  `k` (edit-distance bound); at 5 % edit rate on 2048-byte inputs the
  cumulative overhead of multiple SIMD passes wipes out the SIMD win
  against a single scalar pass — and the scalar pass is now much
  faster than it was before.
* The residual 1.2× to `strsim` at large n is the last bit of scalar
  headroom; a bit-parallel Myers 1999 kernel dispatched via the crate's
  `simd` feature scaffold is the next perf lever, and one where the
  5-10× SIMD headroom actually lives.

### OSA (restricted Damerau)

| size | stringcheese  | strsim        | mult   | verdict     |
| ---: | :------------ | :------------ | -----: | :---------- |
|   32 |  10.8 MiB/s   |  17.1 MiB/s   |  1.58× | competitive |
|  256 |   1.28 MiB/s  |   2.32 MiB/s  |  1.81× | competitive |
| 2048 | 152   KiB/s   | 276   KiB/s   |  1.82× | competitive |

`strsim` is consistently ~1.8× ahead. Sub-2× gap; below the perf-lever
threshold.

### Damerau (full unrestricted)

| size | stringcheese  | strsim        | mult   | verdict     |
| ---: | :------------ | :------------ | -----: | :---------- |
|   32 |   4.26 MiB/s  |   8.33 MiB/s  |  1.96× | competitive |
|  256 | 521   KiB/s   | 952   KiB/s   |  1.83× | competitive |
| 2048 |  61   KiB/s   | 117   KiB/s   |  1.92× | competitive |

Same pattern as OSA — sub-2× gap. stringcheese's `HashMap`-backed
production kernel pays a hash-probe per cell that strsim's array-backed
"last position of symbol" table avoids; the constant-factor cost shows
up as the ~2× multiplier and does not change with n.

### Hamming

| size | stringcheese  | strsim        | triple_accel  | strsim mult | triple_accel mult | verdict                            |
| ---: | :------------ | :------------ | :------------ | ----------: | ----------------: | :--------------------------------- |
|   32 |   3.1 GiB/s   | 956   MiB/s   |   8.01 GiB/s  |       0.30× |             2.58× | medium gap (triple_accel 2.6× ahead) |
|  256 |   5.57 GiB/s  | 969   MiB/s   |  10.77 GiB/s  |       0.17× |             1.93× | competitive vs triple_accel        |
| 2048 |   5.81 GiB/s  |   1.01 GiB/s  |  12.10 GiB/s  |       0.17× |             2.08× | medium gap (triple_accel 2.1× ahead) |

Reads:

* stringcheese's `Hamming::distance_bytes` dispatches to a block-wise
  byte-compare fast path and is 5-6× faster than strsim's per-byte
  scan.
* `triple_accel`'s NEON Hamming is 2-2.6× faster than stringcheese.
  This is memory-bandwidth-bound territory (8-12 GiB/s), so the SIMD
  gap is real but capped by how fast the CPU can stream bytes through
  its comparators.

### Jaro

| size | stringcheese  | strsim        | mult   | verdict                        |
| ---: | :------------ | :------------ | -----: | :----------------------------- |
|   32 | 139   MiB/s   |  54   MiB/s   |  0.39× | stringcheese 2.6× ahead        |
|  256 |  15.5 MiB/s   |   7.0 MiB/s   |  0.45× | stringcheese 2.2× ahead        |
| 2048 |   2.45 MiB/s  | 895   KiB/s   |  0.36× | stringcheese 2.8× ahead        |

stringcheese wins comfortably across all sizes. No action needed.

### Jaro-Winkler

| size | stringcheese  | strsim        | mult   | verdict                        |
| ---: | :------------ | :------------ | -----: | :----------------------------- |
|   32 |  86   MiB/s   |  46   MiB/s   |  0.53× | stringcheese 1.9× ahead        |
|  256 |  13   MiB/s   |   6.7 MiB/s   |  0.52× | stringcheese 1.9× ahead        |
| 2048 |   2.27 MiB/s  | 887   KiB/s   |  0.39× | stringcheese 2.6× ahead        |

Same shape as Jaro — stringcheese wins. No action needed.

## Summary verdict for `stringcheese-compare`

* **Real perf headroom**: Hamming at every size (triple_accel 2×) —
  the only remaining inner-loop vectorisation story after the
  Levenshtein rolling-rows autovectorisation pass. Levenshtein is now
  competitive with `strsim` (≤ 1.24× at every size); the next perf
  lever is a bit-parallel Myers 1999 kernel via the crate's `simd`
  feature scaffold, not further scalar cleanup.
* **Competitive**: Levenshtein at every size, OSA, Damerau, Jaro,
  Jaro-Winkler are within ~2× of best-in-class Rust scalar, and
  Jaro / Jaro-Winkler are actually 2-3× *ahead* of strsim.

See `docs/perf/oracle-gap-summary.md` for the ranked perf-lever list
that combines this data with `stringcheese-align`'s.
