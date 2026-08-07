// JavaScript adapter around the StringCheese WebAssembly component.
//
// This module is the JS-side face of `component/rust-host/`. It loads
// the jco-transpiled ES module produced by `npm run transpile` (see
// `scripts/transpile.js`) and exposes every WIT-declared function as a
// plain JavaScript method — the same shape as the Python adapter at
// `bench-adapters/python/stringcheese_adapter.py`, keeping cross-adapter
// bench files trivially portable.
//
// Design notes
// ============
//
// The StringCheese component is a pure library — no imports from the
// host, no I/O, no clocks. But because the host crate targets
// `wasm32-wasip1`, the produced binary imports a small tail of WASI 0.2
// interfaces (stdio, filesystem, clocks) even though the algorithm code
// never touches any of them. jco satisfies those imports automatically
// via `@bytecodealliance/preview2-shim`, which ships as a transitive
// dep of `@bytecodealliance/jco` — no explicit host-side wiring needed.
//
// Unlike `wasmtime-py`'s per-instance Store/Instance model, jco's
// transpiled output is a *stateless* ES module: the wasm instance is
// materialised at module load time and every exported function is a
// plain synchronous function on the top-level namespace. There is no
// `post_return` bookkeeping to drive at the JS side — jco's generated
// wrapper handles it internally.
//
// Because of that, the `StringCheese` class here is a thin façade that
// binds every exported function to a stable method name. Keeping a
// class shape (rather than a bag of exported functions) matches the
// Python adapter's `StringCheese` type and leaves room for future
// per-instance state (e.g. a lazy async loader for browser targets)
// without breaking callers.
//
// WIT boundary shapes handled here
// --------------------------------
//
// * `list<u8> × list<u8> → u32` — every distance metric except Hamming.
//   Inputs are `Uint8Array`; result is a plain `number` (safe integer).
// * `list<u8> × list<u8> × u32 → variant { within(u32), exceeded(u32) }`
//   — bounded Levenshtein. jco returns `{ tag: 'within'|'exceeded',
//   val: number }`; we translate to a small `BoundedDistance` object.
// * `list<u8> × list<u8> → result<u32, string>` — Hamming. jco throws
//   on `err`, unwrapping the error string; we wrap that in a typed
//   `HammingLengthMismatch` for callers that want to `instanceof`-check.
// * `list<u8> × list<u8> → option<u32>` — search. jco returns `number`
//   for `some` and `undefined` for `none`; we surface `null` for
//   `none` to keep the API pythonic-equivalent.
// * `list<u8> × list<u8> → list<u32>` — search. jco returns a
//   `Uint32Array`; we return it as-is (typed arrays are the more
//   efficient interchange).
// * `string → string` — phonetic. Plain `string` on both sides.
//
// The WIT `full Damerau` function is deliberately absent from the
// component (see `component/README.md` "Deliberately not exposed" — the
// underlying kernel needs a `HashMap` which pulls in `getrandom` on
// wasm32-*). The adapter therefore throws `NotImplementedError` from
// `damerauDistance` so callers can distinguish "not exposed at the WIT
// boundary" from "not implemented in StringCheese", and so bench files
// can catch it and document the gap.
//
// Setup
// -----
//
// Before importing this module, run `npm run transpile` once from
// `bench-adapters/js/` — that generates `./transpiled/stringcheese.js`.
// A missing transpile directory produces a clear import error at
// module-load time rather than a cryptic runtime failure.

// Named import intentionally: distance / similarity / search / phonetic
// are the four WIT interfaces the component exports, each with the
// functions listed in the .d.ts files jco writes into
// `./transpiled/interfaces/`.
import {
  distance,
  similarity,
  search,
  phonetic,
} from "./transpiled/stringcheese.js";

/**
 * Result of a bounded distance call. Mirrors the WIT
 * `variant bounded-distance { within(u32), exceeded(u32) }` and the
 * Python adapter's `BoundedDistance` dataclass.
 *
 * `within` carries the exact distance (guaranteed `<= cutoff`);
 * `exceeded` carries the cutoff itself and signals the true distance
 * is strictly greater.
 */
export class BoundedDistance {
  /**
   * @param {'within' | 'exceeded'} kind
   * @param {number} value
   */
  constructor(kind, value) {
    this.kind = kind;
    this.value = value;
    Object.freeze(this);
  }

  get isWithin() {
    return this.kind === "within";
  }
}

/**
 * Thrown when Hamming distance is asked for on unequal-length inputs.
 * The WIT boundary returns `result<u32, string>` for Hamming; the
 * underlying Rust kernel's typed `LengthMismatch` error is flattened to
 * a diagnostic string, which jco raises as an `Error`. This subclass
 * preserves the diagnostic unchanged so callers can log or match on it.
 */
export class HammingLengthMismatch extends Error {
  constructor(message) {
    super(message);
    this.name = "HammingLengthMismatch";
  }
}

/**
 * A loaded StringCheese component. Unlike `wasmtime-py`, jco's
 * transpiled module is stateless and thread-safe (the underlying wasm
 * instance is single-threaded but every call is synchronous, and jco
 * handles its own memory bookkeeping between calls). Constructing more
 * than one `StringCheese` at once is safe and cheap; the same
 * underlying instance is shared under the hood.
 */
export class StringCheese {
  constructor() {
    // Bind every exported WIT function to a stable method name. Using
    // the function reference directly (rather than `distance.foo(...)`
    // via a wrapper) elides one property lookup per call — trivial per
    // call but noticeable across a bench loop of millions of iterations.
    this._levenshtein = distance.levenshtein;
    this._levenshteinWithin = distance.levenshteinWithin;
    this._hamming = distance.hamming;
    this._osa = distance.osa;
    this._lcsDistance = distance.lcsDistance;

    this._jaro = similarity.jaro;
    this._jaroWinkler = similarity.jaroWinkler;
    this._diceBigrams = similarity.diceBigrams;
    this._jaccardBigrams = similarity.jaccardBigrams;

    this._findFirst = search.findFirst;
    this._findAll = search.findAll;

    this._soundex = phonetic.soundex;
    this._nysiis = phonetic.nysiis;
    this._doubleMetaphonePrimary = phonetic.doubleMetaphonePrimary;
  }

  // ------------------------------------------------------------------ //
  // Distance                                                           //
  // ------------------------------------------------------------------ //

  /**
   * Unit-cost Levenshtein edit distance (byte-level).
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  levenshteinDistance(a, b) {
    return this._levenshtein(a, b);
  }

  /**
   * Bounded Levenshtein — early-terminates when the true distance
   * exceeds `cutoff`. Returns a `BoundedDistance`.
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @param {number} cutoff
   * @returns {BoundedDistance}
   */
  levenshteinWithin(a, b, cutoff) {
    const v = this._levenshteinWithin(a, b, cutoff);
    return new BoundedDistance(v.tag, v.val);
  }

  /**
   * Hamming distance. Throws `HammingLengthMismatch` on unequal
   * lengths (the WIT error path).
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  hammingDistance(a, b) {
    try {
      return this._hamming(a, b);
    } catch (err) {
      // jco raises a plain Error whose message is the WIT-side string;
      // rewrap so callers get a typed error and the original error
      // stays as .cause for diagnostic pretty-printing.
      throw new HammingLengthMismatch(err?.message ?? String(err), {
        cause: err,
      });
    }
  }

  /**
   * Optimal String Alignment / restricted Damerau distance.
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  osaDistance(a, b) {
    return this._osa(a, b);
  }

  /**
   * LCS-derived distance: `|a| + |b| - 2 * lcs(a, b)`.
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  lcsDistance(a, b) {
    return this._lcsDistance(a, b);
  }

  /**
   * Full unrestricted Damerau distance.
   *
   * **Not exposed at the WIT boundary** — see `component/README.md`
   * "Deliberately not exposed". The underlying Rust kernel needs a
   * `HashMap`, which pulls in `getrandom` on wasm32 targets and would
   * require additional host wiring. Throws unconditionally so bench
   * files can catch it and mark the StringCheese cell as N/A.
   * @returns {never}
   */
  damerauDistance() {
    const err = new Error(
      "Full Damerau is not exposed by the StringCheese WIT component; " +
        "use `osaDistance` (restricted Damerau) instead.",
    );
    err.name = "NotImplementedError";
    throw err;
  }

  // ------------------------------------------------------------------ //
  // Similarity                                                         //
  // ------------------------------------------------------------------ //

  /**
   * Jaro similarity in `[0.0, 1.0]`.
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  jaroSimilarity(a, b) {
    return this._jaro(a, b);
  }

  /**
   * Jaro–Winkler (classic: prefix 4, scaling 0.1) in `[0.0, 1.0]`.
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  jaroWinklerSimilarity(a, b) {
    return this._jaroWinkler(a, b);
  }

  /**
   * Dice / Sørensen coefficient over character bigrams.
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  diceBigrams(a, b) {
    return this._diceBigrams(a, b);
  }

  /**
   * Jaccard similarity over character bigrams.
   * @param {Uint8Array} a
   * @param {Uint8Array} b
   * @returns {number}
   */
  jaccardBigrams(a, b) {
    return this._jaccardBigrams(a, b);
  }

  // ------------------------------------------------------------------ //
  // Search                                                             //
  // ------------------------------------------------------------------ //

  /**
   * First occurrence of `needle` in `haystack`, or `null` if absent.
   * @param {Uint8Array} needle
   * @param {Uint8Array} haystack
   * @returns {number | null}
   */
  findFirst(needle, haystack) {
    const r = this._findFirst(needle, haystack);
    return r === undefined ? null : r;
  }

  /**
   * Every occurrence of `needle` in `haystack`, ascending, may overlap.
   * Returns the `Uint32Array` jco produces directly — callers that
   * want a plain `Array` can `Array.from(...)` it.
   * @param {Uint8Array} needle
   * @param {Uint8Array} haystack
   * @returns {Uint32Array}
   */
  findAll(needle, haystack) {
    return this._findAll(needle, haystack);
  }

  // ------------------------------------------------------------------ //
  // Phonetic                                                           //
  // ------------------------------------------------------------------ //

  /**
   * NARA-1918 canonical Soundex encoding.
   * @param {string} name
   * @returns {string}
   */
  soundex(name) {
    return this._soundex(name);
  }

  /**
   * NYSIIS encoding, truncated to six characters.
   * @param {string} name
   * @returns {string}
   */
  nysiis(name) {
    return this._nysiis(name);
  }

  /**
   * Double Metaphone primary key (Philips 1999).
   * @param {string} name
   * @returns {string}
   */
  doubleMetaphonePrimary(name) {
    return this._doubleMetaphonePrimary(name);
  }
}

export default StringCheese;
