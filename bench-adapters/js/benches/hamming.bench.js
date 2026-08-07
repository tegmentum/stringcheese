// Head-to-head: StringCheese (via wasm) vs. JS Hamming implementations.
//
// Contestants
// ===========
//
// * **StringCheese** — Hamming from the wasm component. The WIT
//   boundary returns `result<u32, string>`; the adapter throws
//   `HammingLengthMismatch` on unequal lengths. Every input in this
//   file is equal-length by construction, so the error path is never
//   hit during timing.
// * **natural.HammingDistance** — a general-purpose NLP toolkit's
//   implementation.
// * **hand-rolled reference** — `[...a].filter((c, i) => c !== b[i]).length`,
//   the one-liner most JS answers reach for. Included to give the
//   reader a "what does the naive one-liner cost" baseline; the
//   ecosystem's dedicated Hamming implementations are surprisingly
//   thin on the ground, so an in-file reference is more informative
//   than pulling in a second npm package.
//
// Notable ecosystem gap: `fastest-levenshtein`, `js-levenshtein`, and
// `string-similarity` **do not** ship a Hamming function. If the
// downstream reader is looking for a Hamming-only fast path in the
// pure-JS ecosystem, `natural` is essentially the only turnkey option;
// everyone else writes it inline. That gap is part of the answer to
// "should I use StringCheese-via-wasm for Hamming from JS".
//
// Representation caveat
// =====================
//
// Same as the Levenshtein bench: StringCheese consumes `Uint8Array`,
// natural consumes `string`, and the hand-rolled reference is fed the
// same string. On ASCII input this is semantically equivalent; the FFI
// cost is folded in on purpose.

// `natural` transitively pulls in `dotenvx`; see levenshtein.bench.js
// for the ESM-hoisting rationale behind the side-effect import.
import "./_env.js";

import natural from "natural";

import { StringCheese } from "../stringcheese_adapter.js";
import { LENGTHS, REGIMES, buildPairEqualLen, lenLabel } from "./_inputs.js";
import { makeBench, runAll } from "./_runner.js";

const { HammingDistance: NaturalHamming } = natural;

// Shared with the Rust and Python adapters — Hamming needs equal-length
// inputs on both sides, and using the same salts across adapters means
// the mismatch positions line up for cross-adapter debugging.
const SALTS = /** @type {[number, number, number]} */ ([0xc1, 0xc2, 0xc3]);

/**
 * Naive-baseline Hamming: iterate one side, tally mismatches. Fed
 * strings because that is what a StackOverflow-copy-paste would look
 * like — a `Uint8Array` version would win on speed but would not
 * reflect what the ecosystem actually writes when it needs Hamming.
 * @param {string} a
 * @param {string} b
 * @returns {number}
 */
function handRolledHamming(a, b) {
  if (a.length !== b.length) {
    throw new Error("Hamming: length mismatch");
  }
  let n = 0;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) n++;
  return n;
}

async function main() {
  const sc = new StringCheese();
  const decoder = new TextDecoder("ascii");

  /** @type {Map<string, {aB: Uint8Array, bB: Uint8Array, aS: string, bS: string}>} */
  const pairs = new Map();
  for (const n of LENGTHS) {
    for (const kind of REGIMES) {
      const [aB, bB] = buildPairEqualLen(n, kind, SALTS);
      pairs.set(`${kind}/${lenLabel(n)}`, {
        aB,
        bB,
        aS: decoder.decode(aB),
        bS: decoder.decode(bB),
      });
    }
  }

  const benches = [];
  for (const kind of REGIMES) {
    for (const n of LENGTHS) {
      const key = `${kind}/${lenLabel(n)}`;
      const { aB, bB, aS, bS } = pairs.get(key);
      const bench = makeBench(`hamming/${key}`);
      bench.add("stringcheese", () => sc.hammingDistance(aB, bB));
      bench.add("natural", () => NaturalHamming(aS, bS));
      bench.add("hand-rolled", () => handRolledHamming(aS, bS));
      benches.push(bench);
    }
  }

  await runAll(benches);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
