// Head-to-head: StringCheese (via wasm) vs. Java's Levenshtein libs.
//
// # Contestants
//
//   - StringCheese — loaded from the WebAssembly component built by
//     `cargo component build --release` in ../../component/rust-host/,
//     via Chicory (pure Java, no JNI). Every call pays the wasm
//     boundary cost (parameter lowering via cabi_realloc + memcpy into
//     linear memory, guest execution, u32 return unbox) on top of the
//     underlying DP.
//   - org.apache.commons:commons-text — the ecosystem's go-to
//     Levenshtein implementation, ships as
//     org.apache.commons.text.similarity.LevenshteinDistance.
//   - info.debatty:java-string-similarity — a broader collection of
//     string metrics; its Levenshtein is a plain O(n·m) DP.
//
// # Representation caveat (READ THIS)
//
// StringCheese via wasm takes byte[]; commons-text takes
// CharSequence; java-string-similarity takes String. For ASCII input
// the semantics are equivalent. The FFI cost is folded into the
// comparison **on purpose**: this is the "should I use StringCheese
// through wasm from Java instead of a pure-Java implementation"
// question, and the answer is a whole-stack answer.
package dev.stringcheese.bench;

import info.debatty.java.stringsimilarity.Levenshtein;
import org.apache.commons.text.similarity.LevenshteinDistance;
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
public class LevenshteinBenchmark extends BenchState {

    // Per-family salts; distinct from the Rust adapter's Levenshtein
    // salts (0xA1, 0xA2, 0xA3) so the two harnesses do not
    // accidentally share an unlikely corner-case corpus that would
    // confound cross-harness debugging.
    private static final long[] SALTS = {0xF4L, 0xF5L, 0xF6L};

    @Override
    protected long[] salts() {
        return SALTS;
    }

    // Ecosystem contestants — allocate once per trial, thread-safe
    // and stateless per their JavaDoc so a single instance can
    // service every measurement iteration.
    private final LevenshteinDistance commonsText =
            LevenshteinDistance.getDefaultInstance();
    private final Levenshtein debatty = new Levenshtein();

    @Benchmark
    public void stringcheese(Blackhole bh) {
        bh.consume(sc.levenshteinDistance(aB, bB));
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
