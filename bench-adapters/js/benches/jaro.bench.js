// Head-to-head: StringCheese (via wasm) vs. JS Jaro / Jaro–Winkler libs.
//
// Contestants
// ===========
//
// Jaro-Winkler:
//
// * **StringCheese** — `jaroWinklerSimilarity` (classic: prefix 4,
//   scaling 0.1, no boost threshold).
// * **natural.JaroWinklerDistance** — the same classic defaults; the
//   `natural` package's function name says "Distance" but it actually
//   returns the Jaro–Winkler *similarity* in `[0.0, 1.0]` (see
//   `node_modules/natural/lib/natural/distance/jaro-winkler_distance.js`
//   — it returns `jaro + l * p * (1 - jaro)`, i.e. a similarity).
//
// Jaro (pure):
//
// * **StringCheese** — `jaroSimilarity`.
// * **Ecosystem gap:** `natural` does not expose the pure Jaro
//   similarity as a top-level function — its Jaro implementation is a
//   file-scope `distance()` inside `jaro-winkler_distance.js`. Neither
//   `fastest-levenshtein`, `js-levenshtein`, nor `string-similarity`
//   expose Jaro at all. The StringCheese cell is therefore benched
//   *alongside* a `string-similarity.compareTwoStrings` reference —
//   documented as "not directly comparable" because compareTwoStrings
//   computes a Dice coefficient over bigrams, not a Jaro similarity.
//   The point is to make the FFI-vs-native-JS cost visible at each
//   length; the numeric result is not comparable.
//
// Representation caveat
// =====================
//
// Same as the other files: StringCheese consumes `Uint8Array`, the
// native libraries consume `string`. On ASCII input the semantics
// agree; the FFI cost is folded in on purpose.

// `natural` transitively pulls in `dotenvx`; see levenshtein.bench.js
// for the ESM-hoisting rationale behind the side-effect import.
import "./_env.js";

import natural from "natural";
import * as stringSimilarity from "string-similarity";

import { StringCheese } from "../stringcheese_adapter.js";
import { LENGTHS, REGIMES, buildPair, lenLabel } from "./_inputs.js";
import { makeBench, runAll } from "./_runner.js";

const { JaroWinklerDistance: NaturalJW } = natural;

// Shared with the Rust and Python adapters — same salts because the
// Jaro match-window behaviour is best observed against a known-shared
// corpus for cross-adapter reasoning.
const SALTS = /** @type {[number, number, number]} */ ([0xd1, 0xd2, 0xd3]);

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

  // Pure Jaro. StringCheese is the only turnkey Jaro in the pure-JS
  // ecosystem; string-similarity's `compareTwoStrings` is included as
  // a "not directly comparable" FFI-cost anchor. See the file docstring.
  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aB, bB, aS, bS } = pairs.get(key);
      const bench = makeBench(`jaro/${key}`);
      bench.add("stringcheese", () => sc.jaroSimilarity(aB, bB));
      bench.add("string-similarity (NOT Jaro)", () =>
        stringSimilarity.compareTwoStrings(aS, bS),
      );
      benches.push(bench);
    }
  }

  // Jaro-Winkler. StringCheese vs. natural — both compute the classic
  // Winkler variant with prefix 4, scaling 0.1, no boost threshold.
  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aB, bB, aS, bS } = pairs.get(key);
      const bench = makeBench(`jaro_winkler/${key}`);
      bench.add("stringcheese", () => sc.jaroWinklerSimilarity(aB, bB));
      bench.add("natural", () => NaturalJW(aS, bS));
      benches.push(bench);
    }
  }

  await runAll(benches);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
