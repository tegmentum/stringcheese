package dev.stringcheese.adapter;

/**
 * Thrown by {@link StringCheese#hammingDistance(byte[], byte[])} when
 * the two byte-arrays differ in length. The message field carries the
 * diagnostic string the underlying Rust kernel emits — the WIT boundary
 * flattens the kernel's typed {@code LengthMismatch} error into a
 * plain {@code result<u32, string>}, so the adapter has only the
 * string to work with.
 *
 * <p>Checked at compile-time (extends {@link Exception}, not
 * {@link RuntimeException}) so bench harnesses cannot silently ignore
 * a length mismatch when they think they're timing an equal-length
 * corpus; if a Hamming bench ever throws, the bench regime has a bug.
 */
public final class HammingLengthMismatchException extends Exception {

    private static final long serialVersionUID = 1L;

    public HammingLengthMismatchException(String message) {
        super(message);
    }
}
