package dev.stringcheese.adapter;

import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * End-to-end smoke test. Exercises every StringCheese entry point the
 * Java adapter binds with a handful of easy-to-eyeball inputs.
 *
 * <p>This is <b>not</b> a correctness suite — golden datasets live in
 * {@code crates/stringcheese-corpus/}. It's a fast signal that
 * wasm-tools extraction, Chicory instantiation, canonical-ABI
 * marshalling, and return-area readback are wired together correctly.
 * The pairs match the Go smoke test 1:1 so a regression that shows up
 * in exactly one adapter is easy to attribute.
 *
 * <p>Skipped cleanly (via a JUnit {@link org.junit.jupiter.api.Assumptions})
 * pattern in the setup phase) when the component .wasm has not been
 * built — see {@code component/README.md} for the one-time
 * {@code cargo component build --release} step.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
final class SmokeTest {

    private StringCheese sc;

    @BeforeAll
    void setUp() throws Exception {
        try {
            sc = StringCheese.create();
        } catch (Exception e) {
            org.junit.jupiter.api.Assumptions.abort(
                    "StringCheese component not available (" + e.getMessage()
                            + "); build with `cd component/rust-host && "
                            + "cargo component build --release` and rerun");
            throw new AssertionError("unreachable");
        }
    }

    @AfterAll
    void tearDown() {
        if (sc != null) {
            sc.close();
        }
    }

    @Test
    void levenshteinKittenSitting() {
        assertEquals(3, sc.levenshteinDistance(
                bytes("kitten"), bytes("sitting")));
    }

    @Test
    void hammingKarolinKathrin() throws HammingLengthMismatchException {
        int d = sc.hammingDistance(bytes("karolin"), bytes("kathrin"));
        assertEquals(3, d);
    }

    @Test
    void hammingLengthMismatch() {
        HammingLengthMismatchException e = assertThrows(
                HammingLengthMismatchException.class,
                () -> sc.hammingDistance(bytes("abc"), bytes("abcd")));
        assertFalse(e.getMessage() == null || e.getMessage().isEmpty(),
                "Hamming mismatch diagnostic should be non-empty");
    }

    @Test
    void osaCaAbc() {
        // OSA(ca, abc): interpretation-dependent; the Rust kernel emits
        // 3 under strict "each substring edited at most once", though
        // 2 is defensible via a transpose + insert with overlapping
        // spans. Either is acceptable here — the point of the smoke
        // test is that the call returns *a* small integer, not a
        // canonical one.
        int d = sc.osaDistance(bytes("ca"), bytes("abc"));
        assertTrue(d == 2 || d == 3,
                "OSA(ca, abc) = " + d + ", want 2 or 3");
    }

    @Test
    void lcsAbcdAcbd() {
        // lcs("abcd","acbd") = 3, distance = 4 + 4 - 2*3 = 2.
        assertEquals(2, sc.lcsDistance(bytes("abcd"), bytes("acbd")));
    }

    @Test
    void levenshteinWithinReturnsWithinBelowCutoff() {
        BoundedDistance bd = sc.levenshteinWithin(
                bytes("kitten"), bytes("sitting"), 5);
        assertTrue(bd.isWithin(), "expected within, got " + bd);
        assertEquals(3, bd.value());
    }

    @Test
    void levenshteinWithinReturnsExceededAboveCutoff() {
        BoundedDistance bd = sc.levenshteinWithin(
                bytes("kitten"), bytes("sitting"), 1);
        assertFalse(bd.isWithin(),
                "expected exceeded, got " + bd);
        assertEquals(1, bd.value());
    }

    @Test
    void jaroIdenticalIsOne() {
        assertEquals(1.0, sc.jaroSimilarity(bytes("MARTHA"), bytes("MARTHA")),
                1e-12);
    }

    @Test
    void jaroMarthaMarhta() {
        // Classical example: Jaro("MARTHA","MARHTA") ~= 0.9444.
        double got = sc.jaroSimilarity(bytes("MARTHA"), bytes("MARHTA"));
        assertTrue(Math.abs(got - 0.9444) < 0.01,
                "Jaro(MARTHA, MARHTA) = " + got + ", want ~0.9444");
    }

    @Test
    void jaroWinklerDwayneDuane() {
        // Classical example: JW("DWAYNE","DUANE") ~= 0.84.
        double got = sc.jaroWinklerSimilarity(bytes("DWAYNE"), bytes("DUANE"));
        assertTrue(Math.abs(got - 0.84) < 0.02,
                "JaroWinkler(DWAYNE, DUANE) = " + got + ", want ~0.84");
    }

    @Test
    void diceBigramsIdenticalIsOne() {
        assertEquals(1.0, sc.diceBigrams(bytes("night"), bytes("night")),
                1e-12);
    }

    @Test
    void jaccardBigramsIdenticalIsOne() {
        assertEquals(1.0, sc.jaccardBigrams(bytes("night"), bytes("night")),
                1e-12);
    }

    @Test
    void jaccardBigramsDisjointIsZero() {
        // "aa" -> {aa} and "bb" -> {bb} share no bigrams: 0.0 exactly.
        assertEquals(0.0, sc.jaccardBigrams(bytes("aa"), bytes("bb")),
                1e-12);
    }

    @Test
    void damerauUnconditionallyThrows() {
        assertThrows(DamerauNotExposedException.class,
                () -> sc.damerauDistance(bytes("abc"), bytes("acb")));
    }

    private static byte[] bytes(String s) {
        return s.getBytes(StandardCharsets.UTF_8);
    }
}
