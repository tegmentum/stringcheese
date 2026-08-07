// Head-to-head: StringCheese (via jco-transpiled wasm) vs. the JS
// ecosystem's Levenshtein-distance libraries.
//
// Contestants
// ===========
//
// * **StringCheese** — loaded through jco's transpiled ES module (see
//   `../stringcheese_adapter.js` and `../scripts/transpile.js`). Every
//   call pays the wasm boundary cost (parameter lowering into linear
//   memory, guest DP execution, result lifting) on top of the
//   underlying kernel.
// * **fastest-levenshtein** — a hand-rolled DP kernel in pure JS,
//   widely regarded as the fastest pure-JS implementation for short-to-
//   medium strings.
// * **js-levenshtein** — an older but still popular pure-JS DP kernel;
//   simpler implementation than fastest-levenshtein.
// * **natural** — `natural.LevenshteinDistance`, a general-purpose NLP
//   toolkit's DP implementation. Included as the "ecosystem baseline"
//   contestant even though it is not tuned for raw distance speed.
// * **string-similarity** — `stringSimilarity.compareTwoStrings`
//   computes a Dice coefficient over bigrams, not Levenshtein. It is
//   listed here because it is the most-installed
//   string-comparison package on npm, and users routinely reach for it
//   without noticing that it is not an edit-distance metric. Included
//   as a "not directly comparable" reference; grouped separately in the
//   output so a reader cannot accidentally cross the wires.
//
// Representation caveat (READ THIS)
// =================================
//
// The four native libraries all take JavaScript `string`; StringCheese
// via wasm takes `Uint8Array`. For ASCII input the semantics are
// equivalent, and we hand each library its natural input representation
// — string to the string libraries, bytes to StringCheese. The FFI cost
// is therefore folded into the comparison **on purpose**: this is the
// "should I use StringCheese through wasm from JS instead of a pure-JS
// implementation" question, and the answer is a whole-stack answer,
// not a DP-kernel-only one.

// `natural` transitively pulls in `dotenvx`, which prints a chatty
// banner on load. The `_env.js` side-effect import below sets
// `DOTENV_CONFIG_QUIET=true` *before* `natural` loads (ESM hoists
// imports in graph order, so a side-effect import listed first
// actually runs first — a bare `process.env.X = "true"` above the
// natural import would be a dead assignment).
import "./_env.js";

import { distance as fastLev } from "fastest-levenshtein";
import jsLevenshtein from "js-levenshtein";
import natural from "natural";
import * as stringSimilarity from "string-similarity";

import { StringCheese } from "../stringcheese_adapter.js";
import { LENGTHS, REGIMES, buildPair, lenLabel } from "./_inputs.js";
import { makeBench, runAll } from "./_runner.js";

const { LevenshteinDistance: NaturalLev } = natural;

// Per-length salts; distinct from every sibling adapter's Levenshtein
// salts so the three harnesses do not accidentally share an unlikely
// corner-case corpus that would confound cross-harness debugging.
//   * Rust:   0xA1, 0xA2, 0xA3
//   * Python: 0xB1, 0xB2, 0xB3
//   * JS:     0xF1, 0xF2, 0xF3
const SALTS = /** @type {[number, number, number]} */ ([0xf1, 0xf2, 0xf3]);

async function main() {
  const sc = new StringCheese();

  // Pre-materialise every (length, regime) input pair, alongside its
  // string-decoded counterpart for the string-taking libraries. Doing
  // both flavours once here keeps the timing loop pure — no allocation,
  // no decode, only the library's own work.
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

  // Unbounded Levenshtein — one bench per (regime, length) cell,
  // with all four contestants side-by-side.
  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aB, bB, aS, bS } = pairs.get(key);
      const bench = makeBench(`levenshtein/${key}`);
      bench.add("stringcheese", () => sc.levenshteinDistance(aB, bB));
      bench.add("fastest-levenshtein", () => fastLev(aS, bS));
      bench.add("js-levenshtein", () => jsLevenshtein(aS, bS));
      bench.add("natural", () => NaturalLev(aS, bS));
      benches.push(bench);
    }
  }

  // Bounded (k = 3) — the classical spellcheck cutoff. StringCheese's
  // `levenshtein-within` short-circuits when the true distance exceeds
  // 3; none of the ecosystem contestants expose a bounded variant, so
  // this is a StringCheese-only group per (regime, length). Kept in
  // the same bench file rather than a sibling because a reader will
  // want to read "bounded k3" numbers next to their unbounded twins.
  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aB, bB } = pairs.get(key);
      const bench = makeBench(`levenshtein_k3/${key}`);
      bench.add("stringcheese", () => sc.levenshteinWithin(aB, bB, 3));
      benches.push(bench);
    }
  }

  // string-similarity is Dice-coefficient-over-bigrams, not
  // Levenshtein — kept in its own group so no reader mistakes it for
  // an edit-distance number. The point of the group is to make the
  // FFI-vs-native-JS comparison visible at each length; the numeric
  // result is not directly comparable to the Levenshtein group above.
  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aS, bS } = pairs.get(key);
      const bench = makeBench(`compareTwoStrings/${key} (NOT Levenshtein)`);
      bench.add("string-similarity", () =>
        stringSimilarity.compareTwoStrings(aS, bS),
      );
      benches.push(bench);
    }
  }

  await runAll(benches);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
