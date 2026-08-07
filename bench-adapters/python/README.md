# StringCheese Python Bench Adapters

Head-to-head `pytest-benchmark` benches comparing StringCheese — loaded
as a WebAssembly component via [`wasmtime-py`] — against the
Python ecosystem's three go-to string-distance libraries:
[`python-Levenshtein`], [`jellyfish`], and [`rapidfuzz`].

This subtree is deliberately **not** wired into the outer Cargo
workspace. See `../README.md` for the umbrella "why each adapter is
standalone" rationale and the shared design contract every adapter
follows.

[`wasmtime-py`]: https://docs.wasmtime.dev/lang-python.html
[`python-Levenshtein`]: https://pypi.org/project/python-Levenshtein/
[`jellyfish`]: https://pypi.org/project/jellyfish/
[`rapidfuzz`]: https://pypi.org/project/rapidfuzz/

## Prerequisites

* **Python ≥ 3.10.** The adapter uses `list[int]` and PEP 604 union
  syntax; older Pythons will not parse. Development target is
  CPython 3.13, but 3.10, 3.11, 3.12 also work.
* **Rust toolchain with `wasm32-wasip1`** and **`cargo-component`** —
  needed once to build the component `.wasm` that the adapter loads.
  See `../../component/README.md` for the full toolchain setup.
* **`wasmtime` (Python package) ≥ 41.0.0** — earlier releases lack the
  full `wasmtime.component` surface (Component / Linker / Instance /
  `add_wasip2`) the adapter relies on.
* **`pytest`, `pytest-benchmark`, `python-Levenshtein`, `jellyfish`,
  `rapidfuzz`** — all pinned in `requirements.txt`.

## Build the wasm component

The Python adapter never rebuilds the wasm — it loads a pre-built
`.wasm` at construction time and fails fast if the file is missing. So
one-shot from a fresh clone:

```bash
cd component/rust-host
cargo component build --release
```

That produces:

```
component/rust-host/target/wasm32-wasip1/release/stringcheese_component_host.wasm
```

The adapter discovers this file automatically via a path relative to
the `stringcheese_adapter.py` module. If you have the component
elsewhere, either:

* pass an explicit `wasm_path=` to `StringCheese(...)`, or
* set the `STRINGCHEESE_WASM` environment variable.

## Install the Python side

From this directory (`bench-adapters/python/`):

```bash
python3 -m venv .venv
. .venv/bin/activate
pip install -r requirements.txt
```

Or with `uv`:

```bash
uv venv
uv pip install -r requirements.txt
```

Or Poetry, if that is your workflow:

```bash
poetry install --no-root
```

(`--no-root` because the subtree is a bench harness, not an installed
library.)

## Run the benchmarks

```bash
pytest benches/ --benchmark-only
```

Or, since `pyproject.toml` sets `testpaths = ["benches"]`:

```bash
pytest --benchmark-only
```

Run a single bench file:

```bash
pytest benches/levenshtein_bench.py --benchmark-only
pytest benches/hamming_bench.py     --benchmark-only
pytest benches/jaro_bench.py        --benchmark-only
pytest benches/damerau_bench.py     --benchmark-only
```

Run just one implementation across all lengths & regimes:

```bash
pytest benches/levenshtein_bench.py::test_stringcheese --benchmark-only
```

pytest-benchmark writes a table to stdout and — if you pass
`--benchmark-save=<label>` — a JSON blob to `.benchmarks/`. See the
[pytest-benchmark docs] for the full comparison workflow.

[pytest-benchmark docs]: https://pytest-benchmark.readthedocs.io/

## Reading the output

pytest-benchmark's default table columns (as configured in
`pyproject.toml`):

```
------------------------------------------------------------------------------------
Name                        Min       Median      Mean     StdDev  Rounds   Iters
------------------------------------------------------------------------------------
test_stringcheese ...      12.34 us  13.56 us   13.98 us   1.2 us   500      1
test_python_levenshtein…    1.23 us   1.45 us    1.51 us   0.1 us  5000      1
test_jellyfish ...          2.34 us   2.67 us    2.72 us   0.2 us  2500      1
test_rapidfuzz ...          0.98 us   1.12 us    1.15 us   0.1 us  8000      1
```

Read the **median** column — it is the least sensitive to the tail
noise every timing loop picks up. The `StdDev` is a rough
signal-to-noise indicator: if it is a large fraction of the median,
rerun the bench with `--benchmark-min-rounds=10000` or on a quieter
machine before drawing conclusions.

Every benchmark is tagged with a `benchmark.group` label of the shape
`<algorithm>/<regime>/len<NNNN>`, so pytest-benchmark's grouped output
puts all implementations for a given (algorithm, regime, length) cell
side-by-side.

## What's being measured

Each bench file crosses **input length × similarity regime ×
implementation**. The matrix mirrors the Rust adapter and
`stringcheese-bench` exactly:

* **Lengths:** 8, 32, 128, 512, 2048.
* **Regimes:** `random` (independent inputs), `similar` (5% mixed
  edits), `identical` (byte-equal — the short-circuit corner).
* **Implementations:** vary per file — see the docstring at the top
  of each `*_bench.py`.

The full four-file matrix is:

| Algorithm            | StringCheese | python-Levenshtein | jellyfish | rapidfuzz |
|----------------------|:------------:|:------------------:|:---------:|:---------:|
| Levenshtein          | yes          | yes                | yes       | yes       |
| Levenshtein (k = 3)  | yes          | —                  | —         | yes       |
| Hamming              | yes          | yes                | yes       | —         |
| Jaro                 | yes          | yes                | yes       | —         |
| Jaro–Winkler         | yes          | yes                | yes       | —         |
| OSA (restricted Damerau) | yes      | —                  | —         | yes       |
| Full Damerau         | **N/A** ¹    | —                  | yes       | yes       |

¹ Full unrestricted Damerau is not exposed by the StringCheese WIT
component — the underlying Rust kernel needs a `HashMap`, which pulls
in `getrandom` on `wasm32-*`. See `../../component/README.md`
"Deliberately not exposed". The bench test is `@pytest.mark.skip`ped
with a reason so the run stays honest about what did and did not run.

## What's **not** being measured (READ THIS)

This adapter is not the fair-fight DP-kernel comparison the Rust
adapter's `_vs_strsim_generic_bytes` group is. It answers a different
question:

> **"Should I use StringCheese through wasm from a Python program
> instead of a native C/Rust extension?"**

That is a whole-stack question and its answer is a whole-stack
comparison. Every StringCheese timing here includes:

1. Parameter lowering across the wasm component boundary (a Python
   `bytes` gets copied into the guest's linear memory).
2. Guest DP execution (the same StringCheese kernel measured in
   `bench-adapters/rust/`).
3. Result lifting across the boundary (a wasm `u32` / `f64` gets
   boxed into a Python int/float).
4. `post_return` bookkeeping the Component Model requires between
   successive calls into the same function.

Steps (1), (3), (4) are the "FFI cost". At short input lengths the
FFI cost dominates and the native C extensions win comfortably. At
longer inputs the algorithmic work (step 2) dominates and the
comparison starts to reflect kernel quality. The crossover length is
the interesting datapoint per algorithm — pytest-benchmark's per-length
groups make it visible.

Deliberately excluded from the timing:

* **Component instantiate.** The `wasmtime.Engine`, `Store`,
  `Component`, `Linker`, and `Instance` are constructed once per pytest
  session in a `@pytest.fixture(scope="session")` (see
  `benches/conftest.py`). Instantiate takes ~tens of milliseconds and
  would swamp every per-call measurement if paid per test.
* **Export lookup.** Every WIT function is resolved to a cached
  `wasmtime.Func` at `StringCheese.__init__` time. A lookup per call
  would add per-call `dict` cost that is not part of what a
  well-written application would pay.
* **Corpus generation.** Each bench file materialises its
  `(length, regime)` pairs once in a `@pytest.fixture(scope="module")`
  and re-uses them across pytest-benchmark's rounds. The
  `SplitMix64`-based generator in `benches/_inputs.py` is fast enough
  that this technically does not matter, but the discipline mirrors the
  Rust adapter's design contract.
* **`str → bytes` conversion.** The native libraries take `str`,
  StringCheese takes `bytes`. Each bench decodes / encodes once
  **outside** the `benchmark(...)` call so only the library's own work
  is timed.

## Corpus determinism

The `_Rng` in `benches/_inputs.py` is a byte-for-byte port of the Rust
adapter's `SplitMix64` (same state update, same scramble constants,
same shift amounts, same seed derivation). Running the Python and Rust
adapters against the same `(length, salt)` gives the same corpus on
both sides.

Per-file bench salts are distinct from the Rust adapter's per-file
salts so a debugging session that hits an unlikely coincidence in one
corpus is very unlikely to hit the same in the other:

| Bench file           | Rust adapter salts   | Python adapter salts |
|----------------------|----------------------|----------------------|
| levenshtein          | (0xA1, 0xA2, 0xA3)   | (0xB1, 0xB2, 0xB3)   |
| hamming              | (0xC1, 0xC2, 0xC3)   | (0xC1, 0xC2, 0xC3) ² |
| jaro / jaro-winkler  | (0xD1, 0xD2, 0xD3)   | (0xD1, 0xD2, 0xD3) ² |
| osa / damerau        | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)   |

² Hamming and Jaro share salts with the Rust adapter on purpose:
Hamming needs equal-length inputs (the two adapters must see the same
mismatch positions to compare fairly), and Jaro's match-window
behaviour is best observed against a known-shared corpus.

## Algorithm-variant pairings

Same as the Rust adapter's "Algorithm-variant caveats" section, the
pairings this file lands on are load-bearing:

| Common name            | StringCheese                     | python-Levenshtein   | jellyfish                        | rapidfuzz                                 |
|------------------------|----------------------------------|----------------------|----------------------------------|-------------------------------------------|
| Levenshtein            | `levenshtein_distance` (unit)    | `Levenshtein.distance` | `jellyfish.levenshtein_distance` | `rapidfuzz.distance.Levenshtein.distance` |
| OSA                    | `osa_distance`                   | —                    | —                                | `rapidfuzz.distance.OSA.distance`         |
| Full Damerau           | *(N/A — see above)*              | —                    | `damerau_levenshtein_distance`   | `rapidfuzz.distance.DamerauLevenshtein.distance` |
| Jaro                   | `jaro_similarity`                | `Levenshtein.jaro`   | `jellyfish.jaro_similarity`      | *(not wired — followup)*                  |
| Jaro–Winkler (classic) | `jaro_winkler_similarity`        | `Levenshtein.jaro_winkler` | `jellyfish.jaro_winkler_similarity` | *(not wired — followup)*        |
| Hamming                | `hamming_distance`               | `Levenshtein.hamming` | `jellyfish.hamming_distance`    | *(not wired — followup)*                  |

Note in particular that `jellyfish.damerau_levenshtein_distance` is the
**full unrestricted Damerau**, matching what a StringCheese `Damerau`
would compute if the WIT component exposed it. Pairing it against
StringCheese's `Osa` would put two different algorithms on the same
axis — the pytest tests keep the two variants in separate benchmark
groups (`osa/*` vs. `damerau/*`) to make this explicit at the results
table's axis level.

## FFI cost — the whole-stack picture

Because every StringCheese-through-wasm call pays parameter lowering +
result lifting + `post_return` bookkeeping on top of the underlying DP
kernel, the head-to-head numbers here are **not** apples-to-apples with
what a Rust program calling the StringCheese crates directly, or a JS
program calling a JCO-transpiled StringCheese module, would see. Rules
of thumb the runs on this adapter empirically confirm:

* **At `len = 8`,** the FFI cost is a large fraction of the total.
  The native C extensions win by a lot.
* **At `len = 128`–`512`,** the DP work starts to dominate and the
  gap narrows; the StringCheese cell can be competitive or better on
  algorithms where the underlying kernel is stronger than the C
  extension's implementation.
* **At `len = 2048`,** the DP work dwarfs the FFI cost and the
  comparison is essentially a kernel-quality comparison.

If your workload is many short strings, use a native C extension. If
your workload has long strings or benefits from an algorithm the
native libraries don't expose (bounded distance, OSA), the FFI cost
amortises and StringCheese-via-wasm becomes a serious option even
from Python.

A future iteration of this adapter could switch to a `wasi:preview1` +
`memory-shared` FFI mode that avoids the copy on parameter lowering
for large `list<u8>`; that is an open design question for
`wasmtime-py` and is out of scope for v0.2.

## Non-goals

Same as the umbrella `../README.md`:

* **Not a correctness suite.** Golden-dataset infrastructure lives in
  `crates/stringcheese-corpus/`. If a distance number disagrees
  between two implementations at the same `(length, seed)` corpus,
  that is a bug to file against the toolkit's differential-test
  harness, not a bench-adapter regression.
* **Not a scoreboard.** Numbers depend on CPU microarchitecture,
  wasmtime version, and OS scheduling. A cross-machine comparison of
  raw times is meaningless; a same-machine comparison of two
  implementations is meaningful. Any docs derived from this harness
  should show relative numbers, not absolute ones.

## CI

There is deliberately no default CI job that runs these benches:
pytest-benchmark's noise floor on a shared runner is too high to
produce actionable numbers, and the run takes minutes-to-tens-of-minutes
on the full matrix. If a workflow *does* run them (for e.g. spot-check
on the merge queue), it should use `continue-on-error: true` and only
fire on `push`, not on `pull_request`.
