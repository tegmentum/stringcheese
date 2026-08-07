// StringCheese — Java-side face of the WebAssembly component.
//
// # Runtime choice
//
// This adapter uses Chicory (dylibso/chicory) — the pure-Java, no-JNI
// WebAssembly runtime. Chicory does not currently execute Component
// Model wasm binaries; it is a WASI-preview1 / core-wasm runtime.
// Rather than reach for a JNI-linked wasmtime binding, we mirror the
// Go adapter's approach: extract the inner core module out of the
// component with `wasm-tools component unbundle` and run that in
// Chicory.
//
// # Canonical ABI wiring
//
// The extracted core module still expects the Component Model's
// canonical ABI at every export:
//
//   - list<u8> inputs: caller allocates memory inside guest linear
//     memory via `cabi_realloc(0, 0, align, size)`, copies the payload,
//     and passes (ptr, len) as a pair of i32 params. Ownership
//     transfers to the guest, which frees on drop inside its own Rust
//     code.
//   - Scalar returns (u32, f64): come back as the function's ordinary
//     return value in `long[0]`.
//   - Compound returns that don't fit the flat-1 form
//     (`result<u32, string>`, `variant bounded-distance`): the guest
//     owns a static return-area buffer; the exported function returns
//     a pointer to it, and we read the fields out of linear memory at
//     that offset. When the return contains dynamic allocations (a
//     returned string), we must call the paired `cabi_post_<func>`
//     afterwards to let the guest release that memory.
//
// The layout of each return-area struct is fixed by the canonical
// ABI's field-order rules; see the WebAssembly component-model spec's
// canonical_abi.md for the source of truth.
//
// # Deliberately absent
//
// Full unrestricted Damerau is not exposed at the WIT boundary — see
// component/README.md "Deliberately not exposed" and the sibling
// {@link DamerauNotExposedException} class for the write-up.
package dev.stringcheese.adapter;

import com.dylibso.chicory.runtime.ExportFunction;
import com.dylibso.chicory.runtime.ImportValues;
import com.dylibso.chicory.runtime.Instance;
import com.dylibso.chicory.runtime.Memory;
import com.dylibso.chicory.wasi.WasiOptions;
import com.dylibso.chicory.wasi.WasiPreview1;
import com.dylibso.chicory.wasm.Parser;
import com.dylibso.chicory.wasm.WasmModule;
import dev.stringcheese.wasmtools.CoreModuleExtractor;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Objects;

/**
 * One loaded instance of the StringCheese component's inner core
 * module. Not safe for concurrent use across threads — wasm module
 * instances are single-threaded by design; construct one instance per
 * thread if you need parallel calls. For a JMH benchmark harness (a
 * single benchmark thread per fork by default) a single instance is
 * fine.
 *
 * <p>Every WIT-declared function the component exposes has a
 * corresponding public method on this class. Call {@link #close()}
 * when done to release Chicory runtime state — the class is
 * {@link AutoCloseable} for the standard try-with-resources idiom.
 */
public final class StringCheese implements AutoCloseable {

    // ---- Chicory instance handles -------------------------------------- //

    private final WasiPreview1 wasi;
    private final Instance instance;
    private final Memory memory;

    // ---- Cached export handles ----------------------------------------- //
    //
    // Every WIT function is resolved to an ExportFunction once at
    // construction; the per-call cost is exactly the guest's
    // canonical-ABI marshalling + kernel work, not a hash-map lookup
    // on top.
    private final ExportFunction realloc;
    private final ExportFunction fnLevenshtein;
    private final ExportFunction fnLevenshteinWithin;
    private final ExportFunction fnHamming;
    private final ExportFunction fnHammingPost;
    private final ExportFunction fnOsa;
    private final ExportFunction fnLcsDistance;
    private final ExportFunction fnJaro;
    private final ExportFunction fnJaroWinkler;
    private final ExportFunction fnDiceBigrams;
    private final ExportFunction fnJaccardBigrams;

    private StringCheese(WasiPreview1 wasi, Instance instance) {
        this.wasi = Objects.requireNonNull(wasi);
        this.instance = Objects.requireNonNull(instance);
        this.memory = Objects.requireNonNull(instance.memory(),
                "core module exports no memory");

        this.realloc = requireExport("cabi_realloc");
        this.fnLevenshtein =
                requireExport("stringcheese:core/distance@0.1.0#levenshtein");
        this.fnLevenshteinWithin = requireExport(
                "stringcheese:core/distance@0.1.0#levenshtein-within");
        this.fnHamming =
                requireExport("stringcheese:core/distance@0.1.0#hamming");
        this.fnHammingPost = requireExport(
                "cabi_post_stringcheese:core/distance@0.1.0#hamming");
        this.fnOsa =
                requireExport("stringcheese:core/distance@0.1.0#osa");
        this.fnLcsDistance =
                requireExport("stringcheese:core/distance@0.1.0#lcs-distance");
        this.fnJaro =
                requireExport("stringcheese:core/similarity@0.1.0#jaro");
        this.fnJaroWinkler = requireExport(
                "stringcheese:core/similarity@0.1.0#jaro-winkler");
        this.fnDiceBigrams = requireExport(
                "stringcheese:core/similarity@0.1.0#dice-bigrams");
        this.fnJaccardBigrams = requireExport(
                "stringcheese:core/similarity@0.1.0#jaccard-bigrams");
    }

    // ---- Construction -------------------------------------------------- //

    /**
     * Load the StringCheese component from the standard build-output
     * path relative to the repository root
     * ({@code component/rust-host/target/wasm32-wasip1/release/}). This
     * is the entry point a bench harness or smoke test that expects
     * the repo layout will use.
     *
     * @throws IOException          if the component/core module can't
     *                              be found or extracted
     * @throws InterruptedException if the extraction subprocess is
     *                              interrupted
     */
    public static StringCheese create()
            throws IOException, InterruptedException {
        return create(defaultOptions());
    }

    /**
     * Load the StringCheese component with caller-supplied paths /
     * fallbacks. See {@link Options} for the field semantics.
     */
    public static StringCheese create(Options opts)
            throws IOException, InterruptedException {
        Path corePath = resolveCoreModule(opts);
        byte[] bytes = Files.readAllBytes(corePath);
        WasmModule module = Parser.parse(bytes);

        // The core module imports wasi_snapshot_preview1's
        // environ_get / environ_sizes_get / fd_write / proc_exit — Rust
        // std pulls those in for panic-handling even though the
        // StringCheese algorithm code never calls them. Chicory's
        // built-in preview1 shim satisfies them; stdout/stderr are
        // routed to a discarding stream so a stray Rust panic can't
        // pollute JMH output.
        WasiOptions wasiOpts = WasiOptions.builder()
                .withStdout(new DiscardingOutputStream())
                .withStderr(new DiscardingOutputStream())
                .build();
        WasiPreview1 wasi = WasiPreview1.builder()
                .withOptions(wasiOpts)
                .build();

        ImportValues imports = ImportValues.builder()
                .addFunction(wasi.toHostFunctions())
                .build();

        Instance instance = Instance.builder(module)
                .withImportValues(imports)
                .build();

        return new StringCheese(wasi, instance);
    }

    @Override
    public void close() {
        try {
            wasi.close();
        } catch (RuntimeException ignore) {
            // Best-effort close; the JVM shutdown will finish
            // whatever the WASI shim's file-descriptor bookkeeping
            // needs.
        }
    }

    // ---- WIT exports: distance ---------------------------------------- //

    /**
     * Unit-cost byte-level Levenshtein edit distance via the wasm
     * kernel.
     */
    public int levenshteinDistance(byte[] a, byte[] b) {
        int[] ap = allocList(a);
        int[] bp = allocList(b);
        long[] r = fnLevenshtein.apply(
                (long) ap[0], (long) ap[1], (long) bp[0], (long) bp[1]);
        return (int) r[0];
    }

    /**
     * Optimal String Alignment ("restricted Damerau"). Each substring
     * may be edited at most once; not a true metric.
     */
    public int osaDistance(byte[] a, byte[] b) {
        int[] ap = allocList(a);
        int[] bp = allocList(b);
        long[] r = fnOsa.apply(
                (long) ap[0], (long) ap[1], (long) bp[0], (long) bp[1]);
        return (int) r[0];
    }

    /**
     * {@code |a| + |b| - 2 * lcs(a, b)} — the LCS-derived metric.
     */
    public int lcsDistance(byte[] a, byte[] b) {
        int[] ap = allocList(a);
        int[] bp = allocList(b);
        long[] r = fnLcsDistance.apply(
                (long) ap[0], (long) ap[1], (long) bp[0], (long) bp[1]);
        return (int) r[0];
    }

    /**
     * Hamming distance between two equal-length byte arrays.
     *
     * @throws HammingLengthMismatchException when the inputs differ in
     *     length — the WIT boundary's {@code result<u32, string>}
     *     surfaces as this typed exception.
     */
    public int hammingDistance(byte[] a, byte[] b)
            throws HammingLengthMismatchException {
        int[] ap = allocList(a);
        int[] bp = allocList(b);
        long[] r = fnHamming.apply(
                (long) ap[0], (long) ap[1], (long) bp[0], (long) bp[1]);
        int retPtr = (int) r[0];
        // hamming's `result<u32, string>` doesn't fit the flat-1 slot,
        // so the guest returns a pointer into its return area:
        //   +0 (u32): discriminant (0 = ok, 1 = err)
        //   +4 (u32): if ok — the distance; if err — string ptr
        //   +8 (u32): if err — string len (unused for ok)
        int tag = memory.readInt(retPtr);
        try {
            if (tag == 0) {
                return memory.readInt(retPtr + 4);
            }
            int sPtr = memory.readInt(retPtr + 4);
            int sLen = memory.readInt(retPtr + 8);
            byte[] msg = memory.readBytes(sPtr, sLen);
            // Copy the string out of guest memory *before* cabi_post
            // is allowed to free it.
            String message = new String(msg, StandardCharsets.UTF_8);
            throw new HammingLengthMismatchException(message);
        } finally {
            // cabi_post is a must — even in the ok branch a strict
            // canonical-ABI-compliant guest may need it. wit-bindgen's
            // ok-branch post_return is a no-op today, but calling it
            // costs a single wasm invocation and future-proofs against
            // a guest that starts depending on it.
            fnHammingPost.apply((long) retPtr);
        }
    }

    /**
     * Ukkonen's banded Levenshtein — returns {@code within(d)} if the
     * true distance {@code d <= cutoff}, else {@code exceeded(cutoff)}.
     */
    public BoundedDistance levenshteinWithin(byte[] a, byte[] b, int cutoff) {
        int[] ap = allocList(a);
        int[] bp = allocList(b);
        long[] r = fnLevenshteinWithin.apply(
                (long) ap[0], (long) ap[1],
                (long) bp[0], (long) bp[1],
                (long) cutoff);
        int retPtr = (int) r[0];
        // bounded-distance is `variant { within(u32), exceeded(u32) }`
        // — flat form doesn't fit the flat-1 slot, so the guest returns
        // a return-area pointer. No cabi_post for this function — no
        // dynamic allocations to free.
        int tag = memory.readInt(retPtr);
        int val = memory.readInt(retPtr + 4);
        String kind = (tag == 0) ? "within" : "exceeded";
        return new BoundedDistance(kind, val);
    }

    /**
     * Not exposed at the WIT boundary — see
     * {@link DamerauNotExposedException}. Always throws.
     */
    public int damerauDistance(byte[] a, byte[] b) {
        throw new DamerauNotExposedException();
    }

    // ---- WIT exports: similarity -------------------------------------- //

    /**
     * Jaro similarity in {@code [0.0, 1.0]}.
     */
    public double jaroSimilarity(byte[] a, byte[] b) {
        return callSimF64(fnJaro, a, b);
    }

    /**
     * Classic Jaro–Winkler (prefix 4, scaling 0.1, no boost
     * threshold) in {@code [0.0, 1.0]}.
     */
    public double jaroWinklerSimilarity(byte[] a, byte[] b) {
        return callSimF64(fnJaroWinkler, a, b);
    }

    /**
     * Dice / Sørensen coefficient over character bigrams
     * ({@code n = 2}, no padding).
     */
    public double diceBigrams(byte[] a, byte[] b) {
        return callSimF64(fnDiceBigrams, a, b);
    }

    /**
     * Jaccard similarity over character bigrams ({@code n = 2}, no
     * padding).
     */
    public double jaccardBigrams(byte[] a, byte[] b) {
        return callSimF64(fnJaccardBigrams, a, b);
    }

    // ---- Canonical ABI helpers --------------------------------------- //

    /**
     * Copy a byte-array into the guest's linear memory using
     * {@code cabi_realloc} and return a two-element {@code [ptr, len]}
     * pair. The canonical ABI transfers ownership of the buffer to
     * the guest, which frees it on drop inside the generated Rust
     * wrapper — callers do <b>not</b> free the returned pointer.
     *
     * <p>Alignment is 1 because the payload is a plain byte buffer;
     * wit-bindgen's cabi_realloc respects the requested alignment.
     */
    private int[] allocList(byte[] data) {
        if (data.length == 0) {
            // The canonical ABI permits a null pointer for a
            // zero-length list<u8>. Skip the realloc call so we don't
            // pay per-call overhead on empty-input corner cases.
            return new int[]{0, 0};
        }
        long[] r = realloc.apply(0L, 0L, 1L, (long) data.length);
        int ptr = (int) r[0];
        memory.write(ptr, data);
        return new int[]{ptr, data.length};
    }

    /**
     * Shared plumbing for a WIT function of shape
     * {@code (list<u8>, list<u8>) -> f64}. Every similarity export
     * fits this signature.
     */
    private double callSimF64(ExportFunction fn, byte[] a, byte[] b) {
        int[] ap = allocList(a);
        int[] bp = allocList(b);
        long[] r = fn.apply(
                (long) ap[0], (long) ap[1], (long) bp[0], (long) bp[1]);
        return Double.longBitsToDouble(r[0]);
    }

    private ExportFunction requireExport(String name) {
        ExportFunction f = instance.export(name);
        if (f == null) {
            throw new IllegalStateException(
                    "core module missing export " + name);
        }
        return f;
    }

    // ---- Options + discovery ----------------------------------------- //

    /**
     * Construction knobs. Callers can either supply explicit
     * absolute paths, or rely on defaults that walk up from this
     * source file's on-disk location.
     */
    public static final class Options {
        /**
         * Explicit override for the pre-extracted core module
         * (skips the wasm-tools call entirely). {@code null} means
         * "use STRINGCHEESE_CORE_WASM if set, else the cache path".
         */
        public Path coreWasmPath;

        /**
         * Explicit override for the source component the extractor
         * pulls the core module out of. {@code null} means "use
         * STRINGCHEESE_WASM if set, else the standard build-output
         * path".
         */
        public Path componentPath;
    }

    /**
     * Empty-default {@link Options}. Handy for
     * try-with-resources call sites.
     */
    public static Options defaultOptions() {
        return new Options();
    }

    private static Path resolveCoreModule(Options opts)
            throws IOException, InterruptedException {
        // 1. Explicit override wins.
        if (opts.coreWasmPath != null) {
            if (!Files.isRegularFile(opts.coreWasmPath)) {
                throw new IOException(
                        "coreWasmPath does not exist: " + opts.coreWasmPath);
            }
            return opts.coreWasmPath;
        }
        String envCore = System.getenv("STRINGCHEESE_CORE_WASM");
        if (envCore != null && !envCore.isEmpty()) {
            Path p = Path.of(envCore);
            if (!Files.isRegularFile(p)) {
                throw new IOException(
                        "STRINGCHEESE_CORE_WASM does not exist: " + p);
            }
            return p;
        }

        // 2. Repo-relative extract cache.
        Path base = repoBase();
        Path cachePath = base.resolve(Path.of(
                "component", "rust-host", "target", "wasm32-wasip1",
                "release", "unbundled", "unbundled-module0.wasm"));
        if (Files.isRegularFile(cachePath)) {
            return cachePath;
        }

        // 3. Cache miss — extract from the component.
        Path componentPath = opts.componentPath;
        if (componentPath == null) {
            String envWasm = System.getenv("STRINGCHEESE_WASM");
            if (envWasm != null && !envWasm.isEmpty()) {
                componentPath = Path.of(envWasm);
            } else {
                componentPath = base.resolve(Path.of(
                        "component", "rust-host", "target", "wasm32-wasip1",
                        "release", "stringcheese_component_host.wasm"));
            }
        }
        if (!Files.isRegularFile(componentPath)) {
            throw new IOException(
                    "component .wasm not found at " + componentPath
                            + " — build it first with `cd component/rust-host"
                            + " && cargo component build --release`");
        }
        CoreModuleExtractor.unbundle(componentPath, cachePath.getParent());
        if (!Files.isRegularFile(cachePath)) {
            throw new IOException(
                    "wasm-tools unbundle finished but expected "
                            + cachePath + " is missing");
        }
        return cachePath;
    }

    /**
     * Walk up from this class's compiled location to the repo root.
     * StringCheese.class lives inside a jar or a
     * {@code target/classes/dev/stringcheese/adapter/} tree; either
     * way we can find the repo root by looking for the marker
     * {@code component/wit/stringcheese.wit}.
     */
    private static Path repoBase() throws IOException {
        // Start point: cwd. In a `mvn test` run cwd is the module's
        // pom.xml directory (bench-adapters/java); in an ad-hoc
        // `java -cp ...` invocation cwd is whatever the caller
        // chose. Walk up until we spot the WIT file.
        Path cwd = Path.of("").toAbsolutePath();
        Path cur = cwd;
        for (int i = 0; i < 8; i++) {
            Path marker = cur.resolve(Path.of(
                    "component", "wit", "stringcheese.wit"));
            if (Files.isRegularFile(marker)) {
                return cur;
            }
            Path parent = cur.getParent();
            if (parent == null || parent.equals(cur)) {
                break;
            }
            cur = parent;
        }
        throw new IOException(
                "could not locate the StringCheese repo root by walking up "
                        + "from " + cwd + " — set STRINGCHEESE_WASM or "
                        + "STRINGCHEESE_CORE_WASM to point at the built "
                        + "component or extracted core module");
    }

    /**
     * OutputStream that drops every write — routed into WASI's
     * stdout/stderr so a stray Rust panic in the wasm guest cannot
     * pollute bench output.
     */
    private static final class DiscardingOutputStream
            extends java.io.OutputStream {
        @Override
        public void write(int b) {
            // no-op
        }

        @Override
        public void write(byte[] b, int off, int len) {
            // no-op
        }
    }

    /**
     * Convenience for callers that want to force the extractor to
     * run once up front (e.g. a JMH {@code @Setup(Level.Trial)}) so
     * per-benchmark timings do not include the wasm-tools subprocess
     * cost on their first invocation.
     */
    public static void primeCoreModuleCache()
            throws IOException, InterruptedException {
        Path corePath = resolveCoreModule(defaultOptions());
        // Touch it — reading a few bytes triggers any FS-level lazy
        // materialisation and confirms the file is really there.
        try (var s = Files.newInputStream(corePath)) {
            byte[] buf = new byte[8];
            // ignore return; the point is to force an IOException if
            // the extract is missing/unreadable.
            s.read(buf);
        }
    }
}
