# stringcheese-bench

Criterion wall-clock benchmarks + dhat allocation-tracking harness
for the StringCheese workspace. `publish = false` — this crate is
CI-facing, not a user dep.

## Running the benches

Every bench file is a standard criterion harness. Run one with:

```
cargo bench -p stringcheese-bench --bench <name>
```

Bench inventory:

| Bench | Covers | Group prefix |
|-------|--------|--------------|
| `levenshtein`, `hamming`, `jaro`, `damerau`, `ngram`, `batch` | Original algorithm-crate benches (`stringcheese-compare`, etc.) | one per algorithm |
| `sketches`   | `stringcheese-minhash`, `stringcheese-simhash`, `stringcheese-winnowing` | `sketches/*` |
| `pattern`    | `stringcheese-pattern` (Literal / Wildcard / Glob) | `pattern/*` |
| `diff`       | `stringcheese-diff` (Myers + Patience) | `diff/*` |
| `collate`    | `stringcheese-collate` (UCA / Natural / ASCII-CI) | `collate/*` |
| `stats`      | `stringcheese-stats` (entropy / histogram / ratios / lengths) | `stats/*` |
| `escape`     | `stringcheese-escape` (URI / HTML / JSON / shell) | `escape/*` |
| `textsplit`  | `stringcheese-textsplit` (Recursive / Paragraph / Sentence) | `textsplit/*` |
| `normalize`  | `stringcheese-normalize` (4 pipelines + 4 primitives) | `normalize/*` |
| `ident`      | `stringcheese-ident` (case conversion / slugify / sanitize) | `ident/*` |

Quick smoke test any bench without full statistical measurement:

```
cargo bench -p stringcheese-bench --bench <name> -- --test
```

Filter to one group inside a bench:

```
cargo bench -p stringcheese-bench --bench stats -- ratios
```

## Allocation tracking

Opt-in via the `alloc-tracking` feature — installs `dhat::Alloc`
as the global allocator, so it MUST NOT be enabled during a
regular `cargo bench` run (dhat skews criterion sampling and
invalidates its statistical model).

```
cargo run -p stringcheese-bench --features alloc-tracking \
  --bin alloc_report_levenshtein
```

Reports are TSV to stdout — blocks + bytes allocated per
comparison — the raw evidence for workspace-reuse claims in the
design doc.

## Deterministic inputs

Every helper in `src/inputs.rs` is seeded from a `u64` and threads
that seed through a small deterministic RNG. Benchmarks never
depend on process time or OS entropy so criterion's
sample-to-sample variance stays dominated by the machine rather
than the corpus.

## The bench-driven feedback loop

Nine session bench-driven perf fixes landed following a
consistent pattern: bench → surface concrete regression → land a
targeted fix → re-run the same bench → verify the improvement.
Each fix's rationale is committed inline in the crate that owns
the code, with a table in the crate's own `lib.rs` docs
capturing the baseline that motivated the change.

| # | Crate | Fix | Impact |
|---|---|---|---|
| 1 | `stringcheese-stats::entropy` | `BTreeMap<char, u64>` → `hashbrown::HashMap` | 5× at 8 KB (67 → 348 MiB/s) |
| 2 | `stringcheese-escape::json` | ASCII lookup table + passthrough-run coalescing | 5-6× on plain input (249 MiB/s → 1.78 GiB/s) |
| 3 | `stringcheese-textsplit::sentence` | `O(N²)` `input[cursor..].find(s)` → `O(N)` cursor tracking | 7× at 32 KB (117 → 767 MiB/s) |
| 4 | `stringcheese-stats::ratios` | Six-flag ASCII lookup packed in `u8` | 2× across all sizes (~400 → 700 MiB/s) |
| 5 | `stringcheese-normalize::canonicalize_punctuation` | ASCII passthrough coalescing | +56 % on mixed input (510 → 802 MiB/s) |
| 6 | `stringcheese-normalize::collapse_whitespace` | Per-byte lookup (no coalescing) | 2× (476 → 956 MiB/s) |
| 7 | `stringcheese-normalize::strip_controls` | Coalesced non-control ASCII runs | +26 % (520 → 654 MiB/s) |
| 8 | `stringcheese-stats::histogram` | Fixed-size ASCII-slot accumulator + late `HashMap` merge | +30 % (366 → 476 MiB/s) |
| 9 | `stringcheese-diff::algo` (baseline only) | – | at-risk revisit trigger discharged |

Three attempts landed as **principled neutral or negative
results** rather than wins — the bench itself was the answer:

- **`collapse_whitespace` — first attempt**: "coalesce
  non-whitespace ASCII runs" pattern regressed the primitive by
  20 % because real inputs have whitespace every ~5 bytes and
  the run-scan overhead exceeds the savings. Reverted to a
  per-byte lookup, which gave the 2× win listed above.
- **`NaturalCollator::compare` refactor**: removed unused
  `peekable()` iterators and reordered digit classification.
  Bench (2s + 5s measurements) confirmed no meaningful throughput
  change — the "fix" was purely a code-cleanup, kept for
  maintenance value only.
- **`Ratios::of` NEON attempt**: aarch64 SIMD prototype with
  16-byte vector loads and range-based classifications
  regressed 30 % (700 → 540 MiB/s). Three causes: (1) scattered
  ASCII punctuation forced per-byte scalar fallback inside every
  SIMD chunk, (2) six `vaddvq_u8` cross-lane reductions per
  chunk add latency, (3) the scalar baseline with an
  L1-resident 128-byte lookup table is already fast. Reverted;
  a real SIMD win would need nibble-decomposition
  byte-classification (Langdale/Lemire style) + deferred
  reduction across ~200-chunk batches. See the crate's own
  `## Bench-driven negative result` section.

The pattern is: coalescing wins when the special case is rare
(JSON metachars, punctuation-canonicalisation targets).
Byte-per-byte-with-a-lookup wins when the special case is
frequent (whitespace, sentence terminators). Bench evidence
dictates which pattern applies — not intuition.

For SIMD: the current lesson is that the scalar path with an
L1-resident lookup table is a very high bar. SIMD needs a
matching sophistication (nibble-decomposition classification,
deferred reduction, per-target backends) to beat it. The
existing SIMD story in `stringcheese-compare` demonstrates
what "sophistication" looks like: 6,364 lines of per-target
kernels across five algorithms.

## Adding a new bench

1. New file under `benches/<name>.rs` using the criterion harness
   pattern shared by every existing bench.
2. Add its crate under test as a `[dependencies]` entry in
   `Cargo.toml` (matching the pattern of the existing entries).
3. Add a `[[bench]] name = "<name>"` entry to `Cargo.toml`.
4. Follow the "bench → find issue → fix → re-bench → verify"
   loop when the numbers surface something worth acting on.
   Record the baseline that motivated any fix in the target
   crate's own `lib.rs` docs so the design tradeoff can't
   silently regress.
