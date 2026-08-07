# StringCheese Java Bench Adapter

Head-to-head [JMH] micro-benchmarks comparing StringCheese — loaded as
a WebAssembly component via [Chicory] — against the JVM ecosystem's
two most widely-used string-distance libraries:
[`apache-commons-text`] and [`info.debatty/java-string-similarity`].

This subtree is deliberately **not** a Rust workspace member and it is
not wired into `pom.xml` of any other module. See `../README.md` for
the umbrella "why each adapter is standalone" rationale and the shared
design contract every adapter follows.

[JMH]: https://openjdk.org/projects/code-tools/jmh/
[Chicory]: https://github.com/dylibso/chicory
[`apache-commons-text`]: https://commons.apache.org/proper/commons-text/
[`info.debatty/java-string-similarity`]: https://github.com/tdebatty/java-string-similarity

## Why Chicory (not wasmtime-java)

[Chicory][] is the pure-Java, no-JNI WebAssembly runtime — the same
`javac`/`java` that produces every other JVM artefact in the
ecosystem also runs this adapter. That is the ecosystem-native default
and the one this adapter commits to, mirroring the pure-Go stance
`bench-adapters/go/` takes with wazero.

The alternative — JNI-linked wasmtime-java — supports the Component
Model natively but requires shipping a platform-specific native
library, plus JNI wiring that not every JVM the harness is expected
to run on will cooperate with (in particular no ergonomic story for
GraalVM native-image + JNI). Sticking to Chicory keeps the adapter
buildable with a plain `mvn -o test`, no separate install step.

## Component Model vs. core module

Chicory 1.7.3 runs core wasm modules; it does not yet instantiate a
wasm **component** ([tracking upstream][chicory-cm-issue]). `cargo
component build` in `../../component/rust-host/` produces a
component `.wasm`, so we take a two-step path — the same path the Go
adapter takes:

1. **Extract the core module** with `wasm-tools component unbundle`.
   The StringCheese component wraps a handful of modules — module 0
   is the actual algorithm kernel (imports `wasi_snapshot_preview1`,
   exports the four WIT interfaces the top-level `component/wit/stringcheese.wit`
   declares); modules 1..N are small preview1↔preview2 adapter shims
   we don't need in this setup.
2. **Run module 0 in Chicory** with `wasi_snapshot_preview1` imports
   supplied by Chicory's built-in preview1 shim.

The adapter does step 1 automatically on first construction — it
looks for the core-module cache under
`component/rust-host/target/wasm32-wasip1/release/unbundled/`; on a
cache miss it shells out to `wasm-tools component unbundle` to
populate it. `wasm-tools` is not a Maven dependency; it is a Rust tool
the repo already needs for the component build itself, so shipping it
as an install-once step is not additional friction.

[chicory-cm-issue]: https://github.com/dylibso/chicory/issues

## Prerequisites

* **JDK 17 or newer.** The adapter's source level targets 17 for
  broad compatibility (Java 17 is the current long-term-supported
  baseline the JVM ecosystem consolidates around). Recent JMH
  releases require the same or newer.
* **Maven 3.9+.** The build is Maven-driven.
* **Rust toolchain with `wasm32-wasip1`** and **`cargo-component`** —
  needed once to build the component `.wasm` the adapter loads. See
  `../../component/README.md` for the full toolchain setup.
* **`wasm-tools`** on PATH — needed once to extract the core module
  from the component. Install with `cargo install wasm-tools`.
* **Chicory ≥ 1.7.3** — pulled in via `pom.xml`; no separate install.

## Build the wasm component

The Java adapter never rebuilds the wasm; it loads a pre-built
`.wasm` at construction time and fails fast (or aborts the smoke test
under JUnit Assumptions) if the file is missing. One-shot from a
fresh clone:

```bash
cd component/rust-host
cargo component build --release
```

That produces:

```
component/rust-host/target/wasm32-wasip1/release/stringcheese_component_host.wasm
```

The Java adapter discovers this file automatically. If you have the
component elsewhere, either:

* pass an explicit `coreWasmPath` / `componentPath` on
  `StringCheese.Options`,
* set the `STRINGCHEESE_WASM` environment variable, or
* set `STRINGCHEESE_CORE_WASM` to point directly at a pre-extracted
  core module (skips the wasm-tools invocation entirely).

## Install the Java side

From this directory (`bench-adapters/java/`):

```bash
mvn -o test-compile
```

This resolves Chicory, JMH, JUnit, commons-text, and
java-string-similarity into your local `~/.m2/` cache and compiles
both the adapter and the benchmark classes.

## Run the smoke test

The adapter ships a `SmokeTest` that exercises every WIT function it
binds against small hand-crafted inputs. It is not a correctness
suite (that lives in `crates/stringcheese-corpus/`) — it's a fast
end-to-end signal that wasm-tools extraction, Chicory instantiation,
canonical-ABI marshalling, and return-area readback are wired
together:

```bash
mvn -o test
```

The test aborts cleanly (via JUnit Assumptions) if the component
`.wasm` has not been built. Fourteen sub-assertions cover every
exposed algorithm, plus the `damerauDistance` assert-then-throw
sentinel path.

## Run the benchmarks

JMH's Main class is wired up as the default exec-maven-plugin target
in `pom.xml`. Pass CLI flags via the `-Djmh.args` system property:

```bash
# Quick sanity run — one warmup iter, one measurement iter, one fork:
mvn -o exec:exec -Djmh.args="LevenshteinBenchmark -i 1 -wi 1 -f 1"

# One benchmark method, one corpus cell:
mvn -o exec:exec -Djmh.args="LevenshteinBenchmark.stringcheese -i 1 -wi 1 -f 1 -p length=8 -p regime=random"

# Full matrix — takes minutes; leaves JMH's default fork/warmup/measure defaults:
mvn -o exec:exec -Djmh.args=".*Benchmark.*"

# Print the JMH help for advanced options:
mvn -o exec:exec -Djmh.args="-h"
```

Note: `-Djmh.args`, **not** `-Dexec.args`. `-Dexec.args` replaces the
whole `<arguments>` list on `exec-maven-plugin`, which drops the
`-classpath` and main-class entries the pom carefully wires up; JMH's
forked JVM then can't find any project class.

`mvn -o exec:exec` runs each benchmark family in one or more forked
JVMs (JMH default `-f 2` unless overridden). The forked JVM warmup
takes a few seconds; per-benchmark timings are measured after
JMH-managed warmup completes.

For a longer, statistically more meaningful run, JMH's built-in
defaults are usually enough; drop `-i / -wi / -f` and let it pick.

## Reading the output

JMH prints one row per `(benchmark, (length), (regime))` triple:

```
Benchmark                                     (length)  (regime)  Mode  Cnt   Score   Error  Units
LevenshteinBenchmark.stringcheese                    8    random  avgt        340863  ±  ... ns/op
LevenshteinBenchmark.commonsText                     8    random  avgt          2241  ±  ... ns/op
LevenshteinBenchmark.debatty                         8    random  avgt          1893  ±  ... ns/op
```

Every benchmark method's row follows a uniform `Class.method` prefix
and every `(length, regime)` cell is emitted separately — sortable
and greppable, matching the Go adapter's `<impl>/<regime>/len<NNNN>`
scheme in spirit if not in exact spelling.

## What's being measured

Each benchmark class crosses **input length × similarity regime ×
implementation**. The matrix mirrors the Rust, Python, JS, and Go
adapters exactly:

* **Lengths:** 8, 32, 128, 512, 2048.
* **Regimes:** `random` (independent inputs), `similar` (5% mixed
  edits), `identical` (byte-equal — the short-circuit corner).
* **Implementations:** vary per class — see the file-header comment
  in each `*Benchmark.java`.

The full six-class matrix is:

| Algorithm              | StringCheese | commons-text            | java-string-similarity     |
|------------------------|:------------:|:-----------------------:|:--------------------------:|
| Levenshtein            | yes          | `LevenshteinDistance`   | `Levenshtein`              |
| Hamming                | yes          | `HammingDistance`       | —                          |
| Jaro                   | yes          | —                       | — ¹                        |
| Jaro-Winkler (classic) | yes          | `JaroWinklerSimilarity` | `JaroWinkler`              |
| OSA (restricted Damerau) | yes        | —                       | `OptimalStringAlignment`   |
| Full Damerau           | **N/A** ²    | —                       | `Damerau`                  |
| LCS distance           | yes          | `LongestCommonSubsequenceDistance` | `LongestCommonSubsequence` |

¹ `java-string-similarity` ships a `JaroWinkler` class that uses
Jaro internally but does not expose Jaro as a public metric. There is
no ecosystem Java library that ships a standalone Jaro similarity,
which matches the Go story with `agnivade` vs. `go-edlib` (only the
kitchen-sink library ships it).

² Full unrestricted Damerau is not exposed by the StringCheese WIT
component — the underlying Rust kernel needs a `HashMap`, which
pulls in `getrandom` on `wasm32-*`. See `../../component/README.md`
"Deliberately not exposed". `DamerauBenchmark.stringcheeseDamerau`
asserts that `damerauDistance` still throws `DamerauNotExposedException`
(so the skip is intentional, not a wire that's silently gone dead)
before feeding a placeholder 0 into JMH's Blackhole.

Notable ecosystem observations:

* **commons-text** is the JVM ecosystem's default. Its
  `similarity` package covers Levenshtein, Hamming, Jaro-Winkler,
  LCS, Jaccard, and cosine — everything an application-layer caller
  would reach for.
* **java-string-similarity** is the ecosystem's kitchen-sink
  library. Beyond what commons-text ships, it adds full unrestricted
  Damerau, Optimal String Alignment, weighted Levenshtein, cosine on
  q-grams, Sørensen-Dice, N-gram, and Ratcliff-Obershelp.
* Alternative Levenshtein implementations exist
  (`edulinq/algorithm`, `jsuereth/scala-arm`, various single-file
  gists) but adding them would just crowd the chart; commons-text
  and debatty already bracket the pure-Java ecosystem's
  performance envelope.

## What's **not** being measured (READ THIS)

Same framing as the Go adapter — this is not a fair-fight DP-kernel
comparison. It answers a whole-stack question:

> **"Should I use StringCheese through wasm from a Java program
> instead of a pure-Java implementation?"**

Every StringCheese timing here includes:

1. Parameter lowering across the wasm component boundary. Each
   `list<u8>` input is copied into the guest's linear memory via
   `cabi_realloc` + `Memory.write`; the guest owns and frees the
   buffer inside its Rust wrapper.
2. Guest DP execution (the same StringCheese kernel measured in
   `bench-adapters/rust/`).
3. Result lifting across the boundary — a `u32` / `f64` gets unboxed
   from Chicory's `long[]` result slice; `Hamming`'s
   `result<u32, string>` and `LevenshteinWithin`'s `variant` return
   through a guest return-area we read out of linear memory.
4. `cabi_post_*` bookkeeping for any function whose return contains
   dynamic allocations (Hamming's err branch string in particular).

Steps (1), (3), (4) are the "FFI cost". At short input lengths the
FFI cost dominates and the pure-Java competitors win comfortably. At
longer inputs the algorithmic work (step 2) dominates and the
comparison starts to reflect kernel quality. See the umbrella
`../README.md` "The FFI break" section for the shared framing.

Deliberately excluded from the timing:

* **Chicory runtime + core-module load + instantiate.** Instantiate
  is expensive (compile, link WASI shim, allocate linear memory,
  resolve every WIT export). One `StringCheese` is constructed per
  JMH fork at `@Setup(Level.Trial)` via `SharedStringCheese`;
  subsequent iteration bodies pay only the per-call ABI cost.
* **Core-module extraction.** `wasm-tools component unbundle` runs
  at most once per JVM, and only on a cache miss. Its cost is not
  measured.
* **Export lookup.** Every WIT function is resolved to a cached
  `ExportFunction` at `StringCheese.create` time. A lookup per call
  would add per-call hash-map cost that is not part of what a
  well-written application would pay.
* **Corpus generation.** Each benchmark subclass materialises its
  `(length, regime)` pair once at `@Setup(Level.Trial)`, before the
  first measurement iteration. The SplitMix64 generator in
  `Corpus.java` is a byte-for-byte port of the Rust adapter's,
  matching the Python / JS / Go adapters' seeds.
* **`byte[] ↔ String` conversion.** commons-text and
  java-string-similarity take `CharSequence` / `String`;
  StringCheese takes `byte[]`. Each benchmark pre-computes both
  representations once so only the library's own work is timed.

## Corpus determinism

The SplitMix64 generator in `Corpus.java` is a byte-for-byte port of
the Rust adapter's (same state update, same scramble constants, same
shift amounts, same seed derivation). Java's arithmetic is
two's-complement 64-bit like Rust's `u64` arithmetic when read
through `Long.remainderUnsigned`. Running the Java, Go, Python, and
Rust adapters against the same `(length, salt)` gives the same
corpus on all sides.

Per-benchmark-family salts vs. the other adapters:

| Bench family         | Rust                 | Python               | Go                   | Java                 |
|----------------------|----------------------|----------------------|----------------------|----------------------|
| levenshtein          | (0xA1, 0xA2, 0xA3)   | (0xB1, 0xB2, 0xB3)   | (0xF1, 0xF2, 0xF3)   | (0xF4, 0xF5, 0xF6)   |
| hamming              | (0xC1, 0xC2, 0xC3)   | (0xC1, 0xC2, 0xC3)   | (0xC1, 0xC2, 0xC3) ³ | (0xC1, 0xC2, 0xC3) ³ |
| jaro / jaro-winkler  | (0xD1, 0xD2, 0xD3)   | (0xD1, 0xD2, 0xD3)   | (0xD1, 0xD2, 0xD3) ³ | (0xD1, 0xD2, 0xD3) ³ |
| osa / damerau / lcs  | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)   | (0xE1, 0xE2, 0xE3)   |

³ Hamming and Jaro share salts with the Rust / Python / Go adapters
on purpose: Hamming needs equal-length inputs (all adapters must see
the same mismatch positions to compare fairly), and Jaro's
match-window behaviour is best observed against a known-shared
corpus.

## Deferred / future work

* **Component Model in Chicory.** Once Chicory grows Component Model
  support the wasm-tools extraction step goes away entirely and the
  adapter can load the component directly. The API surface of this
  package will not change; only its innards.
* **Full Damerau at the WIT boundary.** Once the underlying kernel
  gets a wasm-portable hash story (`getrandom`-free `HashMap` or a
  hand-rolled sparse table), the WIT world will grow a `damerau`
  function and `DamerauBenchmark.stringcheeseDamerau` will unskip.
* **JVM warmup nuances.** JMH's default `-wi 5 -i 5` fits most
  DP-kernel work but the wasm path warms up more slowly (JIT has to
  see the guest's canonical-ABI calling convention several times
  before it can inline the alloc + call + read path). A caller who
  wants steady-state numbers should raise `-wi` to 10 or more for
  the StringCheese cells; the ecosystem cells stabilise faster.
* **GraalVM native-image path.** Chicory is designed to work under
  native-image, but the wasm-tools shell-out at construction is not.
  A native-image build would need `STRINGCHEESE_CORE_WASM` set to a
  pre-extracted core module baked in at image-build time. Not yet
  wired up in this adapter.
* **rapidfuzz-equivalent.** There is no rapidfuzz for pure Java. If
  a JNI binding to the C++ engine ever ships with a licence
  compatible with this repo, it will slot in as a third contestant
  the same way `rapidfuzz` does in the Python adapter — the
  addition is scaffolding-only.
* **BatchComparator / prepared-pattern shape.** The wasm boundary
  currently rebuilds the pattern automaton per call. A future WIT
  interface with a `pattern` resource would amortise the
  per-pattern cost.

## Pinned dependency versions

Recorded as Maven `<properties>` in `pom.xml` for one-place bumping:

| Group / Artifact                                 | Version   |
|--------------------------------------------------|-----------|
| `com.dylibso.chicory:runtime` / `wasm` / `wasi`  | `1.7.3`   |
| `org.apache.commons:commons-text`                | `1.13.1`  |
| `info.debatty:java-string-similarity`            | `2.0.0`   |
| `org.openjdk.jmh:jmh-core` / `jmh-generator-annprocess` | `1.37`    |
| `org.junit.jupiter:junit-jupiter`                | `5.10.4`  |

Bumping any of these is a one-liner in `pom.xml`'s `<properties>`
section; the adapter API surface has no dependency on internals of
any of them.

## Non-goals

Same as the umbrella `../README.md`:

* **Not a correctness suite.** Golden-dataset infrastructure lives
  in `crates/stringcheese-corpus/`. If a distance number disagrees
  between two implementations at the same `(length, seed)` corpus,
  that is a bug to file against the toolkit's differential-test
  harness, not a bench-adapter regression.
* **Not a scoreboard.** Numbers depend on JVM version, JIT decisions,
  CPU microarchitecture, and OS scheduling. A cross-machine
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
