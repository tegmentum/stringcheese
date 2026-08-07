// StringCheese Go bench adapter — the fourth non-Rust adapter after
// Rust, Python, and JavaScript. Loads StringCheese from the WebAssembly
// component built by `cargo component build --release` under
// ../../component/rust-host/ and compares against Go ecosystem
// string-distance libraries.
//
// The subtree is deliberately not part of the outer Cargo workspace — see
// ../README.md for the umbrella "why each adapter is standalone" rationale.
//
// Runtime
// -------
//
// This adapter uses github.com/tetratelabs/wazero — the pure-Go, no-CGO
// WebAssembly runtime. wazero does not understand Component Model wasm
// today (see https://github.com/tetratelabs/wazero/issues/2049 and the
// upstream tracking issues linked from it); we work around that by
// extracting the *inner core module* from the built component with
// `wasm-tools component unbundle` at adapter-init time, then running
// that core module in wazero with WASI preview1 imports satisfied by
// wazero's built-in wasi_snapshot_preview1 shim. All parameter
// marshalling then goes through the canonical ABI (cabi_realloc for
// list<u8> inputs, return-area pointers for compound returns,
// cabi_post_* for freeing dynamic memory in returned strings/lists).
//
// The alternative would have been wasmtime-go, which supports the
// Component Model natively but requires CGO. wasmtime-go's status is
// also non-ideal: the pure-Go wasmtime bindings module was archived
// upstream in 2025, superseded by go-modules generated code. Sticking
// to wazero keeps the adapter buildable with `go build` alone.

module github.com/tegmentum/stringcheese/bench-adapters/go

go 1.22.0

require (
	github.com/agnivade/levenshtein v1.2.1
	github.com/hbollon/go-edlib v1.7.0
	github.com/tetratelabs/wazero v1.9.0
)
