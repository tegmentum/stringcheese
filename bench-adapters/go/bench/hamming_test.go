// Head-to-head: StringCheese (via wasm) vs. Go's Hamming implementations.
//
// # Contestants
//
//   - StringCheese — Hamming from the wasm component. The WIT boundary
//     returns `result<u32, string>`; the adapter surfaces
//     *HammingLengthMismatch on unequal-length inputs. Every input in
//     this file is equal-length by construction (buildPairEqualLen),
//     so the error path is never hit during timing.
//   - github.com/hbollon/go-edlib — HammingDistance(str1, str2 string)
//     (int, error). The error is length mismatch, unused here.
//
// github.com/agnivade/levenshtein does not expose Hamming — its
// package is Levenshtein-only. That's the ecosystem story: pure-Go
// Hamming implementations are rare enough that go-edlib is essentially
// the only game in town, so this is a StringCheese-vs-go-edlib file.
//
// # Representation caveat
//
// Same as the Levenshtein bench: StringCheese consumes []byte,
// go-edlib consumes string. For ASCII input the semantics agree and
// the FFI cost is folded into the comparison on purpose.
package bench

import (
	"testing"

	edlib "github.com/hbollon/go-edlib"
)

// Hamming shares its salts with the Rust adapter (0xC1, 0xC2, 0xC3)
// on purpose — Hamming needs equal-length inputs, and using the same
// mismatch positions across harnesses is the whole point of the shared
// SplitMix64 seeding.
var hamSalts = [3]uint64{0xC1, 0xC2, 0xC3}

func hamPairs() map[string]levPair {
	m := make(map[string]levPair, len(LENGTHS)*len(REGIMES))
	for _, n := range LENGTHS {
		for _, k := range REGIMES {
			a, b := buildPairEqualLen(n, k, hamSalts)
			m[k+"/"+itoa4(n)] = levPair{
				aB: a, bB: b,
				aS: string(a), bS: string(b),
			}
		}
	}
	return m
}

func BenchmarkStringCheeseHamming(b *testing.B) {
	sc, err := stringcheese()
	if err != nil {
		b.Skipf("StringCheese unavailable: %v", err)
	}
	pairs := hamPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("stringcheese", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					if _, err := sc.HammingDistance(p.aB, p.bB); err != nil {
						b.Fatalf("HammingDistance: %v", err)
					}
				}
			})
		}
	}
}

func BenchmarkGoEdlibHamming(b *testing.B) {
	pairs := hamPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("go-edlib", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					if _, err := edlib.HammingDistance(p.aS, p.bS); err != nil {
						b.Fatalf("HammingDistance: %v", err)
					}
				}
			})
		}
	}
}
