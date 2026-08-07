// Head-to-head: StringCheese (via wasm) OSA + full Damerau vs. JS libs.
//
// Variant identity is load-bearing
// ================================
//
// There are two commonly-named "Damerau" algorithms and they compute
// different distances. Pairing them incorrectly puts two different
// algorithms on the same axis and produces numbers that look meaningful
// but are not — the failure mode `docs/DESIGN.md` warns about in its
// "Comparative Library Benchmarking" section, and the same trap the
// sibling Rust / Python adapters document.
//
// OSA (Optimal String Alignment / "restricted Damerau")
// ------------------------------------------------------
//
// Each substring can be edited at most once; does not satisfy the
// triangle inequality.
//
// * **StringCheese** — `osaDistance`.
// * **Ecosystem gap:** no widely-installed JS package exposes OSA
//   under an unambiguous name. `natural.DamerauLevenshteinDistance` is
//   the **full unrestricted** variant (see its source); pairing it
//   against StringCheese's `Osa` would be exactly the mispairing
//   above. This file therefore benches StringCheese OSA on its own
//   (no ecosystem contestant in this row).
//
// Full unrestricted Damerau
// -------------------------
//
// Substrings can be edited unlimited times; a true metric.
//
// * **StringCheese** — **not exposed at the WIT boundary.** See
//   `component/README.md` "Deliberately not exposed": the underlying
//   kernel needs a `HashMap`, which pulls in `getrandom` on `wasm32-*`
//   and would require additional host wiring. The adapter throws
//   `NotImplementedError` from `damerauDistance`; the StringCheese
//   task is not added to the full-Damerau group so the run stays
//   honest about what is and is not measured.
// * **natural.DamerauLevenshteinDistance** — pure-JS DP; a common
//   Damerau contestant even though `natural` is a heavyweight
//   dependency.
//
// The natural-only full-Damerau cell is still benched even without a
// StringCheese counterpart so the ecosystem-baseline numbers are on
// the same axis as the OSA StringCheese-only row. A StringCheese
// Damerau cell will appear once the underlying kernel gets a
// wasm-portable hash story.
//
// Representation caveat
// =====================
//
// Same as the other files: StringCheese consumes `Uint8Array`, natural
// consumes `string`. On ASCII input the semantics agree; the FFI cost
// is folded in on purpose.

// `natural` transitively pulls in `dotenvx`; see levenshtein.bench.js
// for the ESM-hoisting rationale behind the side-effect import.
import "./_env.js";

import natural from "natural";

import { StringCheese } from "../stringcheese_adapter.js";
import { LENGTHS, REGIMES, buildPair, lenLabel } from "./_inputs.js";
import { makeBench, runAll } from "./_runner.js";

const { DamerauLevenshteinDistance: NaturalDamerau } = natural;

// Shared with the Rust and Python adapters — same salts, same corpus,
// for cross-adapter reasoning.
const SALTS = /** @type {[number, number, number]} */ ([0xe1, 0xe2, 0xe3]);

async function main() {
  const sc = new StringCheese();
  const decoder = new TextDecoder("ascii");

  /** @type {Map<string, {aB: Uint8Array, bB: Uint8Array, aS: string, bS: string}>} */
  const pairs = new Map();
  for (const n of LENGTHS) {
    for (const kind of REGIMES) {
      const [aB, bB] = buildPair(n, kind, SALTS);
      pairs.set(`${kind}/${lenLabel(n)}`, {
        aB,
        bB,
        aS: decoder.decode(aB),
        bS: decoder.decode(bB),
      });
    }
  }

  const benches = [];

  // OSA — StringCheese on its own. No ecosystem contestant here
  // because no well-known JS package unambiguously exposes OSA (see
  // file docstring). Kept as its own group so a downstream chart can
  // still plot the (regime, length) StringCheese OSA cost.
  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aB, bB } = pairs.get(key);
      const bench = makeBench(`osa/${key}`);
      bench.add("stringcheese", () => sc.osaDistance(aB, bB));
      benches.push(bench);
    }
  }

  // Full Damerau — natural on its own; StringCheese cell is left out
  // per the WIT gap documented above. Verify the gap has not silently
  // closed: call the adapter's `damerauDistance` at startup and expect
  // a `NotImplementedError`. If a future WIT revision exposes full
  // Damerau, this check trips and the file needs a StringCheese row.
  try {
    sc.damerauDistance(pairs.get(`random/${lenLabel(8)}`).aB, pairs.get(`random/${lenLabel(8)}`).bB);
    console.warn(
      "damerauDistance no longer throws — the WIT surface may have grown a full-Damerau export; " +
        "add a stringcheese row to the damerau/* group.",
    );
  } catch (err) {
    if (err?.name !== "NotImplementedError") {
      throw err;
    }
    // Expected. Continue.
  }

  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aS, bS } = pairs.get(key);
      const bench = makeBench(`damerau/${key}`);
      bench.add("natural", () => NaturalDamerau(aS, bS));
      benches.push(bench);
    }
  }

  await runAll(benches);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
