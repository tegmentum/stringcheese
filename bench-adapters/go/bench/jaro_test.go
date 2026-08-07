// Head-to-head: StringCheese (via wasm) vs. Go's Jaro / Jaro-Winkler libs.
//
// # Contestants
//
// Jaro:
//
//   - StringCheese — JaroSimilarity from the wasm component. Returns
//     float64 in [0.0, 1.0].
//   - github.com/hbollon/go-edlib — JaroSimilarity(str1, str2 string)
//     float32.
//
// Jaro-Winkler:
//
//   - StringCheese — JaroWinklerSimilarity (classic: prefix 4,
//     scaling 0.1, no boost threshold). Returns float64.
//   - github.com/hbollon/go-edlib — JaroWinklerSimilarity(str1, str2
//     string) float32. Same classic tuning as StringCheese.
//
// github.com/agnivade/levenshtein does not cover Jaro-family
// similarity — its package is edit-distance only. That's the
// ecosystem story: pure-Go Jaro / Jaro-Winkler is dominated by
// go-edlib, so this is a StringCheese-vs-go-edlib file.
//
// # Variant identity
//
// Both libraries here compute *classic* Jaro-Winkler with prefix 4
// and scaling 0.1. StringCheese's JaroWinkler::classic() matches those
// defaults; component/README.md is clear that only the classic
// variant is exposed at the WIT boundary. If a future non-classic
// tuning goes into the WIT interface, that pairing needs a separate
// bench file to keep the head-to-head axis clean.
//
// # Precision caveat
//
// StringCheese returns float64; go-edlib returns float32. The
// benchmark timings are unaffected; a downstream correctness check
// would need to tolerate the ~1e-7 rounding delta between the two.
package bench

import (
	"testing"

	edlib "github.com/hbollon/go-edlib"
)

var jaroSalts = [3]uint64{0xD1, 0xD2, 0xD3}

func jaroPairs() map[string]levPair {
	m := make(map[string]levPair, len(LENGTHS)*len(REGIMES))
	for _, n := range LENGTHS {
		for _, k := range REGIMES {
			a, b := buildPair(n, k, jaroSalts)
			m[k+"/"+itoa4(n)] = levPair{
				aB: a, bB: b,
				aS: string(a), bS: string(b),
			}
		}
	}
	return m
}

// --------------------------------------------------------------------- //
// Jaro                                                                  //
// --------------------------------------------------------------------- //

func BenchmarkStringCheeseJaro(b *testing.B) {
	sc, err := stringcheese()
	if err != nil {
		b.Skipf("StringCheese unavailable: %v", err)
	}
	pairs := jaroPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("stringcheese", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					if _, err := sc.JaroSimilarity(p.aB, p.bB); err != nil {
						b.Fatalf("JaroSimilarity: %v", err)
					}
				}
			})
		}
	}
}

func BenchmarkGoEdlibJaro(b *testing.B) {
	pairs := jaroPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("go-edlib", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					_ = edlib.JaroSimilarity(p.aS, p.bS)
				}
			})
		}
	}
}

// --------------------------------------------------------------------- //
// Jaro-Winkler                                                          //
// --------------------------------------------------------------------- //

func BenchmarkStringCheeseJaroWinkler(b *testing.B) {
	sc, err := stringcheese()
	if err != nil {
		b.Skipf("StringCheese unavailable: %v", err)
	}
	pairs := jaroPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("stringcheese", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					if _, err := sc.JaroWinklerSimilarity(p.aB, p.bB); err != nil {
						b.Fatalf("JaroWinklerSimilarity: %v", err)
					}
				}
			})
		}
	}
}

func BenchmarkGoEdlibJaroWinkler(b *testing.B) {
	pairs := jaroPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("go-edlib", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					_ = edlib.JaroWinklerSimilarity(p.aS, p.bS)
				}
			})
		}
	}
}
