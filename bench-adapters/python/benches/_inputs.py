"""Shared corpus-generation helpers for the Python bench adapter.

The generator is a byte-for-byte port of ``bench-adapters/rust/src/lib.rs``'s
SplitMix64 corpus builder — same PRNG, same seed derivation, same
edit-injection recipe. Corpora produced from ``(length, salt)`` in this
module match the Rust adapter's corpus for the same ``(length, salt)``,
so a StringCheese datapoint from either harness lands on the same input
family.

Determinism is load-bearing: pytest-benchmark reruns each benchmark
across several rounds and compares the resulting distributions; if the
input were re-randomised each round the noise would be corpus variance,
not implementation variance.

Nothing here is stateful — every function returns a fresh ``bytes`` or a
tuple of ``bytes``. Callers cache the result in a pytest fixture (see
each ``*_bench.py``) so the corpus-generation cost is paid once, outside
the timing region.
"""

from __future__ import annotations

# The canonical length sweep, mirroring ``stringcheese-bench`` and the
# Rust adapter so per-length datapoints line up on a chart across
# harnesses. Keep these in sync when the Rust side changes.
LENGTHS: tuple[int, ...] = (8, 32, 128, 512, 2048)

# The three similarity regimes, in the order the Rust harness emits them.
REGIMES: tuple[str, ...] = ("random", "similar", "identical")


# --------------------------------------------------------------------------- #
# SplitMix64
# --------------------------------------------------------------------------- #

_GOLDEN_GAMMA = 0x9E37_79B9_7F4A_7C15
_MASK64 = (1 << 64) - 1


class _Rng:
    """Minimal SplitMix64 (Vigna). Not cryptographic; deterministic and cheap.

    The state update, both scramble multipliers, and both shift constants
    match the Rust ``Rng`` in ``bench-adapters/rust/src/lib.rs`` bit for
    bit. That is the whole point of duplicating the PRNG here rather than
    using ``random.Random``: we want the exact same bytes out for the
    exact same seed.
    """

    __slots__ = ("_state",)

    def __init__(self, seed: int) -> None:
        self._state = (seed + _GOLDEN_GAMMA) & _MASK64

    def next_u64(self) -> int:
        self._state = (self._state + _GOLDEN_GAMMA) & _MASK64
        z = self._state
        z = ((z ^ (z >> 30)) * 0xBF58_476D_1CE4_E5B9) & _MASK64
        z = ((z ^ (z >> 27)) * 0x94D0_49BB_1331_11EB) & _MASK64
        return z ^ (z >> 31)

    def next_bounded(self, bound: int) -> int:
        assert bound > 0, "next_bounded needs a nonzero bound"
        return self.next_u64() % bound

    def next_ascii_lower(self) -> int:
        return ord("a") + self.next_bounded(26)


def seed_for(length: int, salt: int) -> int:
    """Deterministic per-(length, salt) seed matching the Rust adapter."""
    return ((length * _GOLDEN_GAMMA) & _MASK64) ^ salt


# --------------------------------------------------------------------------- #
# Corpus builders
# --------------------------------------------------------------------------- #


def random_ascii(length: int, seed: int) -> bytes:
    """A fresh ``bytes`` of ``length`` lowercase-ASCII bytes."""
    rng = _Rng(seed)
    return bytes(rng.next_ascii_lower() for _ in range(length))


def identical_pair(length: int, seed: int) -> tuple[bytes, bytes]:
    """The two-byte-equal regime — both sides are the same ``bytes`` value."""
    s = random_ascii(length, seed)
    return s, s


def similar_pair(
    length: int, edit_rate: float, seed: int
) -> tuple[bytes, bytes]:
    """Two strings differing by roughly ``edit_rate * length`` mixed edits.

    Length is only approximate — insertions and deletions cancel on
    average. Callers that need equal-length inputs (Hamming) should use
    :func:`similar_pair_equal_len`.
    """
    assert edit_rate >= 0.0, "edit_rate must be non-negative"
    left = random_ascii(length, seed)
    right = bytearray(left)
    n_edits = max(0, round(length * edit_rate))
    rng = _Rng((seed ^ 0xA5A5_A5A5_A5A5_A5A5) & _MASK64)
    for _ in range(n_edits):
        if len(right) == 0:
            right.append(rng.next_ascii_lower())
            continue
        op = rng.next_bounded(3)
        pos = rng.next_bounded(len(right))
        if op == 0:  # substitute
            right[pos] = rng.next_ascii_lower()
        elif op == 1:  # insert
            right.insert(pos, rng.next_ascii_lower())
        else:  # delete
            del right[pos]
    return left, bytes(right)


def similar_pair_equal_len(
    length: int, edit_rate: float, seed: int
) -> tuple[bytes, bytes]:
    """Two equal-length strings differing in ~``edit_rate * length`` positions.

    Substitutions only. Positions may collide, so the true mismatch
    count can be slightly below the target. Hamming-regime input.
    """
    assert 0.0 <= edit_rate <= 1.0, "edit_rate must be in [0.0, 1.0]"
    left = random_ascii(length, seed)
    right = bytearray(left)
    n_edits = min(length, max(0, round(length * edit_rate)))
    if n_edits == 0 or length == 0:
        return left, bytes(right)
    rng = _Rng((seed ^ 0xC3C3_C3C3_C3C3_C3C3) & _MASK64)
    a_ord = ord("a")
    for _ in range(n_edits):
        pos = rng.next_bounded(length)
        bump = 1 + rng.next_bounded(25)
        right[pos] = a_ord + ((right[pos] - a_ord + bump) % 26)
    return left, bytes(right)


def build_pair(
    length: int, kind: str, salts: tuple[int, int, int]
) -> tuple[bytes, bytes]:
    """Regime dispatcher matching ``stringcheese-bench`` / the Rust adapter."""
    r_a, r_b, sim_or_ident = salts
    if kind == "random":
        return random_ascii(length, seed_for(length, r_a)), random_ascii(
            length, seed_for(length, r_b)
        )
    if kind == "similar":
        return similar_pair(length, 0.05, seed_for(length, sim_or_ident))
    if kind == "identical":
        return identical_pair(length, seed_for(length, sim_or_ident))
    raise ValueError(f"unknown similarity regime: {kind!r}")


def build_pair_equal_len(
    length: int, kind: str, salts: tuple[int, int, int]
) -> tuple[bytes, bytes]:
    """Equal-length variant of :func:`build_pair` for Hamming."""
    r_a, r_b, sim_or_ident = salts
    if kind == "random":
        return random_ascii(length, seed_for(length, r_a)), random_ascii(
            length, seed_for(length, r_b)
        )
    if kind == "similar":
        return similar_pair_equal_len(length, 0.05, seed_for(length, sim_or_ident))
    if kind == "identical":
        return identical_pair(length, seed_for(length, sim_or_ident))
    raise ValueError(f"unknown similarity regime: {kind!r}")


__all__ = [
    "LENGTHS",
    "REGIMES",
    "build_pair",
    "build_pair_equal_len",
    "identical_pair",
    "random_ascii",
    "seed_for",
    "similar_pair",
    "similar_pair_equal_len",
]
