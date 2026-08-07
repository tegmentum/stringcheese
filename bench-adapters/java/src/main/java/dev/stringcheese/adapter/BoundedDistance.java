package dev.stringcheese.adapter;

/**
 * Java mirror of the WIT {@code variant bounded-distance
 * &#123; within(u32), exceeded(u32) &#125;}. {@link #kind()} carries
 * "within" or "exceeded"; {@link #value()} carries the u32 payload for
 * both variants, unboxed from the wasm return-area.
 *
 * <p>Shape matches the Go adapter's {@code BoundedDistance} struct and
 * the Python/JS adapters' {@code (kind, value)} tuple — the point of
 * the discipline is that a caller sees the same variant on the same
 * inputs regardless of which language the harness is written in.
 */
public record BoundedDistance(String kind, int value) {

    /**
     * @return {@code true} when {@link #kind()} is {@code "within"} —
     *     the exact distance fit inside the caller-supplied cutoff.
     */
    public boolean isWithin() {
        return "within".equals(kind);
    }
}
