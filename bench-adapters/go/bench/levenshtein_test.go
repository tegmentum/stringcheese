// Head-to-head: StringCheese (via wasm) vs. Go's Levenshtein libraries.
//
// # Contestants
//
//   - StringCheese — loaded from the WebAssembly component built by
//     `cargo component build --release` in ../../component/rust-host/,
//     via wazero (pure Go, no CGO). Every call pays the wasm boundary
//     cost (parameter lowering via cabi_realloc + memcpy into linear
//     memory, guest execution, u32 return unbox) on top of the
//     underlying DP.
//   - github.com/agnivade/levenshtein — pure-Go Levenshtein. The
//     ecosystem's go-to for a single-file Levenshtein implementation.
//   - github.com/hbollon/go-edlib — pure-Go collection of string
//     metrics (Levenshtein, Damerau-Levenshtein, Hamming, Jaro,
//     Jaro-Winkler, more).
//
// # Representation caveat (READ THIS)
//
// StringCheese via wasm takes []byte; agnivade takes string;
// go-edlib takes string. For ASCII input the semantics are
// equivalent. Each library gets its natural input representation —
// []byte to StringCheese, string to the others. The FFI cost is
// therefore folded into the comparison **on purpose**: this is the
// "should I use StringCheese through wasm from Go instead of a
// pure-Go implementation" question, and the answer is a whole-stack
// answer, not a DP-kernel-only one.
//
// # Matrix
//
// (length ∈ {8, 32, 128, 512, 2048}) × (regime ∈ {random, similar,
// identical}) × (implementation ∈ {stringcheese, agnivade, go-edlib}).
// Same lengths and regimes as bench-adapters/rust/,
// bench-adapters/python/, and bench-adapters/js/.
package bench

import (
	"testing"

	agnivade "github.com/agnivade/levenshtein"
	edlib "github.com/hbollon/go-edlib"
)

// Per-length salts; distinct from the Rust adapter's Levenshtein
// salts (0xA1, 0xA2, 0xA3) so the two harnesses do not accidentally
// share an unlikely corner-case corpus that would confound
// cross-harness debugging.
var levSalts = [3]uint64{0xF1, 0xF2, 0xF3}

// levPair caches one (len, regime) corpus per benchmark run.
type levPair struct {
	aB, bB []byte
	aS, bS string
}

func levPairs() map[string]levPair {
	m := make(map[string]levPair, len(LENGTHS)*len(REGIMES))
	for _, n := range LENGTHS {
		for _, k := range REGIMES {
			a, b := buildPair(n, k, levSalts)
			m[k+"/"+itoa4(n)] = levPair{
				aB: a, bB: b,
				aS: string(a), bS: string(b),
			}
		}
	}
	return m
}

// itoa4 returns a zero-padded 4-digit decimal string. Used for
// benchmark IDs (len0008, len2048); keeps the output sorted.
func itoa4(n int) string {
	var buf [4]byte
	for i := 3; i >= 0; i-- {
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	return "len" + string(buf[:])
}

// BenchmarkStringCheeseLevenshtein runs StringCheese's Levenshtein
// across the full (length × regime) matrix. Instance construction is
// paid outside the b.N loop; the SplitMix64 corpus is materialised
// once per (length, regime) cell.
func BenchmarkStringCheeseLevenshtein(b *testing.B) {
	sc, err := stringcheese()
	if err != nil {
		b.Skipf("StringCheese unavailable: %v", err)
	}
	pairs := levPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("stringcheese", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					if _, err := sc.LevenshteinDistance(p.aB, p.bB); err != nil {
						b.Fatalf("LevenshteinDistance: %v", err)
					}
				}
			})
		}
	}
}

// BenchmarkAgnivadeLevenshtein — pure-Go, single-file Levenshtein.
// Package github.com/agnivade/levenshtein exposes a single top-level
// ComputeDistance(s1, s2 string) int function.
func BenchmarkAgnivadeLevenshtein(b *testing.B) {
	pairs := levPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("agnivade", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					_ = agnivade.ComputeDistance(p.aS, p.bS)
				}
			})
		}
	}
}

// BenchmarkGoEdlibLevenshtein — go-edlib's Levenshtein. The library
// exposes both a raw distance (LevenshteinDistance) and a normalised
// similarity (LevenshteinSimilarity); we benchmark the raw distance
// to match the other cells.
func BenchmarkGoEdlibLevenshtein(b *testing.B) {
	pairs := levPairs()
	for _, kind := range REGIMES {
		for _, n := range LENGTHS {
			p := pairs[kind+"/"+itoa4(n)]
			b.Run(benchID("go-edlib", kind, n), func(b *testing.B) {
				b.ReportAllocs()
				for i := 0; i < b.N; i++ {
					_ = edlib.LevenshteinDistance(p.aS, p.bS)
				}
			})
		}
	}
}
