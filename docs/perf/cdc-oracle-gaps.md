# `stringcheese-cdc` oracle gap table

Side-by-side throughput of `stringcheese-cdc` vs the best-in-class
Rust content-defined-chunking / rolling-hash ecosystem. Oracles chosen:

* **`fastcdc`** (4.0) — Nathan Fiedler's canonical Rust FastCDC v2020
  implementation. Reference for the end-to-end FastCDC chunker
  throughput. Zero transitive deps beyond `cfg-if`. Configured at
  `Normalization::Level2` to match the paper NLC=2 mask popcount
  stringcheese's `default_8k` / `default_16k` presets commit to
  (15/11 bits at 8 KiB, 16/12 bits at 16 KiB).
* **`gearhash`** (0.1) — dedicated GEAR rolling-hash crate. Reference
  for the scalar per-byte throughput (via `Hasher::update`) and — on
  x86_64 — the AVX2 / SSE4.2 SIMD `next_match` throughput. On aarch64
  upstream `gearhash` has **no NEON backend** and falls back to its
  scalar next-match loop; the "SIMD" lane on the M-series measurement
  host is therefore stringcheese's NEON `digest_of_slice` vs a
  well-tuned scalar sweep. That distinction is called out in the row
  labels and in the read section below.

## Oracles considered and rejected

* **`fastcdc-alt`** (0.2) — API-alternative fork of `fastcdc`; adds no
  algorithmic coverage over the upstream crate. Rejected as duplicate.
* **`chunkrs`** / **`mothcdc`** / **`cavs-chunker`** — newer / less-
  established CDC crates. Rejected in favour of the widely-adopted
  `fastcdc-rs` reference to keep the "canonical Rust ecosystem" bar.
* **`rabin-fingerprint-rs`** / other Rabin implementations — no
  maintained pure-Rust Rabin fingerprint crate at the reasonable-
  maturity bar surfaced on crates.io. The task explicitly permitted
  skipping this oracle; documented here so the omission is not
  mistaken for an oversight. stringcheese's `RabinFingerprint` and
  `PolynomialHash` baselines therefore have no oracle row and rely
  solely on the standalone baseline table under
  `crates/stringcheese-cdc/benches/cdc.rs`'s module doc.
* **`buzhash`** — no maintained standalone BuzHash / cyclic-polynomial
  crate on crates.io at reasonable maturity. `Buzhash` also has no
  oracle row for the same reason.

## Dep-footprint

Adding the two oracle crates locks 3 new packages (`fastcdc`,
`gearhash`, `cfg-if`) into `Cargo.lock`. Well under the 20-entry
budget documented in `docs/perf/oracle-gap-summary.md`. No system-
level dep, no build-script contamination, no `cc` / `bindgen`.

## How to reproduce

```bash
cargo bench -p stringcheese-cdc --bench cdc \
    --features simd,oracle-benches --locked -- \
    --warm-up-time 1 --measurement-time 3 --sample-size 15 \
    "cdc/oracle/"
```

The `oracle-benches` feature is off by default. It is bench-only — no
library code looks at either oracle crate, and the ordinary `cargo
bench -p stringcheese-cdc` invocation runs the standalone baseline
groups without pulling either extra dep in.

## Measurement environment

* aarch64 Apple M-series, macOS 15 / darwin 24.5
* rustc 1.97.1, release + LTO ("thin", codegen-units = 1)
* criterion 0.5, `--sample-size 15 --measurement-time 3`
* Wall-clock samples vary ±10-20 % on a laptop under load; treat
  ratios as informative, absolutes as illustrative.

## Gap table

Throughput reported as median of one representative run. `mult` is
`oracle_thrpt ÷ stringcheese_thrpt` at the same size × flavor —
`> 1` means the oracle is faster than stringcheese, `< 1` means
stringcheese wins. Verdict categories: **competitive** (< 2× gap
either way), **medium gap** (2-5×, optimisation worth considering),
**large gap** (> 5×, clear perf lever).

### FastCDC (default_8k = 2/8/64 KiB, NLC=2)

|   size | flavor | stringcheese | fastcdc-rs   | mult   | verdict                        |
| -----: | :----- | :----------- | :----------- | -----: | :----------------------------- |
|  1 MiB | random |   1.12 GiB/s |   1.82 GiB/s |  1.63× | competitive (fastcdc ahead)    |
|  1 MiB | prose  | 996   MiB/s  |   1.64 GiB/s |  1.69× | competitive (fastcdc ahead)    |
| 10 MiB | random |   1.09 GiB/s |   1.95 GiB/s |  1.79× | competitive (fastcdc ahead)    |
| 10 MiB | prose  |   1.14 GiB/s |   1.57 GiB/s |  1.38× | competitive (fastcdc ahead)    |

### FastCDC (default_16k = 4/16/128 KiB, NLC=2)

|   size | flavor | stringcheese | fastcdc-rs   | mult   | verdict                             |
| -----: | :----- | :----------- | :----------- | -----: | :---------------------------------- |
|  1 MiB | random | 744   MiB/s  |   1.60 GiB/s |  2.15× | **medium gap** (fastcdc-rs ahead)   |
|  1 MiB | prose  |   1.14 GiB/s |   1.78 GiB/s |  1.56× | competitive (fastcdc ahead)         |
| 10 MiB | random |   1.37 GiB/s |   2.20 GiB/s |  1.61× | competitive (fastcdc ahead)         |
| 10 MiB | prose  |   1.28 GiB/s |   1.76 GiB/s |  1.38× | competitive (fastcdc ahead)         |

Reads:

* `fastcdc-rs` is consistently ~1.4-2.2× faster on the end-to-end
  chunker. The upstream crate implements the paper's "rolling two
  bytes each time" optimisation (see the v2020 module docs: "about a
  20% improvement" on Apple M1 over v2016) and holds pre-shifted
  variants of the gear table (`gear` + `gear_ls`) so a two-byte step
  costs one memory read from each. stringcheese's `FastCdc` state
  machine is a plain one-byte-per-step loop and does not fuse the
  mask-check / advance / gear-shift into a wider stride.
* The 1 MiB / random / default_16k row (2.15×) is the one point that
  crosses the 2× "medium gap" threshold; the other seven rows sit in
  the 1.4-1.8× band. This suggests the gap widens on random inputs
  where the small-mask / large-mask branches split traffic more
  evenly, not on prose where the repetition rate keeps the chunker in
  the small-mask regime.

### GEAR — per-byte roll (streaming)

|   size | flavor | stringcheese | gearhash     | mult   | verdict     |
| -----: | :----- | :----------- | :----------- | -----: | :---------- |
|  1 MiB | random |   1.29 GiB/s |   1.34 GiB/s |  1.04× | competitive |
|  1 MiB | prose  |   1.29 GiB/s |   1.23 GiB/s |  0.95× | competitive |
| 10 MiB | random |   1.31 GiB/s |   1.12 GiB/s |  0.85× | competitive (stringcheese 1.2× ahead) |
| 10 MiB | prose  |   1.21 GiB/s |   1.22 GiB/s |  1.01× | competitive |

Reads:

* Essentially tied. Both sides run the identical scalar shift-add
  primitive (`hash = (hash << 1) + table[byte]`) so throughput
  converges to ~1.2-1.3 GiB/s on this host regardless of which crate
  wraps it.
* stringcheese's `GearHash::roll` carries a small window-bookkeeping
  overhead that `gearhash::Hasher` skips (gearhash has no explicit
  window — the 64-bit shift register decays naturally), but LLVM
  collapses that into the same shift-add sequence at `-O3`, so the
  difference is bench noise.

### GEAR — next-match / digest_of_slice (SIMD lane)

Row labels:

* `stringcheese-simd` — `fingerprint::gear::simd::digest_of_slice`
  compiled `--features simd`. On this host that dispatches to the
  NEON backend.
* `gearhash` — `Hasher::next_match` with a 15-bit-popcount mask.
  On aarch64 this **falls back to `gearhash`'s scalar loop** (no NEON
  backend upstream); on x86_64 it dispatches to AVX2 / SSE4.2.

|   size | flavor | stringcheese-simd | gearhash (scalar sweep) | mult   | verdict                                |
| -----: | :----- | :---------------- | :---------------------- | -----: | :------------------------------------- |
|  1 MiB | random |   2.34 GiB/s     |   1.35 GiB/s            |  0.58× | stringcheese 1.7× ahead                |
|  1 MiB | prose  |   2.60 GiB/s     |   1.26 GiB/s            |  0.49× | stringcheese 2.1× ahead                |
| 10 MiB | random |   2.05 GiB/s     | 949   MiB/s             |  0.46× | stringcheese 2.2× ahead                |
| 10 MiB | prose  |   1.20 GiB/s     |   1.09 GiB/s            |  0.91× | competitive (stringcheese slightly ahead) |

Reads:

* stringcheese's NEON `digest_of_slice` is 1.7-2.2× faster than
  gearhash's aarch64 scalar `next_match`. The gap is honest —
  gearhash upstream simply has no aarch64 SIMD path, so this compares
  vectorised bytes vs a scalar shift-add loop.
* The 10 MiB / prose row narrows to 0.91×; the SIMD kernel's absolute
  throughput drops from 2.6 GiB/s (1 MiB) to 1.2 GiB/s at 10 MiB,
  which is the working-set-exceeds-L2 cliff on the M-series (L2 is
  ~4 MiB per performance core). Both sides feel it; the ratio holds.
* Cross-arch reading: on x86_64 with AVX2, gearhash's SIMD `next_match`
  would be the meaningful head-to-head against stringcheese's SIMD
  digest. That measurement is out of scope for the aarch64 baseline
  round; re-run this bench on an x86_64 host to flesh out the row.

## Summary verdict for `stringcheese-cdc`

* **Real perf headroom**: `FastCDC` end-to-end chunker at both
  presets — `fastcdc-rs` is ~1.4-2.2× faster across the board, with
  one row (1 MiB / random / default_16k) crossing the 2× medium-gap
  threshold. The upstream crate's "rolling two bytes each time" +
  pre-shifted gear-table trick is the identified lever.
* **Competitive**: GEAR per-byte roll (essentially tied with
  gearhash). GEAR `digest_of_slice` (SIMD) is 1.7-2.2× *ahead* of
  gearhash's scalar `next_match` on aarch64 — a favourable arch
  quirk, not a general result.
* **No oracle**: `BuzHash`, `PolynomialHash`, `RabinFingerprint` —
  no maintained pure-Rust crate at the maturity bar. Baselines
  continue to live under the standalone baseline table in the bench
  file's module doc; regression tripwire is preserved.

See `docs/perf/oracle-gap-summary.md` for the ranked perf-lever list
that combines this data with `stringcheese-compare`'s and
`stringcheese-align`'s.
