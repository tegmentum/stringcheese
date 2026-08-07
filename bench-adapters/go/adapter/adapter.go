// Package adapter is the Go-side face of the StringCheese WebAssembly
// component. It loads the .wasm produced by
// `cargo component build --release` under ../../component/rust-host/
// and exposes every WIT-declared function as a plain Go method.
//
// # Runtime choice
//
// This adapter uses [wazero] — the pure-Go, no-CGO WebAssembly runtime.
// wazero does not currently execute Component Model wasm binaries; it is
// a WASI-preview1 / core-wasm runtime. Rather than reach for a
// CGO-linked wasmtime binding (whose upstream Go package was archived
// in 2025), we work around wazero's Component Model gap by extracting
// the inner core module out of the component with `wasm-tools
// component unbundle` and running that in wazero.
//
// # Canonical ABI wiring
//
// The extracted core module still expects the Component Model's
// canonical ABI at every export:
//
//   - list<u8> inputs: caller allocates memory inside guest linear
//     memory via `cabi_realloc(0, 0, align, size)`, copies the payload,
//     and passes (ptr, len) as a pair of i32 params. Ownership transfers
//     to the guest, which frees on drop inside its own Rust code.
//   - Scalar returns (u32, f64): come back as the function's ordinary
//     return value.
//   - Compound returns that fit in the flat form's single return slot
//     (e.g. option<u32>): come back as a discriminant packed into the
//     return, or use the return-area pattern below.
//   - Compound returns that don't fit (result<u32, string>, variant
//     bounded-distance): the guest owns a static return-area buffer;
//     the exported function returns a pointer to it, and we read the
//     fields from linear memory at that offset. When the return
//     contains dynamic allocations (a returned string), we must call
//     the paired `cabi_post_<func>` afterwards to let the guest release
//     that memory.
//
// The layout of each return-area struct is fixed by the canonical ABI's
// field-order rules; see [canonical_abi.md] in the WebAssembly
// component-model spec repo for the source of truth.
//
// # Deliberately absent
//
// Full unrestricted Damerau is not exposed at the WIT boundary (see
// ../../component/README.md "Deliberately not exposed" — the underlying
// Rust kernel needs a HashMap, which pulls in getrandom on wasm32-*).
// [StringCheese.DamerauDistance] therefore returns
// [ErrDamerauNotExposed] unconditionally so bench files can catch it
// and mark the StringCheese cell as N/A.
//
// [wazero]: https://github.com/tetratelabs/wazero
// [canonical_abi.md]: https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md
package adapter

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

// ErrDamerauNotExposed is returned by [StringCheese.DamerauDistance].
// Full Damerau is not part of the StringCheese WIT world; use
// [StringCheese.OSADistance] for restricted (OSA) Damerau instead.
var ErrDamerauNotExposed = errors.New(
	"full Damerau is not exposed by the StringCheese WIT component; " +
		"use OSADistance (restricted Damerau) instead",
)

// BoundedDistance is the Go mirror of the WIT
// `variant bounded-distance { within(u32), exceeded(u32) }`. Within
// carries the exact distance (guaranteed <= cutoff); Exceeded carries
// the cutoff itself and signals the true distance is strictly greater.
type BoundedDistance struct {
	// Kind is "within" or "exceeded".
	Kind string
	// Value carries the payload for both variants.
	Value uint32
}

// IsWithin is a convenience for `.Kind == "within"`, matching the
// Python/JS adapters' shape.
func (b BoundedDistance) IsWithin() bool { return b.Kind == "within" }

// HammingLengthMismatch is the typed error a StringCheese Hamming call
// produces when the two inputs have unequal length. The wrapped
// diagnostic string is the one the underlying Rust kernel emits, passed
// through the WIT `result<u32, string>` return.
type HammingLengthMismatch struct{ Msg string }

func (e *HammingLengthMismatch) Error() string { return e.Msg }

// Options configure a StringCheese instance. Zero values are fine.
type Options struct {
	// WasmPath overrides the default core-module discovery path. When
	// empty, the adapter looks (in order) at the STRINGCHEESE_CORE_WASM
	// env var, then at the extract-cache path
	// component/rust-host/target/wasm32-wasip1/release/unbundled/
	// unbundled-module0.wasm relative to the repo root.
	WasmPath string
	// ComponentPath overrides the default component .wasm discovery
	// path used when the core module needs to be extracted. When
	// empty, the STRINGCHEESE_WASM env var or the standard
	// component/rust-host/... path is used.
	ComponentPath string
}

// StringCheese is one loaded instance of the StringCheese component's
// inner core module. Not safe for concurrent use — wasm module
// instances are single-threaded by design; construct one per goroutine
// if you need concurrent calls. For a bench harness (Go testing.B runs
// iterations serially per benchmark) a single instance is fine.
type StringCheese struct {
	runtime  wazero.Runtime
	module   api.Module
	memory   api.Memory
	realloc  api.Function
	closeCtx context.Context

	// Cached function handles for every WIT export we call. Resolved
	// once at construction; the per-call cost is exactly the guest's
	// canonical-ABI marshalling + kernel work, not a map lookup on top.
	fnLevenshtein       api.Function
	fnLevenshteinWithin api.Function
	fnHamming           api.Function
	fnHammingPost       api.Function
	fnOsa               api.Function
	fnLcsDistance       api.Function

	fnJaro           api.Function
	fnJaroWinkler    api.Function
	fnDiceBigrams    api.Function
	fnJaccardBigrams api.Function

	// Guard on Close() so double-close is a no-op.
	closeOnce sync.Once
}

// New constructs a StringCheese by loading the extracted core module
// through wazero. The first call in a process will invoke
// `wasm-tools component unbundle` to extract the core module out of the
// component .wasm if the extract cache is missing; that requires
// wasm-tools to be on PATH.
//
// The returned StringCheese owns a wazero.Runtime and its instantiated
// module. Call [StringCheese.Close] when done.
func New(ctx context.Context, opts Options) (*StringCheese, error) {
	corePath, err := resolveCoreModule(opts)
	if err != nil {
		return nil, fmt.Errorf("resolve core module: %w", err)
	}

	wasmBytes, err := os.ReadFile(corePath)
	if err != nil {
		return nil, fmt.Errorf("read core module %q: %w", corePath, err)
	}

	rt := wazero.NewRuntime(ctx)
	// wazero's compiler cache is per-Runtime; nothing to do here.

	// The extracted core module imports wasi_snapshot_preview1
	// (environ_get, environ_sizes_get, fd_write, proc_exit) because
	// the Rust host crate targets wasm32-wasip1. The StringCheese
	// algorithm code never calls any of these; the imports exist only
	// because Rust's std is linked in for panic-handling. Instantiate
	// wazero's built-in shim to satisfy them.
	if _, ierr := wasi_snapshot_preview1.Instantiate(ctx, rt); ierr != nil {
		_ = rt.Close(ctx)
		return nil, fmt.Errorf("instantiate wasi_snapshot_preview1: %w", ierr)
	}

	compiled, cerr := rt.CompileModule(ctx, wasmBytes)
	if cerr != nil {
		_ = rt.Close(ctx)
		return nil, fmt.Errorf("compile core module: %w", cerr)
	}

	// Anonymous name — we never look this module up by name, and giving
	// it one makes wazero refuse subsequent instantiations by the same
	// name. Cleanup happens through the runtime's Close on shutdown.
	mod, merr := rt.InstantiateModule(ctx, compiled, wazero.NewModuleConfig().
		WithName("").
		// stdout/stderr left as no-op writers; the module doesn't
		// write to them under normal execution, but a Rust panic could.
		WithStdout(discard{}).
		WithStderr(discard{}))
	if merr != nil {
		_ = rt.Close(ctx)
		return nil, fmt.Errorf("instantiate core module: %w", merr)
	}

	sc := &StringCheese{
		runtime:  rt,
		module:   mod,
		memory:   mod.Memory(),
		closeCtx: ctx,
	}
	if sc.memory == nil {
		_ = rt.Close(ctx)
		return nil, errors.New("core module exports no memory")
	}

	sc.realloc = mod.ExportedFunction("cabi_realloc")
	if sc.realloc == nil {
		_ = rt.Close(ctx)
		return nil, errors.New("core module missing cabi_realloc export")
	}

	// Resolve every WIT export we plan to call. Missing exports are a
	// hard error at construction rather than a per-call panic.
	if err := sc.resolveExports(); err != nil {
		_ = rt.Close(ctx)
		return nil, err
	}

	return sc, nil
}

// Close releases the wazero runtime and its instantiated module. Safe
// to call more than once; subsequent calls are no-ops.
func (s *StringCheese) Close() error {
	var err error
	s.closeOnce.Do(func() {
		err = s.runtime.Close(s.closeCtx)
	})
	return err
}

// discard is an io.Writer that drops every write — used as the module's
// stdout/stderr so a stray Rust panic can't accidentally pollute the
// bench output.
type discard struct{}

func (discard) Write(p []byte) (int, error) { return len(p), nil }

// --------------------------------------------------------------------- //
// Export resolution
// --------------------------------------------------------------------- //

func (s *StringCheese) resolveExports() error {
	type entry struct {
		name  string
		field *api.Function
	}
	// Every export lives in its WIT interface's namespace, in the shape
	// `stringcheese:core/<iface>@0.1.0#<func>`. Cache them all up front.
	targets := []entry{
		{"stringcheese:core/distance@0.1.0#levenshtein", &s.fnLevenshtein},
		{"stringcheese:core/distance@0.1.0#levenshtein-within", &s.fnLevenshteinWithin},
		{"stringcheese:core/distance@0.1.0#hamming", &s.fnHamming},
		{"stringcheese:core/distance@0.1.0#osa", &s.fnOsa},
		{"stringcheese:core/distance@0.1.0#lcs-distance", &s.fnLcsDistance},
		{"stringcheese:core/similarity@0.1.0#jaro", &s.fnJaro},
		{"stringcheese:core/similarity@0.1.0#jaro-winkler", &s.fnJaroWinkler},
		{"stringcheese:core/similarity@0.1.0#dice-bigrams", &s.fnDiceBigrams},
		{"stringcheese:core/similarity@0.1.0#jaccard-bigrams", &s.fnJaccardBigrams},
	}
	for _, t := range targets {
		f := s.module.ExportedFunction(t.name)
		if f == nil {
			return fmt.Errorf("missing WIT export %q", t.name)
		}
		*t.field = f
	}
	// cabi_post exports — required for any function whose return
	// contains dynamic allocations (strings, lists). Hamming is the
	// only one in the call-path benched from Go for now.
	s.fnHammingPost = s.module.ExportedFunction(
		"cabi_post_stringcheese:core/distance@0.1.0#hamming",
	)
	if s.fnHammingPost == nil {
		return errors.New("missing cabi_post_stringcheese:core/distance@0.1.0#hamming export")
	}
	return nil
}

// --------------------------------------------------------------------- //
// Canonical ABI helpers
// --------------------------------------------------------------------- //

// allocList copies `data` into the guest's linear memory using
// cabi_realloc and returns (ptr, len). Every list<u8> input to a WIT
// function goes through this: the canonical ABI transfers ownership of
// the buffer to the guest, which frees it on drop inside the generated
// Rust wrapper. Callers do NOT free the returned pointer themselves.
//
// Alignment is 1 because the payload is a plain byte buffer;
// wit-bindgen's cabi_realloc respects the requested alignment.
func (s *StringCheese) allocList(ctx context.Context, data []byte) (uint32, uint32, error) {
	if len(data) == 0 {
		// The canonical ABI permits a null pointer for a zero-length
		// list<u8>. Skipping the realloc call avoids the per-call
		// overhead for the edge case (empty input pairs).
		return 0, 0, nil
	}
	// cabi_realloc(old_ptr=0, old_size=0, align=1, new_size=len)
	res, err := s.realloc.Call(ctx, 0, 0, 1, uint64(len(data)))
	if err != nil {
		return 0, 0, fmt.Errorf("cabi_realloc(%d bytes): %w", len(data), err)
	}
	ptr := uint32(res[0])
	if !s.memory.Write(ptr, data) {
		return 0, 0, fmt.Errorf(
			"copy %d bytes into guest memory at ptr=%#x: out of bounds",
			len(data), ptr,
		)
	}
	return ptr, uint32(len(data)), nil
}

// --------------------------------------------------------------------- //
// Distance
// --------------------------------------------------------------------- //

// LevenshteinDistance computes the unit-cost byte-level Levenshtein
// edit distance between a and b via the StringCheese wasm kernel.
func (s *StringCheese) LevenshteinDistance(a, b []byte) (uint32, error) {
	ctx := s.closeCtx
	aPtr, aLen, err := s.allocList(ctx, a)
	if err != nil {
		return 0, err
	}
	bPtr, bLen, err := s.allocList(ctx, b)
	if err != nil {
		return 0, err
	}
	res, err := s.fnLevenshtein.Call(ctx,
		uint64(aPtr), uint64(aLen), uint64(bPtr), uint64(bLen),
	)
	if err != nil {
		return 0, fmt.Errorf("levenshtein: %w", err)
	}
	return uint32(res[0]), nil
}

// OSADistance computes the Optimal String Alignment (restricted
// Damerau) distance between a and b. Each substring can be edited at
// most once — this is *not* a true metric, and it is not the same as
// the full unrestricted Damerau (see [StringCheese.DamerauDistance]).
func (s *StringCheese) OSADistance(a, b []byte) (uint32, error) {
	ctx := s.closeCtx
	aPtr, aLen, err := s.allocList(ctx, a)
	if err != nil {
		return 0, err
	}
	bPtr, bLen, err := s.allocList(ctx, b)
	if err != nil {
		return 0, err
	}
	res, err := s.fnOsa.Call(ctx,
		uint64(aPtr), uint64(aLen), uint64(bPtr), uint64(bLen),
	)
	if err != nil {
		return 0, fmt.Errorf("osa: %w", err)
	}
	return uint32(res[0]), nil
}

// LCSDistance computes |a| + |b| - 2*lcs(a, b) — a true metric derived
// from the longest common subsequence length.
func (s *StringCheese) LCSDistance(a, b []byte) (uint32, error) {
	ctx := s.closeCtx
	aPtr, aLen, err := s.allocList(ctx, a)
	if err != nil {
		return 0, err
	}
	bPtr, bLen, err := s.allocList(ctx, b)
	if err != nil {
		return 0, err
	}
	res, err := s.fnLcsDistance.Call(ctx,
		uint64(aPtr), uint64(aLen), uint64(bPtr), uint64(bLen),
	)
	if err != nil {
		return 0, fmt.Errorf("lcs-distance: %w", err)
	}
	return uint32(res[0]), nil
}

// HammingDistance computes the Hamming distance between two
// equal-length byte slices. Returns a *HammingLengthMismatch when the
// inputs differ in length (the WIT boundary returns
// `result<u32, string>` — the underlying Rust kernel's typed
// LengthMismatch error is flattened to a diagnostic string).
func (s *StringCheese) HammingDistance(a, b []byte) (uint32, error) {
	ctx := s.closeCtx
	aPtr, aLen, err := s.allocList(ctx, a)
	if err != nil {
		return 0, err
	}
	bPtr, bLen, err := s.allocList(ctx, b)
	if err != nil {
		return 0, err
	}
	res, err := s.fnHamming.Call(ctx,
		uint64(aPtr), uint64(aLen), uint64(bPtr), uint64(bLen),
	)
	if err != nil {
		return 0, fmt.Errorf("hamming: %w", err)
	}
	// hamming's `result<u32, string>` doesn't fit the flat-1 return
	// slot (it needs discriminant + payload1 + payload2), so the guest
	// returns a pointer into its own return-area. Layout at that ptr:
	//   +0 (u32): discriminant (0 = ok, 1 = err)
	//   +4 (u32): if ok — the distance; if err — the string pointer
	//   +8 (u32): if err — the string length (unused for ok)
	retPtr := uint32(res[0])
	tag, ok := s.memory.ReadUint32Le(retPtr)
	if !ok {
		return 0, fmt.Errorf("hamming: read discriminant at %#x oob", retPtr)
	}
	if tag == 0 {
		val, ok := s.memory.ReadUint32Le(retPtr + 4)
		if !ok {
			return 0, fmt.Errorf("hamming: read ok payload at %#x oob", retPtr+4)
		}
		// cabi_post is a must — even in the ok branch, the guest's
		// static return-area may still need bookkeeping (a strict
		// canonical-ABI-compliant guest may e.g. increment a
		// generation counter). Wit-bindgen 0.46's post_return for a
		// tagless ok is a no-op today, but calling it costs a single
		// wasm invocation and future-proofs against a guest that
		// starts depending on it.
		if _, perr := s.fnHammingPost.Call(ctx, uint64(retPtr)); perr != nil {
			return 0, fmt.Errorf("hamming post_return: %w", perr)
		}
		return val, nil
	}
	// err branch — read the string, hand it to cabi_post to free.
	sPtr, ok := s.memory.ReadUint32Le(retPtr + 4)
	if !ok {
		return 0, fmt.Errorf("hamming: read err ptr at %#x oob", retPtr+4)
	}
	sLen, ok := s.memory.ReadUint32Le(retPtr + 8)
	if !ok {
		return 0, fmt.Errorf("hamming: read err len at %#x oob", retPtr+8)
	}
	buf, ok := s.memory.Read(sPtr, sLen)
	if !ok {
		return 0, fmt.Errorf(
			"hamming: read err message at %#x len=%d oob", sPtr, sLen,
		)
	}
	msg := string(buf) // copy — cabi_post is about to free the source
	if _, perr := s.fnHammingPost.Call(ctx, uint64(retPtr)); perr != nil {
		return 0, fmt.Errorf("hamming post_return: %w", perr)
	}
	return 0, &HammingLengthMismatch{Msg: msg}
}

// LevenshteinWithin returns Levenshtein(a, b) if the true distance is
// at most cutoff, else signals `exceeded(cutoff)`. Under the hood this
// is Ukkonen's banded kernel.
func (s *StringCheese) LevenshteinWithin(a, b []byte, cutoff uint32) (BoundedDistance, error) {
	ctx := s.closeCtx
	aPtr, aLen, err := s.allocList(ctx, a)
	if err != nil {
		return BoundedDistance{}, err
	}
	bPtr, bLen, err := s.allocList(ctx, b)
	if err != nil {
		return BoundedDistance{}, err
	}
	res, err := s.fnLevenshteinWithin.Call(ctx,
		uint64(aPtr), uint64(aLen), uint64(bPtr), uint64(bLen), uint64(cutoff),
	)
	if err != nil {
		return BoundedDistance{}, fmt.Errorf("levenshtein-within: %w", err)
	}
	// bounded-distance is `variant { within(u32), exceeded(u32) }` —
	// flat form (discriminant, payload) doesn't fit the flat-1 return
	// slot, so the guest returns a pointer to its return-area. There
	// is no cabi_post for this function; no dynamic allocations to free.
	retPtr := uint32(res[0])
	tag, ok := s.memory.ReadUint32Le(retPtr)
	if !ok {
		return BoundedDistance{}, fmt.Errorf(
			"levenshtein-within: read tag at %#x oob", retPtr,
		)
	}
	val, ok := s.memory.ReadUint32Le(retPtr + 4)
	if !ok {
		return BoundedDistance{}, fmt.Errorf(
			"levenshtein-within: read payload at %#x oob", retPtr+4,
		)
	}
	kind := "within"
	if tag != 0 {
		kind = "exceeded"
	}
	return BoundedDistance{Kind: kind, Value: val}, nil
}

// DamerauDistance is not implemented — the StringCheese WIT world does
// not expose full unrestricted Damerau (see
// ../../component/README.md "Deliberately not exposed"). Returns
// [ErrDamerauNotExposed] unconditionally so bench files can catch it
// and mark the StringCheese cell as N/A.
func (s *StringCheese) DamerauDistance(a, b []byte) (uint32, error) {
	return 0, ErrDamerauNotExposed
}

// --------------------------------------------------------------------- //
// Similarity
// --------------------------------------------------------------------- //

// JaroSimilarity computes the Jaro similarity of a and b — 1.0 for
// identical inputs, 0.0 for inputs with no matching characters within
// the match window.
func (s *StringCheese) JaroSimilarity(a, b []byte) (float64, error) {
	return s.callSimF64(s.fnJaro, a, b, "jaro")
}

// JaroWinklerSimilarity computes the classic Jaro–Winkler similarity
// (prefix length 4, scaling 0.1, no boost threshold).
func (s *StringCheese) JaroWinklerSimilarity(a, b []byte) (float64, error) {
	return s.callSimF64(s.fnJaroWinkler, a, b, "jaro-winkler")
}

// DiceBigrams computes the Dice / Sørensen coefficient over character
// bigrams (n=2, no padding).
func (s *StringCheese) DiceBigrams(a, b []byte) (float64, error) {
	return s.callSimF64(s.fnDiceBigrams, a, b, "dice-bigrams")
}

// JaccardBigrams computes the Jaccard similarity over character
// bigrams (n=2, no padding).
func (s *StringCheese) JaccardBigrams(a, b []byte) (float64, error) {
	return s.callSimF64(s.fnJaccardBigrams, a, b, "jaccard-bigrams")
}

// callSimF64 is the shared plumbing for a WIT function of shape
// `(list<u8>, list<u8>) -> f64`: alloc-copy both inputs and unbox the
// f64 result. The similarity family all fits this signature.
func (s *StringCheese) callSimF64(fn api.Function, a, b []byte, label string) (float64, error) {
	ctx := s.closeCtx
	aPtr, aLen, err := s.allocList(ctx, a)
	if err != nil {
		return 0, err
	}
	bPtr, bLen, err := s.allocList(ctx, b)
	if err != nil {
		return 0, err
	}
	res, err := fn.Call(ctx,
		uint64(aPtr), uint64(aLen), uint64(bPtr), uint64(bLen),
	)
	if err != nil {
		return 0, fmt.Errorf("%s: %w", label, err)
	}
	return api.DecodeF64(res[0]), nil
}

// --------------------------------------------------------------------- //
// Core module discovery / extraction
// --------------------------------------------------------------------- //

// resolveCoreModule resolves an absolute path to the extracted core
// wasm module. If the cache file is missing and a component .wasm is
// available, it runs `wasm-tools component unbundle` to populate the
// cache; the unbundle output filename is deterministic
// (unbundled-module0.wasm) so subsequent calls hit the cache.
func resolveCoreModule(opts Options) (string, error) {
	// 1. Explicit path wins.
	if opts.WasmPath != "" {
		if _, err := os.Stat(opts.WasmPath); err != nil {
			return "", fmt.Errorf("WasmPath %q: %w", opts.WasmPath, err)
		}
		return opts.WasmPath, nil
	}
	if env := os.Getenv("STRINGCHEESE_CORE_WASM"); env != "" {
		if _, err := os.Stat(env); err != nil {
			return "", fmt.Errorf("STRINGCHEESE_CORE_WASM=%q: %w", env, err)
		}
		return env, nil
	}

	// 2. Default cache path — repo-relative from this source file's
	// on-disk location (bench-adapters/go/adapter/adapter.go).
	base, err := defaultRepoBase()
	if err != nil {
		return "", err
	}
	cachePath := filepath.Join(
		base,
		"component", "rust-host", "target", "wasm32-wasip1",
		"release", "unbundled", "unbundled-module0.wasm",
	)
	if _, err := os.Stat(cachePath); err == nil {
		return cachePath, nil
	}

	// 3. Cache miss — extract from the component.
	compPath := opts.ComponentPath
	if compPath == "" {
		if env := os.Getenv("STRINGCHEESE_WASM"); env != "" {
			compPath = env
		} else {
			compPath = filepath.Join(
				base,
				"component", "rust-host", "target", "wasm32-wasip1",
				"release", "stringcheese_component_host.wasm",
			)
		}
	}
	if _, err := os.Stat(compPath); err != nil {
		return "", fmt.Errorf(
			"component .wasm not found at %q — build it first with "+
				"`cd component/rust-host && cargo component build --release`: %w",
			compPath, err,
		)
	}

	if err := extractCoreModule(compPath, filepath.Dir(cachePath)); err != nil {
		return "", err
	}
	if _, err := os.Stat(cachePath); err != nil {
		return "", fmt.Errorf(
			"wasm-tools unbundle finished but expected %q is missing: %w",
			cachePath, err,
		)
	}
	return cachePath, nil
}

// defaultRepoBase locates the repository root by walking up from this
// source file's directory. This file lives at
// bench-adapters/go/adapter/adapter.go, so the repo base is three
// directories up.
func defaultRepoBase() (string, error) {
	_, thisFile, _, ok := runtime.Caller(0)
	if !ok {
		return "", errors.New("cannot resolve caller path for repo-base discovery")
	}
	// thisFile = <repo>/bench-adapters/go/adapter/adapter.go
	return filepath.Clean(filepath.Join(filepath.Dir(thisFile), "..", "..", "..")), nil
}
