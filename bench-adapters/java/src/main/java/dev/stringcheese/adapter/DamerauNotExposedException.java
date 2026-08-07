package dev.stringcheese.adapter;

/**
 * Thrown unconditionally by
 * {@link StringCheese#damerauDistance(byte[], byte[])}. Full
 * unrestricted Damerau is not exposed at the StringCheese WIT boundary —
 * the underlying Rust kernel needs a {@code HashMap}, which pulls in
 * {@code getrandom} on {@code wasm32-*}. See
 * {@code component/README.md} "Deliberately not exposed" for the full
 * write-up.
 *
 * <p>Bench code catches this and asserts on it (Damerau is intentionally
 * absent, not silently gone dead) before skipping the cell. The
 * pattern mirrors the Go adapter's {@code ErrDamerauNotExposed}
 * sentinel.
 */
public final class DamerauNotExposedException extends RuntimeException {

    private static final long serialVersionUID = 1L;

    public DamerauNotExposedException() {
        super("full Damerau is not exposed by the StringCheese WIT component; "
                + "use osaDistance (restricted Damerau) instead");
    }
}
