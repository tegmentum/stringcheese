# WebAssembly Binary Size Baseline

Status: Reference
Applies to: StringCheese 0.1 and later
Related: [wasm-build-recipes.md](./wasm-build-recipes.md), [design/wasm-and-wit-interface.md](./design/wasm-and-wit-interface.md)

Per-crate `.wasm` size baselines for every artifact the CI `wasm-size`
job measures, plus the methodology used to measure them and the
tolerances the gate enforces on each. This document is the human-
readable companion to [`.wasm-size-limits.toml`](../.wasm-size-limits.toml),
which is the machine-readable file the CI job actually reads.

## Why a size gate at all

StringCheese commits to WebAssembly as a first-class deployment target
(see [design/wasm-and-wit-interface.md](./design/wasm-and-wit-interface.md)).
That commitment is easy to erode incrementally — a Unicode table added
here, a large `HashMap` initialization there, a formatting path pulled
in transitively — and each individual change looks harmless in
isolation. The size gate makes each such increment visible: a PR whose
`.wasm` grows more than `tolerance_pct` (5 % by default) fails the
`wasm-size` CI job with a summary of which crates regressed.

Growth is not forbidden — an intentional feature that costs 10 KB is
still a merge candidate. The gate exists to make sure the growth is
*noticed and recorded* rather than sneaking in.

## Measurement methodology

### The size-probe wrapper

Every StringCheese library crate in the workspace is an `rlib` — none
declares `crate-type = ["cdylib"]` and none exports
`#[no_mangle] extern "C"` functions on its own. Two consequences:

1. `cargo build --target wasm32-unknown-unknown -p <crate>` produces a
   `.rlib` archive (metadata + bitcode), not a `.wasm`. Nothing to
   measure.
2. Forcing a `cdylib` build via `cargo rustc --crate-type cdylib`
   produces a `.wasm`, but LTO strips every crate item because nothing
   references them. The output is a ~300 byte stub regardless of how
   large the underlying crate actually is.

Neither is a useful size proxy. The workaround lives at
[`wasm-size-probes/`](../wasm-size-probes/) — a standalone (non-
workspace) `cdylib` package with one feature per measured crate. Each
feature turns on the corresponding optional dependency and gates a
`#[unsafe(no_mangle)] pub extern "C" fn probe()` that calls a
representative slice of that crate's public API through
`core::hint::black_box`. LTO retains everything reachable from
`probe`, so the resulting `.wasm` is a realistic upper bound on what
the crate contributes to a downstream browser or edge bundle.

The probe is documentation as much as instrumentation. If a wasm-size
regression traces back to a symbol the probe does not touch, the fix
is to extend the probe — not to widen the tolerance.

### Build recipe

For each measured library crate:

```bash
cd wasm-size-probes
cargo build --release \
    --target wasm32-unknown-unknown \
    --no-default-features \
    --features probe-<crate>
wasm-opt -Oz \
    -o /tmp/<crate>.opt.wasm \
    target/wasm32-unknown-unknown/release/wasm_size_probe.wasm
wc -c /tmp/<crate>.opt.wasm
```

For the component:

```bash
cd component/rust-host
cargo component build --release
wc -c target/wasm32-wasip1/release/stringcheese_component_host.wasm
```

`scripts/measure-wasm-size.sh` does both loops (plus the threshold
comparison) in one invocation; contributors should run it before
pushing size-changing PRs.

### Feature-set choice: why `std`, not `alloc`-only

The canonical minimum surface documented in
[wasm-build-recipes.md](./wasm-build-recipes.md) is
`--no-default-features --features alloc`. The size probes deviate:
they enable `std` on each measured crate. Two reasons:

1. **No allocator without `std`.** `wasm32-unknown-unknown` with
   `no_std + alloc` requires the *final* crate in the tree to provide
   a `#[global_allocator]`. Neither StringCheese nor `alloc` itself
   supplies one; a real deployment would either enable `std` (which
   brings `std::alloc`) or add a third-party allocator like `dlmalloc`
   or `wee_alloc`. The `std` measurement therefore reflects what a
   downstream user actually ships.
2. **Some kernels are `std`-only.** Damerau's `HashMap`-backed
   production kernel and workspace, set-similarity's `Cosine`
   (`f64::sqrt`), and MinHash's weighted CWS sketch (`f64::ln`,
   `f64::exp`, `f64::floor`) all gate on `std`. Measuring `alloc`-
   only would systematically under-report the crate's real surface,
   defeating the point of the gate.

`wasm32-unknown-unknown` still forbids most std syscalls at runtime,
so `std` here means "the panic handler, the allocator, and the
pure-CPU parts of the standard library" — not filesystem or thread
APIs, which trap on this target regardless.

### Release-profile settings

The probe package's `[profile.release]` mirrors what the component
host uses (`lto = "fat"`, `codegen-units = 1`, `opt-level = "s"`,
`strip = "debuginfo"`, `panic = "abort"`) so the measured number is
as close as possible to what a size-tuned production build would
produce. The main workspace's `[profile.release]` uses `lto = "thin"`
and `codegen-units = 1` — noticeably heavier because it optimizes for
compile speed of the test / clippy / doc jobs; contributors comparing
manual `cargo build` output to the probe number should account for
this.

### Post-processing: `wasm-opt -Oz`

The core Rust build already inlines and LTOs aggressively; `wasm-opt
-Oz` on top usually trims another 15-30 % by rewriting the wasm at the
opcode level (dead-block elimination, table/function deduplication,
constant folding across imports). The gate is applied to the
`wasm-opt`ed number so that a change which grows the raw wasm but is
then absorbed by `wasm-opt` does not fire the gate spuriously.

Component-model binaries are the exception. Binaryen (which ships
`wasm-opt`) does not yet parse the component model
([Binaryen #6728](https://github.com/WebAssembly/binaryen/issues/6728)),
so the component baseline is the unoptimized `cargo component build`
output and the `.wasm-size-limits.toml` entry carries
`optimizer_skipped = true`.

### What `twiggy top` shows

`twiggy top /tmp/<crate>.opt.wasm | head -20` reports the largest
symbols in the optimized wasm. Useful when investigating a
regression: a suddenly-heavy `code[N]` is a new function, while a
suddenly-heavy `data[N]` is a new static table. Twiggy is installed
in the CI job for post-mortem inspection but not used by the gate
itself; contributors do not need it locally to reproduce the gate.

## Per-crate baseline

Measured at commit `<HEAD>` on `wasm32-unknown-unknown` via the size-
probe wrapper described above (raw = raw `.wasm` output of the probe
`cdylib`; opt = after `wasm-opt -Oz`). Human-readable columns are
labeled with base-10 kibibyte-style units for a quick scan; the
authoritative numbers are the byte values.

| Crate                       | Kind      | Raw wasm | Opt wasm  |    Opt B | Tol.  | Notes                                                                         |
|-----------------------------|-----------|---------:|----------:|---------:|:-----:|-------------------------------------------------------------------------------|
| `stringcheese`              | library   |  24.6 KB |   17.6 KB |   18 027 |  5 %  | Facade re-export; measured with the same probe surface as `stringcheese-compare`. LTO folds them together. |
| `stringcheese-core`         | library   |  0.79 KB |    724 B  |      724 | 20 %  | Types & traits only. Sub-1 KB baseline: `tolerance_pct = 20` because ±5 % is under the wasm-opt noise floor. |
| `stringcheese-corpus`       | library   |  20.1 KB |   14.5 KB |   14 836 |  5 %  | Exhaustive-generator machinery and difference-classification enum.            |
| `stringcheese-compare`      | library   |  24.1 KB |   17.4 KB |   17 839 |  5 %  | Probe touches Levenshtein, OSA, Jaro, Jaro-Winkler, Hamming.                  |
| `stringcheese-unicode`      | library   |   204 KB |    185 KB |  189 567 |  5 %  | **Largest.** Measures the minimum-useful surface — full default (with `case-fold` + `compiled-case-data`) is ~213 KB, see below. |
| `stringcheese-phonetic`     | library   |  31.4 KB |   23.0 KB |   23 523 |  5 %  | Probe touches Soundex, NYSIIS, Double Metaphone.                              |
| `stringcheese-cdc`          | library   |  26.5 KB |   21.0 KB |   21 511 |  5 %  | FastCDC + four rolling-hash fingerprints (Rabin, polynomial, Gear, Buzhash). |
| `stringcheese-index`        | library   |  43.6 KB |   30.6 KB |   31 375 |  5 %  | BK-tree + VP-tree + q-gram index, exercised with a probe-local metric.        |
| `stringcheese-align`        | library   |  24.0 KB |   17.3 KB |   17 758 |  5 %  | Needleman-Wunsch + Smith-Waterman with linear and affine gap schemes.         |
| `stringcheese-manip`        | library   |  80.6 KB |   69.3 KB |   70 934 |  5 %  | Case module pulls the ICU casemap surface from `stringcheese-unicode`.        |
| `component/rust-host`       | component |   126 KB |    126 KB |  129 291 |  5 %  | `cargo component build --release` output. `wasm-opt` skipped (Binaryen limitation). |

### Non-obvious observations

* **`stringcheese-unicode` is 190 KB opt at the gated baseline, ~213 KB
  at full defaults.** All of it is Unicode data — `twiggy top` on the
  optimized wasm shows the top five entries are `data[N]` segments
  totalling ~130 KB from `unicode-normalization`'s NFC/NFD/NFKC/NFKD
  lookup tables. The two configurations differ by the ~23 KB
  contribution of `icu_casemap`'s baked `compiled_data`, which the
  gate measures WITHOUT (see the feature discussion in the crate's
  `lib.rs`). The gate tracks the smaller number because that is what a
  size-conscious wasm caller actually pays; the fuller default is a
  documented ceiling (see below).

  Two feature flags on `stringcheese-unicode` let downstream callers
  tune this footprint:

  - `case-fold` (default: on for backwards compat) enables the
    `case_folding` module and pulls in `icu_casemap`. Turning it off
    drops `icu_casemap` and its ~30 transitive ICU4X data crates from
    the compile graph entirely.
  - `compiled-case-data` (default: on) bakes the ICU tables into the
    binary. Turning it off (while leaving `case-fold` on) makes only
    the `_with_mapper` API surface available — the caller supplies a
    `DataProvider` at runtime.

  The gate's probe uses `--no-default-features --features std`, which
  disables both flags. In-workspace consumers (`stringcheese-manip`,
  the facade crate, ...) get the same trimmed surface because their
  own dependency lines set `default-features = false` on
  `stringcheese-unicode`; only external users who write
  `stringcheese-unicode = "0.1"` pick up the full-default 213 KB
  configuration.

  Further shrink beyond this is upstream work.
  `unicode-normalization` v0.1.25 does not expose feature flags to
  trim the compat (NFKC/NFKD) tables, and LTO cannot strip them
  because `Decompositions::next()`'s match on `DecompositionType`
  keeps both arms reachable. Splitting the tables would take a patch
  to that crate (or a fork).
* **`stringcheese-manip` (71 KB) inherits from `stringcheese-unicode`.**
  The `case` module delegates only to `str::to_lowercase` /
  `to_uppercase` and pulls grapheme iteration from
  `stringcheese-unicode`; the ICU case-mapping surface is not
  reached from any manip entry point, so LTO already strips it.
  Manip's dep line is `default-features = false` on
  `stringcheese-unicode`, so the `case-fold` and `compiled-case-data`
  features stay off transitively — the ICU crates are dropped from
  the manip build's dep graph entirely.
* **The `stringcheese` facade (18 KB) is smaller than
  `stringcheese-compare` alone (17.8 KB) plus the other subcrates
  the facade re-exports.** LTO folds their shared machinery
  aggressively; the facade probe deliberately covers only the
  compare surface (the largest re-exported subsystem), so the
  measurement reflects "what a user pays if they import the facade
  and call the comparison APIs", not "the sum of every subcrate".
* **`component/rust-host` (129 KB) is the only shipping wasm today.**
  Every workspace crate is `rlib` — the numbers above measure a
  synthetic probe cdylib, not an artifact StringCheese itself
  publishes to a registry.

## Excluded from the size gate

* **`stringcheese-bench`.** The benchmark harness depends on
  criterion, which needs host-only timing / IO. Benchmark code is not
  a wasm target — the outer `wasm` and `wasm-runtime` CI jobs already
  exclude it, and the `wasm-size` job follows the same convention.
* **`bench-adapters/rust`.** Same rationale; standalone package with
  its own workspace, not measured.
* **`fuzz`.** Same rationale.
* **`wasm-size-probes` itself.** The probe crate is instrumentation
  for the gate, not an artifact worth gating.

## Tolerance selection

Default is `tolerance_pct = 5`. Rationale:

* At 5 %, a 1 KB baseline fires at +50 B growth. That is above the
  wasm-opt noise floor (single-pass reordering of function slots can
  move a small binary by ~10-20 B).
* At 5 %, a 100 KB baseline fires at +5 KB growth. That is a real
  code / data addition, not noise.
* At 5 %, the 190 KB unicode baseline fires at +9.5 KB growth. That
  is roughly one new NFC-adjacent Unicode table — a meaningful
  regression that a reviewer should see.

The one deviation is `stringcheese-core` at `tolerance_pct = 20`:
its 706 B baseline is small enough that a ±5 % band (±35 B) sits
inside the wasm-opt noise floor. Twenty percent lets it move
meaningfully before firing, without going so wide that a real code
addition (say +200 B) slips through unnoticed.

## Updating the baseline

When a change intentionally grows a `.wasm`:

1. Run `scripts/measure-wasm-size.sh` locally to get the new numbers
   (or read them off the CI failure log).
2. Update the corresponding `optimized_bytes` in
   `.wasm-size-limits.toml` to the new value.
3. Update this document's table if the change also affects the human-
   readable summary (kilobyte column or notes).
4. Note the reason for the growth in the PR description. The gate is
   deliberately not a merge blocker for intentional growth — it is a
   noticing mechanism.

Do **not** widen `tolerance_pct` to smother a regression that has not
been reviewed. The tolerance is a noise-floor allowance, not an
escape hatch.
