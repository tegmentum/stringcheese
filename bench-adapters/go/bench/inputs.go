// Shared corpus-generation helpers for the Go bench adapter.
//
// The generator is a byte-for-byte port of
// bench-adapters/rust/src/lib.rs's SplitMix64 corpus builder — same
// PRNG, same seed derivation, same edit-injection recipe. Corpora
// produced from (length, salt) in this file match the Rust and Python
// adapters' corpora for the same (length, salt), so a StringCheese
// datapoint from any of the three harnesses lands on the same input
// family.
//
// Determinism is load-bearing: Go's testing.B reruns each benchmark
// with a range of b.N values and compares the resulting timings; if
// the input were re-randomised each round the noise would be corpus
// variance, not implementation variance.
//
// Nothing here is stateful — every function returns fresh byte slices.
// Callers cache the result in a package-level helper (see each
// *_test.go) so the corpus-generation cost is paid once per BenchmarkX
// invocation, outside the b.ResetTimer() boundary.
package bench

// LENGTHS is the canonical input-length sweep, matching
// stringcheese-bench and the Rust/Python/JS adapters so per-length
// datapoints line up on a chart across harnesses. Keep in sync when
// the Rust side changes.
var LENGTHS = []int{8, 32, 128, 512, 2048}

// REGIMES is the three similarity regimes, in the order the Rust
// harness emits them.
var REGIMES = []string{"random", "similar", "identical"}

// --------------------------------------------------------------------- //
// SplitMix64
// --------------------------------------------------------------------- //

// splitmix64GoldenGamma matches the constant in the Rust adapter's
// SplitMix64 implementation (Vigna's original golden gamma). Keep bit
// for bit — every corpus determinism guarantee depends on this.
const splitmix64GoldenGamma uint64 = 0x9E3779B97F4A7C15

type rng struct{ state uint64 }

func newRng(seed uint64) *rng { return &rng{state: seed + splitmix64GoldenGamma} }

func (r *rng) nextU64() uint64 {
	r.state += splitmix64GoldenGamma
	z := r.state
	z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9
	z = (z ^ (z >> 27)) * 0x94D049BB133111EB
	return z ^ (z >> 31)
}

func (r *rng) nextBounded(bound uint64) uint64 {
	if bound == 0 {
		panic("nextBounded needs a nonzero bound")
	}
	return r.nextU64() % bound
}

func (r *rng) nextAsciiLower() byte {
	return byte('a') + byte(r.nextBounded(26))
}

// seedFor derives the per-(length, salt) seed the Rust adapter uses.
func seedFor(length int, salt uint64) uint64 {
	return uint64(length)*splitmix64GoldenGamma ^ salt
}

// --------------------------------------------------------------------- //
// Corpus builders
// --------------------------------------------------------------------- //

func randomAscii(length int, seed uint64) []byte {
	r := newRng(seed)
	out := make([]byte, length)
	for i := range out {
		out[i] = r.nextAsciiLower()
	}
	return out
}

func identicalPair(length int, seed uint64) ([]byte, []byte) {
	s := randomAscii(length, seed)
	// Both sides intentionally alias the same underlying array — the
	// benched libraries all read but never write, and identical
	// slices are what the "identical" regime is meant to model.
	return s, s
}

// similarPair returns two slices differing by roughly editRate*length
// mixed edits (substitute / insert / delete). Length is only
// approximate — insertions and deletions cancel on average. Callers
// that need equal-length input (Hamming) should use
// similarPairEqualLen.
func similarPair(length int, editRate float64, seed uint64) ([]byte, []byte) {
	if editRate < 0 {
		panic("editRate must be non-negative")
	}
	left := randomAscii(length, seed)
	right := make([]byte, len(left))
	copy(right, left)
	nEdits := int(float64(length)*editRate + 0.5)
	if nEdits < 0 {
		nEdits = 0
	}
	r := newRng(seed ^ 0xA5A5A5A5A5A5A5A5)
	for i := 0; i < nEdits; i++ {
		if len(right) == 0 {
			right = append(right, r.nextAsciiLower())
			continue
		}
		op := r.nextBounded(3)
		pos := int(r.nextBounded(uint64(len(right))))
		switch op {
		case 0: // substitute
			right[pos] = r.nextAsciiLower()
		case 1: // insert
			right = append(right[:pos], append([]byte{r.nextAsciiLower()}, right[pos:]...)...)
		default: // delete
			right = append(right[:pos], right[pos+1:]...)
		}
	}
	return left, right
}

// similarPairEqualLen is the equal-length variant needed for Hamming:
// substitutions only, positions may collide so the true mismatch count
// can be slightly under the target.
func similarPairEqualLen(length int, editRate float64, seed uint64) ([]byte, []byte) {
	if editRate < 0 || editRate > 1.0 {
		panic("editRate must be in [0.0, 1.0]")
	}
	left := randomAscii(length, seed)
	right := make([]byte, len(left))
	copy(right, left)
	nEdits := int(float64(length)*editRate + 0.5)
	if nEdits > length {
		nEdits = length
	}
	if nEdits <= 0 || length == 0 {
		return left, right
	}
	r := newRng(seed ^ 0xC3C3C3C3C3C3C3C3)
	for i := 0; i < nEdits; i++ {
		pos := int(r.nextBounded(uint64(length)))
		bump := 1 + r.nextBounded(25)
		right[pos] = byte('a') + byte(((int(right[pos])-int('a'))+int(bump))%26)
	}
	return left, right
}

// buildPair dispatches on the regime name, matching the Rust adapter's
// build_pair contract. salts is (r_a, r_b, sim_or_ident) — the first
// two feed the random regime's two independent-seed generators; the
// third seeds the shared corpus for similar / identical.
func buildPair(length int, kind string, salts [3]uint64) ([]byte, []byte) {
	switch kind {
	case "random":
		return randomAscii(length, seedFor(length, salts[0])),
			randomAscii(length, seedFor(length, salts[1]))
	case "similar":
		return similarPair(length, 0.05, seedFor(length, salts[2]))
	case "identical":
		return identicalPair(length, seedFor(length, salts[2]))
	}
	panic("unknown similarity regime: " + kind)
}

// buildPairEqualLen is buildPair's equal-length sibling for Hamming.
func buildPairEqualLen(length int, kind string, salts [3]uint64) ([]byte, []byte) {
	switch kind {
	case "random":
		return randomAscii(length, seedFor(length, salts[0])),
			randomAscii(length, seedFor(length, salts[1]))
	case "similar":
		return similarPairEqualLen(length, 0.05, seedFor(length, salts[2]))
	case "identical":
		return identicalPair(length, seedFor(length, salts[2]))
	}
	panic("unknown similarity regime: " + kind)
}
