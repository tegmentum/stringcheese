# StringCheese Rust Bench Adapters

Head-to-head criterion benches comparing StringCheese's core algorithms
against the Rust ecosystem's two most-used sequence-comparison crates.

This crate is deliberately **not** a member of the outer StringCheese
workspace. See the parent `../README.md` for the rationale.

## Running

From the repository root:

```
cargo bench --manifest-path bench-adapters/rust/Cargo.toml
```

Or from inside this directory:

```
cargo bench
```

To run only one head-to-head:

```
cargo bench --manifest-path bench-adapters/rust/Cargo.toml --bench levenshtein_vs_strsim
cargo bench --manifest-path bench-adapters/rust/Cargo.toml --bench levenshtein_vs_rapidfuzz
cargo bench --manifest-path bench-adapters/rust/Cargo.toml --bench jaro_vs_strsim
cargo bench --manifest-path bench-adapters/rust/Cargo.toml --bench hamming_vs_strsim
cargo bench --manifest-path bench-adapters/rust/Cargo.toml --bench damerau_vs_strsim
```

Criterion writes HTML reports to
`bench-adapters/rust/target/criterion/` (isolated from the toolkit's
own `target/`). Open `index.html` in that directory to browse the
matrix.

## Comparison libraries (v0.1)

| Library      | Version | Since  | API shape                                            |
|--------------|---------|--------|------------------------------------------------------|
| `strsim`     | 0.11.1  | 2024   | `fn f(&str, &str) -> {usize, f64}`; also `generic_levenshtein(&[T], &[T])` |
| `rapidfuzz`  | 0.5.0   | 2023   | `fn distance(impl IntoIterator, impl IntoIterator) -> usize`; `distance_with_args` for cutoffs; `BatchComparator` for many-vs-one |

Both are pinned to their current-stable minor. `cargo update` inside
this workspace picks up patch releases automatically. Bumping either
to a new major requires editing this crate's `Cargo.toml` and
re-running the compile check below — no reason to touch the outer
workspace.

## Coverage matrix

Each bench file crosses **input length × similarity regime ×
implementation**. Lengths and regimes match `stringcheese-bench` exactly
(`LENGTHS = [8, 32, 128, 512, 2048]`, regimes = `random`, `similar`,
`identical`) so a criterion report from this crate and a criterion
report from `stringcheese-bench` line up on the same axis.

| StringCheese algorithm | `strsim`                        | `rapidfuzz`                    | Bench file                           |
|---------------------|---------------------------------|--------------------------------|--------------------------------------|
| `Levenshtein`       | `levenshtein`, `generic_levenshtein` | `distance`, `distance_with_args` (`k=3` cutoff) | `levenshtein_vs_strsim.rs`, `levenshtein_vs_rapidfuzz.rs` |
| `Jaro`              | `jaro`                          | *(not yet covered — planned)*  | `jaro_vs_strsim.rs`                  |
| `JaroWinkler::classic()` | `jaro_winkler`             | *(not yet covered — planned)*  | `jaro_vs_strsim.rs`                  |
| `hamming_distance`  | `hamming`                       | *(not covered)*                | `hamming_vs_strsim.rs`               |
| `Osa`               | `osa_distance`                  | *(not yet covered — planned)*  | `damerau_vs_strsim.rs`               |
| `Damerau`           | `damerau_levenshtein`           | *(not yet covered — planned)*  | `damerau_vs_strsim.rs`               |

`rapidfuzz` covers every one of these algorithms; the adapter
currently only wires up its Levenshtein because the Jaro / Damerau /
OSA slots are structurally identical to the `strsim` versions and
the marginal insight per additional bench file is low. Wiring them
in is future work — the per-file scaffolding is fixed and additive.

## Representation caveats (READ THIS)

StringCheese's algorithm crates consume `&[T: Eq]` — most usefully,
`&[u8]` for ASCII input. `strsim` consumes `&str` and iterates
`chars` internally. `rapidfuzz` consumes any `IntoIterator` whose
item is `Hash + Eq`.

Direct implication for the benches in this crate:

* **`levenshtein_vs_strsim.rs`** carries two groups.
  * `vs_strsim_str_ascii` — StringCheese on `&[u8]` vs. `strsim` on `&str`.
    This is the realistic-usage comparison and deliberately gives
    StringCheese the byte-slice fast-path advantage.
  * `vs_strsim_generic_bytes` — StringCheese on `&[u8]` vs.
    `strsim::generic_levenshtein` on `&[u8]`. Same input
    representation on both sides; the fair-fight DP-kernel comparison.
* **`levenshtein_vs_rapidfuzz.rs`** feeds `rapidfuzz` an iterator of
  `u8` — `pair.a_bytes.iter().copied()` — so both implementations
  see the same representation.
* **`jaro_vs_strsim.rs`** / **`hamming_vs_strsim.rs`** /
  **`damerau_vs_strsim.rs`** compare StringCheese-on-bytes to
  strsim-on-str. `strsim` exposes no generic-slice overload of
  these functions, so no fair-fight variant exists to wire up.
  For ASCII input the two are semantically identical; strsim
  simply pays for UTF-8 iteration on top of the algorithm work.

## Algorithm-variant caveats (READ THIS TOO)

Several of these names refer to more than one algorithm. This adapter
pairs them as follows:

| Common name              | StringCheese                    | `strsim`                | `rapidfuzz`                          |
|--------------------------|------------------------------|-------------------------|--------------------------------------|
| Levenshtein              | `Levenshtein` (unit cost)    | `levenshtein`           | `distance::levenshtein::distance`    |
| OSA / "restricted Damerau" | `Osa`                     | `osa_distance`          | `distance::osa::distance` *(unused)* |
| Full/unrestricted Damerau | `Damerau`                  | `damerau_levenshtein`   | `distance::damerau_levenshtein::distance` *(unused)* |
| Jaro                     | `Jaro`                       | `jaro`                  | `distance::jaro::similarity` *(unused)* |
| Jaro-Winkler (classic)   | `JaroWinkler::classic()`     | `jaro_winkler`          | `distance::jaro_winkler::similarity` *(unused)* |
| Hamming                  | `hamming_distance`           | `hamming`               | *(unused)*                           |

The **pairing is load-bearing**. `strsim::damerau_levenshtein` is the
full unrestricted variant (its docs cite the triangle inequality),
which matches StringCheese's `Damerau`; pairing it with StringCheese's
`Osa` would put two different algorithms on the same axis and
produce numbers that look meaningful but are not. The bench-file
group names (`osa/vs_strsim`, `damerau/vs_strsim`) keep the variant
choice explicit at the criterion axis so a downstream reader
generating a chart cannot accidentally cross the wires.

## Cutoff variants

* StringCheese's banded Levenshtein at `k = 3` (the classical
  spellcheck bound) is compared to `rapidfuzz`'s
  `levenshtein::distance_with_args(_, _, &Args::default().score_cutoff(3))`
  in `levenshtein_vs_rapidfuzz.rs::bench_cutoff_k3`. Both return
  early when the true distance exceeds `3`, so the "random" regime
  should show a dramatic win at long lengths for both
  implementations relative to the unbounded group.
* `strsim` has no bounded variant of any of its algorithms. The
  StringCheese cutoff kernels are still benched in the `_vs_strsim.rs`
  files (e.g. `hamming/within_k3_stringcheese_only`) as
  StringCheese-only groups so that a reader can read across from the
  strsim head-to-head to the StringCheese cutoff-vs-StringCheese
  unbounded number in the same criterion report.

## Determinism

Every benchmark's inputs are generated deterministically from a
per-length seed derived from the input length and a per-bench salt
via SplitMix64. The RNG is duplicated verbatim from
`stringcheese-bench::inputs` (see `src/lib.rs::Rng` for the source note).
Running the same bench twice in a row on the same commit will produce
the same corpus twice — criterion's own noise is then the only
per-run variance.

## Future languages

Per `docs/DESIGN.md` "Comparative Library Benchmarking", the
`bench-adapters/` directory will grow sibling slots for `python/`,
`java/`, `javascript/`, `cpp/`, and `go/`. Each will follow the same
per-adapter design contract documented in `../README.md`:
common input matrix, common output shape, no cross-variant
mispairings, no scoreboard framing. Those slots are out of scope for
this v0.1 delivery.

## Notes on interpreting a run

Criterion's HTML report will show, for each `(kind, length)` cell,
one bar per implementation. Read the shape of the difference across
lengths, not the individual number — the point of the head-to-head
is to see how a StringCheese kernel and its ecosystem alternative scale
relative to each other, not to publish a single "X is Y ns faster"
figure that will decay with the next benchmark machine.

Cross-machine or cross-release absolute numbers should never be
quoted from this harness. See the parent `README.md` "Non-goals"
section.
