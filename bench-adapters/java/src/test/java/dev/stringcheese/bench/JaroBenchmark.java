// Head-to-head: StringCheese Jaro / Jaro-Winkler vs. Java's libs.
//
// # Contestants
//
// Jaro:
//
//   - StringCheese — jaroSimilarity from the wasm component. Returns
//     double in [0.0, 1.0].
//   - info.debatty.java.stringsimilarity.Jaro — NOT PROVIDED as a
//     standalone metric; debatty ships JaroWinkler which contains a
//     Jaro implementation but does not expose it separately. So Jaro
//     is a StringCheese-only cell here — there is no ecosystem Java
//     library that ships standalone Jaro. This matches the Go story
//     with agnivade / go-edlib (only go-edlib ships Jaro).
//
// Jaro-Winkler:
//
//   - StringCheese — jaroWinklerSimilarity (classic: prefix 4,
//     scaling 0.1, no boost threshold). Returns double.
//   - org.apache.commons.text.similarity.JaroWinklerSimilarity —
//     classic Jaro-Winkler. Signature: apply(CharSequence, CharSequence)
//     -> Double.
//   - info.debatty.java.stringsimilarity.JaroWinkler — classic
//     Jaro-Winkler with a configurable threshold; defaulted to 0.7 in
//     that library's constructor. The threshold is a similarity floor
//     below which the Winkler boost is not applied; commons-text
//     applies the boost unconditionally. This is documented as a
//     variant difference — the two Jaro-Winkler cells here
//     deliberately compare against *different classic tunings*.
//
// # Variant identity
//
// The commons-text and StringCheese cells share tuning (prefix 4,
// scaling 0.1, no boost threshold). The debatty cell uses a 0.7 boost
// threshold and is documented as such — not the same algorithm on
// the same axis unless the caller is prepared to interpret the
// threshold delta.
package dev.stringcheese.bench;

import info.debatty.java.stringsimilarity.JaroWinkler;
import org.apache.commons.text.similarity.JaroWinklerSimilarity;
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
public class JaroBenchmark extends BenchState {

    // Jaro shares its salts with the Rust adapter (0xD1, 0xD2, 0xD3)
    // on purpose — Jaro's match-window behaviour is best observed
    // against a known-shared corpus.
    private static final long[] SALTS = {0xD1L, 0xD2L, 0xD3L};

    @Override
    protected long[] salts() {
        return SALTS;
    }

    private final JaroWinklerSimilarity commonsJW = new JaroWinklerSimilarity();
    private final JaroWinkler debattyJW = new JaroWinkler();

    // Jaro cell: StringCheese only. See file header for ecosystem
    // rationale.
    @Benchmark
    public void stringcheeseJaro(Blackhole bh) {
        bh.consume(sc.jaroSimilarity(aB, bB));
    }

    @Benchmark
    public void stringcheeseJaroWinkler(Blackhole bh) {
        bh.consume(sc.jaroWinklerSimilarity(aB, bB));
    }

    @Benchmark
    public void commonsTextJaroWinkler(Blackhole bh) {
        bh.consume(commonsJW.apply(aS, bS));
    }

    @Benchmark
    public void debattyJaroWinkler(Blackhole bh) {
        bh.consume(debattyJW.similarity(aS, bS));
    }
}
