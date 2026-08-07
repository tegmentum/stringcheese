package dev.stringcheese.bench;

import dev.stringcheese.adapter.StringCheese;
import org.openjdk.jmh.annotations.Level;
import org.openjdk.jmh.annotations.Param;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.Setup;
import org.openjdk.jmh.annotations.State;
import org.openjdk.jmh.annotations.TearDown;


/**
 * Shared JMH fixture — one {@link StringCheese} instance per JMH
 * trial. Wazero and Chicory alike take a few hundred milliseconds to
 * compile and instantiate a wasm module (WASI shim wiring,
 * canonical-ABI export resolution); paying that per-invocation would
 * dwarf every per-call timing this harness is meant to measure. JMH's
 * {@link Level#Trial Trial}-scope {@code @Setup} runs the constructor
 * exactly once per fork.
 *
 * <p>Subclasses parameterise on {@link #length} and {@link #regime}
 * and produce a fresh corpus once at {@link Level#Trial Trial} time.
 */
@State(Scope.Benchmark)
public abstract class BenchState {

    /**
     * Input length in bytes. Matches
     * {@link Corpus#LENGTHS} — kept in sync manually because JMH
     * requires string-valued {@code @Param}s.
     */
    @Param({"8", "32", "128", "512", "2048"})
    public int length;

    /**
     * Similarity regime. Matches {@link Corpus#REGIMES}.
     */
    @Param({"random", "similar", "identical"})
    public String regime;

    /**
     * StringCheese wasm adapter — {@code null} when the component
     * .wasm wasn't found (the benchmark then exits before touching
     * it, so JMH won't record time-honoured timings on a broken
     * setup). Subclasses that only care about the ecosystem
     * contestants can ignore the field entirely.
     */
    public StringCheese sc;

    /** Pre-materialised {@code (a, b)} bytes for the wasm path. */
    public byte[] aB;
    public byte[] bB;

    /** Pre-materialised {@code (a, b)} strings for the ecosystem path. */
    public String aS;
    public String bS;

    /** {@code true} when this benchmark family constructs the wasm adapter. */
    protected boolean wantsWasm() {
        return true;
    }

    /**
     * Salts for the family's SplitMix64 seed derivation.
     * Sub-classes override to match the Rust adapter's per-family
     * salts (e.g. {@code {0xC1, 0xC2, 0xC3}} for Hamming).
     */
    protected abstract long[] salts();

    /**
     * {@code true} when the family needs equal-length input
     * (Hamming). Levenshtein / OSA / Jaro / LCS families leave
     * this default {@code false}.
     */
    protected boolean equalLength() {
        return false;
    }

    /**
     * Overridable corpus builder that lets a subclass pin the
     * (length, regime, salts) triple. Default plumbs through to
     * {@link Corpus#buildPair} / {@link Corpus#buildPairEqualLen}.
     */
    protected byte[][] buildPair(int len, String kind, long[] s) {
        return equalLength()
                ? Corpus.buildPairEqualLen(len, kind, s)
                : Corpus.buildPair(len, kind, s);
    }

    @Setup(Level.Trial)
    public final void setUp() throws Exception {
        // Shared adapter — created lazily per subclass. Using a
        // static holder keeps one Chicory instance across every
        // benchmark class rather than one-per-class; the JMH
        // fork model means each fork already gets its own JVM,
        // and per-fork sharing is the maximum the wasm-instance
        // discipline (single-threaded) allows.
        if (wantsWasm()) {
            sc = SharedStringCheese.instance();
        }
        byte[][] p = buildPair(length, regime, salts());
        aB = p[0];
        bB = p[1];
        aS = Corpus.asString(aB);
        bS = Corpus.asString(bB);
    }

    @TearDown(Level.Trial)
    public final void tearDown() {
        // SharedStringCheese owns the lifecycle; leave it in place.
        aB = null;
        bB = null;
        aS = null;
        bS = null;
    }

    /**
     * Process-lifetime StringCheese singleton. Chicory instantiate
     * is expensive; JMH forks each benchmark into a fresh JVM, so
     * this is at worst once per fork. Failure to construct is
     * remembered so subsequent trials in the same JVM see the same
     * reason.
     */
    static final class SharedStringCheese {
        private static volatile StringCheese instance;
        private static volatile Exception constructionError;

        private SharedStringCheese() {
        }

        static synchronized StringCheese instance() throws Exception {
            if (instance != null) {
                return instance;
            }
            if (constructionError != null) {
                throw constructionError;
            }
            try {
                instance = StringCheese.create();
                // Close on JVM shutdown; JMH's own harness doesn't
                // shut down between iterations, so this is the
                // safe hook.
                Runtime.getRuntime().addShutdownHook(new Thread(() -> {
                    try {
                        if (instance != null) {
                            instance.close();
                        }
                    } catch (RuntimeException ignore) {
                    }
                }));
                return instance;
            } catch (Exception e) {
                constructionError = e;
                throw e;
            }
        }
    }

}
