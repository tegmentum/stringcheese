package bench

import (
	"context"
	"fmt"
	"sync"

	"github.com/tegmentum/stringcheese/bench-adapters/go/adapter"
)

// A single StringCheese adapter is shared across every benchmark in
// the package. Instantiating the wazero runtime + extracted core
// module takes a few hundred milliseconds (compile, link, WASI shim
// wiring, canonical-ABI export resolution); paying that per-benchmark
// would dwarf every per-call timing we're actually trying to measure.
//
// The instance is *not* goroutine-safe, but Go's testing.B runs the
// b.N iteration loop on a single goroutine per (sub)test, so sharing
// is fine here.
var (
	stringcheeseOnce sync.Once
	stringcheeseInst *adapter.StringCheese
	stringcheeseErr  error
)

// stringcheese returns the process-wide StringCheese instance,
// constructing it on first call. On construction failure (e.g. the
// component .wasm has not been built) the returned error is remembered
// and returned on every subsequent call — subsequent B.Fatal calls
// will report the same reason.
func stringcheese() (*adapter.StringCheese, error) {
	stringcheeseOnce.Do(func() {
		stringcheeseInst, stringcheeseErr = adapter.New(
			context.Background(), adapter.Options{},
		)
	})
	return stringcheeseInst, stringcheeseErr
}

// benchID is the uniform (impl, kind, len) label prefix — sortable and
// greppable, matching the Python adapter's benchmark.group scheme.
func benchID(impl, kind string, n int) string {
	return fmt.Sprintf("%s/%s/len%04d", impl, kind, n)
}
