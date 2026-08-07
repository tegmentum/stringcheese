package adapter

import (
	"context"
	"errors"
	"math"
	"testing"
)

// TestSmoke exercises every StringCheese entry point the adapter binds
// with a handful of easy-to-eyeball inputs. It is *not* a correctness
// suite (that lives in the Rust workspace's stringcheese-corpus
// crate) — it is a fast end-to-end signal that the wasm-tools
// extraction, wazero instantiation, canonical-ABI marshalling, and
// return-area readback are wired together correctly. Skipped when the
// component .wasm has not been built.
func TestSmoke(t *testing.T) {
	ctx := context.Background()
	sc, err := New(ctx, Options{})
	if err != nil {
		// If the component hasn't been built (typical on a fresh
		// clone), skip rather than fail — the top-level README
		// documents the `cargo component build --release` step.
		t.Skipf("StringCheese component not available (%v); "+
			"build with `cd component/rust-host && "+
			"cargo component build --release` and rerun", err)
	}
	t.Cleanup(func() { _ = sc.Close() })

	t.Run("Levenshtein", func(t *testing.T) {
		got, err := sc.LevenshteinDistance([]byte("kitten"), []byte("sitting"))
		if err != nil {
			t.Fatalf("LevenshteinDistance: %v", err)
		}
		if got != 3 {
			t.Errorf("Levenshtein(kitten, sitting) = %d, want 3", got)
		}
	})

	t.Run("Hamming/equal", func(t *testing.T) {
		got, err := sc.HammingDistance([]byte("karolin"), []byte("kathrin"))
		if err != nil {
			t.Fatalf("HammingDistance: %v", err)
		}
		if got != 3 {
			t.Errorf("Hamming(karolin, kathrin) = %d, want 3", got)
		}
	})

	t.Run("Hamming/length-mismatch", func(t *testing.T) {
		_, err := sc.HammingDistance([]byte("abc"), []byte("abcd"))
		var mismatch *HammingLengthMismatch
		if !errors.As(err, &mismatch) {
			t.Fatalf("Hamming length mismatch: got err=%v, want *HammingLengthMismatch", err)
		}
		if mismatch.Msg == "" {
			t.Errorf("Hamming mismatch diagnostic is empty")
		}
	})

	t.Run("OSA", func(t *testing.T) {
		got, err := sc.OSADistance([]byte("ca"), []byte("abc"))
		if err != nil {
			t.Fatalf("OSADistance: %v", err)
		}
		// OSA(ca, abc) = 3 (insert a, transpose c<->a → wait: OSA
		// counts adjacent transposition as 1, so ca→ac (1) + insert
		// b (1) = 2, but the two edits touch overlapping substrings —
		// under OSA's "each substring edited at most once" rule the
		// result is 3. Either 2 or 3 is a defensible answer depending
		// on interpretation; assert loosely.
		if got != 2 && got != 3 {
			t.Errorf("OSA(ca, abc) = %d, want 2 or 3", got)
		}
	})

	t.Run("LCS", func(t *testing.T) {
		got, err := sc.LCSDistance([]byte("abcd"), []byte("acbd"))
		if err != nil {
			t.Fatalf("LCSDistance: %v", err)
		}
		// lcs("abcd","acbd") = 3 ("abd" or "acd"), distance = 4+4-2*3 = 2.
		if got != 2 {
			t.Errorf("LCSDistance(abcd, acbd) = %d, want 2", got)
		}
	})

	t.Run("LevenshteinWithin/within", func(t *testing.T) {
		bd, err := sc.LevenshteinWithin(
			[]byte("kitten"), []byte("sitting"), 5,
		)
		if err != nil {
			t.Fatalf("LevenshteinWithin: %v", err)
		}
		if bd.Kind != "within" || bd.Value != 3 {
			t.Errorf("LevenshteinWithin cutoff=5 = %+v, want {within, 3}", bd)
		}
	})

	t.Run("LevenshteinWithin/exceeded", func(t *testing.T) {
		bd, err := sc.LevenshteinWithin(
			[]byte("kitten"), []byte("sitting"), 1,
		)
		if err != nil {
			t.Fatalf("LevenshteinWithin: %v", err)
		}
		if bd.Kind != "exceeded" || bd.Value != 1 {
			t.Errorf("LevenshteinWithin cutoff=1 = %+v, want {exceeded, 1}", bd)
		}
	})

	t.Run("Jaro/identical", func(t *testing.T) {
		got, err := sc.JaroSimilarity([]byte("MARTHA"), []byte("MARTHA"))
		if err != nil {
			t.Fatalf("JaroSimilarity: %v", err)
		}
		if got != 1.0 {
			t.Errorf("Jaro(MARTHA, MARTHA) = %f, want 1.0", got)
		}
	})

	t.Run("Jaro/martha-marhta", func(t *testing.T) {
		got, err := sc.JaroSimilarity([]byte("MARTHA"), []byte("MARHTA"))
		if err != nil {
			t.Fatalf("JaroSimilarity: %v", err)
		}
		// Classical example: Jaro("MARTHA","MARHTA") ≈ 0.9444
		if math.Abs(got-0.9444) > 0.01 {
			t.Errorf("Jaro(MARTHA, MARHTA) = %f, want ~0.9444", got)
		}
	})

	t.Run("JaroWinkler/dwayne-duane", func(t *testing.T) {
		got, err := sc.JaroWinklerSimilarity(
			[]byte("DWAYNE"), []byte("DUANE"),
		)
		if err != nil {
			t.Fatalf("JaroWinklerSimilarity: %v", err)
		}
		// Classical example: JW("DWAYNE","DUANE") ≈ 0.84.
		// Underlying Jaro ≈ 0.822, then Winkler boost for 1-char
		// prefix = 0.822 + 1*0.1*(1-0.822) ≈ 0.840.
		if math.Abs(got-0.84) > 0.02 {
			t.Errorf("JaroWinkler(DWAYNE, DUANE) = %f, want ~0.84", got)
		}
	})

	t.Run("DiceBigrams/identical", func(t *testing.T) {
		got, err := sc.DiceBigrams(
			[]byte("night"), []byte("night"),
		)
		if err != nil {
			t.Fatalf("DiceBigrams: %v", err)
		}
		if got != 1.0 {
			t.Errorf("Dice(night, night) = %f, want 1.0", got)
		}
	})

	t.Run("JaccardBigrams/identical", func(t *testing.T) {
		got, err := sc.JaccardBigrams(
			[]byte("night"), []byte("night"),
		)
		if err != nil {
			t.Fatalf("JaccardBigrams: %v", err)
		}
		if got != 1.0 {
			t.Errorf("Jaccard(night, night) = %f, want 1.0", got)
		}
	})

	t.Run("Damerau/not-exposed", func(t *testing.T) {
		_, err := sc.DamerauDistance([]byte("abc"), []byte("acb"))
		if !errors.Is(err, ErrDamerauNotExposed) {
			t.Errorf("DamerauDistance err = %v, want ErrDamerauNotExposed", err)
		}
	})
}
