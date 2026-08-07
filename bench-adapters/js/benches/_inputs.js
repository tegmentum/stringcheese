// Shared corpus-generation helpers for the JS bench adapter.
//
// This module is a byte-for-byte port of
// `bench-adapters/rust/src/lib.rs`'s SplitMix64 corpus builder and
// `bench-adapters/python/benches/_inputs.py`'s Python port. Corpora
// produced from `(length, salt)` here match the Rust and Python
// adapters' corpora for the same `(length, salt)`, so a StringCheese
// datapoint from any of the three harnesses lands on the same input
// family.
//
// Determinism is load-bearing: tinybench reruns each task across many
// samples and compares the resulting distributions; if the input were
// re-randomised each sample the noise would be corpus variance, not
// implementation variance.
//
// JavaScript does not have native `u64` arithmetic — every arithmetic
// op has to use `BigInt` to preserve the exact 64-bit wrap semantics
// the Rust/Python implementations rely on. That is expensive, but the
// corpus is built **outside** the timing region (each bench file caches
// the pairs in a Map before entering the tinybench loop), so BigInt
// overhead is a one-shot startup cost per bench file.

/** Canonical length sweep. Same as `stringcheese-bench` and every
 *  sibling bench adapter — do not diverge without updating all three. */
export const LENGTHS = Object.freeze([8, 32, 128, 512, 2048]);

/** Similarity regimes, in the order the Rust harness emits them. */
export const REGIMES = Object.freeze(["random", "similar", "identical"]);

// --------------------------------------------------------------------------- //
// SplitMix64 — BigInt port of the Rust/Python `_Rng`.                         //
// --------------------------------------------------------------------------- //

const GOLDEN_GAMMA = 0x9e3779b97f4a7c15n;
const MASK64 = (1n << 64n) - 1n;
const SCRAMBLE1 = 0xbf58476d1ce4e5b9n;
const SCRAMBLE2 = 0x94d049bb133111ebn;

class SplitMix64 {
  /** @param {bigint} seed */
  constructor(seed) {
    this._state = (seed + GOLDEN_GAMMA) & MASK64;
  }

  /** @returns {bigint} */
  nextU64() {
    this._state = (this._state + GOLDEN_GAMMA) & MASK64;
    let z = this._state;
    z = ((z ^ (z >> 30n)) * SCRAMBLE1) & MASK64;
    z = ((z ^ (z >> 27n)) * SCRAMBLE2) & MASK64;
    return z ^ (z >> 31n);
  }

  /** @param {number} bound — small positive integer. @returns {number} */
  nextBounded(bound) {
    // Convert the u64 result down through Number for small bounds; the
    // Rust adapter uses `next_u64() % bound` on a `u64`, and for the
    // bounds we care about (26, 3, |length|) that fits in a Number.
    // Reduce first to a manageable BigInt before Number-casting.
    const boundB = BigInt(bound);
    const r = this.nextU64() % boundB;
    return Number(r);
  }

  /** @returns {number} — a lowercase ASCII byte value. */
  nextAsciiLower() {
    return 97 /* 'a' */ + this.nextBounded(26);
  }
}

/**
 * Deterministic per-(length, salt) seed matching the Rust adapter.
 * @param {number} length
 * @param {number|bigint} salt
 * @returns {bigint}
 */
export function seedFor(length, salt) {
  const l = BigInt(length);
  const s = typeof salt === "bigint" ? salt : BigInt(salt);
  return ((l * GOLDEN_GAMMA) & MASK64) ^ s;
}

// --------------------------------------------------------------------------- //
// Corpus builders                                                             //
// --------------------------------------------------------------------------- //

/**
 * A fresh `Uint8Array` of `length` lowercase-ASCII bytes.
 * @param {number} length
 * @param {bigint} seed
 * @returns {Uint8Array}
 */
export function randomAscii(length, seed) {
  const rng = new SplitMix64(seed);
  const out = new Uint8Array(length);
  for (let i = 0; i < length; i++) out[i] = rng.nextAsciiLower();
  return out;
}

/**
 * Two-byte-equal regime — both sides are the same `Uint8Array` value.
 * (Same reference, matching the Python adapter's `identical_pair`.)
 * @param {number} length
 * @param {bigint} seed
 * @returns {[Uint8Array, Uint8Array]}
 */
export function identicalPair(length, seed) {
  const s = randomAscii(length, seed);
  return [s, s];
}

/**
 * Two strings differing by roughly `editRate * length` mixed edits.
 * Length is only approximate — insertions and deletions cancel on
 * average. Callers that need equal-length inputs (Hamming) should use
 * `similarPairEqualLen`.
 * @param {number} length
 * @param {number} editRate
 * @param {bigint} seed
 * @returns {[Uint8Array, Uint8Array]}
 */
export function similarPair(length, editRate, seed) {
  const left = randomAscii(length, seed);
  const right = Array.from(left);
  const nEdits = Math.max(0, Math.round(length * editRate));
  const rng = new SplitMix64((seed ^ 0xa5a5a5a5a5a5a5a5n) & MASK64);
  for (let i = 0; i < nEdits; i++) {
    if (right.length === 0) {
      right.push(rng.nextAsciiLower());
      continue;
    }
    const op = rng.nextBounded(3);
    const pos = rng.nextBounded(right.length);
    if (op === 0) {
      right[pos] = rng.nextAsciiLower();
    } else if (op === 1) {
      right.splice(pos, 0, rng.nextAsciiLower());
    } else {
      right.splice(pos, 1);
    }
  }
  return [left, Uint8Array.from(right)];
}

/**
 * Two equal-length strings differing in ~`editRate * length` positions.
 * Substitutions only. Positions may collide, so the true mismatch
 * count can be slightly below the target. Hamming-regime input.
 * @param {number} length
 * @param {number} editRate
 * @param {bigint} seed
 * @returns {[Uint8Array, Uint8Array]}
 */
export function similarPairEqualLen(length, editRate, seed) {
  const left = randomAscii(length, seed);
  const right = new Uint8Array(left); // structural copy
  const nEdits = Math.min(length, Math.max(0, Math.round(length * editRate)));
  if (nEdits === 0 || length === 0) return [left, right];
  const rng = new SplitMix64((seed ^ 0xc3c3c3c3c3c3c3c3n) & MASK64);
  const A = 97;
  for (let i = 0; i < nEdits; i++) {
    const pos = rng.nextBounded(length);
    const bump = 1 + rng.nextBounded(25);
    right[pos] = A + ((right[pos] - A + bump) % 26);
  }
  return [left, right];
}

/**
 * Regime dispatcher matching `stringcheese-bench` / the sibling adapters.
 * @param {number} length
 * @param {'random'|'similar'|'identical'} kind
 * @param {[number|bigint, number|bigint, number|bigint]} salts
 * @returns {[Uint8Array, Uint8Array]}
 */
export function buildPair(length, kind, salts) {
  const [rA, rB, simOrIdent] = salts;
  switch (kind) {
    case "random":
      return [
        randomAscii(length, seedFor(length, rA)),
        randomAscii(length, seedFor(length, rB)),
      ];
    case "similar":
      return similarPair(length, 0.05, seedFor(length, simOrIdent));
    case "identical":
      return identicalPair(length, seedFor(length, simOrIdent));
    default:
      throw new Error(`unknown similarity regime: ${kind}`);
  }
}

/**
 * Equal-length variant of `buildPair` for Hamming.
 * @param {number} length
 * @param {'random'|'similar'|'identical'} kind
 * @param {[number|bigint, number|bigint, number|bigint]} salts
 * @returns {[Uint8Array, Uint8Array]}
 */
export function buildPairEqualLen(length, kind, salts) {
  const [rA, rB, simOrIdent] = salts;
  switch (kind) {
    case "random":
      return [
        randomAscii(length, seedFor(length, rA)),
        randomAscii(length, seedFor(length, rB)),
      ];
    case "similar":
      return similarPairEqualLen(length, 0.05, seedFor(length, simOrIdent));
    case "identical":
      return identicalPair(length, seedFor(length, simOrIdent));
    default:
      throw new Error(`unknown similarity regime: ${kind}`);
  }
}

/**
 * Zero-padded length label, matching the sibling adapters' `len0032` /
 * `len0128` output for greppable, sortable bench ids.
 * @param {number} n
 */
export function lenLabel(n) {
  return `len${String(n).padStart(4, "0")}`;
}
