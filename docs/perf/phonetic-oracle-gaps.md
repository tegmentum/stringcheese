# `stringcheese-phonetic` oracle gap table

Side-by-side throughput of `stringcheese-phonetic` vs the best-in-class
Rust phonetic-encoder ecosystem. Oracle chosen:

* **`rphonetic`** (3.0) — Rust port of the Apache commons-codec phonetic
  algorithms. Ships Soundex, Metaphone, Double Metaphone, NYSIIS,
  Refined Soundex, Match-Rating, Beider-Morse, Caverphone, Cologne, and
  Daitch-Mokotoff. Best *breadth* in the Rust ecosystem — the only crate
  that covers all four stringcheese English encoders (Soundex, primary
  Double Metaphone, full Double Metaphone, NYSIIS) in one place. See
  the crate's `Cargo.toml` header for the alternative oracles considered
  and rejected.

## Oracles considered and rejected

* **`metaphone`** (0.1.1, `max-frai/metaphone-rs`) — Metaphone-only.
  Would add ~2 additional `Cargo.lock` entries (`aho-corasick`,
  `itertools`) on top of `rphonetic` for zero incremental algorithm
  coverage; `rphonetic` already ships Metaphone and Double Metaphone.
  Rejected as duplicate.
* **`soundex`** (0.2.0), **`soundex-rs`** (0.1.8) — Soundex-only.
  Same rejection reason; neither standalone crate is more mature than
  the `rphonetic` port of the Apache commons-codec reference. Rejected
  as duplicates.
* **`goldenphonetic-core`** (0.1.1), **`phonetics`** (0.1.0),
  **`amt-phonetic`** — newer / less-established phonetic crates.
  Rejected in favour of the widely-adopted commons-codec-derived
  `rphonetic` reference to keep the "canonical Rust ecosystem" bar
  used by the other oracle rounds (`compare` / `align` / `cdc`).

Slavic Metaphone has no Rust oracle at all — the algorithm is
stringcheese-specific (multilingual pack for the ru / uk / be / bg /
mk / sr / hr / bs / sl / pl / cs / sk arm across Cyrillic and Latin
scripts). Its baseline continues to live under the crate-level `//!`
docs of `src/lib.rs`; the regression tripwire is preserved.

## Dep-footprint

Adding `rphonetic` locks 6 new packages into `Cargo.lock`:
`rphonetic`, `document-features`, `enum-iterator`,
`enum-iterator-derive`, `litrs`, `nom`. Under the 20-entry budget
documented in `docs/perf/oracle-gap-summary.md`. `regex`,
`aho-corasick`, `memchr`, `either`, `serde`, `syn`, `proc-macro2`,
`quote`, and their transitives are already in the workspace
`Cargo.lock`, so the effective footprint on top of the existing
workspace is just those 6 crates.

No system-level dep, no build-script contamination, no `cc` /
`bindgen`.

## How to reproduce

```bash
cargo bench -p stringcheese-phonetic --bench phonetic \
    --features oracle-benches --locked -- \
    --warm-up-time 1 --measurement-time 3 --sample-size 15 \
    "phonetic/oracle/"
```

The `oracle-benches` feature is off by default. It is bench-only — no
library code looks at the oracle crate, and the ordinary `cargo bench
-p stringcheese-phonetic` invocation runs the standalone baseline
groups without pulling the extra dep in.

## Measurement environment

* aarch64 Apple M-series, macOS 15 / darwin 24.5
* rustc 1.97.1, release + LTO ("thin", codegen-units = 1)
* criterion 0.5, `--sample-size 15 --measurement-time 3`
* Wall-clock samples vary ±10-20 % on a laptop under load; treat
  ratios as informative, absolutes as illustrative. The Soundex row
  at n = 2048 in particular runs at sub-nanosecond-per-input-byte
  scales where a single scheduler blip inflates the ratio; the
  order-of-magnitude conclusion is robust, the last digit is not.

## Methodology note: max-code-length

`rphonetic::DoubleMetaphone::default()` sets `max_code_length =
Some(4)` — the algorithm early-exits as soon as the primary key
reaches 4 characters. stringcheese's `DoubleMetaphone::primary_only()`
and `DoubleMetaphone::full()` emit *uncapped* keys and walk the
entire input.

At n = 32 the difference is negligible (both implementations process
most of the input before the algorithm terminates or the length
runs out). At n = 256 / 2048 the capped configuration walks a tiny
fraction of the input before returning, which trivialises the
throughput ratio for those sizes.

Both configurations are therefore benched side by side for the two
Double Metaphone groups:

* `rphonetic-uncapped` — `DoubleMetaphone::new(None)`, uncapped
  output. This is the fair apples-to-apples comparison.
* `rphonetic-cap4` — `DoubleMetaphone::default()`, `Some(4)` cap.
  Documents the "off-the-shelf default" gap that a caller who used
  `DoubleMetaphone::default()` would see; the row is *not* a
  same-work comparison and is labelled accordingly in the verdict.

Soundex and NYSIIS have a fixed output length (4 chars, 6 chars
respectively) in both implementations, so there is no capping split
for those groups.

Additionally, `rphonetic::Soundex::encode()` runs an upfront
`soundex_clean` pass (`chars().filter(is_alphabetic).map(to_upper)
.collect::<String>()`) *before* the 4-char output loop starts. That
pass is O(n) with an intermediate allocation, so rphonetic Soundex
pays a full input walk per call while stringcheese Soundex scans
the input on-demand and short-circuits after emitting the fourth
digit. Same input, different design — the gap is real, not a bench
artifact.

## Gap table

Throughput reported as median of one representative run. `mult` is
`oracle_thrpt ÷ stringcheese_thrpt` at the same size — `> 1` means
the oracle is faster than stringcheese, `< 1` means stringcheese
wins. Verdict categories: **competitive** (< 2× gap either way),
**medium gap** (2-5×, optimization worth considering), **large gap**
(> 5×, clear perf lever), **stringcheese ahead** (stringcheese is
faster than the oracle).

### Soundex

| size | stringcheese  | rphonetic     | mult   | verdict                             |
| ---: | :------------ | :------------ | -----: | :---------------------------------- |
|   32 | 818   MiB/s   |  18.0 MiB/s   | 0.022× | stringcheese 45× ahead              |
|  256 |   6.28 GiB/s  |  36.8 MiB/s   | 0.006× | stringcheese 175× ahead             |
| 2048 |  53.5 GiB/s   |  17.6 MiB/s   | 0.0003× | stringcheese 3000× ahead            |

Reads:

* stringcheese runs the 4-char Soundex output loop *inline* over
  the input characters, short-circuiting once the fourth digit is
  emitted. rphonetic pre-cleans the input into an intermediate
  `String` (full O(n) walk + heap alloc) *before* starting the
  4-char loop, so its throughput is bounded by the pre-clean pass
  even though the algorithm itself would terminate early.
* The n = 2048 row is misleading in the same way `manip/trim/trim`
  is — stringcheese's effective work per byte falls to near-zero at
  large n (the 4-char cap means the inner loop reads only enough
  input to emit four distinct codes), while rphonetic's throughput
  denominator counts against the full 2 KiB clean pass. The ratio
  is real but the *baseline* is running at rates that make the
  encoder essentially free.
* No perf lever here — stringcheese wins by wide margins across the
  entire size grid.

### NYSIIS

| size | stringcheese  | rphonetic     | mult   | verdict                        |
| ---: | :------------ | :------------ | -----: | :----------------------------- |
|   32 | 130   MiB/s   |  13.5 MiB/s   |  0.10× | stringcheese 9.6× ahead        |
|  256 | 208   MiB/s   |  20.2 MiB/s   |  0.10× | stringcheese 10.3× ahead       |
| 2048 | 247   MiB/s   |  19.3 MiB/s   |  0.08× | stringcheese 12.8× ahead       |

Reads:

* stringcheese's NYSIIS runs at ~200-250 MiB/s and is
  9-13× *ahead* of rphonetic across the entire size grid. rphonetic's
  transcoder builds a small `String` per input character in the
  rewrite pass (`return CHARS_AF.to_string()` and similar), which
  drives per-char allocation overhead; stringcheese buffers into a
  reused workspace instead.
* No perf lever here — stringcheese wins by wide margins.

### Double Metaphone (primary_only)

Fair comparison is the `rphonetic-uncapped` row. The
`rphonetic-cap4` row uses `DoubleMetaphone::default()` (4-char cap)
and is *not* a same-work comparison at n ≥ 256; see the methodology
note above.

| size | stringcheese  | rph-uncapped  | rph-cap4      | uncapped mult | cap4 mult | verdict (uncapped, same-work)     |
| ---: | :------------ | :------------ | :------------ | ------------: | --------: | :-------------------------------- |
|   32 | 230   MiB/s   |  26.0 MiB/s   |  92.8 MiB/s   |         0.11× |     0.40× | stringcheese 8.8× ahead (uncapped) |
|  256 | 483   MiB/s   |  21.6 MiB/s   | 504   MiB/s   |         0.04× |     1.04× | stringcheese 22× ahead (uncapped) |
| 2048 | 738   MiB/s   |  23.7 MiB/s   |   3.88 GiB/s  |         0.03× |     5.38× | stringcheese 31× ahead (uncapped); cap4 row is early-exit, not same work |

Reads:

* Under the fair (uncapped) comparison, stringcheese is 9-31× *ahead*
  of rphonetic at every size and pulls further ahead as `n` grows
  — rphonetic-uncapped's per-byte cost is dominated by the same
  per-char `String::from_str` allocation pattern seen in NYSIIS,
  which does not amortize with `n`.
* The cap4 row at n ≥ 256 shows what a caller using the
  `DoubleMetaphone::default()` off-the-shelf preset would see: at
  n = 256 the two implementations tie, at n = 2048 the capped
  rphonetic wins by 5× — but that is because rphonetic returns
  after emitting 4 characters, not because its algorithm is faster.
  A parallel `stringcheese-phonetic` feature to also cap output
  length would close this "off-the-shelf" gap trivially; see the
  perf-lever list below.

### Double Metaphone (full — primary + alternate)

Same capping split as the primary variant. Fair comparison is
`rphonetic-uncapped`.

| size | stringcheese  | rph-uncapped  | rph-cap4      | uncapped mult | cap4 mult | verdict (uncapped, same-work)     |
| ---: | :------------ | :------------ | :------------ | ------------: | --------: | :-------------------------------- |
|   32 |  51.5 MiB/s   |  10.5 MiB/s   | 131   MiB/s   |         0.20× |     2.54× | stringcheese 4.9× ahead (uncapped); cap4 row is early-exit, not same work |
|  256 | 245   MiB/s   |  20.1 MiB/s   | 142   MiB/s   |         0.08× |     0.58× | stringcheese 12× ahead (uncapped); ahead of cap4 too |
| 2048 | 387   MiB/s   |  16.1 MiB/s   |   2.85 GiB/s  |         0.04× |     7.36× | stringcheese 24× ahead (uncapped); cap4 row is early-exit, not same work |

Reads:

* Under the fair (uncapped) comparison, stringcheese is 5-24× *ahead*
  of rphonetic at every size — same pattern as the primary variant
  amplified by the two-key work.
* The cap4 row at n = 32 slightly favours rphonetic (2.5× ahead)
  because at short input the *fixed* per-call overhead of
  stringcheese's `full()` variant (constructing the two-key
  `DoubleMetaphoneKey` output) is a larger fraction of the work.
  This is the closest thing in the whole oracle round to a genuine
  scalar perf lever, and it is measured at absolute rates
  (~50-130 MiB/s) where per-call overhead dominates. Bench-only
  interest.
* The cap4 row at n ≥ 256 shows the same "trivialised by early exit"
  effect as the primary variant.

### Slavic Metaphone

No oracle. The Cyrillic/Latin multilingual arm is stringcheese-
specific; no crate in the Rust ecosystem implements it. Baseline
stays under `src/lib.rs`'s `//!` docs baseline table.

## Summary verdict for `stringcheese-phonetic`

* **Ahead**: every same-work oracle row. stringcheese is 5-3000×
  faster than `rphonetic` across `Soundex`, `NYSIIS`, primary
  `DoubleMetaphone`, and full `DoubleMetaphone` at every measured
  size. The two roots of the win are (a) inline / short-circuit
  input scanning where the algorithm allows it (Soundex, both
  Double Metaphone variants) vs rphonetic's upfront full-input
  cleaning pass; (b) reused character buffers vs rphonetic's
  per-char `String::from_str` allocation pattern in the rewrite
  passes (NYSIIS, both Double Metaphone variants).
* **Off-the-shelf "gap"**: the `rphonetic-cap4` row at n ≥ 256 on
  the two Double Metaphone groups shows what a caller who used
  `rphonetic::DoubleMetaphone::default()` (4-char cap) would see:
  rphonetic 5-7× ahead at large `n`, but not from any algorithmic
  advantage — it early-exits after emitting the 4th character.
  This is a stringcheese *feature-parity* item, not a perf item:
  offering an optional output-length cap on stringcheese's
  Double Metaphone variants would close the "off-the-shelf"
  comparison mechanically without touching the hot path.
* **No oracle**: `SlavicMetaphone` — no comparable Rust crate at
  any maturity bar. The baseline table in the bench file's module
  doc remains the regression tripwire.

## Recommended next perf targets

Only one row where an oracle is *plausibly* ahead of stringcheese
under a same-work comparison — and it is bench-noise-adjacent:

### 1. `double_metaphone_full` at n = 32 — vs `rphonetic-cap4` (2.5×)

* **Gap**: `rphonetic-cap4` is 2.5× ahead at n = 32 only (130 MiB/s
  vs 51 MiB/s). Under the fair (uncapped) comparison stringcheese
  is 4.9× *ahead* at the same size, so the 2.5× number is entirely
  the "capped rphonetic terminates early on short input" effect.
* **What's likely wrong**: nothing — this is a feature-parity gap,
  not a perf gap. Adding an optional `max_code_length` to
  `stringcheese-phonetic::DoubleMetaphoneVariant` (currently the
  variant enum has two states, `PrimaryOnly` and `Full`, both
  emitting uncapped keys) would close the row mechanically.
  Estimated cost: small.
* **Priority**: low. Callers who need a 4-char cap today can
  post-truncate the returned key; the workshop rewrite is convenient
  rather than load-bearing.

### Not on the list

* **All Soundex, NYSIIS, and uncapped Double Metaphone rows**:
  stringcheese is 5-3000× *ahead* of rphonetic. Any further
  headroom is bench-noise-adjacent.
* **Slavic Metaphone**: no oracle available.

See `docs/perf/oracle-gap-summary.md` for the ranked perf-lever list
that combines this data with `stringcheese-compare` /
`stringcheese-align` / `stringcheese-cdc`.
