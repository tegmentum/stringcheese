# StringCheese Go Bench Adapters

Head-to-head Go `testing.B` benches comparing StringCheese — loaded
as a WebAssembly component via [`wazero`] — against the Go ecosystem's
two go-to pure-Go string-distance libraries: [`agnivade/levenshtein`]
and [`hbollon/go-edlib`].

This subtree is deliberately **not** wired into the outer Cargo
workspace. See `../README.md` for the umbrella "why each adapter is
standalone" rationale and the shared design contract every adapter
follows.

[`wazero`]: https://github.com/tetratelabs/wazero
[`agnivade/levenshtein`]: https://github.com/agnivade/levenshtein
[`hbollon/go-edlib`]: https://github.com/hbollon/go-edlib

## Why wazero (not wasmtime-go)

[`wazero`] is the pure-Go, no-CGO WebAssembly runtime — the same
`go build` that produces every other Go binary in the ecosystem also
builds this adapter. That is the ecosystem-native default and the one
this adapter commits to.

The alternative — CGO-linked [`wasmtime-go`] — supports the Component
Model natively, but its upstream repo was archived in early 2025 in
favour of code-generated bindings via the Bytecode Alliance's
`go-modules`, and pulling in a CGO dep for the sake of a benchmark
harness is a heavy trade. wazero's lack of Component Model support is
the one gap in this story and we work around it as described below.

[`wasmtime-go`]: https://github.com/bytecodealliance/wasmtime-go

## Component Model vs. core module

wazero runs core wasm modules; it does not (yet) instantiate a wasm
**component**. `cargo component build` in `../../component/rust-host/`
produces a component `.wasm`, so we take a two-step path:

1. **Extract the core module** with `wasm-tools component unbundle`.
   The StringCheese component wraps two modules — module 0 is the
   actual algorithm kernel (imports `wasi_snapshot_preview1`, exports
   the four WIT interfaces the top-level `component/wit/stringcheese.wit`
   declares); module 1 is a small preview1→preview2 adapter shim we
   don't need in this setup.
2. **Run module 0 in wazero** with `wasi_snapshot_preview1` imports
   supplied by wazero's built-in shim.

The adapter does step 1 automatically on first construction — it looks
for the core-module cache under
`component/rust-host/target/wasm32-wasip1/release/unbundled/`; on a
cache miss it shells out to `wasm-tools component unbundle` to
populate it. wasm-tools is not a Go dependency; it is a Rust tool the
repo already needs for the component build itself, so shipping it as
an install-once step is not additional friction.

## Prerequisites

* **Go 1.22 or newer.** The adapter uses only stdlib + three tagged
  external deps.
* **Rust toolchain with `wasm32-wasip1`** and **`cargo-component`** —
  needed once to build the component `.wasm` that the adapter loads.
  See `../../component/README.md` for the full toolchain setup.
* **`wasm-tools`** on PATH — needed once to extract the core module
  from the component. Install with `cargo install wasm-tools`.
* **wazero ≥ v1.9.0** — pulled in via `go.mod`; no separate install.

## Build the wasm component

The Go adapter never rebuilds the wasm — it loads a pre-built `.wasm`
at construction time and fails fast (or `Skip`s the test) if the file
is missing. So one-shot from a fresh clone:

```bash
cd component/rust-host
cargo component build --release
```

That produces:

```
component/rust-host/target/wasm32-wasip1/release/stringcheese_component_host.wasm
```

The Go adapter discovers this file automatically. If you have the
component elsewhere, either:

* pass an explicit `ComponentPath` in `adapter.Options`,
* set the `STRINGCHEESE_WASM` environment variable, or
* set `STRINGCHEESE_CORE_WASM` to point directly at a pre-extracted
  core module (skips the wasm-tools invocation entirely).

## Install the Go side

From this directory (`bench-adapters/go/`):

```bash
go mod download
```

## Run the benchmarks

```bash
go test -bench=. -benchmem ./bench/...
```

Run a single bench file:

```bash
go test -bench=. -benchmem ./bench/ -run '^$' -bench BenchmarkStringCheeseLevenshtein
go test -bench=BenchmarkAgnivadeLevenshtein -benchmem ./bench/
go test -bench=BenchmarkStringCheeseHamming -benchmem ./bench/
go test -bench=BenchmarkStringCheeseJaro -benchmem ./bench/
go test -bench=BenchmarkStringCheeseOSA -benchmem ./bench/
```

Quick sanity run (one iteration per subtest, useful when validating a
build):

```bash
go test -bench=BenchmarkStringCheeseLevenshtein -benchtime=1x ./bench/...
```

Longer, statistically more meaningful run (10 seconds per subtest):

```bash
go test -bench=. -benchmem -benchtime=10s ./bench/...
```

`go test`'s bench output goes to stdout. Pipe through
[`benchstat`](https://pkg.go.dev/golang.org/x/perf/cmd/benchstat) for
side-by-side comparisons across two runs.

## Run the smoke test

The adapter ships a `TestSmoke` that exercises every WIT function it
binds against small hand-crafted inputs. It is not a correctness
suite (that lives in `crates/stringcheese-corpus/`) — it's a fast
end-to-end signal that wasm-tools extraction, wazero instantiation,
canonical-ABI marshalling, and return-area readback are all wired up:

```bash
go test ./adapter/ -run TestSmoke -v
```

The test `Skip`s cleanly if the component `.wasm` has not been built.

## Reading the output

Go's `-benchmem` adds two columns to each row: `B/op` (bytes allocated
per op) and `allocs/op` (allocation count per op). For a StringCheese
row, `allocs/op` will include the `cabi_realloc` guest allocations for
each `list<u8>` input plus any Go-side wazero call bookkeeping; for
`agnivade` and `go-edlib`, it will show the pure-Go allocation cost.
Both are load-bearing signals — an implementation that "wins" on
`ns/op` while losing on `allocs/op` under GC pressure may not win in
your program.

Every benchmark's `b.Run` sub-name follows a uniform
`<impl>/<regime>/len<NNNN>` scheme — sortable, greppable, matches the
Python adapter's `benchmark.group` labels.

## What's being measured

Each bench file crosses **input length × similarity regime ×
implementation**. The matrix mirrors the Rust, Python, and JS
adapters exactly:

* **Lengths:** 8, 32, 128, 512, 2048.
* **Regimes:** `random` (independent inputs), `similar` (5% mixed
  edits), `identical` (byte-equal — the short-circuit corner).
* **Implementations:** vary per file — see the file-header comment
  in each `*_test.go`.

The full four-file matrix is:

| Algorithm              | StringCheese | agnivade/levenshtein | hbollon/go-edlib |
|------------------------|:------------:|:--------------------:|:----------------:|
| Levenshtein            | yes          | yes                  | yes              |
| Hamming                | yes          | —                    | yes              |
| Jaro                   | yes          | —                    | yes              |
| Jaro-Winkler           | yes          | —                    | yes              |
| OSA (restricted Damerau) | yes        | —                    | yes              |
| Full Damerau           | **N/A** ¹    | —                    | yes              |

¹ Full unrestricted Damerau is not exposed by the StringCheese WIT
component — the underlying Rust kernel needs a `HashMap`, which pulls
in `getrandom` on `wasm32-*`. See `../../component/README.md`
"Deliberately not exposed". `BenchmarkStringCheeseDamerau` asserts
that `DamerauDistance` still returns `ErrDamerauNotExposed` (so the
skip is intentional, not a wire that's silently gone dead) and then
`b.Skip`s.

Notable ecosystem observations:

* **agnivade/levenshtein** covers exactly one algorithm — Levenshtein —
  and does it well. Nothing else in its package.
* **go-edlib** is the ecosystem's kitchen-sink pure-Go string-metric
  library: Levenshtein, both Damerau variants, Hamming, Jaro,
  Jaro-Winkler, LCS, Q-gram, cosine, Jaccard. Every non-Levenshtein
  cell here goes against go-edlib because there is no ecosystem-native
  competitor for those algorithms.
* Alternative Levenshtein implementations exist
  (`github.com/agext/levenshtein`, `github.com/dgryski/trigram` for a
  different problem shape) but adding them here would just crowd the
  chart without materially widening the comparison — those two
  contestants already bracket the pure-Go ecosystem's performance
  envelope.

## What's **not** being measured (READ THIS)

This adapter is not the fair-fight DP-kernel comparison the Rust
adapter's `_vs_strsim_generic_bytes` group is. It answers a different
question:

> **"Should I use StringCheese through wasm from a Go program instead
> of a pure-Go implementation?"**

That is a whole-stack question and its answer is a whole-stack
comparison. Every StringCheese timing here includes:

1. Parameter lowering across the wasm component boundary. Each
   `list<u8>` input is copied into the guest's linear memory via
   `cabi_realloc` + `wazero.api.Memory.Write`; the guest owns and
   frees the buffer inside its Rust wrapper.
2. Guest DP execution (the same StringCheese kernel measured in
   `bench-adapters/rust/`).
3. Result lifting across the boundary — a `u32` / `f64` gets unboxed
   from wazero's `[]uint64` result slice; `Hamming`'s
   `result<u32, string>` and `LevenshteinWithin`'s `variant` return
   through a guest return-area we read out of linear memory.
4. `cabi_post_*` bookkeeping for any function whose return contains
   dynamic allocations (Hamming's err branch string in particular).

Steps (1), (3), (4) are the "FFI cost". At short input lengths the
FFI cost dominates and the pure-Go competitors win comfortably. At
longer inputs the algorithmic work (step 2) dominates and the
comparison starts to reflect kernel quality. See the umbrella
`../README.md` "The FFI break" section for the shared framing.

Deliberately excluded from the timing:

* **Wazero runtime + core-module load + instantiate.** Instantiate
  takes on the order of ~50 ms (compile, link WASI shim, allocate
  linear memory, resolve every WIT export). One `*adapter.StringCheese`
  is constructed per package on first bench call via a `sync.Once`
  in `bench/fixtures.go`; subsequent benchmarks reuse it.
* **Core-module extraction.** `wasm-tools component unbundle` runs
  at most once per process, and only on a cache miss. Its cost is not
  measured.
* **Export lookup.** Every WIT function is resolved to a cached
  `api.Function` at `adapter.New` time. A lookup per call would add
  per-call map cost that is not part of what a well-written
  application would pay.
* **Corpus generation.** Each bench file materialises its
  `(length, regime)` pairs once at the top of the benchmark function,
  before the `b.Run` subtests. The SplitMix64 generator in
  `bench/inputs.go` is a byte-for-byte port of the Rust adapter's,
  matching the Python and JS adapters' seeds.
* **`[]byte → string` conversion.** go-edlib and agnivade take
  `string`; StringCheese takes `[]byte`. Each bench pre-computes both
  representations once so only the library's own work is timed.

## Corpus determinism

The `rng` in `bench/inputs.go` is a byte-for-byte port of the Rust
adapter's SplitMix64 (same state update, same scramble constants, same
shift amounts, same seed derivation). Running the Go, Python, and Rust
adapters against the same `(length, salt)` gives the same corpus on
all sides.

Per-file bench salts vs. the other adapters:

| Bench file           | Rust adapter salts   | Python adapter salts | Go adapter salts     |
|----------------------|----------------------|----------------------|----------------------|
| levenshtein          | (0xA1, 0xA2, 0xA3)   | (0xB1, 0xB2, 0xB3)   | (0xF1, 0xF2, 0xF3)   |
| hamming              | (0xC1, 0xC2, 0xC3)   | (0xC1, 0xC2, 0xC3)   | (0xC1, 0xC2, 0xC3) ² |
| jaro / jaro-winkler  | (0xD1, 0xD2, 0xD3)   | (0xD1, 0xD2, 0xD3)   | (0xD1, 0xD2, 0xD3) ² |
| osa / damerau        | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)   |

² Hamming and Jaro share salts with the Rust adapter on purpose:
Hamming needs equal-length inputs (all three adapters must see the
same mismatch positions to compare fairly), and Jaro's match-window
behaviour is best observed against a known-shared corpus.

## Algorithm-variant pairings

Same discipline as the Rust and Python adapters — the pairings are
load-bearing:

| Common name            | StringCheese              | agnivade/levenshtein   | hbollon/go-edlib               |
|------------------------|---------------------------|------------------------|--------------------------------|
| Levenshtein            | `LevenshteinDistance`     | `ComputeDistance`      | `LevenshteinDistance`          |
| OSA                    | `OSADistance`             | —                      | `OSADamerauLevenshteinDistance`|
| Full Damerau           | *(N/A — see above)*       | —                      | `DamerauLevenshteinDistance`   |
| Jaro                   | `JaroSimilarity`          | —                      | `JaroSimilarity` (float32)     |
| Jaro-Winkler (classic) | `JaroWinklerSimilarity`   | —                      | `JaroWinklerSimilarity`        |
| Hamming                | `HammingDistance`         | —                      | `HammingDistance`              |

Note that `go-edlib.DamerauLevenshteinDistance` is the **full
unrestricted Damerau** (its source file confirms the O(n·m) DP with
unrestricted transpositions), matching what a StringCheese `Damerau`
would compute if the WIT component exposed it. Pairing it against
StringCheese's `OSADistance` would put two different algorithms on the
same axis — the benchmark IDs keep the two variants in separate
groups (`osa/*` vs. `damerau/*`) to make this explicit at the
results-table level.

## Deferred / future work

* **Component Model in wazero.** Once wazero grows Component Model
  support (upstream tracking issue: [tetratelabs/wazero#2049]), the
  wasm-tools extraction step goes away entirely and the adapter can
  load the component directly with the generated WIT bindings. The
  API surface of this package will not change; only its innards.
* **Full Damerau at the WIT boundary.** Once the underlying kernel
  gets a wasm-portable hash story (`getrandom`-free `HashMap` or a
  hand-rolled sparse table), the WIT world will grow a `damerau`
  function and `BenchmarkStringCheeseDamerau` will unskip.
* **rapidfuzz-equivalent.** There is no rapidfuzz for pure Go. If a
  Go binding to rapidfuzz's C++ engine ever ships with a licence
  compatible with this repo, it will slot in as a fourth contestant
  the same way `rapidfuzz` does in the Python adapter — the addition
  is scaffolding-only.
* **BatchComparator / prepared-pattern shape.** The wasm boundary
  currently rebuilds the pattern automaton per call for every
  algorithm. A future WIT interface with a `pattern` resource would
  amortise the per-pattern cost; the Rust adapter's rapidfuzz slot
  hints at what that will look like.

[tetratelabs/wazero#2049]: https://github.com/tetratelabs/wazero/issues/2049

## Pinned dependency versions

Recorded in `go.mod` and locked in `go.sum`:

| Module                                | Version |
|---------------------------------------|---------|
| `github.com/tetratelabs/wazero`       | v1.9.0  |
| `github.com/agnivade/levenshtein`     | v1.2.1  |
| `github.com/hbollon/go-edlib`         | v1.7.0  |

Bumping any of these is a one-liner in `go.mod` + `go mod tidy`; the
adapter API surface has no dependency on internals of any of them.

## Non-goals

Same as the umbrella `../README.md`:

* **Not a correctness suite.** Golden-dataset infrastructure lives
  in `crates/stringcheese-corpus/`. If a distance number disagrees
  between two implementations at the same `(length, seed)` corpus,
  that is a bug to file against the toolkit's differential-test
  harness, not a bench-adapter regression.
* **Not a scoreboard.** Numbers depend on CPU microarchitecture,
  wazero version, Go version, and OS scheduling. A cross-machine
  comparison of raw times is meaningless; a same-machine comparison
  of two implementations is meaningful. Any docs derived from this
  harness should show relative numbers, not absolute ones.

## CI

There is deliberately no default CI job that runs these benches: the
noise floor on a shared runner is too high to produce actionable
numbers, and the full matrix takes minutes. If a workflow does run
them (e.g. spot-check on merge-queue), it should use
`continue-on-error: true` and only fire on `push`, not on
`pull_request`.
