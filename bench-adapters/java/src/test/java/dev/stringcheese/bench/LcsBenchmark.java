// Head-to-head: StringCheese LCS distance vs. Java's LCS libs.
//
// # Variant identity
//
// StringCheese's `lcsDistance` returns `|a| + |b| - 2 * lcs(a, b)` —
// the LCS-derived metric. The ecosystem contestants ship different
// LCS shapes:
//
//   - org.apache.commons.text.similarity.LongestCommonSubsequenceDistance
//     — returns the same `|a| + |b| - 2 * lcs`. Same axis as
//     StringCheese; head-to-head-legal.
//   - info.debatty.java.stringsimilarity.LongestCommonSubsequence —
//     returns `|a| + |b| - 2 * lcs`. Also head-to-head-legal.
//
// Both contestants agree with StringCheese on the metric shape.
package dev.stringcheese.bench;

import info.debatty.java.stringsimilarity.LongestCommonSubsequence;
import org.apache.commons.text.similarity.LongestCommonSubsequenceDistance;
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
public class LcsBenchmark extends BenchState {

    private static final long[] SALTS = {0xE1L, 0xE2L, 0xE3L};

    @Override
    protected long[] salts() {
        return SALTS;
    }

    private final LongestCommonSubsequenceDistance commonsText =
            new LongestCommonSubsequenceDistance();
    private final LongestCommonSubsequence debatty =
            new LongestCommonSubsequence();

    @Benchmark
    public void stringcheese(Blackhole bh) {
        bh.consume(sc.lcsDistance(aB, bB));
    }

    @Benchmark
    public void commonsText(Blackhole bh) {
        bh.consume(commonsText.apply(aS, bS));
    }

    @Benchmark
    public void debatty(Blackhole bh) {
        bh.consume(debatty.distance(aS, bS));
    }
}
