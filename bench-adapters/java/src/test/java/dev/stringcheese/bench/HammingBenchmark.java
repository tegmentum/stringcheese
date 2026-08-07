// Head-to-head: StringCheese Hamming vs. Java's Hamming implementations.
//
// # Contestants
//
//   - StringCheese — Hamming from the wasm component. WIT boundary
//     surfaces the length-mismatch case as
//     dev.stringcheese.adapter.HammingLengthMismatchException; every
//     input in this file is equal-length by construction
//     (Corpus.buildPairEqualLen), so the error path is not hit
//     during timing.
//   - org.apache.commons.text.similarity.HammingDistance — the
//     commons-text implementation. Throws IllegalArgumentException
//     on unequal-length input; not exercised here.
//
// java-string-similarity does not expose Hamming — its
// interfaces are all normalised similarity variants, and Hamming
// doesn't fit that shape. That's the ecosystem story: pure-Java
// Hamming is dominated by commons-text.
package dev.stringcheese.bench;

import dev.stringcheese.adapter.HammingLengthMismatchException;
import org.apache.commons.text.similarity.HammingDistance;
import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.BenchmarkMode;
import org.openjdk.jmh.annotations.Fork;
import org.openjdk.jmh.annotations.Measurement;
import org.openjdk.jmh.annotations.Mode;
import org.openjdk.jmh.annotations.OutputTimeUnit;
import org.openjdk.jmh.annotations.Warmup;
import org.openjdk.jmh.infra.Blackhole;

import java.util.concurrent.TimeUnit;

@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.NANOSECONDS)
@Warmup(iterations = 3, time = 1)
@Measurement(iterations = 5, time = 1)
@Fork(value = 1)
public class HammingBenchmark extends BenchState {

    // Hamming shares its salts with the Rust adapter (0xC1, 0xC2,
    // 0xC3) on purpose — Hamming needs equal-length inputs, and
    // using the same mismatch positions across harnesses is the
    // whole point of the shared SplitMix64 seeding.
    private static final long[] SALTS = {0xC1L, 0xC2L, 0xC3L};

    @Override
    protected long[] salts() {
        return SALTS;
    }

    @Override
    protected boolean equalLength() {
        return true;
    }

    private final HammingDistance commonsText = new HammingDistance();

    @Benchmark
    public void stringcheese(Blackhole bh) throws HammingLengthMismatchException {
        bh.consume(sc.hammingDistance(aB, bB));
    }

    @Benchmark
    public void commonsText(Blackhole bh) {
        bh.consume(commonsText.apply(aS, bS));
    }
}
