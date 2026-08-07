# StringCheese JavaScript Bench Adapters

Head-to-head [`tinybench`] benches comparing StringCheese — loaded as a
WebAssembly component via [`jco`]'s transpiler — against the JavaScript
ecosystem's four most-installed string-distance libraries:
[`fastest-levenshtein`], [`js-levenshtein`], [`natural`], and
[`string-similarity`].

This subtree is deliberately **not** wired into the outer Cargo
workspace or any npm workspace. See `../README.md` for the umbrella
"why each adapter is standalone" rationale and the shared design
contract every adapter follows.

[`tinybench`]: https://github.com/tinylibs/tinybench
[`jco`]: https://github.com/bytecodealliance/jco
[`fastest-levenshtein`]: https://www.npmjs.com/package/fastest-levenshtein
[`js-levenshtein`]: https://www.npmjs.com/package/js-levenshtein
[`natural`]: https://www.npmjs.com/package/natural
[`string-similarity`]: https://www.npmjs.com/package/string-similarity

## Prerequisites

* **Node.js >= 20.** The adapter uses top-level `await`, ES modules,
  `TextDecoder`, and `Uint32Array` — all stable in Node 20. Development
  target is Node 22 LTS but 20 and 24 work.
* **npm** (any version shipped with Node 20+).
* **Rust toolchain with `wasm32-wasip1`** and **`cargo-component`** —
  needed once to build the component `.wasm` that the adapter loads.
  See `../../component/README.md` for the full toolchain setup.
* **`@bytecodealliance/jco` >= 1.0**, pinned in `package.json`. Every
  other dependency is pinned to an exact version — no `^` or `~`
  prefixes — because tinybench's statistical output is only meaningful
  across runs on the same environment.

## Build the wasm component

The JS adapter never rebuilds the wasm — it loads a pre-built `.wasm`
at transpile time and fails fast if the file is missing. So one-shot
from a fresh clone:

```bash
cd component/rust-host
cargo component build --release
```

That produces:

```
component/rust-host/target/wasm32-wasip1/release/stringcheese_component_host.wasm
```

`scripts/transpile.js` discovers the file automatically via a path
relative to itself. If you have the component elsewhere, set the
`STRINGCHEESE_WASM` environment variable to an explicit path before
running the transpile step.

## Install and transpile

From this directory (`bench-adapters/js/`):

```bash
npm install
npm run transpile
```

`npm run transpile` invokes `jco transpile` on the pre-built `.wasm`
and writes the transpiled ES module + core-wasm files into
`./transpiled/` (git-ignored — regenerated whenever the wasm changes).
`stringcheese_adapter.js` imports from `./transpiled/stringcheese.js`;
running the benches without a completed transpile step produces an
import error at module load with a clear diagnostic.

Once `./transpiled/` exists, the transpile step does not need to be
repeated unless the wasm changes. If you edit any of the algorithm
crates or the WIT interface, rerun `cargo component build --release`
and then `npm run transpile` before rerunning benches.

## Run the benchmarks

All four bench files, in sequence:

```bash
npm run bench
```

One file at a time:

```bash
npm run bench:levenshtein
npm run bench:hamming
npm run bench:jaro
npm run bench:damerau
```

Or directly (no npm shim), which is what the `bench:*` scripts do
under the hood:

```bash
node benches/levenshtein.bench.js
node benches/hamming.bench.js
node benches/jaro.bench.js
node benches/damerau.bench.js
```

Environment overrides:

| Variable         | Default | Meaning                                             |
|------------------|---------|-----------------------------------------------------|
| `BENCH_TIME_MS`  | `500`   | Measured time budget per task (per (regime, length) per implementation). |
| `BENCH_MIN_ITERS`| `64`    | Minimum iteration count per task; keeps very-fast tasks well-sampled. |
| `BENCH_JSON`     | *unset* | If set to any truthy value, emit a JSON summary of all tasks to stdout after the human table. |

Example — a longer-per-task, JSON-emitting Levenshtein run:

```bash
BENCH_TIME_MS=2000 BENCH_JSON=1 node benches/levenshtein.bench.js > out.json
```

## Reading the output

Each bench file emits one `console.table` per `(algorithm, regime,
length)` cell. Tinybench's default table columns:

```
=== levenshtein/similar/len0128 ===
┌─────────┬───────────────────────┬─────────────────────┬─────────────────────┬────────┐
│ (index) │ Task name             │ Latency avg (ns)    │ Throughput avg (ops)│ Samples│
├─────────┼───────────────────────┼─────────────────────┼─────────────────────┼────────┤
│ 0       │ 'stringcheese'        │ '12345.67 ± 1.2 %'  │ '81 000 ± 0.87 %'   │ 40 500 │
│ 1       │ 'fastest-levenshtein' │ '789.01 ± 0.4 %'    │ '1 267 000 ± 0.4 %' │ 633 000│
│ 2       │ 'js-levenshtein'      │ '9 876.5 ± 0.9 %'   │ '101 250 ± 0.9 %'   │ 50 600 │
│ 3       │ 'natural'             │ '54 321.0 ± 1.5 %'  │ '18 400 ± 1.4 %'    │  9 200 │
└─────────┴───────────────────────┴─────────────────────┴─────────────────────┴────────┘
```

Read the **median / average** latency column — it is the least
sensitive to the tail noise every timing loop picks up. The `±%` is a
signal-to-noise indicator: if it exceeds ~2 %, rerun with a bigger
`BENCH_TIME_MS` or on a quieter machine before drawing conclusions.

Cell ids follow `<algorithm>/<regime>/<len>`, always with a
zero-padded four-digit length, so grep / sort / a downstream chart
generator can slice the matrix cleanly.

## What's being measured

Each bench file crosses **input length × similarity regime ×
implementation**. The matrix mirrors the Rust and Python adapters and
`stringcheese-bench` exactly:

* **Lengths:** 8, 32, 128, 512, 2048.
* **Regimes:** `random` (independent inputs), `similar` (5 % mixed
  edits), `identical` (byte-equal — the short-circuit corner).
* **Implementations:** vary per file — see the docstring at the top
  of each `*.bench.js`.

The full four-file matrix:

| Algorithm            | StringCheese | fastest-levenshtein | js-levenshtein | natural | string-similarity |
|----------------------|:------------:|:-------------------:|:--------------:|:-------:|:-----------------:|
| Levenshtein          | yes          | yes                 | yes            | yes     | not comparable ¹  |
| Levenshtein (k = 3)  | yes          | —                   | —              | —       | —                 |
| Hamming              | yes          | —                   | —              | yes ²   | —                 |
| Jaro                 | yes          | —                   | —              | — ³     | not comparable ¹  |
| Jaro–Winkler         | yes          | —                   | —              | yes     | —                 |
| OSA (restricted Damerau) | yes      | —                   | —              | — ⁴     | —                 |
| Full Damerau         | **N/A** ⁵    | —                   | —              | yes     | —                 |

¹ `string-similarity.compareTwoStrings` computes a Dice coefficient
over character bigrams, not Levenshtein / Jaro. It is included in the
`compareTwoStrings/*` and `jaro/*` groups as a "not directly
comparable" FFI-cost anchor, kept in a separate `console.table` so a
reader cannot cross the wires.

² `hamming.bench.js` also includes a **hand-rolled** reference
(`[...a].filter((c, i) => c !== b[i]).length` — the one-liner most
StackOverflow answers reach for). Every other npm package that
routinely lands in a Levenshtein comparison — `fastest-levenshtein`,
`js-levenshtein`, `string-similarity` — omits Hamming, so the
in-file reference is more informative than pulling in a second npm
package.

³ `natural`'s Jaro implementation is not exposed as a top-level
function — it lives as a file-scope `distance()` inside
`jaro-winkler_distance.js` and is only reachable via
`JaroWinklerDistance` (which then computes JW on top). The
`jaro.bench.js` file therefore uses `string-similarity` as its
FFI-cost anchor instead of `natural`.

⁴ `natural.DamerauLevenshteinDistance` is the **full unrestricted**
Damerau, not OSA. Pairing it with StringCheese's `osa_distance` would
put two different algorithms on the same axis. The `osa/*` group is
StringCheese-only; the `damerau/*` group is `natural`-only.

⁵ Full unrestricted Damerau is not exposed by the StringCheese WIT
component — the underlying Rust kernel needs a `HashMap`, which pulls
in `getrandom` on `wasm32-*`. See `../../component/README.md`
"Deliberately not exposed". The bench file additionally asserts the
gap has not silently closed at startup (calls `damerauDistance` and
expects a `NotImplementedError`); if a future WIT revision exposes
full Damerau, the assertion warns and the file needs a StringCheese
row.

## What's **not** being measured (READ THIS)

This adapter is not the fair-fight DP-kernel comparison the Rust
adapter's `_vs_strsim_generic_bytes` group is. It answers a different
question:

> **"Should I use StringCheese through wasm from a Node.js program
> instead of a native pure-JS implementation?"**

That is a whole-stack question and its answer is a whole-stack
comparison. Every StringCheese timing here includes:

1. Parameter lowering across the wasm component boundary (a JS
   `Uint8Array` gets copied into the guest's linear memory).
2. Guest DP execution (the same StringCheese kernel measured in
   `bench-adapters/rust/`).
3. Result lifting across the boundary (a wasm `u32` / `f64` gets
   boxed into a JS Number).
4. jco's internal post-call bookkeeping between successive invocations
   of the same function.

Steps (1), (3), (4) are the "FFI cost". At short input lengths the
FFI cost dominates and pure-JS wins comfortably. At longer inputs the
algorithmic work (step 2) dominates and the comparison starts to
reflect kernel quality. The crossover length is the interesting
datapoint per algorithm.

Deliberately excluded from the timing:

* **Component instantiate.** jco's transpiled module materialises the
  wasm instance at module-load time; `new StringCheese()` in each
  bench file's `main()` is a thin function-binding step that runs
  once, before any tinybench task is added.
* **Corpus generation.** Each bench file materialises its
  `(length, regime)` pairs once in a `Map` before entering the
  tinybench loop and re-uses them across tinybench samples. The
  `SplitMix64`-in-BigInt generator in `benches/_inputs.js` is not
  free (BigInt arithmetic is expensive in JS), but the discipline of
  running it exactly once mirrors the sibling adapters.
* **`Uint8Array → string` conversion.** The native libraries take
  strings, StringCheese takes `Uint8Array`. Each bench decodes to a
  string once, outside the timing loop, so only the library's own
  work is timed.

## Corpus determinism

The `SplitMix64` in `benches/_inputs.js` is a BigInt-based port of the
Rust adapter's `SplitMix64` (same state update, same scramble
constants, same shift amounts, same seed derivation). Running the JS,
Python, and Rust adapters against the same `(length, salt)` gives the
same corpus on all three sides.

Per-file bench salts:

| Bench file           | Rust adapter salts   | Python adapter salts | JS adapter salts    |
|----------------------|----------------------|----------------------|---------------------|
| levenshtein          | (0xA1, 0xA2, 0xA3)   | (0xB1, 0xB2, 0xB3)   | (0xF1, 0xF2, 0xF3)  |
| hamming              | (0xC1, 0xC2, 0xC3)   | (0xC1, 0xC2, 0xC3) ⁶ | (0xC1, 0xC2, 0xC3) ⁶ |
| jaro / jaro-winkler  | (0xD1, 0xD2, 0xD3)   | (0xD1, 0xD2, 0xD3) ⁶ | (0xD1, 0xD2, 0xD3) ⁶ |
| osa / damerau        | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)  |

⁶ Hamming, Jaro, and OSA/Damerau share salts across every adapter on
purpose: Hamming needs equal-length inputs on both sides (a
mismatch-position disagreement would confound cross-adapter
debugging), Jaro's match-window behaviour is best observed against a
known-shared corpus, and OSA/Damerau's transposition behaviour is
similarly best observed shared.

## Algorithm-variant pairings

Same as the Rust and Python adapters' "Algorithm-variant caveats"
sections, the pairings this adapter lands on are load-bearing:

| Common name            | StringCheese                    | natural                            | Others (Levenshtein)               |
|------------------------|---------------------------------|------------------------------------|------------------------------------|
| Levenshtein            | `levenshteinDistance` (unit)    | `LevenshteinDistance`              | `fastest-levenshtein` / `js-levenshtein` |
| OSA                    | `osaDistance`                   | *(not exposed)*                    | —                                  |
| Full Damerau           | *(N/A — see above)*             | `DamerauLevenshteinDistance`       | —                                  |
| Jaro                   | `jaroSimilarity`                | *(not top-level exposed)*          | —                                  |
| Jaro–Winkler (classic) | `jaroWinklerSimilarity`         | `JaroWinklerDistance`              | —                                  |
| Hamming                | `hammingDistance`               | `HammingDistance`                  | hand-rolled reference              |

## FFI cost — the whole-stack picture

Because every StringCheese-through-wasm call pays parameter lowering +
result lifting + jco bookkeeping on top of the underlying DP kernel,
the head-to-head numbers here are **not** apples-to-apples with what
a Rust program calling the StringCheese crates directly would see.
Cross-reference the "The FFI break" section in `../README.md` for the
umbrella framing.

Rules of thumb the runs on this adapter empirically confirm:

* **At `len = 8`,** the FFI cost is a large fraction of the total.
  `fastest-levenshtein` in particular wins by a lot.
* **At `len = 128`–`512`,** the DP work starts to dominate and the
  gap narrows; the StringCheese cell can be competitive with `natural`
  (which is not tuned for raw distance speed).
* **At `len = 2048`,** the DP work dwarfs the FFI cost. StringCheese
  ends up close to `fastest-levenshtein` and comfortably faster than
  `natural` on the algorithms where the underlying kernel is stronger.

If your workload is many short strings, use `fastest-levenshtein`. If
your workload has long strings or benefits from an algorithm the
ecosystem does not turnkey-expose (bounded Levenshtein, OSA, pure
Jaro), the FFI cost amortises and StringCheese-via-wasm becomes a
serious option even from Node.js.

## Deno / Bun / browser targets

The adapter is written as pure ES modules with no Node-specific APIs
beyond `node:child_process` (only in `scripts/transpile.js`) and
`node:test` conventions the bench files do not use. In practice:

* **Deno** should work out of the box on the transpiled module —
  `jco transpile` produces standard ESM that Deno parses. `natural`
  and friends are the constraint (some pull in `node:*` imports); the
  StringCheese cell alone would run in Deno unchanged.
* **Bun** runs the transpiled module the same way Node does; the
  bench files should work with `bun benches/levenshtein.bench.js` if
  Bun's `tinybench` compatibility is up to date.
* **Browser** targets need the jco transpile step run without the
  `--wasi-shim` (jco has a `--no-wasi-shim` flag for that); the
  StringCheese component's WASI imports are unreachable code paths,
  so a stub would satisfy them. This is an open followup; the current
  adapter targets Node only.

## Non-goals

Same as the umbrella `../README.md`:

* **Not a correctness suite.** Golden-dataset infrastructure lives in
  `crates/stringcheese-corpus/`. If a distance number disagrees
  between two implementations at the same `(length, seed)` corpus,
  that is a bug to file against the toolkit's differential-test
  harness, not a bench-adapter regression.
* **Not a scoreboard.** Numbers depend on CPU microarchitecture, V8
  version, and OS scheduling. A cross-machine comparison of raw times
  is meaningless; a same-machine comparison of two implementations is
  meaningful. Any docs derived from this harness should show relative
  numbers, not absolute ones.

## CI

There is deliberately no default CI job that runs these benches:
tinybench's noise floor on a shared runner is too high to produce
actionable numbers, and the full matrix takes a few minutes on a
modern laptop. This subtree is standalone — not wired into the outer
Cargo workspace or any npm workspace — so `cargo test --workspace`
does not touch it.
