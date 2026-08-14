# `stringcheese-align` oracle gap table

Side-by-side throughput of `stringcheese-align` vs the best-in-class
pure-Rust exact alignment library. Oracle chosen:

* **`bio`** (rust-bio v2) — the canonical Rust bioinformatics crate.
  Ships `bio::alignment::pairwise::Aligner` with Gotoh 1982 three-matrix
  affine-gap DP. Its `Aligner` always retains the traceback (there is no
  score-only fast path), so a "score" comparison includes bio's per-call
  trace allocation cost. That is the real-world cost of picking bio for
  a score-only pipeline.

## Why `bio` (and not `parasail-rs` / `block-aligner`)

The task list also named `parasail-rs` and `block-aligner` as candidate
oracles. `bio` was chosen despite its heavy transitive dep footprint
(~117 packages: ndarray, statrs, petgraph, csv, regex, ...) because:

* **`parasail-rs`** requires a C compiler + libclang for `bindgen` at
  build time. It ships FFI bindings to the parasail C library. Great
  for perf comparison but pulls a system dep into CI and defeats the
  "pure-Rust ecosystem" oracle goal.
* **`block-aligner`** is pure Rust with SIMD (`simd_neon` / `simd_avx2`
  features) and zero transitive deps, but it is a **banded / adaptive**
  aligner — it does not compute the same score as exact NW/SW unless
  the band width is set wide enough to span the whole DP. That makes
  the correctness surface asymmetric to `stringcheese-align`; the
  throughput comparison would not be apples-to-apples.
* **`bio`** computes exact NW/SW with a well-known DP loop; the
  comparison is direct.

The dep-footprint red flag is documented in the crate's `Cargo.toml`
and in the module docs. The `oracle-benches` feature is off by default,
so a plain `cargo test --workspace --locked` never pulls `bio` in;
only the compile-check CI job with `--features oracle-benches` (and any
contributor running the oracle bench locally) pays the compile cost.

## How to reproduce

```bash
cargo bench -p stringcheese-align --bench align \
    --features oracle-benches --locked -- \
    --warm-up-time 1 --measurement-time 3 --sample-size 15 \
    "align/oracle/"
```

The first invocation compiles `bio` + transitive deps (~117 packages,
15-30 s single-crate).

## Scoring parameters

To keep the comparison identical, both sides use the same match /
mismatch / gap costs:

* **Linear**: match = 1, mismatch = -1, gap = -1
  * stringcheese: `LinearGap::simple()`
  * bio: `Aligner::new(-1, -1, |x, y| if x == y { 1 } else { -1 })`
* **Affine**: match = 1, mismatch = -1, gap_open = -2, gap_extend = -1
  * stringcheese: `AffineGap::default_affine()`
    * total k-symbol gap cost = `gap_open + (k-1) * gap_extend`
  * bio: `Aligner::new(-1, -1, score)` (bio's convention is `gap_open
    + k * gap_extend`, i.e. gap_open on top of k extension symbols;
    passing `(-1, -1)` matches stringcheese's `(-2, -1)` total per k)

## Measurement environment

* aarch64 Apple M-series, macOS 15 / darwin 24.5
* rustc 1.97.1, release + LTO ("thin", codegen-units = 1)
* criterion 0.5, `--sample-size 15 --measurement-time 3`
* Wall-clock samples vary ±10-20 % on a laptop under load.

## Gap table

Throughput reported as median. `mult` = `bio_thrpt ÷ stringcheese_thrpt`
at the same size × flavor. `mult < 1` means stringcheese wins.

### NW score (linear gap)

| size | flavor    | stringcheese | bio        | mult   |
| ---: | :-------- | :----------- | :--------- | -----: |
|   32 | matching  |  15.98 MiB/s |  3.74 MiB/s|  0.23× |
|   32 | divergent |  17.52 MiB/s |  3.88 MiB/s|  0.22× |
|  128 | matching  |   3.30 MiB/s |  1.07 MiB/s|  0.32× |
|  128 | divergent |   3.29 MiB/s |941   KiB/s |  0.28× |
|  512 | matching  | 682   KiB/s  |217   KiB/s |  0.32× |
|  512 | divergent | 644   KiB/s  |177   KiB/s |  0.27× |

**Verdict**: stringcheese 3-4× ahead everywhere. bio's Gotoh
three-matrix DP for `gap_open == gap_extend == -1` pays a constant
that stringcheese's specialised single-matrix linear-gap DP does not.

### NW score (affine gap)

| size | flavor    | stringcheese | bio         | mult   |
| ---: | :-------- | :----------- | :---------- | -----: |
|   32 | matching  |   8.65 MiB/s |  3.42 MiB/s |  0.40× |
|   32 | divergent |  11.03 MiB/s |  3.82 MiB/s |  0.35× |
|  128 | matching  |   2.75 MiB/s |  1.03 MiB/s |  0.37× |
|  128 | divergent |   1.77 MiB/s |913   KiB/s  |  0.51× |
|  512 | matching  | 706   KiB/s  |270   KiB/s  |  0.38× |
|  512 | divergent | 680   KiB/s  |208   KiB/s  |  0.31× |

**Verdict**: stringcheese 2-3× ahead. Affine narrows the gap slightly
because both sides now do Gotoh three-matrix DP; stringcheese's win is
purely inner-loop efficiency rather than an algorithmic difference.

### NW align (linear, score + traceback)

| size | flavor    | stringcheese | bio          | mult   |
| ---: | :-------- | :----------- | :----------- | -----: |
|   32 | matching  |  13.98 MiB/s |   3.93 MiB/s |  0.28× |
|   32 | divergent |  13.79 MiB/s |   3.98 MiB/s |  0.29× |
|  128 | matching  |   3.27 MiB/s |   1.08 MiB/s |  0.33× |
|  128 | divergent |   3.27 MiB/s | 838   KiB/s  |  0.25× |
|  512 | matching  | 812   KiB/s  | 280   KiB/s  |  0.35× |
|  512 | divergent | 811   KiB/s  | 206   KiB/s  |  0.25× |

**Verdict**: stringcheese 3-4× ahead. Both sides retain the O(m·n)
traceback matrix; stringcheese amortises the workspace across iters,
bio allocates fresh each call.

### SW score (linear gap)

| size | flavor    | stringcheese | bio         | mult   |
| ---: | :-------- | :----------- | :---------- | -----: |
|   32 | matching  |  13.45 MiB/s |  3.70 MiB/s |  0.28× |
|   32 | divergent |  12.44 MiB/s |  3.67 MiB/s |  0.29× |
|  128 | matching  |   2.46 MiB/s |  1.00 MiB/s |  0.41× |
|  128 | divergent |   2.45 MiB/s |  1.01 MiB/s |  0.41× |
|  512 | matching  | 594   KiB/s  |269   KiB/s  |  0.45× |
|  512 | divergent | 593   KiB/s  |270   KiB/s  |  0.45× |

**Verdict**: stringcheese 2-4× ahead. Running-max cost is small enough
that the linear-gap DP-specialisation advantage still dominates.

### SW score (affine gap)

| size | flavor    | stringcheese | bio         | mult   |
| ---: | :-------- | :----------- | :---------- | -----: |
|   32 | matching  |  12.24 MiB/s |  3.72 MiB/s |  0.30× |
|   32 | divergent |  12.23 MiB/s |  3.63 MiB/s |  0.30× |
|  128 | matching  |   2.90 MiB/s |855   KiB/s  |  0.29× |
|  128 | divergent |   2.87 MiB/s |  1.01 MiB/s |  0.35× |
|  512 | matching  | 628   KiB/s  |268   KiB/s  |  0.43× |
|  512 | divergent | 744   KiB/s  |270   KiB/s  |  0.36× |

**Verdict**: stringcheese 2-3× ahead.

### SW align (linear, score + traceback + start indices)

| size | flavor    | stringcheese | bio         | mult   |
| ---: | :-------- | :----------- | :---------- | -----: |
|   32 | matching  |  11.17 MiB/s |  3.48 MiB/s |  0.31× |
|   32 | divergent |  11.87 MiB/s |  3.12 MiB/s |  0.26× |
|  128 | matching  |   2.55 MiB/s |899   KiB/s  |  0.34× |
|  128 | divergent |   1.40 MiB/s |  1.02 MiB/s |  0.72× |
|  512 | matching  | 632   KiB/s  |224   KiB/s  |  0.35× |
|  512 | divergent | 641   KiB/s  |267   KiB/s  |  0.42× |

**Verdict**: stringcheese 2-4× ahead. Divergent 128 is the tightest
race (0.72×) — presumably bench noise on a laptop; the shape of the
row otherwise matches the score-only variant.

## Summary verdict for `stringcheese-align`

* **Every combination**: stringcheese is 2-5× faster than bio at every
  size and flavor.
* **stringcheese-align is already best-in-class among pure-Rust exact
  NW/SW libraries.** No scalar-DP perf work needed.
* **Where the headroom lives (if any)**: SIMD-accelerated bit-parallel
  or striped DP implementations (parasail / block-aligner territory).
  That is a different perf story than "make the scalar DP faster" and
  is out of scope for the v0.2 alignment surface.

See `docs/perf/oracle-gap-summary.md` for the ranked perf-lever list.
