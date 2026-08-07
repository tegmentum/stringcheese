// Shared tinybench runner used by every `*.bench.js` file.
//
// Every bench file wires up an entry per (algorithm, implementation,
// regime, length) cell using `add(id, fn)` and then hands the array of
// `Bench` instances to `runAll` — which:
//
//   1. Warms up + runs each Bench.
//   2. Prints one grouped console.table per Bench.
//   3. Emits a machine-readable JSON summary to stdout (guarded behind
//      the `BENCH_JSON` env var so the human-readable table stays the
//      default).
//
// The output shape mirrors what the Python adapter's pytest-benchmark
// tables show — one row per implementation, columns for latency
// statistics, grouped side-by-side by (regime, length). Cross-adapter
// readers can therefore diff a JS run against the equivalent Python
// run without a translation step.
//
// Deliberately excluded from the timing:
//
// * **Component load.** `StringCheese` is instantiated once at the top
//   of each bench file, before any `add(...)` calls. jco's transpiled
//   module materialises the wasm instance at import time; the caller
//   pays that once per process.
// * **Corpus generation.** Each bench file materialises its
//   `(length, regime)` pairs in a `Map` before entering the tinybench
//   loop. The `SplitMix64`-in-BigInt generator is slow (BigInt
//   arithmetic is not free) but it runs exactly once per (length, kind)
//   cell per file.
// * **`Uint8Array → string` conversion.** The native libraries take
//   strings, StringCheese takes `Uint8Array`. Each bench pre-decodes
//   the bytes to a string once, outside the timing loop, so only the
//   library's own work is timed.

import { Bench } from "tinybench";

/**
 * Zero-padded length label, matching the sibling adapters' output.
 * @param {number} n
 */
export function lenLabel(n) {
  return `len${String(n).padStart(4, "0")}`;
}

/**
 * Construct a fresh `Bench` for a single (algorithm, regime, length)
 * cell. Bench options mirror the defaults tinybench 6 ships with, but
 * make the time budget and iteration floor explicit so future tuning
 * lands in one place.
 *
 * @param {string} name
 * @returns {Bench}
 */
export function makeBench(name) {
  return new Bench({
    name,
    // 500 ms of measured time per task is a compromise between a full
    // matrix that runs in a couple of minutes and confidence intervals
    // narrow enough to distinguish implementations that are within
    // ~10 % of each other. Raise via BENCH_TIME_MS for tighter runs.
    time: readNumericEnv("BENCH_TIME_MS", 500),
    // The floor keeps very-fast tasks (e.g. StringCheese Hamming on
    // len=8) from being under-sampled when the 500 ms budget lets them
    // finish 500k iterations — tinybench will run at least this many
    // even if `time` would let it stop earlier.
    iterations: readNumericEnv("BENCH_MIN_ITERS", 64),
    warmup: true,
  });
}

/**
 * Run a list of benches sequentially, printing one grouped table per
 * bench and (optionally) emitting a JSON summary to stdout.
 *
 * @param {ReadonlyArray<Bench>} benches
 * @returns {Promise<void>}
 */
export async function runAll(benches) {
  for (const bench of benches) {
    console.log(`\n=== ${bench.name} ===`);
    await bench.run();
    // tinybench's built-in `table()` converts every task to a plain
    // object; hand it directly to console.table for the human view.
    console.table(bench.table());
  }
  if (process.env.BENCH_JSON) {
    const summary = benches.map((b) => ({
      name: b.name,
      tasks: b.tasks.map((t) => ({
        name: t.name,
        runs: t.runs,
        result: t.result,
      })),
    }));
    process.stdout.write(JSON.stringify(summary, null, 2) + "\n");
  }
}

/**
 * Read a positive integer from `process.env[key]`, falling back to
 * `fallback` when absent or unparseable. Used for the `BENCH_TIME_MS`
 * and `BENCH_MIN_ITERS` overrides — the bench harness does not need a
 * full flag parser and env vars compose cleanly with `npm run …`.
 * @param {string} key
 * @param {number} fallback
 */
function readNumericEnv(key, fallback) {
  const raw = process.env[key];
  if (raw === undefined || raw === "") return fallback;
  const n = Number.parseInt(raw, 10);
  return Number.isFinite(n) && n > 0 ? n : fallback;
}
