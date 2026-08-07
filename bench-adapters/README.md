# StringCheese Bench Adapters

This directory holds StringCheese's **comparative-benchmark harness** — the
head-to-head performance-measurement subsystem `docs/DESIGN.md` commits
to in its "Comparative Library Benchmarking" section. Each language
adapter lets StringCheese's own criterion suite be compared apples-to-apples
against widely-used sequence-comparison libraries in that language's
ecosystem.

## Why a separate directory

The main workspace at `/Cargo.toml` deliberately does not depend on any
external comparison library. Adding `strsim` or `rapidfuzz` as a
StringCheese-crate `dev-dependency` would pull those crates into every
downstream consumer's dependency graph the moment a workspace-wide
`cargo test --all-targets` ran; publishing users of StringCheese would see
new transitive dependencies on library releases they never asked for.

Putting the adapters under a sibling directory that is **not** a
workspace member solves that: adapters build only when `cargo bench`
is invoked from inside the adapter directory itself, and their
dependency trees are isolated in per-adapter `Cargo.lock` files.

## Design contract

Every adapter in this subtree — whatever language it targets — MUST:

1. **Preload inputs** outside the timing region. The corpus-generation
   cost is not part of any measurement.
2. **Separate startup from steady-state.** Warmup runs are what
   criterion (or its language-equivalent) handles by default; the
   adapter must not add its own first-call overhead into the
   steady-state sample.
3. **Use the same input matrix** as `stringcheese-bench` — the same
   input lengths, the same similarity regimes (random / similar /
   identical), the same seeds where practical. A number that says
   "StringCheese at length 32 on similar input" must correspond to the
   same corpus in both places.
4. **Never compare differently defined algorithms under the same
   label.** OSA and full Damerau are different algorithms; several
   Jaro-Winkler configurations are different algorithms; restricted
   and unrestricted Damerau are different algorithms. Every adapter
   must document the variant it lands on and refuse to cross-pair.
5. **Emit results in a common machine-readable format.** For criterion
   (Rust) that is the `target/criterion/**/estimates.json` tree; for
   other harnesses this repository will grow adapter-specific writers
   as those language slots ship.

## Language slots

| Language     | Status              | Directory                  | Comparison libraries                  |
|--------------|---------------------|----------------------------|---------------------------------------|
| Rust         | shipping (v0.1)     | `bench-adapters/rust/`     | `strsim` 0.11, `rapidfuzz` 0.5        |
| Python       | shipping (v0.2)     | `bench-adapters/python/`   | `python-Levenshtein`, `jellyfish`, `rapidfuzz` |
| JavaScript   | shipping (v0.2)     | `bench-adapters/js/`       | `fastest-levenshtein`, `js-levenshtein`, `natural`, `string-similarity` |
| Go           | shipping (v0.3)     | `bench-adapters/go/`       | `agnivade/levenshtein` 1.2, `hbollon/go-edlib` 1.7 |
| Java         | shipping (v0.3)     | `bench-adapters/java/`     | `apache-commons-text` 1.13, `info.debatty/java-string-similarity` 2.0 |
| C++          | planned (v0.3)      | `bench-adapters/cpp/`      | `rapidfuzz-cpp`, `edlib`              |

The Rust, Python, JavaScript, Go, and Java slots ship in v0.1/v0.2/v0.3.
The remaining slots are recorded here so that the directory layout is
committed early and the sequencing matches `docs/DESIGN.md`'s
"Implementation Sequence".

## The FFI break — Rust vs. everyone else

The Rust adapter measures the StringCheese kernels **statically linked**
against the comparison crates: the whole binary is Rust and there is
no boundary between the timing loop and the algorithm. That is the
kernel-quality comparison the toolkit's own criterion suite also
measures.

Every non-Rust adapter (Python first, then JavaScript, Java, Go, C++)
loads StringCheese as the WebAssembly component built by
`cargo component build --release` under `component/rust-host/`. The
per-call cost includes parameter lowering across the wasm boundary,
guest execution, result lifting, and `post_return` bookkeeping. At
short input lengths that FFI tail dominates; at long input lengths the
kernel work does. Each non-Rust adapter's README documents the
crossover behaviour for its host language's ecosystem contestants —
the point of these adapters is a whole-stack answer to "should I use
StringCheese from this language", not a DP-kernel-only comparison.

## Running Rust adapters

```
cargo bench --manifest-path bench-adapters/rust/Cargo.toml
```

See `bench-adapters/rust/README.md` for the per-adapter matrix,
representation caveats, and interpretation notes.

## Running Python adapters

The Python adapter loads the wasm component built by
`cargo component build --release`, so build the component first:

```
cd component/rust-host && cargo component build --release
```

Then install the Python dependencies and run pytest-benchmark:

```
cd bench-adapters/python
pip install -r requirements.txt
pytest --benchmark-only
```

See `bench-adapters/python/README.md` for prerequisites, per-adapter
matrix, FFI-cost caveats, and interpretation notes.

## Running Go adapters

The Go adapter loads the same wasm component. wazero (the pure-Go
runtime the adapter uses) does not yet run Component Model wasm
directly, so the adapter shells out to `wasm-tools component
unbundle` on first construction to extract the core module. Install
`wasm-tools` once (`cargo install wasm-tools`) if it is not already
on PATH.

```
cd component/rust-host && cargo component build --release
cd bench-adapters/go
go mod download
go test -bench=. -benchmem ./bench/...
```

See `bench-adapters/go/README.md` for prerequisites, per-adapter
matrix, FFI-cost caveats, and the wazero / Component Model gap
writeup.

## Running Java adapters

The Java adapter loads the same wasm component and follows the same
`wasm-tools component unbundle` pattern as the Go adapter — Chicory
(the pure-Java runtime the adapter uses) does not yet run Component
Model wasm directly.

```
cd component/rust-host && cargo component build --release
cd bench-adapters/java
mvn -o test                                           # smoke tests
mvn -o exec:exec -Djmh.args="LevenshteinBenchmark"    # JMH bench matrix
```

See `bench-adapters/java/README.md` for prerequisites, per-adapter
matrix, FFI-cost caveats, and the Chicory / Component Model gap
writeup.

## Non-goals

* **This is not a correctness suite.** Correctness is the job of
  `crates/stringcheese-corpus/` and the golden-dataset infrastructure.
  Adapter benches assume the algorithms they compare produce the
  same answer on the same input; they do not verify it. Any
  discrepancy the benches surface (e.g. a distance count that
  doesn't line up between two implementations) is a bug to file
  against the toolkit's differential-test harness, not a
  bench-adapter regression.
* **This is not a scoreboard.** These benches produce numbers on
  the machine that ran them; those numbers depend on CPU
  microarchitecture, cache size, compiler version, and OS
  scheduling. A cross-machine comparison of raw times is
  meaningless; a same-machine comparison of two implementations
  is meaningful. The published `docs/` output from this harness
  will always show relative numbers, never absolute ones.
