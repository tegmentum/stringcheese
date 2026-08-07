#!/usr/bin/env node
// StringCheese JS bench adapter — jco transpile driver.
//
// Runs `jco transpile` on the wasm component built by
// `cargo component build --release` inside `component/rust-host/` and
// writes the resulting JavaScript + core-wasm files into ./transpiled/.
// After a successful run, ./transpiled/stringcheese.js is a plain ES
// module that any Node.js 20+ process can `import` — that is what the
// sibling `stringcheese_adapter.js` consumes.
//
// This script is invoked via `npm run transpile` (see package.json).
// The output directory is git-ignored so the transpile step is fully
// reproducible: delete ./transpiled/, run `npm run transpile`, and the
// three files (stringcheese.js, stringcheese.core.wasm, and the
// interfaces/ subtree of .d.ts stubs) reappear identical to the previous
// run for the same input .wasm.
//
// Why a Node script and not a raw `npx jco …` in package.json's
// `scripts` block: the discovery of the input .wasm has to be robust to
// (a) being run from the adapter directory, (b) being run from the
// repo root via `npm --prefix bench-adapters/js run transpile`, and
// (c) the wasm having been built to an out-of-tree target directory (via
// `CARGO_TARGET_DIR` or the `WASM` env var). Doing that discovery in
// JavaScript keeps `package.json` short and the failure modes readable.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Default path to the pre-built component .wasm produced by
// `cargo component build --release`. The relative path here matches the
// layout the repository ships — `bench-adapters/js/scripts/` two levels
// up to the repo root, then into `component/rust-host/target/…`.
const DEFAULT_WASM = resolve(
  __dirname,
  "../../../component/rust-host/target/wasm32-wasip1/release/stringcheese_component_host.wasm",
);

// Output directory. Matches the .gitignore rule and the import path in
// stringcheese_adapter.js — do not rename in isolation.
const OUT_DIR = resolve(__dirname, "../transpiled");

function main() {
  const wasm = process.env.STRINGCHEESE_WASM
    ? resolve(process.env.STRINGCHEESE_WASM)
    : DEFAULT_WASM;

  if (!existsSync(wasm)) {
    console.error(
      `[transpile] StringCheese component .wasm not found at:\n  ${wasm}\n\n` +
        "Build it first with:\n" +
        "  cd component/rust-host && cargo component build --release\n\n" +
        "Or set STRINGCHEESE_WASM to an explicit path.",
    );
    process.exit(1);
  }

  // jco writes into --out-dir but does not create the parent chain, so
  // ensure it exists before invoking. This is a no-op when --out-dir
  // already exists.
  mkdirSync(OUT_DIR, { recursive: true });

  // Invoke `npx jco transpile <wasm> --out-dir <out>` via child_process
  // rather than importing the jco JavaScript API — the CLI surface is
  // the documented public interface, so the adapter tracks whatever
  // `@bytecodealliance/jco` is installed as a dev dependency. See
  // README.md "jco setup" for the pinned version.
  //
  // `--name stringcheese` pins the transpiled entry filename to
  // `stringcheese.js` regardless of the input .wasm basename, so
  // `stringcheese_adapter.js`'s `import from './transpiled/stringcheese.js'`
  // resolves consistently across builds (a cargo-component rename or
  // an out-of-tree wasm path would otherwise produce a different
  // filename and silently break the adapter's import).
  const args = ["jco", "transpile", wasm, "--name", "stringcheese", "--out-dir", OUT_DIR];
  console.log(`[transpile] npx ${args.join(" ")}`);
  const result = spawnSync("npx", args, { stdio: "inherit" });

  if (result.error) {
    console.error("[transpile] failed to invoke npx:", result.error.message);
    process.exit(1);
  }
  if (typeof result.status === "number" && result.status !== 0) {
    process.exit(result.status);
  }
  console.log(`[transpile] wrote ${OUT_DIR}`);
}

main();
