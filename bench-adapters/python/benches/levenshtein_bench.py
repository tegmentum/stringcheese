"""Head-to-head: StringCheese (via wasm) vs. Python's Levenshtein libraries.

# Contestants

* **StringCheese** — loaded from the WebAssembly component built by
  ``cargo component build --release`` in ``component/rust-host/``, then
  called through ``wasmtime-py``. Every call pays the wasm boundary
  cost (parameter lowering, guest execution, result lifting) on top of
  the underlying DP.
* **python-Levenshtein** (``Levenshtein.distance``) — C extension over
  Python strings.
* **jellyfish** (``jellyfish.levenshtein_distance``) — Rust extension
  (formerly C) over Python strings.
* **rapidfuzz** (``rapidfuzz.distance.Levenshtein.distance``) — C++
  extension over Python strings; supports a ``score_cutoff`` for the
  bounded variant we compare separately.

# Representation caveat (READ THIS)

The three native libraries all take Python ``str``; StringCheese via
wasm takes ``bytes``. For ASCII input the semantics are equivalent, and
we hand each library its natural input representation — string to the
str libraries, bytes to StringCheese. The FFI cost is therefore folded
into the comparison **on purpose**: this is the "should I use
StringCheese through wasm from Python instead of a native C extension"
question, and the answer is a whole-stack answer, not a DP-kernel-only
one.

# Matrix

(length ∈ {8, 32, 128, 512, 2048}) × (regime ∈ {random, similar,
identical}) × (implementation ∈ {stringcheese, Levenshtein, jellyfish,
rapidfuzz}). Same lengths and regimes as
``bench-adapters/rust/benches/levenshtein_vs_*``.
"""

from __future__ import annotations

import Levenshtein
import jellyfish
import pytest
from rapidfuzz.distance import Levenshtein as RfLev

from _inputs import LENGTHS, REGIMES, build_pair

# Per-length salts; distinct from the Rust adapter's Levenshtein salts
# (0xA1, 0xA2, 0xA3) so the two harnesses do not accidentally share an
# unlikely corner-case corpus that would confound cross-harness debugging.
_SALTS = (0xB1, 0xB2, 0xB3)


@pytest.fixture(scope="module")
def pairs() -> dict[tuple[int, str], tuple[bytes, bytes]]:
    """Pre-materialised corpus for every (length, regime) cell."""
    return {(n, k): build_pair(n, k, _SALTS) for n in LENGTHS for k in REGIMES}


def _id(impl: str, kind: str, n: int) -> str:
    """Uniform benchmark id — impl/regime/length; sortable and greppable."""
    return f"{impl}/{kind}/len{n:04d}"


# --------------------------------------------------------------------------- #
# StringCheese (via wasm)
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_stringcheese(benchmark, stringcheese, pairs, n, kind):
    benchmark.group = f"levenshtein/{kind}/len{n:04d}"
    a, b = pairs[(n, kind)]
    benchmark(stringcheese.levenshtein_distance, a, b)


# --------------------------------------------------------------------------- #
# python-Levenshtein
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_python_levenshtein(benchmark, pairs, n, kind):
    benchmark.group = f"levenshtein/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    # Decode outside the timing loop — pytest-benchmark's ``benchmark``
    # call is the only thing timed; the ``.decode()`` cost belongs to
    # the caller not to the library under test.
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(Levenshtein.distance, a, b)


# --------------------------------------------------------------------------- #
# jellyfish
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_jellyfish(benchmark, pairs, n, kind):
    benchmark.group = f"levenshtein/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(jellyfish.levenshtein_distance, a, b)


# --------------------------------------------------------------------------- #
# rapidfuzz
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_rapidfuzz(benchmark, pairs, n, kind):
    benchmark.group = f"levenshtein/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(RfLev.distance, a, b)


# --------------------------------------------------------------------------- #
# Bounded (k = 3) — the classical spellcheck cutoff
# --------------------------------------------------------------------------- #
#
# StringCheese's `levenshtein-within` and rapidfuzz's `distance` with
# `score_cutoff=3` short-circuit when the true distance exceeds 3. On
# random inputs at length >> 3 both should collapse to a small,
# length-linear constant; on identical inputs both should be zero-cost.
# python-Levenshtein and jellyfish expose no bounded variant, so this
# group is StringCheese-vs-rapidfuzz only.


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_stringcheese_bounded_k3(benchmark, stringcheese, pairs, n, kind):
    benchmark.group = f"levenshtein_k3/{kind}/len{n:04d}"
    a, b = pairs[(n, kind)]
    benchmark(stringcheese.levenshtein_within, a, b, 3)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_rapidfuzz_bounded_k3(benchmark, pairs, n, kind):
    benchmark.group = f"levenshtein_k3/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(RfLev.distance, a, b, score_cutoff=3)
