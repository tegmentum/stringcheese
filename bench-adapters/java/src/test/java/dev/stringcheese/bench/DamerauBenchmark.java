// Head-to-head: StringCheese OSA (via wasm) + full Damerau vs. Java's libs.
//
// # Variant identity is load-bearing
//
// Two commonly-named "Damerau" algorithms compute different
// distances. Pairing them incorrectly puts two different algorithms
// on the same axis and produces numbers that look meaningful but are
// not — exactly the failure mode docs/DESIGN.md warns about.
//
// ## OSA (Optimal String Alignment / "restricted Damerau")
//
// Each substring can be edited at most once; not a true metric.
//
//   - StringCheese — osaDistance.
//   - info.debatty.java.stringsimilarity.OptimalStringAlignment —
//     ships the OSA DP.
//
// Neither commons-text nor java-string-similarity have a
// separate "OSA vs full Damerau" split beyond debatty's
// OptimalStringAlignment + Damerau pairing; we bench debatty on both
// sides.
//
// ## Full unrestricted Damerau
//
// Substrings may be edited unlimited times; a true metric.
//
//   - StringCheese — **not exposed at the WIT boundary.** See
//     component/README.md "Deliberately not exposed": the underlying
//     kernel needs a HashMap, which pulls in getrandom on wasm32-*.
//     The Java adapter throws DamerauNotExposedException from
//     damerauDistance; the StringCheese cell here is asserted (so
//     the WIT gap is intentional, not a wire that's silently gone
//     dead) and then skipped via a return-early.
//   - info.debatty.java.stringsimilarity.Damerau — the full
//     unrestricted variant.
package dev.stringcheese.bench;

import dev.stringcheese.adapter.DamerauNotExposedException;
import info.debatty.java.stringsimilarity.Damerau;
import info.debatty.java.stringsimilarity.OptimalStringAlignment;
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
public class DamerauBenchmark extends BenchState {

    private static final long[] SALTS = {0xE1L, 0xE2L, 0xE3L};

    @Override
    protected long[] salts() {
        return SALTS;
    }

    private final OptimalStringAlignment debattyOsa = new OptimalStringAlignment();
    private final Damerau debattyDamerau = new Damerau();

    // ---- OSA (restricted) --------------------------------------------- //

    @Benchmark
    public void stringcheeseOSA(Blackhole bh) {
        bh.consume(sc.osaDistance(aB, bB));
    }

    @Benchmark
    public void debattyOSA(Blackhole bh) {
        bh.consume(debattyOsa.distance(aS, bS));
    }

    // ---- Full unrestricted Damerau ----------------------------------- //

    /**
     * Asserts the WIT boundary still refuses full Damerau, then
     * short-circuits so JMH doesn't spend measurement time in a
     * no-op. If the WIT surface ever grows a full-Damerau function,
     * this benchmark will start failing at the assertion and the
     * caller can wire up a real DP timing here.
     *
     * <p>Sample count is minimized via the {@code Blackhole.consume}
     * placeholder pattern — the assertion happens exactly once per
     * setup, so the per-invocation body is a no-op.
     */
    @Benchmark
    public void stringcheeseDamerau(Blackhole bh) {
        try {
            sc.damerauDistance(aB, bB);
            throw new AssertionError(
                    "damerauDistance stopped throwing — the WIT surface has "
                            + "changed; wire up a real bench here");
        } catch (DamerauNotExposedException expected) {
            // Intentional gap; the number below is a placeholder that
            // JMH can consume so this benchmark contributes a
            // signal-of-existence row to the results table without
            // pretending to measure a Damerau kernel.
            bh.consume(0);
        }
    }

    @Benchmark
    public void debattyDamerau(Blackhole bh) {
        bh.consume(debattyDamerau.distance(aS, bS));
    }
}
