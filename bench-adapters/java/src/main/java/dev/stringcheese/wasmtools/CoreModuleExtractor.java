package dev.stringcheese.wasmtools;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.TimeUnit;

/**
 * Wrapper around {@code wasm-tools component unbundle} — used by
 * {@link dev.stringcheese.adapter.StringCheese} to extract the inner
 * algorithm-kernel core module from a StringCheese component
 * {@code .wasm} on first construction.
 *
 * <p>Chicory (like wazero) is a core-wasm runtime; it does not yet
 * execute Component Model wasm directly. This class encapsulates the
 * one-time extraction step that bridges the gap. Once Chicory grows
 * component-model support the class becomes a no-op the constructor
 * can bypass — the adapter API stays the same.
 *
 * <p>{@code wasm-tools} is <b>not</b> a Maven dependency of this
 * adapter — it is a separate Rust tool the repo already requires for
 * the {@code cargo component build} step itself. If it is absent from
 * {@code PATH} at extraction time the caller sees the same
 * "install with {@code cargo install wasm-tools}" diagnostic the Go
 * adapter emits.
 */
public final class CoreModuleExtractor {

    private CoreModuleExtractor() {
        // Static utility.
    }

    /**
     * Run {@code wasm-tools component unbundle} on the component at
     * {@code componentPath}, writing every embedded core module into
     * {@code outDir}. The subcommand's own stdout — a stripped-down
     * outer component with the modules imported rather than embedded —
     * is written to a scratch file that is removed on success; we do
     * not need it for execution.
     *
     * <p>Blocks until the child process exits.
     *
     * @param componentPath path to the component-model wasm file
     * @param outDir        directory to place extracted core modules in;
     *                      created if missing
     * @throws IOException           if the child process failed, the
     *                               directory could not be created, or
     *                               {@code wasm-tools} was not on PATH
     * @throws InterruptedException  if the current thread was
     *                               interrupted while waiting for the
     *                               subprocess to exit
     */
    public static void unbundle(Path componentPath, Path outDir)
            throws IOException, InterruptedException {
        if (!isWasmToolsOnPath()) {
            throw new IOException(
                    "wasm-tools not found on PATH — install with "
                            + "`cargo install wasm-tools`, then rerun; needed "
                            + "once to extract the core wasm module from the "
                            + "built component");
        }
        Files.createDirectories(outDir);

        // `wasm-tools component unbundle` prints an outer component to
        // stdout / -o. We do not need that here — the algorithm-kernel
        // core module lives in outDir. Route the outer file to a
        // scratch temp and delete it after the process exits.
        Path scratch = Files.createTempFile("stringcheese-unbundle-", ".wasm");
        try {
            List<String> cmd = List.of(
                    "wasm-tools",
                    "component",
                    "unbundle",
                    "--module-dir",
                    outDir.toString(),
                    "--threshold",
                    "0",
                    "-o",
                    scratch.toString(),
                    componentPath.toString());
            ProcessBuilder pb = new ProcessBuilder(cmd);
            pb.redirectErrorStream(true);
            Process p = pb.start();

            // Drain stdout/stderr so a large error message can't
            // deadlock the child on a full pipe.
            byte[] out;
            try (var in = p.getInputStream()) {
                out = in.readAllBytes();
            }
            if (!p.waitFor(120, TimeUnit.SECONDS)) {
                p.destroyForcibly();
                throw new IOException(
                        "wasm-tools component unbundle timed out after 120s");
            }
            int code = p.exitValue();
            if (code != 0) {
                throw new IOException(
                        "wasm-tools component unbundle failed with exit "
                                + code + ": " + new String(out));
            }
        } finally {
            try {
                Files.deleteIfExists(scratch);
            } catch (IOException ignore) {
                // Best-effort scratch cleanup; not fatal.
            }
        }
    }

    /**
     * Cheap PATH probe. A single {@code which wasm-tools} would be
     * enough on POSIX, but this variant avoids assuming a shell — the
     * adapter runs on Windows JVMs too where the ProcessBuilder shell
     * story is different.
     */
    static boolean isWasmToolsOnPath() {
        String pathEnv = System.getenv("PATH");
        if (pathEnv == null || pathEnv.isEmpty()) {
            return false;
        }
        String sep = System.getProperty("path.separator", ":");
        boolean windows = System.getProperty("os.name", "")
                .toLowerCase().contains("win");
        String[] names = windows
                ? new String[]{"wasm-tools.exe", "wasm-tools.cmd", "wasm-tools"}
                : new String[]{"wasm-tools"};
        for (String dir : pathEnv.split(sep)) {
            if (dir.isEmpty()) {
                continue;
            }
            Path base;
            try {
                base = Path.of(dir);
            } catch (RuntimeException e) {
                // Malformed PATH entry — skip.
                continue;
            }
            for (String name : names) {
                Path candidate = base.resolve(name);
                try {
                    if (Files.isRegularFile(candidate)
                            && Files.isExecutable(candidate)) {
                        return true;
                    }
                } catch (RuntimeException ignore) {
                    // Filesystem quirk; keep scanning.
                }
            }
        }
        return false;
    }

    /**
     * Rethrows an IOException wrapped as unchecked. Convenience for
     * lambdas that cannot declare {@link IOException}.
     */
    public static UncheckedIOException wrap(IOException e) {
        return new UncheckedIOException(e);
    }
}
