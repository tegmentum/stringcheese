// Shared corpus-generation helpers for the Java bench adapter.
//
// The generator is a byte-for-byte port of the Rust adapter's
// SplitMix64 corpus builder (same PRNG, same seed derivation, same
// edit-injection recipe) with identical semantics to the Python, JS,
// and Go adapter ports. Corpora produced from (length, salt) here
// match the Rust/Python/JS/Go corpora for the same (length, salt), so
// a StringCheese datapoint from any of the five harnesses lands on
// the same input family.
//
// Nothing here is stateful; every method returns fresh byte arrays.
// JMH benchmark classes cache the result at @Setup(Level.Trial) so
// per-invocation timings never include corpus-generation cost.
package dev.stringcheese.bench;

import java.nio.charset.StandardCharsets;

public final class Corpus {

    /**
     * Input-length sweep, matching stringcheese-bench and the
     * Rust / Python / JS / Go adapters so per-length datapoints line
     * up on a chart across harnesses.
     */
    public static final int[] LENGTHS = {8, 32, 128, 512, 2048};

    /**
     * Similarity regimes in the order the Rust harness emits them.
     */
    public static final String[] REGIMES = {"random", "similar", "identical"};

    // ---- SplitMix64 --------------------------------------------------- //

    /**
     * Vigna's golden gamma. Byte-for-byte match with the Rust
     * adapter's constant; the corpus-determinism guarantee across
     * harnesses depends on this bit-for-bit.
     */
    private static final long GOLDEN_GAMMA = 0x9E3779B97F4A7C15L;

    private long state;

    private Corpus(long seed) {
        // Match the Rust `Rng::new` constructor.
        this.state = seed + GOLDEN_GAMMA;
    }

    private long nextU64() {
        state += GOLDEN_GAMMA;
        long z = state;
        z = (z ^ (z >>> 30)) * 0xBF58476D1CE4E5B9L;
        z = (z ^ (z >>> 27)) * 0x94D049BB133111EBL;
        return z ^ (z >>> 31);
    }

    /**
     * Uniform in {@code [0, bound)}. Uses unsigned modulo semantics
     * (Java's {@code Long.remainderUnsigned}) to match the Rust port.
     */
    private long nextBounded(long bound) {
        if (bound <= 0) {
            throw new IllegalArgumentException("bound must be positive");
        }
        return Long.remainderUnsigned(nextU64(), bound);
    }

    private byte nextAsciiLower() {
        return (byte) ('a' + (int) nextBounded(26));
    }

    private static long seedFor(int length, long salt) {
        return ((long) length) * GOLDEN_GAMMA ^ salt;
    }

    // ---- Corpus builders --------------------------------------------- //

    public static byte[] randomAscii(int length, long seed) {
        Corpus r = new Corpus(seed);
        byte[] out = new byte[length];
        for (int i = 0; i < length; i++) {
            out[i] = r.nextAsciiLower();
        }
        return out;
    }

    /**
     * Two byte-equal copies of the same random string. The bench
     * function reads but never writes both sides, so aliasing is
     * safe and models the "input is byte-identical" corner case.
     */
    public static byte[][] identicalPair(int length, long seed) {
        byte[] s = randomAscii(length, seed);
        return new byte[][]{s, s};
    }

    /**
     * ~{@code editRate * length} mixed edits (substitute / insert /
     * delete) applied to a random source. Length differs from the
     * source by up to that many bytes; callers that need equal-length
     * input (Hamming) must use {@link #similarPairEqualLen}.
     */
    public static byte[][] similarPair(int length, double editRate, long seed) {
        if (editRate < 0) {
            throw new IllegalArgumentException("editRate must be non-negative");
        }
        byte[] left = randomAscii(length, seed);
        // Grow-on-demand mirror of the Go/Rust adapter's Vec<u8>-style
        // right side. Start same-size, then apply edits in place.
        java.util.ArrayList<Byte> right = new java.util.ArrayList<>(length);
        for (byte value : left) {
            right.add(value);
        }
        int nEdits = (int) Math.round(length * editRate);
        if (nEdits < 0) {
            nEdits = 0;
        }
        Corpus r = new Corpus(seed ^ 0xA5A5A5A5A5A5A5A5L);
        for (int i = 0; i < nEdits; i++) {
            if (right.isEmpty()) {
                right.add(r.nextAsciiLower());
                continue;
            }
            long op = r.nextBounded(3);
            int pos = (int) r.nextBounded(right.size());
            if (op == 0) {
                right.set(pos, r.nextAsciiLower());
            } else if (op == 1) {
                right.add(pos, r.nextAsciiLower());
            } else {
                right.remove(pos);
            }
        }
        byte[] out = new byte[right.size()];
        for (int i = 0; i < out.length; i++) {
            out[i] = right.get(i);
        }
        return new byte[][]{left, out};
    }

    /**
     * Equal-length variant for Hamming: substitutions only. Positions
     * may collide so the true mismatch count can be slightly under
     * the target — matches the Go adapter's variant.
     */
    public static byte[][] similarPairEqualLen(int length, double editRate, long seed) {
        if (editRate < 0 || editRate > 1.0) {
            throw new IllegalArgumentException("editRate must be in [0.0, 1.0]");
        }
        byte[] left = randomAscii(length, seed);
        byte[] right = left.clone();
        int nEdits = (int) Math.round(length * editRate);
        if (nEdits > length) {
            nEdits = length;
        }
        if (nEdits <= 0 || length == 0) {
            return new byte[][]{left, right};
        }
        Corpus r = new Corpus(seed ^ 0xC3C3C3C3C3C3C3C3L);
        for (int i = 0; i < nEdits; i++) {
            int pos = (int) r.nextBounded(length);
            long bump = 1 + r.nextBounded(25);
            int at = (right[pos] & 0xFF) - 'a';
            right[pos] = (byte) ('a' + ((at + (int) bump) % 26));
        }
        return new byte[][]{left, right};
    }

    // ---- Dispatchers ------------------------------------------------- //

    /**
     * Kind → pair. {@code salts[0]}, {@code salts[1]} feed the
     * random regime's two independent generators; {@code salts[2]}
     * seeds the shared corpus for similar / identical. Matches the
     * Go adapter's {@code buildPair} exactly.
     */
    public static byte[][] buildPair(int length, String kind, long[] salts) {
        switch (kind) {
            case "random":
                return new byte[][]{
                        randomAscii(length, seedFor(length, salts[0])),
                        randomAscii(length, seedFor(length, salts[1])),
                };
            case "similar":
                return similarPair(length, 0.05, seedFor(length, salts[2]));
            case "identical":
                return identicalPair(length, seedFor(length, salts[2]));
            default:
                throw new IllegalArgumentException("unknown regime: " + kind);
        }
    }

    /**
     * Equal-length {@link #buildPair} for Hamming.
     */
    public static byte[][] buildPairEqualLen(int length, String kind, long[] salts) {
        switch (kind) {
            case "random":
                return new byte[][]{
                        randomAscii(length, seedFor(length, salts[0])),
                        randomAscii(length, seedFor(length, salts[1])),
                };
            case "similar":
                return similarPairEqualLen(length, 0.05, seedFor(length, salts[2]));
            case "identical":
                return identicalPair(length, seedFor(length, salts[2]));
            default:
                throw new IllegalArgumentException("unknown regime: " + kind);
        }
    }

    // ---- Convenience: byte[] <-> String ----------------------------- //

    /**
     * StringCheese's WIT surface takes {@code list<u8>}; the
     * ecosystem contestants (commons-text, java-string-similarity)
     * take {@code CharSequence}. For ASCII input the semantics agree.
     * Bench code pre-computes both once per corpus cell.
     */
    public static String asString(byte[] b) {
        return new String(b, StandardCharsets.US_ASCII);
    }
}
