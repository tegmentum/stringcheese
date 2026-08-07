// Head-to-head: StringCheese (via wasm) OSA + full Damerau vs. Go's libs.
//
// # Variant identity is load-bearing
//
// There are two commonly-named "Damerau" algorithms and they compute
// different distances. Pairing them incorrectly puts two different
// algorithms on the same axis and produces numbers that look
// meaningful but are not — exactly the failure mode docs/DESIGN.md
// warns about in "Comparative Library Benchmarking".
//
// ## OSA (Optimal String Alignment / "restricted Damerau")
//
// Each substring can be edited at most once; not a true metric (does
// not satisfy triangle inequality).
//
//   - StringCheese — OSADistance.
//   - github.com/hbollon/go-edlib — OSADamerauLevenshteinDistance.
//
// agnivade/levenshtein does not expose either Damerau variant, so the
// OSA group is StringCheese-vs-go-edlib only.
//
// ## Full unrestricted Damerau
//
// Substrings may be edited unlimited times; a true metric.
//
//   - StringCheese — **not exposed at the WIT boundary.** See
//     component/README.md "Deliberately not exposed": the underlying
//     kernel needs a HashMap, which pulls in getrandom on wasm32-*.
//     The adapter returns adapter.ErrDamerauNotExposed from
//     DamerauDistance; the StringCheese cell here is skipped with a
//     b.Skip so the run stays honest about what did and did not run.
//   - github.com/hbollon/go-edlib — DamerauLevenshteinDistance.
//
// The go-edlib full-Damerau cell is still benched even without a
// StringCheese counterpart so the ecosystem-baseline number is on the
// same axis as the OSA head-to-head. A StringCheese Damerau cell will
// appear once the underlying kernel gets a wasm-portable hash story.
//
// # Representation caveat
//
// StringCheese takes []byte; go-edlib takes string. For ASCII input
// the semantics agree and the FFI cost is folded in on purpose.
package bench

import (
	"errors"
	"testing"

	"github.com/tegmentum/stringcheese/bench-adapters/go/adapter"

	edlib "github.com/hbollon/go-edlib"
)

var damSalts = [3]uint64{0xE1, 0xE2, 0xE3}

func damPairs() map[string]levPair {
	m := make(map[string]levPair, len(LENGTHS)*len(REGIMES))
	for _, n := range LENGTHS {
		for _, k := range REGIMES {
			a, b := buildPair(n, k, damSalts)
			m[k+"/"+itoa4(n)] = levPair{
				aB: a, bB: b,
				aS: string(a), bS: string(b),
			}
		}
	}
	return m
}

// --------------------------------------------------------------------- //
// OSA (restricted Damerau)                                              //
// --------------------------------------------------------------------- //

func BenchmarkStringCheeseOSA(b *testing.B) {
	sc, err := stringcheese()
	if err != nil {
		b.Skipf("StringCheese unavailable: %v", err)
	}
	pairs := damPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("stringcheese", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					if _, err := sc.OSADistance(p.aB, p.bB); err != nil {
						b.Fatalf("OSADistance: %v", err)
					}
				}
			})
		}
	}
}

func BenchmarkGoEdlibOSA(b *testing.B) {
	pairs := damPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("go-edlib", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					_ = edlib.OSADamerauLevenshteinDistance(p.aS, p.bS)
				}
			})
		}
	}
}

// --------------------------------------------------------------------- //
// Full unrestricted Damerau                                             //
// --------------------------------------------------------------------- //

// BenchmarkStringCheeseDamerau documents the gap. StringCheese's
// DamerauDistance returns ErrDamerauNotExposed unconditionally
// (see component/README.md "Deliberately not exposed"); this
// benchmark calls it once so a fresh run explicitly records the
// current state of the WIT surface, then Skip()s so the bench harness
// doesn't spend time in a no-op.
func BenchmarkStringCheeseDamerau(b *testing.B) {
	sc, err := stringcheese()
	if err != nil {
		b.Skipf("StringCheese unavailable: %v", err)
	}
	if _, err := sc.DamerauDistance([]byte("abc"), []byte("acb")); !errors.Is(err, adapter.ErrDamerauNotExposed) {
		b.Fatalf(
			"expected ErrDamerauNotExposed from DamerauDistance, got %v — "+
				"the WIT surface has changed; wire up a real bench here",
			err,
		)
	}
	b.Skip("full Damerau is not exposed by the StringCheese WIT component " +
		"(see component/README.md 'Deliberately not exposed'). Remove this " +
		"skip once the kernel gets a wasm-portable hash story.")
}

func BenchmarkGoEdlibDamerau(b *testing.B) {
	pairs := damPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("go-edlib", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					_ = edlib.DamerauLevenshteinDistance(p.aS, p.bS)
				}
			})
		}
	}
}
