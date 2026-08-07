"""Head-to-head: StringCheese (via wasm) vs. Python's Hamming implementations.

# Contestants

* **StringCheese** — Hamming from the wasm component. The WIT boundary
  returns ``result<u32, string>``; the adapter raises
  :class:`HammingLengthMismatch` on unequal lengths. Every input in this
  file is equal-length by construction, so the error path is never hit
  during timing.
* **python-Levenshtein.hamming** — C extension over Python strings.
* **jellyfish.hamming_distance** — Rust extension over Python strings.

rapidfuzz **is not** included in this file: as of ``rapidfuzz 3.14``
Hamming is exposed but the calling convention is
``rapidfuzz.distance.Hamming.distance(s1, s2, *, pad=False)``; the
default ``pad=False`` matches strsim/jellyfish/python-Levenshtein
semantics and would slot in cleanly, but the ecosystem-standard-pair
for Hamming in Python is still ``Levenshtein.hamming`` +
``jellyfish.hamming_distance``. Adding rapidfuzz here is a followup;
the group scaffolding is fixed and additive.

# Representation caveat

Same as the Levenshtein bench: StringCheese consumes ``bytes``, the two
native libraries consume ``str``. On ASCII input this is
semantically equivalent; the FFI cost is folded in on purpose.
"""

from __future__ import annotations

import Levenshtein
import jellyfish
import pytest

from _inputs import LENGTHS, REGIMES, build_pair_equal_len

_SALTS = (0xC1, 0xC2, 0xC3)


@pytest.fixture(scope="module")
def pairs() -> dict[tuple[int, str], tuple[bytes, bytes]]:
    """Equal-length pairs — Hamming requires the two sides to match in length."""
    return {(n, k): build_pair_equal_len(n, k, _SALTS) for n in LENGTHS for k in REGIMES}


# --------------------------------------------------------------------------- #
# StringCheese (via wasm)
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_stringcheese(benchmark, stringcheese, pairs, n, kind):
    benchmark.group = f"hamming/{kind}/len{n:04d}"
    a, b = pairs[(n, kind)]
    benchmark(stringcheese.hamming_distance, a, b)


# --------------------------------------------------------------------------- #
# python-Levenshtein.hamming
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_python_levenshtein(benchmark, pairs, n, kind):
    benchmark.group = f"hamming/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(Levenshtein.hamming, a, b)


# --------------------------------------------------------------------------- #
# jellyfish.hamming_distance
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_jellyfish(benchmark, pairs, n, kind):
    benchmark.group = f"hamming/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(jellyfish.hamming_distance, a, b)
